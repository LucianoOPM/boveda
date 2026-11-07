//! Pruebas unitarias de la API `Vault` (`vault/mod.rs`).

use std::fs;
use std::path::PathBuf;
use std::time::{Duration, UNIX_EPOCH};

use secrecy::SecretString;
use zeroize::ZeroizeOnDrop;

use super::*;
use crate::crypto::kdf::KdfParams;

#[test]
fn clock_before_epoch_is_an_error() {
    let before = UNIX_EPOCH - Duration::from_secs(1);
    assert_eq!(millis_since_epoch(before), Err(Error::Clock));
}

#[test]
fn clock_at_epoch_is_zero() {
    assert_eq!(millis_since_epoch(UNIX_EPOCH), Ok(0));
    assert_eq!(
        millis_since_epoch(UNIX_EPOCH + Duration::from_millis(1500)),
        Ok(1500)
    );
}

#[test]
fn now_is_after_2026() {
    // 2026-01-01T00:00:00Z
    assert!(now_millis().unwrap() > 1_767_225_600_000);
}

// --- Utilitarios ------------------------------------------------------

fn password(text: &str) -> SecretString {
    SecretString::from(text)
}

/// Directorio temporal (debe vivir toda la prueba) y una ruta que no existe.
fn temp_path() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("boveda.db");
    (dir, path)
}

fn new_vault() -> (tempfile::TempDir, PathBuf, Vault) {
    let (dir, path) = temp_path();
    let vault = Vault::create_with_params(&path, &password("correcta"), KdfParams::MIN).unwrap();
    (dir, path, vault)
}

/// Datos de búsqueda y secreto de ejemplo; la contraseña es `clave-{service}`.
fn sample(service: &str) -> (SearchData, Secret) {
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

/// Conexión directa, para leer y alterar filas.
fn raw(path: &Path) -> rusqlite::Connection {
    rusqlite::Connection::open(path).unwrap()
}

/// Servicio, URL, usuario y etiquetas de cada entrada, por id.
/// Id, servicio, URL, usuario y etiquetas.
type Row = (EntryId, String, Option<String>, Option<String>, Vec<String>);

fn snapshot(vault: &Vault) -> Vec<Row> {
    let mut rows: Vec<_> = vault
        .entries()
        .unwrap()
        .iter()
        .map(|e| {
            let s = e.search();
            (
                e.id(),
                s.service().to_owned(),
                s.url().map(str::to_owned),
                s.username().map(str::to_owned),
                s.tags().to_vec(),
            )
        })
        .collect();
    rows.sort_by_key(|r| *r.0.as_bytes());
    rows
}

fn assert_zod<T: ZeroizeOnDrop>() {}

// --- create, open, unlock, lock, entries y add --------------------------

#[test]
fn create_unlock_round_trip() {
    let (_dir, path, mut vault) = new_vault();
    for service in ["gmail", "banco", "github"] {
        let (search, secret) = sample(service);
        vault.add(search, secret).unwrap();
    }
    let before = snapshot(&vault);
    assert_eq!(before.len(), 3);

    vault.lock();
    let report = vault.unlock(&password("correcta")).unwrap();
    assert_eq!(
        report,
        UnlockReport {
            corrupt: vec![],
            unidentified_corrupt: 0
        }
    );
    assert_eq!(snapshot(&vault), before);

    drop(vault);
    let mut reopened = Vault::open(&path).unwrap();
    reopened.unlock(&password("correcta")).unwrap();
    assert_eq!(snapshot(&reopened), before);
}

#[test]
fn open_returns_locked_vault() {
    let (_dir, path, vault) = new_vault();
    drop(vault);
    let vault = Vault::open(&path).unwrap();
    assert!(vault.is_locked());
    assert_eq!(vault.entries().err(), Some(Error::Locked));
}

#[test]
fn wrong_password_keeps_vault_locked() {
    let (_dir, _path, mut vault) = new_vault();
    vault.lock();
    assert_eq!(
        vault.unlock(&password("incorrecta")).err(),
        Some(Error::WrongPassword)
    );
    assert!(vault.is_locked());
    assert_eq!(vault.entries().err(), Some(Error::Locked));
}

#[test]
fn non_ascii_master_password() {
    let (_dir, path) = temp_path();
    let mut vault =
        Vault::create_with_params(&path, &password("contraseña 🔐"), KdfParams::MIN).unwrap();
    vault.lock();
    assert_eq!(
        vault.unlock(&password("contrasena 🔐")).err(),
        Some(Error::WrongPassword)
    );
    assert!(vault.unlock(&password("contraseña 🔐")).is_ok());
}

#[test]
fn add_after_failed_unlock_is_locked() {
    let (_dir, path, mut vault) = new_vault();
    vault.lock();
    let _ = vault.unlock(&password("incorrecta"));
    let (search, secret) = sample("gmail");
    assert_eq!(vault.add(search, secret).err(), Some(Error::Locked));
    let count: i64 = raw(&path)
        .query_row("SELECT count(*) FROM entries", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn unlock_twice_is_already_unlocked() {
    let (_dir, _path, mut vault) = new_vault();
    let (search, secret) = sample("gmail");
    vault.add(search, secret).unwrap();
    let before = snapshot(&vault);
    assert_eq!(
        vault.unlock(&password("correcta")).err(),
        Some(Error::AlreadyUnlocked)
    );
    assert_eq!(snapshot(&vault), before);
}

#[test]
fn lock_is_idempotent() {
    let (_dir, _path, mut vault) = new_vault();
    vault.lock();
    vault.lock();
    assert!(vault.is_locked());
    assert_eq!(vault.entries().err(), Some(Error::Locked));
}

#[test]
fn summary_zeroizes_on_drop() {
    assert_zod::<EntrySummary>();
    assert_zod::<SearchData>();
}

#[test]
fn add_sets_modified_at_and_no_last_used() {
    let (_dir, _path, mut vault) = new_vault();
    let (search, secret) = sample("gmail");
    let id = vault.add(search, secret).unwrap();
    let entries = vault.entries().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].id(), id);
    assert!(entries[0].modified_at() > 0);
    assert_eq!(entries[0].last_used_at(), None);
}

#[test]
fn create_on_existing_path_is_already_exists() {
    let (_dir, path) = temp_path();
    fs::write(&path, b"previo").unwrap();
    let err = Vault::create_with_params(&path, &password("correcta"), KdfParams::MIN).err();
    assert_eq!(err, Some(Error::AlreadyExists));
    assert_eq!(fs::read(&path).unwrap(), b"previo");
}

#[test]
fn create_failure_leaves_path_free() {
    let (_dir, path) = temp_path();
    // Un directorio donde SQLite quiere crear su diario: la transacción falla.
    fs::create_dir(path.with_file_name("boveda.db-journal")).unwrap();
    let result = Vault::create_with_params(&path, &password("correcta"), KdfParams::MIN);
    assert!(result.is_err());
    assert!(!path.exists());
}

#[test]
fn unlock_storage_failure_keeps_locked() {
    let (_dir, path, mut vault) = new_vault();
    let (search, secret) = sample("gmail");
    vault.add(search, secret).unwrap();
    vault.lock();
    raw(&path).execute_batch("DROP TABLE entries").unwrap();
    assert!(matches!(
        vault.unlock(&password("correcta")),
        Err(Error::Storage(_))
    ));
    assert!(vault.is_locked());
}
