//! Ayudantes compartidos por las pruebas unitarias de `vault` (`storage`,
//! la API `Vault` y `backup`). Solo existe con `cfg(test)`.

use std::fs;
use std::path::{Path, PathBuf};

use secrecy::SecretString;

use super::entry::{EntryId, SearchData, Secret};

pub(crate) fn password(text: &str) -> SecretString {
    SecretString::from(text)
}

/// Directorio temporal (debe vivir toda la prueba) y una ruta que no existe.
pub(crate) fn temp_path() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("boveda.db");
    (dir, path)
}

/// Directorio temporal único dentro de `<workspace>/target` (misma unidad
/// que el repositorio, ignorado por git) y la ruta `boveda.db` en él, en
/// dos formas: absoluta y relativa al directorio de trabajo de las pruebas
/// (`core/`), por ejemplo `../target/boveda-rel-XXXX/boveda.db`.
///
/// No usa `set_current_dir`, que es global al proceso. Tampoco sigue
/// `CARGO_TARGET_DIR`: si el `target` real está en otra parte, igual crea
/// `<workspace>/target` (ignorado por git), donde queda vacío al terminar.
pub(crate) fn relative_temp_path() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest.parent().unwrap();
    let target = root.join("target");
    fs::create_dir_all(&target).unwrap();
    let dir = tempfile::Builder::new()
        .prefix("boveda-rel-")
        .tempdir_in(&target)
        .unwrap();
    let absolute = dir.path().join("boveda.db");
    let relative = Path::new("..").join(absolute.strip_prefix(root).unwrap());
    assert!(relative.is_relative());
    // Las pruebas corren con `core/` como directorio de trabajo.
    assert_eq!(
        fs::canonicalize(".").unwrap(),
        fs::canonicalize(manifest).unwrap()
    );
    (dir, absolute, relative)
}

/// Conexión directa, para leer pragmas y escribir o alterar filas.
pub(crate) fn raw(path: &Path) -> rusqlite::Connection {
    rusqlite::Connection::open(path).unwrap()
}

/// Ruta con un sufijo agregado al nombre del archivo (`-journal`, `-wal`...).
pub(crate) fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

pub(crate) fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

/// Datos de búsqueda y secreto de ejemplo; la contraseña es `clave-{service}`.
pub(crate) fn sample(service: &str) -> (SearchData, Secret) {
    let search = SearchData::new(
        service,
        Some(&format!("https://{service}.example")),
        Some("usuario@example.com"),
        &["trabajo", "correo"],
    )
    .unwrap();
    let secret = Secret::new(
        password(&format!("clave-{service}")),
        Some(password("notas de prueba")),
    )
    .unwrap();
    (search, secret)
}

/// Invierte un bit dentro del texto cifrado de `column` (`search` o `secret`).
pub(crate) fn tamper(path: &Path, id: EntryId, column: &str) {
    let conn = raw(path);
    let mut blob: Vec<u8> = conn
        .query_row(
            &format!("SELECT {column} FROM entries WHERE id = ?1"),
            [id.as_bytes().as_slice()],
            |r| r.get(0),
        )
        .unwrap();
    blob[30] ^= 1;
    conn.execute(
        &format!("UPDATE entries SET {column} = ?1 WHERE id = ?2"),
        rusqlite::params![blob, id.as_bytes().as_slice()],
    )
    .unwrap();
}

/// Copia `src` a `dst` en mitad de una transacción que ya escribió páginas
/// en el archivo, junto con su `-journal`: la copia queda con un diario
/// caliente, como tras un corte de energía.
pub(crate) fn make_hot_journal_copy(src: &Path, dst: &Path) {
    let conn = raw(src);
    conn.pragma_update(None, "cache_size", 1).unwrap();
    conn.execute_batch("BEGIN IMMEDIATE").unwrap();
    let part = vec![0xAB_u8; 4096];
    for i in 0..200_u32 {
        let mut id = [0_u8; 16];
        id[..4].copy_from_slice(&i.to_le_bytes());
        conn.execute(
            "INSERT INTO entries (id, entry_schema_version, search, secret, modified_at) \
             VALUES (?1, 1, ?2, ?2, 0)",
            rusqlite::params![id.as_slice(), part.as_slice()],
        )
        .unwrap();
    }
    fs::copy(src, dst).unwrap();
    fs::copy(sibling(src, "-journal"), sibling(dst, "-journal")).unwrap();
    conn.execute_batch("ROLLBACK").unwrap();
    drop(conn);

    let journal = fs::metadata(sibling(dst, "-journal")).unwrap();
    assert!(journal.len() > 512, "el diario copiado no está caliente");
}

/// Pasa la bóveda `src` a WAL, modifica su cabecera y sus entradas sin
/// checkpoint y copia el `-wal` resultante a `<dst>-wal`, con la conexión
/// aún abierta: un `-wal` válido, como el que deja un cierre forzado.
pub(crate) fn copy_live_wal(src: &Path, dst: &Path) {
    let conn = raw(src);
    let mode: String = conn
        .pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get(0))
        .unwrap();
    assert_eq!(mode, "wal");
    conn.pragma_update(None, "wal_autocheckpoint", 0).unwrap();
    conn.execute_batch(
        "UPDATE header SET kdf_iterations = kdf_iterations + 1; \
         UPDATE entries SET modified_at = modified_at + 1;",
    )
    .unwrap();
    fs::copy(sibling(src, "-wal"), sibling(dst, "-wal")).unwrap();
    drop(conn);

    let wal = fs::metadata(sibling(dst, "-wal")).unwrap();
    assert!(wal.len() > 32, "el -wal copiado no tiene frames");
}

/// Daña la página raíz de `entries`: llena sus 8 primeros bytes con `0xFF`.
/// La cabecera (otra página) sigue legible.
pub(crate) fn damage_entries_root_page(path: &Path) {
    let (page_size, root_page): (i64, i64) = {
        let conn = raw(path);
        let page_size = conn
            .pragma_query_value(None, "page_size", |row| row.get(0))
            .unwrap();
        let root_page = conn
            .query_row(
                "SELECT rootpage FROM sqlite_schema WHERE name = 'entries'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        (page_size, root_page)
    };
    assert!(root_page > 1);

    let mut bytes = fs::read(path).unwrap();
    let offset = usize::try_from((root_page - 1) * page_size).unwrap();
    bytes[offset..offset + 8].fill(0xFF);
    fs::write(path, bytes).unwrap();
}

/// Contraseña de prueba de `core/tests/fixtures/vault-v1.db`. Se repite en
/// `core/tests/vault.rs`, que no ve el código `cfg(test)` del crate.
pub(crate) const FIXTURE_V1_PASSWORD: &str = "prueba-bóveda-v1";

/// Copia `core/tests/fixtures/vault-v1.db` a un directorio temporal (debe
/// vivir toda la prueba). Abrirla con contraseña la migra, así que nunca se
/// usa el archivo del repositorio.
pub(crate) fn fixture_v1_copy() -> (tempfile::TempDir, PathBuf) {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/vault-v1.db");
    let (dir, path) = temp_path();
    fs::copy(source, &path).unwrap();
    (dir, path)
}

/// Esquema completo de una base: `(type, name, tbl_name, sql)` de cada fila
/// de `sqlite_schema`, ordenado por nombre.
pub(crate) fn schema_of(path: &Path) -> Vec<(String, String, String, Option<String>)> {
    let conn = raw(path);
    let mut stmt = conn
        .prepare("SELECT type, name, tbl_name, sql FROM sqlite_schema ORDER BY name")
        .unwrap();
    stmt.query_map([], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
    })
    .unwrap()
    .collect::<rusqlite::Result<_>>()
    .unwrap()
}
