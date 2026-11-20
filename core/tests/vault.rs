//! Pruebas de punta a punta de la API pública `vault_core::vault`, con la
//! calibración real de Argon2id. Tardan algunos segundos.
//!
//! También abren las bóvedas de ejemplo `tests/fixtures/vault-v1.db` y
//! `vault-v2.db`, siempre sobre una copia.

use std::fs;
use std::path::{Path, PathBuf};

use secrecy::{ExposeSecret, SecretBox, SecretString};
use vault_core::vault::{
    EntryId, Error, NewPairedPc, SearchData, Secret, SecretChange, UnlockReport, Vault,
};

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
    let mut vault = Vault::create(&path, &password("maestra-larga")).unwrap();
    let search = SearchData::new("gmail", None, Some("yo@example.com"), &["correo"]).unwrap();
    let secret = Secret::new(password("clave-gmail"), None).unwrap();
    let id = vault.add(search, secret).unwrap();

    vault.lock();
    let report = vault.unlock(&password("maestra-larga")).unwrap();
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
    let mut vault = Vault::create(&path, &password("vieja-maestra")).unwrap();
    vault
        .change_master_password(&password("vieja-maestra"), &password("nueva-maestra"))
        .unwrap();
    vault.lock();
    assert_eq!(
        vault.unlock(&password("vieja-maestra")).err(),
        Some(Error::WrongPassword)
    );
    assert!(vault.unlock(&password("nueva-maestra")).is_ok());
}

// --- Contraseña maestra en NFC (#57) -------------------------------------
//
// `é` compuesta (U+00E9) y `e` + acento combinante (U+0065 U+0301) son la misma
// contraseña escrita desde otro teclado: tienen que abrir la misma bóveda.

const COMPOSED: &str = "contrase\u{00f1}a-caf\u{00e9}-larga";
const DECOMPOSED: &str = "contrasen\u{0303}a-cafe\u{0301}-larga";

#[test]
fn composed_and_decomposed_master_passwords_are_interchangeable() {
    for (created_with, opened_with) in [(COMPOSED, DECOMPOSED), (DECOMPOSED, COMPOSED)] {
        let (_dir, path) = temp_path();
        let mut vault = Vault::create(&path, &password(created_with)).unwrap();
        let search = SearchData::new("gmail", None, None, &[]).unwrap();
        let id = vault
            .add(search, Secret::new(password("clave-gmail"), None).unwrap())
            .unwrap();
        vault.lock();

        let report = vault.unlock(&password(opened_with)).unwrap();
        assert!(report.corrupt.is_empty());
        assert_eq!(
            vault.reveal(id).unwrap().password().expose_secret(),
            "clave-gmail"
        );
    }
}

#[test]
fn change_master_password_accepts_either_form_of_the_current_one() {
    let (_dir, path) = temp_path();
    let mut vault = Vault::create(&path, &password(COMPOSED)).unwrap();
    // La actual escrita descompuesta; la nueva, descompuesta.
    vault
        .change_master_password(&password(DECOMPOSED), &password(DECOMPOSED))
        .unwrap();
    vault.lock();
    assert!(vault.unlock(&password(COMPOSED)).is_ok());
    vault.lock();
    assert!(vault.unlock(&password(DECOMPOSED)).is_ok());
}

#[test]
fn import_accepts_either_form_of_the_master_password() {
    let (dir, path) = temp_path();
    let mut vault = Vault::create(&path, &password(COMPOSED)).unwrap();
    let search = SearchData::new("banco", None, None, &[]).unwrap();
    let id = vault
        .add(search, Secret::new(password("clave-banco"), None).unwrap())
        .unwrap();
    let backup = dir.path().join("respaldo.db");
    vault.export(&backup).unwrap();
    drop(vault);
    std::fs::remove_file(&path).unwrap();

    let (mut imported, report) = Vault::import(&backup, &path, &password(DECOMPOSED)).unwrap();

    assert!(report.corrupt.is_empty());
    assert_eq!(
        imported.reveal(id).unwrap().password().expose_secret(),
        "clave-banco"
    );
}

#[test]
fn a_different_accent_is_still_a_wrong_password() {
    let (_dir, path) = temp_path();
    let mut vault = Vault::create(&path, &password(COMPOSED)).unwrap();
    vault.lock();
    let other = "contrase\u{00f1}a-caf\u{00e8}-larga";
    assert_eq!(
        vault.unlock(&password(other)).err(),
        Some(Error::WrongPassword)
    );
}

#[test]
fn search_with_public_api() {
    let (_dir, path) = temp_path();
    let mut vault = Vault::create(&path, &password("maestra-larga")).unwrap();
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
    let mut vault = Vault::create(&path, &password("maestra-larga")).unwrap();
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

    let (mut imported, report) = Vault::import(&backup, &path, &password("maestra-larga")).unwrap();

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

/// Contraseña de `tests/fixtures/vault-v3.db` (ver `tests/fixtures/README.md`).
const FIXTURE_V3_PASSWORD: &str = "prueba-bóveda-v3";

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

/// `vault-v2.db` es de verdad de la versión 2 (#53, #93): desbloquearla la
/// migra a la 3 y el archivo cambia (una v1 también cambiaría, pero queda en la
/// 3 tras pasar por la 2). El segundo desbloqueo ya no escribe nada: una v3 no
/// escribe al desbloquear. Aquí no se revela ningún secreto, porque revelar
/// actualiza `last_used_at`.
#[test]
fn fixture_v2_unlock_migrates_to_v3_and_then_writes_nothing() {
    let (_dir, path) = fixture_copy_of("vault-v2.db");
    let original = fs::read(&path).unwrap();
    let version = |path: &Path| -> i64 {
        rusqlite::Connection::open(path)
            .unwrap()
            .query_row("SELECT format_version FROM header WHERE id = 1", [], |r| {
                r.get(0)
            })
            .unwrap()
    };
    assert_eq!(version(&path), 2);

    let mut vault = Vault::open(&path).unwrap();
    assert_eq!(
        vault.unlock(&password(FIXTURE_V2_PASSWORD)).unwrap(),
        clean_report()
    );
    drop(vault);
    assert_eq!(version(&path), 3);
    let migrated = fs::read(&path).unwrap();
    assert_ne!(migrated, original);

    let mut vault = Vault::open(&path).unwrap();
    assert_eq!(
        vault.unlock(&password(FIXTURE_V2_PASSWORD)).unwrap(),
        clean_report()
    );
    drop(vault);
    assert_eq!(fs::read(&path).unwrap(), migrated);
}

/// Abrir (`Vault::open`) nunca modifica el archivo, sea de la versión que sea
/// (la especificación lo exige): solo desbloquear una v1 o una v2 las migra.
#[test]
fn open_never_modifies_any_fixture_version() {
    for name in ["vault-v1.db", "vault-v2.db", "vault-v3.db"] {
        let (_dir, path) = fixture_copy_of(name);
        let before = fs::read(&path).unwrap();
        drop(Vault::open(&path).unwrap());
        assert_eq!(fs::read(&path).unwrap(), before, "{name}");
    }
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
        .change_master_password(&password(FIXTURE_V1_PASSWORD), &password("nueva-maestra"))
        .unwrap();
    vault.lock();
    assert_eq!(vault.unlock_with_key(key).unwrap(), clean_report());
    assert_eq!(vault.entries().unwrap().len(), 3);
}

#[test]
fn partial_secret_replace_survives_reopen() {
    let (_dir, path) = fixture_copy_of("vault-v2.db");
    let mut vault = Vault::open(&path).unwrap();
    vault.unlock(&password(FIXTURE_V2_PASSWORD)).unwrap();
    let mail = id_of(&vault, "Correo Electrónico");
    let bank = id_of(&vault, "Banco");

    // Solo la contraseña: las notas no salen del núcleo y se conservan.
    let search = SearchData::new("Correo Electrónico", None, None, &[]).unwrap();
    vault
        .edit(
            mail,
            search,
            SecretChange::ReplacePassword(password("clave-nueva")),
        )
        .unwrap();
    // Solo las notas: la contraseña se conserva.
    let search = SearchData::new("Banco", None, None, &[]).unwrap();
    vault
        .edit(
            bank,
            search,
            SecretChange::ReplaceNotes(Some(password("PIN en la libreta"))),
        )
        .unwrap();

    drop(vault);
    let mut vault = Vault::open(&path).unwrap();
    assert_eq!(
        vault.unlock(&password(FIXTURE_V2_PASSWORD)).unwrap(),
        clean_report()
    );
    assert_eq!(
        revealed(&mut vault, "Correo Electrónico"),
        (
            "clave-nueva".to_owned(),
            Some("Línea uno\nLínea dos".to_owned())
        )
    );
    assert_eq!(
        revealed(&mut vault, "Banco"),
        (
            "clave-minima".to_owned(),
            Some("PIN en la libreta".to_owned())
        )
    );
    assert_eq!(
        revealed(&mut vault, "Nube"),
        ("clave-nube".to_owned(), None)
    );
}

#[test]
fn entry_by_id_on_fixture() {
    let (_dir, path) = fixture_copy_of("vault-v2.db");
    let mut vault = Vault::open(&path).unwrap();
    let id = EntryId::from_bytes([7; 16]);
    assert_eq!(vault.entry(id).err(), Some(Error::Locked));
    vault.unlock(&password(FIXTURE_V2_PASSWORD)).unwrap();
    assert_eq!(vault.entry(id).err(), Some(Error::EntryNotFound));

    let mail = id_of(&vault, "Correo Electrónico");
    let entry = vault.entry(mail).unwrap();
    assert_eq!(entry.id(), mail);
    assert_eq!(entry.search().username(), Some("ana@example.com"));
    assert_eq!(entry.last_used_at(), None);

    vault.reveal(mail).unwrap();
    assert!(vault.entry(mail).unwrap().last_used_at().is_some());
    vault.delete(mail).unwrap();
    assert_eq!(vault.entry(mail).err(), Some(Error::EntryNotFound));
}

// --- Identidad y PCs emparejadas por la API pública (#93) ------------------

#[test]
fn identity_and_paired_pcs_through_the_public_api() {
    // Una v2 desbloqueada se migra a v3 y ya puede guardar la identidad y PCs.
    let (_dir, path) = fixture_copy_of("vault-v2.db");
    let mut vault = Vault::open(&path).unwrap();
    vault.unlock(&password(FIXTURE_V2_PASSWORD)).unwrap();
    assert!(vault.identity().unwrap().is_none());
    assert!(vault.paired_pcs().unwrap().valid.is_empty());

    let identity = vault.ensure_identity().unwrap();
    let private = *identity.private_key().expose_secret();
    let id = vault
        .add_paired_pc(NewPairedPc {
            public_key: [5; 32],
            name: "Escritorio".to_owned(),
            last_ip: Some("192.168.1.50".parse().unwrap()),
        })
        .unwrap();
    vault.lock();
    drop(vault);

    let mut reopened = Vault::open(&path).unwrap();
    assert_eq!(
        reopened.unlock(&password(FIXTURE_V2_PASSWORD)).unwrap(),
        clean_report()
    );
    assert_eq!(
        *reopened
            .identity()
            .unwrap()
            .unwrap()
            .private_key()
            .expose_secret(),
        private
    );
    let listed = reopened.paired_pcs().unwrap();
    assert!(listed.corrupt.is_empty());
    assert_eq!(listed.valid.len(), 1);
    assert_eq!(listed.valid[0].id(), id);
    assert_eq!(listed.valid[0].name(), "Escritorio");
    reopened.remove_paired_pc(id).unwrap();
    assert!(reopened.paired_pcs().unwrap().valid.is_empty());
}

// --- Formato 3: bóveda de ejemplo con identidad y PCs (#93) ----------------

/// Abre y desbloquea una copia de `vault-v3.db`.
fn unlocked_v3() -> (tempfile::TempDir, PathBuf, Vault) {
    let (dir, path) = fixture_copy_of("vault-v3.db");
    let mut vault = Vault::open(&path).unwrap();
    assert_eq!(
        vault.unlock(&password(FIXTURE_V3_PASSWORD)).unwrap(),
        clean_report()
    );
    (dir, path, vault)
}

/// `format_version` guardada en el archivo.
fn stored_version(path: &Path) -> i64 {
    rusqlite::Connection::open(path)
        .unwrap()
        .query_row("SELECT format_version FROM header WHERE id = 1", [], |r| {
            r.get(0)
        })
        .unwrap()
}

/// Filas de `identity` y de `paired_pcs`.
fn pairing_rows(path: &Path) -> (i64, i64) {
    let conn = rusqlite::Connection::open(path).unwrap();
    let count = |table: &str| -> i64 {
        conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
            .unwrap()
    };
    (count("identity"), count("paired_pcs"))
}

#[test]
fn fixture_v3_opens_with_known_contents() {
    let (_dir, path, mut vault) = unlocked_v3();
    assert_eq!(stored_version(&path), 3);
    assert_fixture_contents(&mut vault);

    // Identidad fija del generador.
    let identity = vault.identity().unwrap().unwrap();
    assert_eq!(*identity.private_key().expose_secret(), [7u8; 32]);

    // Dos PCs: una con IP y fecha de la última vez, y otra sin IP.
    let listed = vault.paired_pcs().unwrap();
    assert!(listed.corrupt.is_empty());
    assert_eq!(listed.unidentified_corrupt, 0);
    let mut pcs = listed.valid;
    pcs.sort_by(|a, b| a.name().cmp(b.name()));
    assert_eq!(pcs.len(), 2);

    assert_eq!(pcs[0].name(), "Escritorio");
    assert_eq!(pcs[0].public_key(), &[0x11; 32]);
    assert_eq!(pcs[0].last_ip(), Some("192.168.1.50".parse().unwrap()));
    assert!(pcs[0].paired_at() > 1_767_225_600_000);
    assert!(pcs[0].last_seen_at().unwrap() >= pcs[0].paired_at());

    assert_eq!(pcs[1].name(), "Portátil");
    assert_eq!(pcs[1].public_key(), &[0x22; 32]);
    assert_eq!(pcs[1].last_ip(), None);
    assert!(pcs[1].paired_at() > 1_767_225_600_000);
    assert_eq!(pcs[1].last_seen_at(), None);
}

#[test]
fn fixture_v3_unlock_does_not_write() {
    let (_dir, path) = fixture_copy_of("vault-v3.db");
    let before = fs::read(&path).unwrap();
    let mut vault = Vault::open(&path).unwrap();
    assert_eq!(
        vault.unlock(&password(FIXTURE_V3_PASSWORD)).unwrap(),
        clean_report()
    );
    // Leer la identidad y las PCs tampoco escribe.
    vault.identity().unwrap();
    vault.paired_pcs().unwrap();
    drop(vault);
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn fixture_v3_wrong_password() {
    let (_dir, path) = fixture_copy_of("vault-v3.db");
    let mut vault = Vault::open(&path).unwrap();
    assert_eq!(
        vault.unlock(&password("incorrecta")).err(),
        Some(Error::WrongPassword)
    );
    assert!(vault.is_locked());
}

#[test]
fn fixture_v3_unlocks_with_the_vault_key() {
    let (_dir, path, mut vault) = unlocked_v3();
    let key = vault.export_vault_key().unwrap();
    vault.lock();
    drop(vault);
    let mut reopened = Vault::open(&path).unwrap();
    assert_eq!(reopened.unlock_with_key(key).unwrap(), clean_report());
    assert_eq!(services(&reopened), ["Banco", "Correo Electrónico", "Nube"]);
}

#[test]
fn fixture_v1_and_v2_still_open_and_migrate_to_v3() {
    for (name, pw) in [
        ("vault-v1.db", FIXTURE_V1_PASSWORD),
        ("vault-v2.db", FIXTURE_V2_PASSWORD),
    ] {
        let (_dir, path) = fixture_copy_of(name);
        let mut vault = Vault::open(&path).unwrap();
        assert_eq!(
            vault.unlock(&password(pw)).unwrap(),
            clean_report(),
            "{name}"
        );
        assert_fixture_contents(&mut vault);
        // Migrada: sin identidad ni PCs, y lista para guardarlas.
        assert!(vault.identity().unwrap().is_none(), "{name}");
        assert!(vault.paired_pcs().unwrap().valid.is_empty(), "{name}");
        drop(vault);
        assert_eq!(stored_version(&path), 3, "{name}");
        assert_eq!(pairing_rows(&path), (0, 0), "{name}");
    }
}

#[test]
fn tampered_identity_in_fixture_v3_is_detected_without_blocking_the_vault() {
    let (_dir, path) = fixture_copy_of("vault-v3.db");
    let conn = rusqlite::Connection::open(&path).unwrap();
    let mut data: Vec<u8> = conn
        .query_row("SELECT data FROM identity WHERE id = 1", [], |r| r.get(0))
        .unwrap();
    data[40] ^= 1;
    conn.execute("UPDATE identity SET data = ?1", [data])
        .unwrap();
    drop(conn);

    let mut vault = Vault::open(&path).unwrap();
    assert_eq!(
        vault.unlock(&password(FIXTURE_V3_PASSWORD)).unwrap(),
        clean_report()
    );
    assert_eq!(services(&vault), ["Banco", "Correo Electrónico", "Nube"]);
    assert_eq!(vault.identity().err(), Some(Error::CorruptIdentity));
    assert_eq!(vault.ensure_identity().err(), Some(Error::CorruptIdentity));
    // Las PCs no se ven afectadas.
    assert_eq!(vault.paired_pcs().unwrap().valid.len(), 2);
}

#[test]
fn tampered_pc_in_fixture_v3_is_reported_and_the_other_is_listed() {
    let (_dir, path) = fixture_copy_of("vault-v3.db");
    let conn = rusqlite::Connection::open(&path).unwrap();
    let (id, mut data): (Vec<u8>, Vec<u8>) = conn
        .query_row("SELECT id, data FROM paired_pcs LIMIT 1", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    data[30] ^= 1;
    conn.execute(
        "UPDATE paired_pcs SET data = ?1 WHERE id = ?2",
        rusqlite::params![data, id],
    )
    .unwrap();
    drop(conn);

    let mut vault = Vault::open(&path).unwrap();
    vault.unlock(&password(FIXTURE_V3_PASSWORD)).unwrap();
    let listed = vault.paired_pcs().unwrap();
    assert_eq!(listed.valid.len(), 1);
    assert_eq!(listed.corrupt.len(), 1);
    assert_eq!(listed.corrupt[0].as_bytes().as_slice(), id.as_slice());
    // Se puede quitar la dañada.
    vault.remove_paired_pc(listed.corrupt[0]).unwrap();
    assert_eq!(vault.paired_pcs().unwrap().corrupt.len(), 0);
}

/// Importar una bóveda de ejemplo deja la instalada sin identidad ni PCs
/// (la garantía es lógica: las tablas quedan vacías) y con las entradas
/// intactas.
fn assert_import_leaves_no_pairing(name: &str, pw: &str) {
    let (dir, backup) = fixture_copy_of(name);
    let dest = dir.path().join("instalada.db");
    let (mut vault, report) = Vault::import(&backup, &dest, &password(pw)).unwrap();
    assert_eq!(report, clean_report(), "{name}");
    assert_fixture_contents(&mut vault);
    assert!(matches!(vault.identity(), Ok(None)), "{name}");
    assert!(vault.paired_pcs().unwrap().valid.is_empty(), "{name}");
    drop(vault);
    assert_eq!(pairing_rows(&dest), (0, 0), "{name}");
    assert_eq!(stored_version(&dest), 3, "{name}");
}

#[test]
fn import_of_fixture_v3_leaves_no_pairing() {
    // El respaldo trae una identidad y dos PCs.
    let (_dir, backup) = fixture_copy_of("vault-v3.db");
    assert_eq!(pairing_rows(&backup), (1, 2));
    assert_import_leaves_no_pairing("vault-v3.db", FIXTURE_V3_PASSWORD);
}

#[test]
fn import_of_fixture_v2_leaves_no_pairing() {
    assert_import_leaves_no_pairing("vault-v2.db", FIXTURE_V2_PASSWORD);
}

#[test]
fn import_of_fixture_v1_leaves_no_pairing() {
    assert_import_leaves_no_pairing("vault-v1.db", FIXTURE_V1_PASSWORD);
}
