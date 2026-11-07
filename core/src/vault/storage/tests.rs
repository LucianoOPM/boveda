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

// --- Cabecera y solo lectura ---

#[test]
fn replace_header_persists_and_old_password_fails() {
    let (_dir, path, mut storage) = new_vault();
    let (new, _) = VaultHeader::create(&password("nueva"), KdfParams::MIN).unwrap();
    storage.replace_header(&new).unwrap();
    storage.close().unwrap();

    let read = Storage::open(&path).unwrap().read_header().unwrap();
    assert_eq!(read.salt(), new.salt());
    assert!(read.unlock(&password("nueva")).is_ok());
    assert_eq!(
        read.unlock(&password("correcta")).err(),
        Some(Error::WrongPassword)
    );
}

#[test]
fn header_out_of_range_value_is_corrupt_without_value() {
    let (_dir, path, storage) = new_vault();
    storage.close().unwrap();
    raw(&path)
        .execute("UPDATE header SET kdf_memory_kib = 987654321987", [])
        .unwrap();

    let err = Storage::open(&path).err().unwrap();
    assert_eq!(err, Error::Storage(StorageErrorKind::Corrupt));
    assert!(!format!("{err}").contains("987654321987"));
    assert!(!format!("{err:?}").contains("987654321987"));
}

#[test]
fn open_read_only_reads_header() {
    let (_dir, path, storage) = new_vault();
    storage.close().unwrap();
    let storage = Storage::open_read_only(&path).unwrap();
    assert!(
        storage
            .read_header()
            .unwrap()
            .unlock(&password("correcta"))
            .is_ok()
    );
}

#[test]
fn read_only_rejects_writes() {
    let (_dir, path, storage) = new_vault();
    storage.close().unwrap();
    let before = fs::read(&path).unwrap();
    let mut storage = Storage::open_read_only(&path).unwrap();
    let (other, _) = new_header();

    assert_eq!(
        storage.replace_header(&other).err(),
        Some(Error::Storage(StorageErrorKind::ReadOnly))
    );
    storage.close().unwrap();
    assert_eq!(fs::read(&path).unwrap(), before);
}

/// Ruta con un sufijo agregado al nombre del archivo (`-journal`, `-wal`...).
fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

#[test]
fn open_read_only_rejects_wal() {
    let (_dir, path, storage) = new_vault();
    storage.close().unwrap();
    {
        let conn = raw(&path);
        let _: String = conn
            .pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get(0))
            .unwrap();
    }
    let before = fs::read(&path).unwrap();
    assert!(!sibling(&path, "-wal").exists());

    let err = Storage::open_read_only(&path).err().unwrap();
    assert_eq!(err, Error::Storage(StorageErrorKind::JournalMode));
    assert_eq!(fs::read(&path).unwrap(), before);
    assert!(!sibling(&path, "-wal").exists());
    assert!(!sibling(&path, "-shm").exists());
}

/// Copia `src` a `dst` en mitad de una transacción que ya escribió páginas
/// en el archivo, junto con su `-journal`: la copia queda con un diario
/// caliente, como tras un corte de energía.
fn make_hot_journal_copy(src: &Path, dst: &Path) {
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

#[test]
fn open_read_only_rejects_hot_journal() {
    let (dir, path, storage) = new_vault();
    storage.close().unwrap();
    let copy = dir.path().join("copia.db");
    make_hot_journal_copy(&path, &copy);
    let db_before = fs::read(&copy).unwrap();
    let journal_before = fs::read(sibling(&copy, "-journal")).unwrap();

    let err = Storage::open_read_only(&copy).err().unwrap();
    assert_eq!(err, Error::Storage(StorageErrorKind::HotJournal));
    assert_eq!(fs::read(&copy).unwrap(), db_before);
    assert_eq!(
        fs::read(sibling(&copy, "-journal")).unwrap(),
        journal_before
    );
}

#[test]
fn open_read_only_missing_path_fails() {
    let (_dir, path) = temp_path();
    let err = Storage::open_read_only(&path).err().unwrap();
    assert_eq!(err, Error::Storage(StorageErrorKind::Io));
    assert!(!path.exists());
}
