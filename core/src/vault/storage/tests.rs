//! Pruebas unitarias de `vault/storage.rs`.

use std::fs;
use std::path::PathBuf;

use secrecy::{ExposeSecret, SecretString};

use super::*;
use crate::crypto::aead::EncryptedPart;
use crate::crypto::kdf::KdfParams;
use crate::crypto::keys::VaultKey;
use crate::vault::entry::{ENTRY_SCHEMA_VERSION, EntryId};
use crate::vault::test_support::*;

fn new_header() -> (VaultHeader, VaultKey) {
    VaultHeader::create(&password("correcta"), KdfParams::MIN).unwrap()
}

fn new_vault() -> (tempfile::TempDir, PathBuf, Storage) {
    let (dir, path) = temp_path();
    let (header, _) = new_header();
    let storage = Storage::create(&path, &header).unwrap();
    (dir, path, storage)
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
    set_wal(&raw(&path));

    let storage = Storage::open(&path).unwrap();
    assert_eq!(journal_mode(&storage.conn), "delete");
}

#[test]
fn create_on_existing_path_fails_and_keeps_file() {
    let (_dir, path) = temp_path();
    fs::write(&path, b"contenido previo").unwrap();
    let (header, _) = new_header();

    let err = Storage::create(&path, &header).err().unwrap();
    assert_eq!(err, Error::AlreadyExists);
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
        .execute("UPDATE header SET format_version = 3", [])
        .unwrap();
    assert_open_fails_unchanged(&path, Error::UnsupportedFormatVersion(3));
}

/// Pasa a WAL la base de `conn` y comprueba el modo resultante.
fn set_wal(conn: &rusqlite::Connection) {
    let mode: String = conn
        .pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get(0))
        .unwrap();
    assert_eq!(mode, "wal");
}

#[test]
fn open_rejects_future_format_version_in_wal() {
    let (_dir, path, storage) = new_vault();
    storage.close().unwrap();
    {
        let conn = raw(&path);
        conn.execute("UPDATE header SET format_version = 3", [])
            .unwrap();
        set_wal(&conn);
    }
    assert!(!sibling(&path, "-wal").exists());

    assert_open_fails_unchanged(&path, Error::UnsupportedFormatVersion(3));
    assert!(!sibling(&path, "-wal").exists());
    assert!(!sibling(&path, "-shm").exists());
}

/// Límite conocido: si la bóveda futura trae transacciones pendientes en su
/// `-wal`, el checkpoint de SQLite al cerrar la última conexión las integra
/// al archivo principal. Los bytes cambian, pero el contenido se conserva:
/// sigue en WAL y con su `format_version`.
#[test]
fn open_future_wal_with_pending_frames_checkpoints_and_keeps_content() {
    let (dir, src, storage) = new_vault();
    storage.close().unwrap();
    let path = dir.path().join("futura-copia.db");
    {
        let conn = raw(&src);
        set_wal(&conn);
        conn.pragma_update(None, "wal_autocheckpoint", 0).unwrap();
        conn.execute("UPDATE header SET format_version = 3", [])
            .unwrap();
        // Copia sin checkpoint, como tras un cierre forzado.
        fs::copy(&src, &path).unwrap();
        fs::copy(sibling(&src, "-wal"), sibling(&path, "-wal")).unwrap();
    }
    assert!(fs::metadata(sibling(&path, "-wal")).unwrap().len() > 0);
    let before = fs::read(&path).unwrap();

    assert_eq!(
        Storage::open(&path).err(),
        Some(Error::UnsupportedFormatVersion(3))
    );
    // El checkpoint al cerrar integró el `-wal` y lo borró.
    assert!(!sibling(&path, "-wal").exists());
    assert_ne!(fs::read(&path).unwrap(), before);

    let conn = raw(&path);
    assert_eq!(journal_mode(&conn), "wal");
    let version: i64 = conn
        .query_row(
            "SELECT format_version FROM header WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(version, 3);
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
    let (_dir, path, mut storage) = new_vault();
    storage
        .insert_entry(&fake_entry(1, b"busqueda", b"secreto"), T1)
        .unwrap();
    storage.close().unwrap();
    let before = fs::read(&path).unwrap();
    let mut storage = Storage::open_read_only(&path).unwrap();
    let (other, _) = new_header();
    let id = EntryId::from_bytes([1; 16]);
    let read_only = Some(Error::Storage(StorageErrorKind::ReadOnly));

    assert_eq!(storage.replace_header(&other).err(), read_only);
    assert_eq!(
        storage.insert_entry(&fake_entry(2, b"b", b"s"), T2).err(),
        read_only
    );
    assert_eq!(
        storage.update_entry(&fake_entry(1, b"b", b"s"), T2).err(),
        read_only
    );
    assert_eq!(storage.delete_entry(id, T2).err(), read_only);
    assert_eq!(storage.touch_entry(id, T2).err(), read_only);
    assert!(storage.get_entry(id).is_ok());
    assert_eq!(storage.list_entries().unwrap().entries.len(), 1);
    storage.close().unwrap();
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn open_read_only_with_relative_path_is_io_and_keeps_vault() {
    let (_dir, absolute, relative) = relative_temp_path();
    let (header, _) = new_header();
    Storage::create(&absolute, &header)
        .unwrap()
        .close()
        .unwrap();
    let before = fs::read(&absolute).unwrap();

    assert_eq!(
        Storage::open_read_only(&relative).err(),
        Some(Error::Storage(StorageErrorKind::Io))
    );
    assert_eq!(fs::read(&absolute).unwrap(), before);
}

/// En Windows, `C:boveda.db` (relativa a la unidad) y `\boveda.db`
/// (relativa a la raíz de la unidad actual) no son absolutas.
#[cfg(windows)]
#[test]
fn require_absolute_rejects_windows_partial_paths() {
    let io = Err(Error::Storage(StorageErrorKind::Io));
    assert_eq!(require_absolute(Path::new(r"C:boveda.db")), io);
    assert_eq!(require_absolute(Path::new(r"\boveda.db")), io);
    assert_eq!(require_absolute(Path::new(r"C:\boveda.db")), Ok(()));
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

// --- Entradas ---

const T1: i64 = 1_700_000_000_000;
const T2: i64 = T1 + 1_000;
const T3: i64 = T1 + 2_000;

/// Entrada con partes arbitrarias, sin cifrar: el almacenamiento no las
/// interpreta.
fn fake_entry(id_byte: u8, search: &[u8], secret: &[u8]) -> SealedEntry {
    SealedEntry::from_parts(
        EntryId::from_bytes([id_byte; 16]),
        ENTRY_SCHEMA_VERSION,
        EncryptedPart::from_bytes(search.to_vec()),
        EncryptedPart::from_bytes(secret.to_vec()),
    )
}

fn assert_parts(stored: &StoredEntry, search: &[u8], secret: &[u8]) {
    assert_eq!(stored.sealed.search_bytes().as_bytes(), search);
    assert_eq!(stored.sealed.secret_bytes().as_bytes(), secret);
    assert_eq!(stored.sealed.entry_schema_version(), ENTRY_SCHEMA_VERSION);
}

/// Lee con SQL directo `(deleted, modified_at, search IS NULL, secret IS NULL)`.
fn raw_row(path: &Path, id: EntryId) -> (i64, i64, bool, bool) {
    raw(path)
        .query_row(
            "SELECT deleted, modified_at, search IS NULL, secret IS NULL \
             FROM entries WHERE id = ?1",
            [id.as_bytes().as_slice()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap()
}

#[test]
fn insert_then_get_returns_same_parts() {
    let (_dir, _path, mut storage) = new_vault();
    storage
        .insert_entry(&fake_entry(1, b"busqueda", b"secreto"), T1)
        .unwrap();

    let stored = storage.get_entry(EntryId::from_bytes([1; 16])).unwrap();
    assert_parts(&stored, b"busqueda", b"secreto");
    assert_eq!(stored.sealed.id(), EntryId::from_bytes([1; 16]));
    assert_eq!(stored.modified_at, T1);
    assert_eq!(stored.last_used_at, None);
}

#[test]
fn entries_survive_reopen() {
    let (_dir, path, mut storage) = new_vault();
    storage
        .insert_entry(&fake_entry(1, b"busqueda", b"secreto"), T1)
        .unwrap();
    storage.close().unwrap();

    let storage = Storage::open(&path).unwrap();
    let stored = storage.get_entry(EntryId::from_bytes([1; 16])).unwrap();
    assert_parts(&stored, b"busqueda", b"secreto");
    assert_eq!(stored.modified_at, T1);
}

#[test]
fn touch_does_not_change_modified_at() {
    let (_dir, _path, mut storage) = new_vault();
    let id = EntryId::from_bytes([1; 16]);
    storage
        .insert_entry(&fake_entry(1, b"b", b"s"), T1)
        .unwrap();
    storage.touch_entry(id, T2).unwrap();

    let stored = storage.get_entry(id).unwrap();
    assert_eq!(stored.modified_at, T1);
    assert_eq!(stored.last_used_at, Some(T2));
}

#[test]
fn update_replaces_parts_and_keeps_last_used_at() {
    let (_dir, _path, mut storage) = new_vault();
    let id = EntryId::from_bytes([1; 16]);
    storage
        .insert_entry(&fake_entry(1, b"b", b"s"), T1)
        .unwrap();
    storage.touch_entry(id, T2).unwrap();
    storage
        .update_entry(&fake_entry(1, b"otra", b"otro"), T3)
        .unwrap();

    let stored = storage.get_entry(id).unwrap();
    assert_parts(&stored, b"otra", b"otro");
    assert_eq!(stored.modified_at, T3);
    assert_eq!(stored.last_used_at, Some(T2));
}

#[test]
fn delete_leaves_tombstone() {
    let (_dir, path, mut storage) = new_vault();
    let id1 = EntryId::from_bytes([1; 16]);
    storage
        .insert_entry(&fake_entry(1, b"b1", b"s1"), T1)
        .unwrap();
    storage
        .insert_entry(&fake_entry(2, b"b2", b"s2"), T1)
        .unwrap();
    storage.delete_entry(id1, T2).unwrap();

    let listing = storage.list_entries().unwrap();
    assert_eq!(listing.entries.len(), 1);
    assert_eq!(listing.entries[0].sealed.id(), EntryId::from_bytes([2; 16]));
    assert!(listing.corrupt.is_empty());
    assert_eq!(listing.unidentified_corrupt, 0);
    assert_eq!(storage.get_entry(id1).err(), Some(Error::EntryNotFound));
    assert_eq!(raw_row(&path, id1), (1, T2, true, true));
}

#[test]
fn delete_twice_fails_and_keeps_tombstone() {
    let (_dir, path, mut storage) = new_vault();
    let id = EntryId::from_bytes([1; 16]);
    storage
        .insert_entry(&fake_entry(1, b"b", b"s"), T1)
        .unwrap();
    storage.delete_entry(id, T2).unwrap();

    assert_eq!(
        storage.delete_entry(id, T3).err(),
        Some(Error::EntryNotFound)
    );
    assert_eq!(raw_row(&path, id), (1, T2, true, true));
}

#[test]
fn missing_or_deleted_entry_is_not_found() {
    let (_dir, _path, mut storage) = new_vault();
    let deleted = EntryId::from_bytes([1; 16]);
    storage
        .insert_entry(&fake_entry(1, b"b", b"s"), T1)
        .unwrap();
    storage.delete_entry(deleted, T2).unwrap();

    for (byte, id) in [(9, EntryId::from_bytes([9; 16])), (1, deleted)] {
        let not_found = Some(Error::EntryNotFound);
        assert_eq!(
            storage
                .update_entry(&fake_entry(byte, b"x", b"y"), T3)
                .err(),
            not_found
        );
        assert_eq!(storage.delete_entry(id, T3).err(), not_found);
        assert_eq!(storage.touch_entry(id, T3).err(), not_found);
        assert_eq!(storage.get_entry(id).err(), not_found);
        assert_eq!(storage.get_last_used_at(id).err(), not_found);
    }
}

#[test]
fn get_last_used_at_reads_unreadable_row() {
    let (_dir, path, mut storage) = new_vault();
    let id = EntryId::from_bytes([1; 16]);
    storage
        .insert_entry(&fake_entry(1, b"b", b"s"), T1)
        .unwrap();
    assert_eq!(storage.get_last_used_at(id), Ok(None));

    storage.touch_entry(id, T2).unwrap();
    raw(&path)
        .execute(
            "UPDATE entries SET entry_schema_version = 70000 WHERE id = ?1",
            [id.as_bytes().as_slice()],
        )
        .unwrap();
    assert_eq!(storage.get_entry(id).err(), Some(Error::CorruptEntry));
    assert_eq!(storage.get_last_used_at(id), Ok(Some(T2)));
}

#[test]
fn tombstone_check_constraint_both_ways() {
    let (_dir, path, mut storage) = new_vault();
    let id = EntryId::from_bytes([1; 16]);
    storage
        .insert_entry(&fake_entry(1, b"b", b"s"), T1)
        .unwrap();
    storage.delete_entry(id, T2).unwrap();
    storage.close().unwrap();
    let conn = raw(&path);

    let live_without_parts = conn.execute(
        "INSERT INTO entries (id, entry_schema_version, search, secret, modified_at) \
         VALUES (?1, 1, NULL, x'00', 0)",
        [[2_u8; 16].as_slice()],
    );
    assert!(live_without_parts.is_err());
    let tombstone_with_part = conn.execute(
        "UPDATE entries SET secret = x'00' WHERE id = ?1",
        [id.as_bytes().as_slice()],
    );
    assert!(tombstone_with_part.is_err());
}

#[test]
fn extreme_timestamps_round_trip() {
    for t in [i64::MIN, 0, i64::MAX] {
        let (_dir, _path, mut storage) = new_vault();
        let id = EntryId::from_bytes([1; 16]);
        storage.insert_entry(&fake_entry(1, b"b", b"s"), t).unwrap();
        assert_eq!(storage.get_entry(id).unwrap().modified_at, t);
        storage.touch_entry(id, t).unwrap();
        storage.update_entry(&fake_entry(1, b"b", b"s"), t).unwrap();
        let stored = storage.get_entry(id).unwrap();
        assert_eq!(stored.modified_at, t);
        assert_eq!(stored.last_used_at, Some(t));
    }
}

mod props {
    use proptest::prelude::*;

    use super::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(32))]

        #[test]
        fn prop_parts_round_trip(
            search in proptest::collection::vec(any::<u8>(), 0..=20_000),
            secret in proptest::collection::vec(any::<u8>(), 0..=20_000),
        ) {
            let (_dir, _path, mut storage) = new_vault();
            storage.insert_entry(&fake_entry(1, &search, &secret), T1).unwrap();
            let stored = storage.get_entry(EntryId::from_bytes([1; 16])).unwrap();
            prop_assert_eq!(stored.sealed.search_bytes().as_bytes(), search.as_slice());
            prop_assert_eq!(stored.sealed.secret_bytes().as_bytes(), secret.as_slice());
        }
    }
}

// --- Filas dañadas, archivo dañado y texto plano ---

/// Escribe una fila anómala con SQL directo.
fn raw_insert(path: &Path, id: &[u8], version: i64) {
    raw(path)
        .execute(
            "INSERT INTO entries (id, entry_schema_version, search, secret, modified_at) \
             VALUES (?1, ?2, x'00', x'00', 0)",
            rusqlite::params![id, version],
        )
        .unwrap();
}

#[test]
fn corrupt_rows_are_reported_without_blocking() {
    let (_dir, path, mut storage) = new_vault();
    storage
        .insert_entry(&fake_entry(1, b"b1", b"s1"), T1)
        .unwrap();
    storage
        .insert_entry(&fake_entry(2, b"b2", b"s2"), T1)
        .unwrap();
    raw_insert(&path, &[1_u8; 15], 1);
    raw_insert(&path, &[9_u8; 16], 70_000);

    let listing = storage.list_entries().unwrap();
    assert_eq!(listing.entries.len(), 2);
    assert_eq!(listing.unidentified_corrupt, 1);
    assert_eq!(listing.corrupt, vec![EntryId::from_bytes([9; 16])]);
    assert_eq!(
        storage.get_entry(EntryId::from_bytes([9; 16])).err(),
        Some(Error::CorruptEntry)
    );
}

#[test]
fn row_conversion_error_hides_value() {
    let (_dir, path, storage) = new_vault();
    raw_insert(&path, &[7_u8; 16], 987_654_321);

    let err = storage
        .get_entry(EntryId::from_bytes([7; 16]))
        .err()
        .unwrap();
    assert_eq!(err, Error::CorruptEntry);
    assert!(!format!("{err}").contains("987654321"));
    assert!(!format!("{err:?}").contains("987654321"));
}

#[test]
fn damaged_entries_page_is_storage_corrupt() {
    let (_dir, path, mut storage) = new_vault();
    for i in 1..=3 {
        storage
            .insert_entry(&fake_entry(i, b"b", b"s"), T1)
            .unwrap();
    }
    storage.close().unwrap();
    damage_entries_root_page(&path);

    let storage = Storage::open(&path).unwrap();
    assert_eq!(
        storage.list_entries().err(),
        Some(Error::Storage(StorageErrorKind::Corrupt))
    );
}

#[test]
fn file_contains_no_known_plaintext() {
    use crate::vault::entry::{SearchData, Secret};

    let (_dir, path) = temp_path();
    let (header, key) = new_header();
    let mut storage = Storage::create(&path, &header).unwrap();
    let seal = |n: u32| {
        let search = SearchData::new(
            &format!("canario-servicio-{n}"),
            Some(&format!("https://canario-url-{n}.example")),
            Some(&format!("canario-usuario-{n}")),
            &[&format!("canario-etiqueta-{n}")],
        )
        .unwrap();
        let secret = Secret::new(
            SecretString::from(format!("canario-pass-{n}")),
            Some(SecretString::from(format!("canario-notas-{n}"))),
        )
        .unwrap();
        SealedEntry::seal(&key, EntryId::from_bytes([n as u8; 16]), &search, &secret).unwrap()
    };
    for n in 1..=3 {
        storage.insert_entry(&seal(n), T1).unwrap();
    }
    storage.update_entry(&seal(1), T2).unwrap();
    storage
        .delete_entry(EntryId::from_bytes([2; 16]), T2)
        .unwrap();
    // Control: la misma búsqueda sí detecta texto plano guardado sin cifrar.
    storage
        .insert_entry(&fake_entry(9, b"control-en-claro", b"x"), T2)
        .unwrap();
    storage.close().unwrap();

    let bytes = fs::read(&path).unwrap();
    assert!(contains(&bytes, b"control-en-claro"));
    assert!(!contains(&bytes, b"canario-"));
    assert!(!sibling(&path, "-journal").exists());
}

// --- Correcciones de la revisión final ---

/// SQLite ajeno en WAL con transacciones pendientes en su `-wal`, copiado
/// mientras la conexión de origen sigue abierta (sin checkpoint).
fn make_foreign_wal_copy(dir: &Path) -> PathBuf {
    let src = dir.join("ajena.db");
    let dst = dir.join("ajena-copia.db");
    let conn = raw(&src);
    let _: String = conn
        .pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get(0))
        .unwrap();
    conn.pragma_update(None, "wal_autocheckpoint", 0).unwrap();
    conn.execute_batch("CREATE TABLE t (x BLOB); INSERT INTO t VALUES (randomblob(3000));")
        .unwrap();
    fs::copy(&src, &dst).unwrap();
    fs::copy(sibling(&src, "-wal"), sibling(&dst, "-wal")).unwrap();
    drop(conn);
    assert!(fs::metadata(sibling(&dst, "-wal")).unwrap().len() > 0);
    dst
}

#[test]
fn open_does_not_touch_foreign_wal_database() {
    let (dir, _) = temp_path();
    let path = make_foreign_wal_copy(dir.path());
    let db_before = fs::read(&path).unwrap();
    let wal_before = fs::read(sibling(&path, "-wal")).unwrap();

    assert_eq!(Storage::open(&path).err(), Some(Error::NotAVault));
    assert_eq!(fs::read(&path).unwrap(), db_before);
    assert_eq!(fs::read(sibling(&path, "-wal")).unwrap(), wal_before);
    assert!(!sibling(&path, "-shm").exists());
}

#[test]
fn open_read_only_rejects_foreign_wal_as_not_a_vault() {
    let (dir, _) = temp_path();
    let path = make_foreign_wal_copy(dir.path());
    assert_eq!(Storage::open_read_only(&path).err(), Some(Error::NotAVault));
}

/// Prueba `open` y `open_read_only` sobre una bóveda con la cabecera
/// alterada por `sql` (simula una versión futura del formato).
fn assert_future_header_is_unsupported(sql: &str) {
    let (_dir, path, storage) = new_vault();
    storage.close().unwrap();
    raw(&path).execute_batch(sql).unwrap();

    assert_eq!(
        Storage::open_read_only(&path).err(),
        Some(Error::UnsupportedFormatVersion(3))
    );
    assert_eq!(
        Storage::open(&path).err(),
        Some(Error::UnsupportedFormatVersion(3))
    );
}

#[test]
fn future_header_without_v1_column_is_unsupported() {
    assert_future_header_is_unsupported(
        "UPDATE header SET format_version = 3; ALTER TABLE header DROP COLUMN kdf_memory_kib;",
    );
}

#[test]
fn future_header_with_wider_value_is_unsupported() {
    assert_future_header_is_unsupported(
        "UPDATE header SET format_version = 3, kdf_memory_kib = 99999999999;",
    );
}

#[test]
fn format_version_out_of_u16_is_corrupt() {
    let (_dir, path, storage) = new_vault();
    storage.close().unwrap();
    raw(&path)
        .execute("UPDATE header SET format_version = 70000", [])
        .unwrap();
    assert_eq!(
        Storage::open(&path).err(),
        Some(Error::Storage(StorageErrorKind::Corrupt))
    );
}

// --- Respaldos (#9) ----------------------------------------------------

#[test]
fn vacuum_into_relative_path_is_io_and_creates_nothing() {
    let (_vault_dir, _path, storage) = new_vault();
    let (_dir, absolute, relative) = relative_temp_path();

    assert_eq!(
        storage.vacuum_into(&relative),
        Err(Error::Storage(StorageErrorKind::Io))
    );
    assert!(!absolute.exists());
}

#[test]
fn vacuum_into_copies_with_synchronous_full() {
    let (dir, _path, mut storage) = new_vault();
    storage
        .insert_entry(&fake_entry(1, b"b1", b"s1"), T1)
        .unwrap();
    storage
        .insert_entry(&fake_entry(2, b"b2", b"s2"), T1)
        .unwrap();
    // Con `synchronous = FULL`, SQLite hace el fsync del archivo generado
    // por `VACUUM INTO` (lang_vacuum.html). Si esto cambia, hay que
    // sincronizar el respaldo a mano.
    let synchronous: i64 = storage
        .conn
        .pragma_query_value(None, "synchronous", |row| row.get(0))
        .unwrap();
    assert_eq!(synchronous, 2);

    let dest = dir.path().join("respaldo.db");
    fs::File::create(&dest).unwrap();
    storage.vacuum_into(&dest).unwrap();

    let copy = Storage::open_read_only(&dest).unwrap();
    let listing = copy.list_entries().unwrap();
    assert_eq!(listing.entries.len(), 2);
    let one = copy.get_entry(EntryId::from_bytes([1; 16])).unwrap();
    assert_parts(&one, b"b1", b"s1");
}

#[test]
fn check_integrity_ok_on_new_vault() {
    let (_dir, _path, storage) = new_vault();
    assert_eq!(storage.check_integrity(), Ok(()));
}

#[test]
fn check_integrity_detects_damaged_page() {
    let (_dir, path, mut storage) = new_vault();
    for i in 1..=3 {
        storage
            .insert_entry(&fake_entry(i, b"b", b"s"), T1)
            .unwrap();
    }
    storage.close().unwrap();
    damage_entries_root_page(&path);

    let storage = Storage::open_read_only(&path).unwrap();
    assert_eq!(
        storage.check_integrity(),
        Err(Error::Storage(StorageErrorKind::Corrupt))
    );
}

// --- Verificador de la llave y formato v2 (#34) -------------------------

#[test]
fn created_vault_stores_key_check() {
    let (_dir, path) = temp_path();
    let (header, key) = new_header();
    let storage = Storage::create(&path, &header).unwrap();
    let (version, len): (i64, i64) = raw(&path)
        .query_row(
            "SELECT format_version, length(key_check) FROM header WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!((version, len), (2, 40));
    assert_eq!(storage.read_header().unwrap().verify_key(&key), Ok(()));
}

#[test]
fn replace_header_writes_key_check() {
    let (_dir, path) = temp_path();
    let (header, key) = new_header();
    let mut storage = Storage::create(&path, &header).unwrap();
    let before = *storage.read_header().unwrap().key_check().unwrap();

    let rekeyed = VaultHeader::rekey(&key, &password("nueva"), KdfParams::MIN).unwrap();
    storage.replace_header(&rekeyed).unwrap();

    let stored = storage.read_header().unwrap();
    assert_ne!(*stored.key_check().unwrap(), before);
    assert_eq!(stored.key_check(), rekeyed.key_check());
    assert_eq!(stored.verify_key(&key), Ok(()));
}

#[test]
fn migrate_header_on_v2_does_nothing() {
    let (_dir, path) = temp_path();
    let (header, _) = new_header();
    let mut storage = Storage::create(&path, &header).unwrap();
    let before = fs::read(&path).unwrap();

    let (other, _) = new_header();
    assert_eq!(
        storage.migrate_header(&other).unwrap(),
        Migration::NotNeeded
    );
    drop(storage);
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn migrate_header_without_header_row_is_corrupt() {
    let (_dir, path) = fixture_v1_copy();
    let mut storage = Storage::open(&path).unwrap();
    let (header, _) = new_header();
    raw(&path).execute("DELETE FROM header", []).unwrap();
    assert_eq!(
        storage.migrate_header(&header).err(),
        Some(Error::Storage(StorageErrorKind::Corrupt))
    );
}
