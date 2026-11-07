//! Pruebas unitarias de `vault/storage.rs`.

use std::fs;
use std::path::PathBuf;

use secrecy::{ExposeSecret, SecretString};

use super::*;
use crate::crypto::kdf::KdfParams;
use crate::crypto::keys::VaultKey;

fn password(text: &str) -> SecretString {
    SecretString::from(text)
}

fn new_header() -> (VaultHeader, VaultKey) {
    VaultHeader::create(&password("correcta"), KdfParams::MIN).unwrap()
}

/// Directorio temporal (debe vivir toda la prueba) y una ruta que no existe.
fn temp_path() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("boveda.db");
    (dir, path)
}

fn new_vault() -> (tempfile::TempDir, PathBuf, Storage) {
    let (dir, path) = temp_path();
    let (header, _) = new_header();
    let storage = Storage::create(&path, &header).unwrap();
    (dir, path, storage)
}

/// Conexión directa, para leer pragmas y escribir filas anómalas.
fn raw(path: &Path) -> rusqlite::Connection {
    rusqlite::Connection::open(path).unwrap()
}

fn journal_mode(conn: &rusqlite::Connection) -> String {
    conn.pragma_query_value(None, "journal_mode", |row| row.get(0))
        .unwrap()
}

fn assert_same_header(a: &VaultHeader, b: &VaultHeader) {
    assert_eq!(a.format_version(), b.format_version());
    assert_eq!(a.algorithm(), b.algorithm());
    assert_eq!(a.params(), b.params());
    assert_eq!(a.salt(), b.salt());
    assert!(a.encrypted_key() == b.encrypted_key());
}

#[test]
fn create_close_reopen_keeps_header() {
    let (_dir, path) = temp_path();
    let (header, vault_key) = new_header();
    Storage::create(&path, &header).unwrap().close().unwrap();

    let storage = Storage::open(&path).unwrap();
    let read = storage.read_header().unwrap();
    assert_same_header(&read, &header);
    let unlocked = read.unlock(&password("correcta")).unwrap();
    assert_eq!(unlocked.expose_secret(), vault_key.expose_secret());
}

#[test]
fn create_sets_application_id() {
    let (_dir, path, storage) = new_vault();
    storage.close().unwrap();
    let id: i32 = raw(&path)
        .pragma_query_value(None, "application_id", |row| row.get(0))
        .unwrap();
    assert_eq!(id, 0x4256_4C54);
    assert_eq!(id, APPLICATION_ID);
}

#[test]
fn open_sets_pragmas() {
    let (_dir, path, storage) = new_vault();
    storage.close().unwrap();
    let storage = Storage::open(&path).unwrap();

    let secure_delete: i64 = storage
        .conn
        .pragma_query_value(None, "secure_delete", |row| row.get(0))
        .unwrap();
    let synchronous: i64 = storage
        .conn
        .pragma_query_value(None, "synchronous", |row| row.get(0))
        .unwrap();
    assert_eq!(secure_delete, 1);
    assert_eq!(journal_mode(&storage.conn), "delete");
    assert_eq!(synchronous, 2);
}

#[test]
fn open_converts_wal_back_to_delete() {
    let (_dir, path, storage) = new_vault();
    storage.close().unwrap();
    {
        let conn = raw(&path);
        let mode: String = conn
            .pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get(0))
            .unwrap();
        assert_eq!(mode, "wal");
    }

    let storage = Storage::open(&path).unwrap();
    assert_eq!(journal_mode(&storage.conn), "delete");
}

#[test]
fn create_on_existing_path_fails_and_keeps_file() {
    let (_dir, path) = temp_path();
    fs::write(&path, b"contenido previo").unwrap();
    let (header, _) = new_header();

    let err = Storage::create(&path, &header).err().unwrap();
    assert_eq!(err, Error::Storage(StorageErrorKind::AlreadyExists));
    assert_eq!(fs::read(&path).unwrap(), b"contenido previo");
}

#[test]
fn create_in_missing_directory_fails() {
    let (dir, _) = temp_path();
    let missing = dir.path().join("no-existe");
    let (header, _) = new_header();

    let err = Storage::create(&missing.join("boveda.db"), &header)
        .err()
        .unwrap();
    assert_eq!(err, Error::Storage(StorageErrorKind::Io));
    assert!(!missing.exists());
}

#[test]
fn create_and_open_non_ascii_path() {
    let (dir, _) = temp_path();
    let folder = dir.path().join("Jesús ñ");
    fs::create_dir(&folder).unwrap();
    let path = folder.join("bóveda.db");
    let (header, _) = new_header();

    Storage::create(&path, &header).unwrap().close().unwrap();
    let storage = Storage::open(&path).unwrap();
    assert_same_header(&storage.read_header().unwrap(), &header);
}

#[test]
fn open_missing_path_fails_without_creating() {
    let (_dir, path) = temp_path();
    let err = Storage::open(&path).err().unwrap();
    assert_eq!(err, Error::Storage(StorageErrorKind::Io));
    assert!(!path.exists());
}

/// Abre `path`, comprueba el error y que el archivo no cambió.
fn assert_open_fails_unchanged(path: &Path, expected: Error) {
    let before = fs::read(path).unwrap();
    let err = Storage::open(path).err().unwrap();
    assert_eq!(err, expected);
    assert_eq!(fs::read(path).unwrap(), before);
}

#[test]
fn open_rejects_text_file() {
    let (_dir, path) = temp_path();
    fs::write(&path, b"no soy sqlite ".repeat(300)).unwrap();
    assert_open_fails_unchanged(&path, Error::NotAVault);
}

#[test]
fn open_rejects_foreign_empty_sqlite() {
    let (_dir, path) = temp_path();
    raw(&path).pragma_update(None, "user_version", 1).unwrap();
    assert_open_fails_unchanged(&path, Error::NotAVault);
}

#[test]
fn open_rejects_foreign_sqlite_with_header_table() {
    let (_dir, path) = temp_path();
    raw(&path)
        .execute_batch("CREATE TABLE header (id INTEGER);")
        .unwrap();
    assert_open_fails_unchanged(&path, Error::NotAVault);
}

#[test]
fn open_rejects_future_format_version() {
    let (_dir, path, storage) = new_vault();
    storage.close().unwrap();
    raw(&path)
        .execute("UPDATE header SET format_version = 2", [])
        .unwrap();
    assert_open_fails_unchanged(&path, Error::UnsupportedFormatVersion(2));
}
