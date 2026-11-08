//! Pruebas de punta a punta de la API pública `vault_core::vault`, con la
//! calibración real de Argon2id. Tardan algunos segundos.
//!
//! También abren las bóvedas de ejemplo `tests/fixtures/vault-v1.db` y
//! `vault-v2.db`, siempre sobre una copia.

use std::fs;
use std::path::{Path, PathBuf};

use secrecy::{ExposeSecret, SecretBox, SecretString};
use vault_core::vault::{EntryId, Error, SearchData, Secret, UnlockReport, Vault};

fn password(text: &str) -> SecretString {
    SecretString::from(text)
}

fn temp_path() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("boveda.db");
    (dir, path)
}

#[test]
fn create_lock_unlock_with_calibration() {
    let (_dir, path) = temp_path();
    let mut vault = Vault::create(&path, &password("maestra")).unwrap();
    let search = SearchData::new("gmail", None, Some("yo@example.com"), &["correo"]).unwrap();
    let secret = Secret::new(password("clave-gmail"), None).unwrap();
    let id = vault.add(search, secret).unwrap();

    vault.lock();
    let report = vault.unlock(&password("maestra")).unwrap();
    assert!(report.corrupt.is_empty());
    let entries = vault.entries().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].id(), id);
    assert_eq!(entries[0].search().service(), "gmail");
    assert_eq!(
        vault.reveal(id).unwrap().password().expose_secret(),
        "clave-gmail"
    );

    // Parámetros calibrados dentro de los límites, sin fijar valores exactos.
    let (memory, iterations, parallelism): (i64, i64, i64) = rusqlite::Connection::open(&path)
        .unwrap()
        .query_row(
            "SELECT kdf_memory_kib, kdf_iterations, kdf_parallelism FROM header",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert!((65_536..=131_072).contains(&memory), "memoria: {memory}");
    assert!(iterations >= 3, "iteraciones: {iterations}");
    assert_eq!(parallelism, 1);
}

#[test]
fn change_password_with_calibration() {
    let (_dir, path) = temp_path();
    let mut vault = Vault::create(&path, &password("vieja")).unwrap();
    vault
        .change_master_password(&password("vieja"), &password("nueva"))
        .unwrap();
    vault.lock();
    assert_eq!(
        vault.unlock(&password("vieja")).err(),
        Some(Error::WrongPassword)
    );
    assert!(vault.unlock(&password("nueva")).is_ok());
}

#[test]
fn search_with_public_api() {
    let (_dir, path) = temp_path();
    let mut vault = Vault::create(&path, &password("maestra")).unwrap();
    for service in ["gmail", "facebook"] {
        let search = SearchData::new(service, None, None, &[]).unwrap();
        let secret = Secret::new(password("clave"), None).unwrap();
        vault.add(search, secret).unwrap();
    }

    let found = vault.search("gm").unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].search().service(), "gmail");

    vault.lock();
    assert_eq!(vault.search("gm").err(), Some(Error::Locked));
}

#[test]
fn export_delete_import_with_calibration() {
    let (dir, path) = temp_path();
    let mut vault = Vault::create(&path, &password("maestra")).unwrap();
    let mut expected = Vec::new();
    for service in ["gmail", "banco"] {
        let search = SearchData::new(service, None, Some("yo@example.com"), &[]).unwrap();
        let secret = Secret::new(password(&format!("clave-{service}")), None).unwrap();
        expected.push((vault.add(search, secret).unwrap(), service));
    }
    let backup = dir.path().join("respaldo.db");
    vault.export(&backup).unwrap();
    drop(vault);
    std::fs::remove_file(&path).unwrap();

    let (mut imported, report) = Vault::import(&backup, &path, &password("maestra")).unwrap();

    assert!(report.corrupt.is_empty());
    assert_eq!(report.unidentified_corrupt, 0);
    assert_eq!(imported.entries().unwrap().len(), expected.len());
    for (id, service) in expected {
        let entry = imported.entries().unwrap().iter().find(|e| e.id() == id);
        assert_eq!(entry.unwrap().search().service(), service);
        assert_eq!(
            imported.reveal(id).unwrap().password().expose_secret(),
            format!("clave-{service}")
        );
    }
}

// --- Bóveda de ejemplo del formato 1 -------------------------------------
//
// Las pruebas trabajan sobre una copia: abrir la bóveda y `reveal` modifican
// el archivo. Con `rusqlite` sobre la copia se simula a quien puede escribir
// el archivo, como un atacante.

/// Contraseña de `tests/fixtures/vault-v1.db` (ver `tests/fixtures/README.md`).
const FIXTURE_V1_PASSWORD: &str = "prueba-bóveda-v1";

/// Contraseña de `tests/fixtures/vault-v2.db` (ver `tests/fixtures/README.md`).
const FIXTURE_V2_PASSWORD: &str = "prueba-bóveda-v2";

/// Copia la bóveda de ejemplo `name` a un directorio temporal (que debe
/// vivir toda la prueba) y devuelve la ruta absoluta de la copia.
fn fixture_copy_of(name: &str) -> (tempfile::TempDir, PathBuf) {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(name);
    fs::copy(source, &path).unwrap();
    (dir, path)
}

/// Copia de `vault-v1.db`.
fn fixture_copy() -> (tempfile::TempDir, PathBuf) {
    fixture_copy_of("vault-v1.db")
}

/// Abre y desbloquea `path` con la contraseña de prueba.
fn unlock_at(path: &Path) -> (Vault, UnlockReport) {
    let mut vault = Vault::open(path).unwrap();
    let report = vault.unlock(&password(FIXTURE_V1_PASSWORD)).unwrap();
    (vault, report)
}

fn clean_report() -> UnlockReport {
    UnlockReport {
        corrupt: vec![],
        unidentified_corrupt: 0,
    }
}

/// Copia de la bóveda de ejemplo, desbloqueada y sin entradas dañadas.
fn unlocked_fixture() -> (tempfile::TempDir, PathBuf, Vault) {
    let (dir, path) = fixture_copy();
    let (vault, report) = unlock_at(&path);
    assert_eq!(report, clean_report());
    (dir, path, vault)
}

fn id_of(vault: &Vault, service: &str) -> EntryId {
    vault
        .entries()
        .unwrap()
        .iter()
        .find(|e| e.search().service() == service)
        .unwrap()
        .id()
}

/// Servicios de las entradas válidas, ordenados.
fn services(vault: &Vault) -> Vec<String> {
    let mut all: Vec<_> = vault
        .entries()
        .unwrap()
        .iter()
        .map(|e| e.search().service().to_owned())
        .collect();
    all.sort();
    all
}

/// Contraseña y notas reveladas de la entrada con ese servicio.
fn revealed(vault: &mut Vault, service: &str) -> (String, Option<String>) {
    let secret = vault.reveal(id_of(vault, service)).unwrap();
    (
        secret.password().expose_secret().to_owned(),
        secret.notes().map(|n| n.expose_secret().to_owned()),
    )
}

/// Ids de "Banco" y "Nube" en la copia; la bóveda queda cerrada.
fn fixture_ids(path: &Path) -> (EntryId, EntryId) {
    let (vault, _) = unlock_at(path);
    (id_of(&vault, "Banco"), id_of(&vault, "Nube"))
}

/// Invierte un bit dentro del texto cifrado de `column` (`search` o `secret`).
fn flip_byte(path: &Path, id: EntryId, column: &str) {
    let conn = rusqlite::Connection::open(path).unwrap();
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

/// Intercambia la columna `secret` de dos entradas.
fn swap_secrets(path: &Path, a: EntryId, b: EntryId) {
    let conn = rusqlite::Connection::open(path).unwrap();
    let read = |id: EntryId| -> Vec<u8> {
        conn.query_row(
            "SELECT secret FROM entries WHERE id = ?1",
            [id.as_bytes().as_slice()],
            |r| r.get(0),
        )
        .unwrap()
    };
    let (secret_a, secret_b) = (read(a), read(b));
    for (id, secret) in [(a, secret_b), (b, secret_a)] {
        conn.execute(
            "UPDATE entries SET secret = ?1 WHERE id = ?2",
            rusqlite::params![secret, id.as_bytes().as_slice()],
        )
        .unwrap();
    }
}

#[test]
fn fixture_v1_opens_with_known_contents() {
    let (_dir, _path, mut vault) = unlocked_fixture();
    assert_fixture_contents(&mut vault);
}

/// Comprueba el contenido descrito en `tests/fixtures/README.md`, igual en
/// todas las bóvedas de ejemplo.
fn assert_fixture_contents(vault: &mut Vault) {
    assert_eq!(vault.entries().unwrap().len(), 3);

    {
        // Sin texto: la usada primero y después por servicio.
        let recent = vault.search("").unwrap();
        let order: Vec<&str> = recent.iter().map(|e| e.search().service()).collect();
        assert_eq!(order, ["Nube", "Banco", "Correo Electrónico"]);
        assert!(recent[0].last_used_at().is_some());
        assert!(recent[1].last_used_at().is_none());
        assert!(recent[2].last_used_at().is_none());

        let full = recent[2].search();
        assert_eq!(full.url(), Some("https://correo.example.com/entrar"));
        assert_eq!(full.username(), Some("ana@example.com"));
        assert_eq!(full.tags(), ["Trabajo", "Médico", "Año"]);
        for minimal in &recent[..2] {
            let search = minimal.search();
            assert_eq!(search.url(), None);
            assert_eq!(search.username(), None);
            assert!(search.tags().is_empty());
        }
    }

    assert_eq!(
        revealed(vault, "Correo Electrónico"),
        (
            "clave-completa-ñ".to_owned(),
            Some("Línea uno\nLínea dos".to_owned())
        )
    );
    assert_eq!(revealed(vault, "Banco"), ("clave-minima".to_owned(), None));
    assert_eq!(revealed(vault, "Nube"), ("clave-nube".to_owned(), None));

    // La lápida no aparece.
    assert!(vault.search("Eliminada").unwrap().is_empty());
    assert!(
        vault
            .entries()
            .unwrap()
            .iter()
            .all(|e| e.search().service() != "Eliminada")
    );
}

#[test]
fn fixture_v1_wrong_password() {
    let (_dir, path) = fixture_copy();
    let mut vault = Vault::open(&path).unwrap();
    // La misma contraseña sin el acento.
    assert_eq!(
        vault.unlock(&password("prueba-boveda-v1")).err(),
        Some(Error::WrongPassword)
    );
    assert!(vault.is_locked());
    assert!(vault.unlock(&password(FIXTURE_V1_PASSWORD)).is_ok());
}

#[test]
fn tampered_byte_is_detected() {
    // Un byte del secreto de "Banco": abre limpia, pero no se revela.
    let (_dir, path) = fixture_copy();
    let (banco, _) = fixture_ids(&path);
    flip_byte(&path, banco, "secret");
    let (mut vault, report) = unlock_at(&path);
    assert_eq!(report, clean_report());
    assert_eq!(vault.reveal(banco).err(), Some(Error::CorruptEntry));
    assert_eq!(revealed(&mut vault, "Nube").0, "clave-nube");
    assert_eq!(
        revealed(&mut vault, "Correo Electrónico").0,
        "clave-completa-ñ"
    );

    // Un byte de los datos de búsqueda de "Nube": se reporta y abren las demás.
    let (_dir, path) = fixture_copy();
    let (_, nube) = fixture_ids(&path);
    flip_byte(&path, nube, "search");
    let (vault, report) = unlock_at(&path);
    assert_eq!(report.corrupt, vec![nube]);
    assert_eq!(report.unidentified_corrupt, 0);
    assert_eq!(services(&vault), ["Banco", "Correo Electrónico"]);
}

#[test]
fn swapped_secret_between_entries_is_rejected() {
    let (_dir, path) = fixture_copy();
    let (banco, nube) = fixture_ids(&path);
    swap_secrets(&path, banco, nube);
    let (mut vault, report) = unlock_at(&path);
    // Los datos de búsqueda no cambiaron.
    assert_eq!(report, clean_report());
    assert_eq!(vault.reveal(banco).err(), Some(Error::CorruptEntry));
    assert_eq!(vault.reveal(nube).err(), Some(Error::CorruptEntry));
    assert_eq!(
        revealed(&mut vault, "Correo Electrónico").0,
        "clave-completa-ñ"
    );
}

#[test]
fn backup_restore_recovers_everything() {
    let (dir, path, vault) = unlocked_fixture();
    let backup = dir.path().join("respaldo.db");
    vault.export(&backup).unwrap();
    // En Windows no se puede borrar un archivo que SQLite tiene abierto.
    drop(vault);
    fs::remove_file(&path).unwrap();

    let (mut imported, report) =
        Vault::import(&backup, &path, &password(FIXTURE_V1_PASSWORD)).unwrap();

    assert_eq!(report, clean_report());
    assert_eq!(services(&imported), ["Banco", "Correo Electrónico", "Nube"]);
    assert_eq!(
        revealed(&mut imported, "Correo Electrónico"),
        (
            "clave-completa-ñ".to_owned(),
            Some("Línea uno\nLínea dos".to_owned())
        )
    );
    assert_eq!(
        revealed(&mut imported, "Banco"),
        ("clave-minima".to_owned(), None)
    );
    assert_eq!(
        revealed(&mut imported, "Nube"),
        ("clave-nube".to_owned(), None)
    );
}

/// Crear, agregar, bloquear, desbloquear, buscar, revelar, cambiar la
/// contraseña maestra, exportar e importar, sobre la misma bóveda.
#[test]
fn full_flow_through_public_api() {
    let (dir, path) = temp_path();
    let mut vault = Vault::create(&path, &password("maestra-vieja")).unwrap();
    let gmail = SearchData::new(
        "gmail",
        Some("https://mail.google.com"),
        Some("yo@example.com"),
        &[],
    )
    .unwrap();
    let gmail = vault
        .add(gmail, Secret::new(password("clave-gmail"), None).unwrap())
        .unwrap();
    let banco = SearchData::new("banco", None, None, &[]).unwrap();
    let banco = vault
        .add(banco, Secret::new(password("clave-banco"), None).unwrap())
        .unwrap();

    vault.lock();
    assert_eq!(vault.entries().err(), Some(Error::Locked));
    assert_eq!(
        vault.unlock(&password("maestra-vieja")).unwrap(),
        clean_report()
    );

    // Con un error de escritura.
    let found = vault.search("gmial").unwrap();
    assert_eq!(found.first().map(|e| e.id()), Some(gmail));
    assert_eq!(
        vault.reveal(gmail).unwrap().password().expose_secret(),
        "clave-gmail"
    );

    vault
        .change_master_password(&password("maestra-vieja"), &password("maestra-nueva"))
        .unwrap();
    vault.lock();
    assert_eq!(
        vault.unlock(&password("maestra-vieja")).err(),
        Some(Error::WrongPassword)
    );
    vault.unlock(&password("maestra-nueva")).unwrap();

    let backup = dir.path().join("respaldo.db");
    vault.export(&backup).unwrap();
    drop(vault);
    fs::remove_file(&path).unwrap();

    let (mut imported, report) = Vault::import(&backup, &path, &password("maestra-nueva")).unwrap();
    assert_eq!(report, clean_report());
    assert_eq!(imported.entries().unwrap().len(), 2);
    for (id, expected) in [(gmail, "clave-gmail"), (banco, "clave-banco")] {
        assert_eq!(
            imported.reveal(id).unwrap().password().expose_secret(),
            expected
        );
    }
}

// --- Formato 2 y desbloqueo con la llave de bóveda (#34) ------------------

#[test]
fn fixture_v2_opens_with_known_contents() {
    let (_dir, path) = fixture_copy_of("vault-v2.db");
    let mut vault = Vault::open(&path).unwrap();
    let report = vault.unlock(&password(FIXTURE_V2_PASSWORD)).unwrap();
    assert_eq!(report, clean_report());
    assert_fixture_contents(&mut vault);
}

#[test]
fn fixture_v1_migrates_and_keeps_working() {
    let (_dir, path) = fixture_copy();
    let (mut vault, report) = unlock_at(&path);
    assert_eq!(report, clean_report());
    assert_fixture_contents(&mut vault);
    let key = vault.export_vault_key().unwrap();
    drop(vault);

    // Solo una v2 desbloquea con la llave: prueba que quedó migrada.
    let mut vault = Vault::open(&path).unwrap();
    assert_eq!(vault.unlock_with_key(key).unwrap(), clean_report());
    assert_eq!(services(&vault), ["Banco", "Correo Electrónico", "Nube"]);
    assert_eq!(
        revealed(&mut vault, "Banco"),
        ("clave-minima".to_owned(), None)
    );

    // La misma contraseña sigue abriendo.
    vault.lock();
    assert!(vault.unlock(&password(FIXTURE_V1_PASSWORD)).is_ok());
}

#[test]
fn unlock_with_key_on_unmigrated_v1_is_wrong_vault_key() {
    // La llave real, sacada de otra copia ya migrada.
    let (_dir, migrated) = fixture_copy();
    let (mut vault, _) = unlock_at(&migrated);
    let real_key = vault.export_vault_key().unwrap();
    drop(vault);

    let (_dir2, path) = fixture_copy();
    let mut vault = Vault::open(&path).unwrap();
    assert_eq!(
        vault
            .unlock_with_key(SecretBox::new(Box::new([7u8; 32])))
            .err(),
        Some(Error::WrongVaultKey)
    );
    assert_eq!(
        vault.unlock_with_key(real_key).err(),
        Some(Error::WrongVaultKey)
    );
    assert!(vault.is_locked());
}

#[test]
fn migrated_vault_key_survives_password_change_with_calibration() {
    let (_dir, path) = fixture_copy();
    let (mut vault, _) = unlock_at(&path);
    let key = vault.export_vault_key().unwrap();
    vault
        .change_master_password(&password(FIXTURE_V1_PASSWORD), &password("nueva"))
        .unwrap();
    vault.lock();
    assert_eq!(vault.unlock_with_key(key).unwrap(), clean_report());
    assert_eq!(vault.entries().unwrap().len(), 3);
}
