//! Pruebas unitarias de la API `Vault` (`vault/mod.rs`).

use std::fs;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use secrecy::ExposeSecret;
use zeroize::ZeroizeOnDrop;

use super::*;
use crate::crypto::kdf::{DERIVE_CALLS, KdfParams};
use crate::vault::test_support::*;

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

fn new_vault() -> (tempfile::TempDir, PathBuf, Vault) {
    let (dir, path) = temp_path();
    let vault =
        Vault::create_with_params(&path, &password("correcta-larga"), KdfParams::MIN).unwrap();
    (dir, path, vault)
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

// --- Rutas relativas (#22) ----------------------------------------------

/// `file:<relativa>?mode=ro`, con `/` como separador: una URI que SQLite
/// interpretaría por `SQLITE_USE_URI`.
///
/// Los casos con esta URI son solo de regresión, no un rojo real: aun sin
/// `require_absolute`, las comprobaciones previas de Rust fallan sobre el
/// archivo literal `file:…`, que no existe. Reproducir el riesgo exigiría
/// crear ese archivo literal bajo `core/` (en Windows ni siquiera es un
/// nombre válido) o cambiar el directorio de trabajo, global al proceso.
fn uri_of(relative: &Path) -> PathBuf {
    let path = relative.to_str().unwrap().replace('\\', "/");
    PathBuf::from(format!("file:{path}?mode=ro"))
}

#[test]
fn create_with_relative_path_is_io_and_creates_nothing() {
    let (_dir, absolute, relative) = relative_temp_path();
    let io = Some(Error::Storage(StorageErrorKind::Io));

    for path in [relative.clone(), uri_of(&relative)] {
        assert_eq!(
            Vault::create_with_params(&path, &password("correcta-larga"), KdfParams::MIN).err(),
            io
        );
        assert!(!absolute.exists());
    }
    // `Vault::create` también da `Storage(Io)`. La comprobación temprana
    // busca evitar la calibración; esta prueba no lo verifica (sin ella,
    // `Storage::create` daría el mismo error después de calibrar).
    assert_eq!(
        Vault::create(&relative, &password("correcta-larga")).err(),
        io
    );
    assert!(!absolute.exists());
}

#[test]
fn open_with_relative_path_is_io_and_keeps_vault() {
    let (_dir, absolute, relative) = relative_temp_path();
    drop(
        Vault::create_with_params(&absolute, &password("correcta-larga"), KdfParams::MIN).unwrap(),
    );
    let before = fs::read(&absolute).unwrap();

    for path in [relative.clone(), uri_of(&relative)] {
        assert_eq!(
            Vault::open(&path).err(),
            Some(Error::Storage(StorageErrorKind::Io))
        );
        assert_eq!(fs::read(&absolute).unwrap(), before);
    }
}

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
    let report = vault.unlock(&password("correcta-larga")).unwrap();
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
    reopened.unlock(&password("correcta-larga")).unwrap();
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
        vault.unlock(&password("correcta-larga")).err(),
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
    let err = Vault::create_with_params(&path, &password("correcta-larga"), KdfParams::MIN).err();
    assert_eq!(err, Some(Error::AlreadyExists));
    assert_eq!(fs::read(&path).unwrap(), b"previo");
}

#[test]
fn create_failure_leaves_path_free() {
    let (_dir, path) = temp_path();
    // Un directorio donde SQLite quiere crear su diario: la transacción falla.
    fs::create_dir(path.with_file_name("boveda.db-journal")).unwrap();
    let result = Vault::create_with_params(&path, &password("correcta-larga"), KdfParams::MIN);
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
        vault.unlock(&password("correcta-larga")),
        Err(Error::Storage(_))
    ));
    assert!(vault.is_locked());
}

// --- edit, delete y reveal ---------------------------------------------

/// `search`, `secret`, `modified_at` y `last_used_at` de la fila.
type RowBytes = (Option<Vec<u8>>, Option<Vec<u8>>, i64, Option<i64>);

fn row_bytes(path: &Path, id: EntryId) -> RowBytes {
    raw(path)
        .query_row(
            "SELECT search, secret, modified_at, last_used_at FROM entries WHERE id = ?1",
            [id.as_bytes().as_slice()],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap()
}

fn add_sample(vault: &mut Vault, service: &str) -> EntryId {
    let (search, secret) = sample(service);
    vault.add(search, secret).unwrap()
}

fn search_with_tags(service: &str, tags: &[&str]) -> SearchData {
    SearchData::new(
        service,
        Some(&format!("https://{service}.example")),
        Some("usuario@example.com"),
        tags,
    )
    .unwrap()
}

fn find(vault: &Vault, id: EntryId) -> &EntrySummary {
    vault
        .entries()
        .unwrap()
        .iter()
        .find(|e| e.id() == id)
        .unwrap()
}

#[test]
fn locked_operations_fail() {
    let (_dir, _path, mut vault) = new_vault();
    let id = add_sample(&mut vault, "gmail");
    vault.lock();
    let (search, secret) = sample("otro");
    assert_eq!(vault.entries().err(), Some(Error::Locked));
    assert_eq!(vault.add(search, secret).err(), Some(Error::Locked));
    let (search, _) = sample("otro");
    assert_eq!(
        vault.edit(id, search, SecretChange::Keep).err(),
        Some(Error::Locked)
    );
    assert_eq!(vault.delete(id).err(), Some(Error::Locked));
    assert_eq!(vault.reveal(id).err(), Some(Error::Locked));
}

#[test]
fn reveal_returns_secret_and_touches() {
    let (_dir, path, mut vault) = new_vault();
    let id = add_sample(&mut vault, "gmail");
    let secret = vault.reveal(id).unwrap();
    assert_eq!(secret.password().expose_secret(), "clave-gmail");
    assert_eq!(
        secret.notes().map(|n| n.expose_secret()),
        Some("notas de prueba")
    );
    let cached = find(&vault, id).last_used_at();
    assert!(cached.is_some());
    assert_eq!(row_bytes(&path, id).3, cached);
}

#[test]
fn reveal_corrupt_secret() {
    let (_dir, path, mut vault) = new_vault();
    let id = add_sample(&mut vault, "gmail");
    tamper(&path, id, "secret");
    assert_eq!(vault.reveal(id).err(), Some(Error::CorruptEntry));
    assert_eq!(row_bytes(&path, id).3, None);
    assert_eq!(find(&vault, id).last_used_at(), None);
}

#[test]
fn reveal_missing_id() {
    let (_dir, _path, mut vault) = new_vault();
    assert_eq!(
        vault.reveal(EntryId::from_bytes([7; 16])).err(),
        Some(Error::EntryNotFound)
    );
}

#[test]
fn reveal_survives_write_failure() {
    let (_dir, _path, mut vault) = new_vault();
    let id = add_sample(&mut vault, "gmail");
    vault.storage.deny_writes_for_test();
    let secret = vault.reveal(id).unwrap();
    assert_eq!(secret.password().expose_secret(), "clave-gmail");
    let (search, secret) = sample("otro");
    assert_eq!(
        vault.add(search, secret).err(),
        Some(Error::Storage(crate::error::StorageErrorKind::ReadOnly))
    );
}

#[test]
fn edit_keep_changes_only_search() {
    let (_dir, path, mut vault) = new_vault();
    let id = add_sample(&mut vault, "gmail");
    vault.reveal(id).unwrap();
    let before = row_bytes(&path, id);

    vault
        .edit(
            id,
            search_with_tags("gmail", &["personal"]),
            SecretChange::Keep,
        )
        .unwrap();

    let after = row_bytes(&path, id);
    assert_ne!(after.0, before.0);
    assert!(after.2 >= before.2);
    assert_eq!(after.3, before.3);
    let summary = find(&vault, id);
    assert_eq!(summary.search().tags(), ["personal"]);
    assert_eq!(summary.last_used_at(), before.3);
    assert_eq!(summary.modified_at(), after.2);
    assert_eq!(
        vault.reveal(id).unwrap().password().expose_secret(),
        "clave-gmail"
    );
}

#[test]
fn edit_keep_with_corrupt_secret() {
    let (_dir, path, mut vault) = new_vault();
    let id = add_sample(&mut vault, "gmail");
    tamper(&path, id, "secret");
    let before = row_bytes(&path, id);
    assert_eq!(
        vault
            .edit(id, search_with_tags("gmail", &["x"]), SecretChange::Keep)
            .err(),
        Some(Error::CorruptEntry)
    );
    assert_eq!(row_bytes(&path, id), before);
}

#[test]
fn edit_replace_persists() {
    let (_dir, _path, mut vault) = new_vault();
    let id = add_sample(&mut vault, "gmail");
    let secret = Secret::new(password("nueva"), None).unwrap();
    vault
        .edit(
            id,
            search_with_tags("gmail", &[]),
            SecretChange::Replace(secret),
        )
        .unwrap();
    vault.lock();
    vault.unlock(&password("correcta-larga")).unwrap();
    let revealed = vault.reveal(id).unwrap();
    assert_eq!(revealed.password().expose_secret(), "nueva");
    assert!(revealed.notes().is_none());
}

#[test]
fn edit_missing_or_deleted() {
    let (_dir, _path, mut vault) = new_vault();
    let missing = EntryId::from_bytes([7; 16]);
    assert_eq!(
        vault
            .edit(missing, search_with_tags("x", &[]), SecretChange::Keep)
            .err(),
        Some(Error::EntryNotFound)
    );
    let id = add_sample(&mut vault, "gmail");
    vault.delete(id).unwrap();
    let secret = Secret::new(password("nueva"), None).unwrap();
    assert_eq!(
        vault
            .edit(
                id,
                search_with_tags("x", &[]),
                SecretChange::Replace(secret)
            )
            .err(),
        Some(Error::EntryNotFound)
    );
}

#[test]
fn delete_makes_tombstone() {
    let (_dir, path, mut vault) = new_vault();
    let id = add_sample(&mut vault, "gmail");
    vault.delete(id).unwrap();
    assert!(vault.entries().unwrap().is_empty());
    let deleted: i64 = raw(&path)
        .query_row(
            "SELECT deleted FROM entries WHERE id = ?1",
            [id.as_bytes().as_slice()],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(deleted, 1);
    let row = row_bytes(&path, id);
    assert_eq!((row.0, row.1), (None, None));
}

#[test]
fn delete_twice() {
    let (_dir, _path, mut vault) = new_vault();
    let id = add_sample(&mut vault, "gmail");
    add_sample(&mut vault, "banco");
    vault.delete(id).unwrap();
    assert_eq!(vault.delete(id).err(), Some(Error::EntryNotFound));
    assert_eq!(vault.entries().unwrap().len(), 1);
}

#[test]
fn reveal_and_edit_do_not_leak_in_errors() {
    // Contraseñas que no aparecen dentro de ningún mensaje ("incorrecta" contiene "correcta").
    let (_dir, path) = temp_path();
    let mut vault =
        Vault::create_with_params(&path, &password("maestra-Xy7-larga"), KdfParams::MIN).unwrap();
    let id = add_sample(&mut vault, "gmail");
    tamper(&path, id, "secret");
    let mut errors = vec![
        vault.reveal(id).err().unwrap(),
        vault
            .edit(id, search_with_tags("gmail", &[]), SecretChange::Keep)
            .err()
            .unwrap(),
        vault.reveal(EntryId::from_bytes([7; 16])).err().unwrap(),
    ];
    vault.lock();
    errors.push(vault.unlock(&password("intento-Qz4")).err().unwrap());
    errors.push(vault.reveal(id).err().unwrap());
    for e in errors {
        let text = format!("{e} {e:?}");
        for secret in ["maestra-Xy7", "intento-Qz4", "clave-", "gmail"] {
            assert!(!text.contains(secret), "{text}");
        }
    }
}

// --- Entradas dañadas, lápidas y reparación ----------------------------

fn relock(vault: &mut Vault) -> UnlockReport {
    vault.lock();
    vault.unlock(&password("correcta-larga")).unwrap()
}

fn ids(vault: &Vault) -> Vec<EntryId> {
    vault.entries().unwrap().iter().map(|e| e.id()).collect()
}

#[test]
fn unlock_reports_corrupt_search() {
    let (_dir, path, mut vault) = new_vault();
    let a = add_sample(&mut vault, "a");
    let b = add_sample(&mut vault, "b");
    let c = add_sample(&mut vault, "c");
    tamper(&path, b, "search");

    let report = relock(&mut vault);
    assert_eq!(report.corrupt, vec![b]);
    assert_eq!(report.unidentified_corrupt, 0);
    let valid = ids(&vault);
    assert_eq!(valid.len(), 2);
    assert!(valid.contains(&a) && valid.contains(&c));
}

#[test]
fn unlock_counts_unidentified_rows() {
    let (_dir, path, mut vault) = new_vault();
    let a = add_sample(&mut vault, "a");
    raw(&path)
        .execute(
            "INSERT INTO entries (id, entry_schema_version, search, secret, modified_at) \
             VALUES (?1, 1, x'00', x'00', 0)",
            [[1_u8; 15].as_slice()],
        )
        .unwrap();

    let report = relock(&mut vault);
    assert_eq!(report.unidentified_corrupt, 1);
    assert!(report.corrupt.is_empty());
    assert_eq!(ids(&vault), vec![a]);
}

#[test]
fn tombstone_not_reported() {
    let (_dir, _path, mut vault) = new_vault();
    let a = add_sample(&mut vault, "a");
    let b = add_sample(&mut vault, "b");
    vault.delete(b).unwrap();

    let report = relock(&mut vault);
    assert!(report.corrupt.is_empty());
    assert_eq!(ids(&vault), vec![a]);
}

#[test]
fn replace_repairs_corrupt_search() {
    let (_dir, path, mut vault) = new_vault();
    add_sample(&mut vault, "a");
    let b = add_sample(&mut vault, "b");
    tamper(&path, b, "search");
    assert_eq!(relock(&mut vault).corrupt, vec![b]);

    let (search, secret) = sample("b");
    vault
        .edit(b, search, SecretChange::Replace(secret))
        .unwrap();
    assert!(ids(&vault).contains(&b));

    let report = relock(&mut vault);
    assert!(report.corrupt.is_empty());
    assert!(ids(&vault).contains(&b));
}

#[test]
fn replace_repairs_corrupt_secret() {
    let (_dir, path, mut vault) = new_vault();
    let b = add_sample(&mut vault, "b");
    tamper(&path, b, "secret");
    let secret = Secret::new(password("reparada"), None).unwrap();
    vault
        .edit(b, search_with_tags("b", &[]), SecretChange::Replace(secret))
        .unwrap();
    assert_eq!(
        vault.reveal(b).unwrap().password().expose_secret(),
        "reparada"
    );
}

#[test]
fn delete_corrupt_entry() {
    let (_dir, path, mut vault) = new_vault();
    add_sample(&mut vault, "a");
    let b = add_sample(&mut vault, "b");
    tamper(&path, b, "search");
    assert_eq!(relock(&mut vault).corrupt, vec![b]);

    vault.delete(b).unwrap();
    assert!(relock(&mut vault).corrupt.is_empty());
}

/// Fija una columna entera de la fila de `id` por fuera de la API.
fn set_column(path: &Path, id: EntryId, column: &str, value: i64) {
    raw(path)
        .execute(
            &format!("UPDATE entries SET {column} = ?1 WHERE id = ?2"),
            rusqlite::params![value, id.as_bytes().as_slice()],
        )
        .unwrap();
}

/// Ids en el orden de `search("")`: por uso reciente.
fn recent_order(vault: &Vault) -> Vec<EntryId> {
    vault.search("").unwrap().iter().map(|e| e.id()).collect()
}

/// Repara con `Replace` una entrada con `entry_schema_version = version`
/// (fuera de `u16`) que tenía un último uso: lo conserva en la caché y en el
/// archivo, también tras `lock` y `unlock`.
fn assert_replace_repairs_schema_version(version: i64) {
    let (_dir, path, mut vault) = new_vault();
    let a = add_sample(&mut vault, "a");
    let b = add_sample(&mut vault, "b");
    set_column(&path, a, "last_used_at", 1_000);
    set_column(&path, b, "last_used_at", 2_000);
    set_column(&path, b, "entry_schema_version", version);
    assert_eq!(relock(&mut vault).corrupt, vec![b]);

    let secret = Secret::new(password("reparada"), None).unwrap();
    vault
        .edit(b, search_with_tags("b", &[]), SecretChange::Replace(secret))
        .unwrap();
    assert_eq!(find(&vault, b).last_used_at(), Some(2_000));
    assert_eq!(row_bytes(&path, b).3, Some(2_000));
    assert_eq!(recent_order(&vault), vec![b, a]);

    let report = relock(&mut vault);
    assert!(report.corrupt.is_empty());
    assert_eq!(find(&vault, b).last_used_at(), Some(2_000));
    assert_eq!(recent_order(&vault), vec![b, a]);
    assert_eq!(
        vault.reveal(b).unwrap().password().expose_secret(),
        "reparada"
    );
}

#[test]
fn replace_repairs_schema_version_above_u16() {
    assert_replace_repairs_schema_version(70_000);
}

#[test]
fn replace_repairs_negative_schema_version() {
    assert_replace_repairs_schema_version(-1);
}

#[test]
fn keep_with_out_of_range_schema_version_is_corrupt_and_unchanged() {
    let (_dir, path, mut vault) = new_vault();
    let b = add_sample(&mut vault, "b");
    set_column(&path, b, "entry_schema_version", 70_000);
    relock(&mut vault);
    let before = fs::read(&path).unwrap();

    assert_eq!(
        vault
            .edit(b, search_with_tags("b", &[]), SecretChange::Keep)
            .err(),
        Some(Error::CorruptEntry)
    );
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn replace_on_missing_entry_is_not_found() {
    let (_dir, _path, mut vault) = new_vault();
    let missing = EntryId::from_bytes([7; 16]);
    let secret = Secret::new(password("nueva"), None).unwrap();
    assert_eq!(
        vault
            .edit(
                missing,
                search_with_tags("x", &[]),
                SecretChange::Replace(secret)
            )
            .err(),
        Some(Error::EntryNotFound)
    );
}

// --- Reemplazo de una sola parte del secreto ---------------------------

fn replace_password(text: &str) -> SecretChange {
    SecretChange::ReplacePassword(password(text))
}

fn replace_notes(text: Option<&str>) -> SecretChange {
    SecretChange::ReplaceNotes(text.map(password))
}

#[test]
fn replace_password_keeps_notes() {
    let (_dir, _path, mut vault) = new_vault();
    let id = add_sample(&mut vault, "gmail");
    vault
        .edit(
            id,
            search_with_tags("gmail", &[]),
            replace_password("nueva"),
        )
        .unwrap();
    let revealed = vault.reveal(id).unwrap();
    assert_eq!(revealed.password().expose_secret(), "nueva");
    assert_eq!(revealed.notes().unwrap().expose_secret(), "notas de prueba");

    // También sobre una entrada sin notas.
    let search = search_with_tags("sin-notas", &[]);
    let secret = Secret::new(password("vieja"), None).unwrap();
    let bare = vault.add(search, secret).unwrap();
    vault
        .edit(
            bare,
            search_with_tags("sin-notas", &[]),
            replace_password("otra"),
        )
        .unwrap();
    let revealed = vault.reveal(bare).unwrap();
    assert_eq!(revealed.password().expose_secret(), "otra");
    assert!(revealed.notes().is_none());
}

#[test]
fn replace_notes_keeps_password() {
    let (_dir, _path, mut vault) = new_vault();
    let id = add_sample(&mut vault, "gmail");
    vault
        .edit(
            id,
            search_with_tags("gmail", &[]),
            replace_notes(Some("notas nuevas")),
        )
        .unwrap();
    let revealed = vault.reveal(id).unwrap();
    assert_eq!(revealed.password().expose_secret(), "clave-gmail");
    assert_eq!(revealed.notes().unwrap().expose_secret(), "notas nuevas");

    vault
        .edit(
            id,
            search_with_tags("gmail", &[]),
            replace_notes(Some("  ")),
        )
        .unwrap();
    let revealed = vault.reveal(id).unwrap();
    assert_eq!(revealed.password().expose_secret(), "clave-gmail");
    assert!(revealed.notes().is_none());

    vault
        .edit(id, search_with_tags("gmail", &[]), replace_notes(Some("x")))
        .unwrap();
    vault
        .edit(id, search_with_tags("gmail", &[]), replace_notes(None))
        .unwrap();
    assert!(vault.reveal(id).unwrap().notes().is_none());
}

#[test]
fn partial_replace_updates_modified_not_last_used() {
    for change in [replace_password("nueva"), replace_notes(Some("otras"))] {
        let (_dir, path, mut vault) = new_vault();
        let id = add_sample(&mut vault, "gmail");
        vault.reveal(id).unwrap();
        let before = row_bytes(&path, id);

        vault
            .edit(id, search_with_tags("gmail", &["personal"]), change)
            .unwrap();

        let after = row_bytes(&path, id);
        assert_ne!(after.0, before.0);
        assert_ne!(after.1, before.1);
        assert!(after.2 >= before.2);
        assert_eq!(after.3, before.3);
        let summary = find(&vault, id);
        assert_eq!(summary.search().tags(), ["personal"]);
        assert_eq!(summary.modified_at(), after.2);
        assert_eq!(summary.last_used_at(), before.3);
    }
}

#[test]
fn partial_replace_on_corrupt_secret_is_corrupt_and_unchanged() {
    for change in [replace_password("nueva"), replace_notes(Some("otras"))] {
        let (_dir, path, mut vault) = new_vault();
        let id = add_sample(&mut vault, "gmail");
        tamper(&path, id, "secret");
        let before = fs::read(&path).unwrap();
        assert_eq!(
            vault.edit(id, search_with_tags("gmail", &[]), change).err(),
            Some(Error::CorruptEntry)
        );
        assert_eq!(fs::read(&path).unwrap(), before);
    }
}

#[test]
fn partial_replace_on_unreadable_row_is_corrupt_and_unchanged() {
    for change in [replace_password("nueva"), replace_notes(Some("otras"))] {
        let (_dir, path, mut vault) = new_vault();
        let id = add_sample(&mut vault, "b");
        set_column(&path, id, "entry_schema_version", 70_000);
        relock(&mut vault);
        let before = fs::read(&path).unwrap();
        assert_eq!(
            vault.edit(id, search_with_tags("b", &[]), change).err(),
            Some(Error::CorruptEntry)
        );
        assert_eq!(fs::read(&path).unwrap(), before);
    }
}

#[test]
fn partial_replace_with_invalid_value_is_invalid_and_unchanged() {
    use crate::error::InvalidEntryField::{NotesTooLong, PasswordEmpty, PasswordTooLong};
    let cases = [
        (replace_password(""), PasswordEmpty),
        (replace_password(&"a".repeat(1025)), PasswordTooLong),
        (replace_notes(Some(&"n".repeat(16385))), NotesTooLong),
    ];
    for (change, field) in cases {
        let (_dir, path, mut vault) = new_vault();
        let id = add_sample(&mut vault, "gmail");
        let before = fs::read(&path).unwrap();
        assert_eq!(
            vault.edit(id, search_with_tags("gmail", &[]), change).err(),
            Some(Error::InvalidEntry(field))
        );
        assert_eq!(fs::read(&path).unwrap(), before);
    }
}

#[test]
fn invalid_value_wins_over_corrupt_secret() {
    use crate::error::InvalidEntryField::{NotesTooLong, PasswordEmpty};
    let cases = [
        (replace_password(""), PasswordEmpty),
        (replace_notes(Some(&"n".repeat(16385))), NotesTooLong),
    ];
    for (change, field) in cases {
        let (_dir, path, mut vault) = new_vault();
        let id = add_sample(&mut vault, "gmail");
        tamper(&path, id, "secret");
        assert_eq!(
            vault.edit(id, search_with_tags("gmail", &[]), change).err(),
            Some(Error::InvalidEntry(field))
        );
    }
}

#[test]
fn partial_replace_locked_and_missing() {
    let (_dir, _path, mut vault) = new_vault();
    let id = add_sample(&mut vault, "gmail");
    let deleted = add_sample(&mut vault, "borrada");
    vault.delete(deleted).unwrap();
    let missing = EntryId::from_bytes([7; 16]);
    for target in [missing, deleted] {
        assert_eq!(
            vault
                .edit(target, search_with_tags("x", &[]), replace_password("n"))
                .err(),
            Some(Error::EntryNotFound)
        );
        assert_eq!(
            vault
                .edit(target, search_with_tags("x", &[]), replace_notes(None))
                .err(),
            Some(Error::EntryNotFound)
        );
    }
    vault.lock();
    assert_eq!(
        vault
            .edit(id, search_with_tags("x", &[]), replace_password("n"))
            .err(),
        Some(Error::Locked)
    );
    assert_eq!(
        vault
            .edit(id, search_with_tags("x", &[]), replace_notes(None))
            .err(),
        Some(Error::Locked)
    );
}

#[test]
fn partial_replace_does_not_leak_in_debug_or_errors() {
    let texts = [
        format!("{:?}", replace_password("clave-Zk9-secreta")),
        format!("{:?}", replace_notes(Some("notas-Zk9-secretas"))),
    ];
    for text in texts {
        assert!(!text.contains("Zk9"), "{text}");
    }

    let (_dir, path, mut vault) = new_vault();
    let id = add_sample(&mut vault, "gmail");
    tamper(&path, id, "secret");
    for change in [
        replace_password("clave-Zk9-secreta"),
        replace_notes(Some("notas-Zk9-secretas")),
    ] {
        let e = vault
            .edit(id, search_with_tags("gmail", &[]), change)
            .err()
            .unwrap();
        let text = format!("{e} {e:?}");
        assert!(!text.contains("Zk9"), "{text}");
    }
}

// --- change_master_password --------------------------------------------

/// La fila de `header` completa: sal, nonce y llave cifrada.
fn header_row(path: &Path) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    raw(path)
        .query_row(
            "SELECT salt, key_nonce, encrypted_key FROM header WHERE id = 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap()
}

#[test]
fn change_password_keeps_entries() {
    let (_dir, path, mut vault) = new_vault();
    let a = add_sample(&mut vault, "a");
    let b = add_sample(&mut vault, "b");
    let rows_before = (row_bytes(&path, a), row_bytes(&path, b));
    let before = snapshot(&vault);

    vault
        .change_master_password_with_params(
            &password("correcta-larga"),
            &password("nueva-maestra"),
            KdfParams::MIN,
        )
        .unwrap();
    vault.lock();

    assert_eq!(
        vault.unlock(&password("correcta-larga")).err(),
        Some(Error::WrongPassword)
    );
    vault.unlock(&password("nueva-maestra")).unwrap();
    assert_eq!(snapshot(&vault), before);
    assert_eq!((row_bytes(&path, a), row_bytes(&path, b)), rows_before);
}

#[test]
fn change_password_wrong_current() {
    let (_dir, path, mut vault) = new_vault();
    let before = header_row(&path);
    assert_eq!(
        vault
            .change_master_password_with_params(
                &password("incorrecta"),
                &password("nueva-maestra"),
                KdfParams::MIN,
            )
            .err(),
        Some(Error::WrongPassword)
    );
    assert_eq!(header_row(&path), before);
}

#[test]
fn change_password_locked() {
    let (_dir, _path, mut vault) = new_vault();
    vault.lock();
    assert_eq!(
        vault
            .change_master_password_with_params(
                &password("correcta-larga"),
                &password("nueva-maestra"),
                KdfParams::MIN,
            )
            .err(),
        Some(Error::Locked)
    );
}

#[test]
fn change_password_to_same() {
    let (_dir, path, mut vault) = new_vault();
    let before = header_row(&path);
    vault
        .change_master_password_with_params(
            &password("correcta-larga"),
            &password("correcta-larga"),
            KdfParams::MIN,
        )
        .unwrap();
    assert_ne!(header_row(&path).0, before.0);
    vault.lock();
    assert!(vault.unlock(&password("correcta-larga")).is_ok());
}

// --- Búsqueda ----------------------------------------------------------

#[test]
fn search_when_locked_is_an_error() {
    let (_dir, _path, mut vault) = new_vault();
    vault.lock();
    assert_eq!(vault.search("gm").err(), Some(Error::Locked));
}

#[test]
fn search_skips_deleted_entries() {
    let (_dir, _path, mut vault) = new_vault();
    let (search, secret) = sample("gmail");
    let deleted = vault.add(search, secret).unwrap();
    let (search, secret) = sample("banco");
    vault.add(search, secret).unwrap();
    vault.delete(deleted).unwrap();

    assert_eq!(vault.search("").unwrap().len(), 1);
    assert!(
        vault
            .search("gmail")
            .unwrap()
            .iter()
            .all(|e| e.id() != deleted)
    );
}

#[test]
fn search_after_reveal_puts_entry_first() {
    let (_dir, _path, mut vault) = new_vault();
    let mut added = Vec::new();
    for service in ["uno", "dos", "tres"] {
        let (search, secret) = sample(service);
        added.push(vault.add(search, secret).unwrap());
    }
    vault.reveal(added[1]).unwrap();
    assert_eq!(vault.search("").unwrap()[0].id(), added[1]);
}

#[test]
fn search_never_matches_password() {
    let (_dir, _path, mut vault) = new_vault();
    let search = SearchData::new(
        "Banco",
        Some("https://banco.example"),
        Some("usuario@example.com"),
        &["trabajo", "correo"],
    )
    .unwrap();
    let secret = Secret::new(password("zqxw-secreto-98"), None).unwrap();
    vault.add(search, secret).unwrap();
    assert!(vault.search("zqxw-secreto-98").unwrap().is_empty());
}

// --- Bóvedas de ejemplo ---------------------------------------------------

/// Llena una bóveda de ejemplo con el contenido descrito en
/// `core/tests/fixtures/README.md`. La usan todos los generadores.
fn fill_fixture(vault: &mut Vault) {
    // Todos los campos opcionales, con acentos y un salto de línea.
    let full = SearchData::new(
        "Correo Electrónico",
        Some("https://correo.example.com/entrar"),
        Some("ana@example.com"),
        &["Trabajo", "Médico", "Año"],
    )
    .unwrap();
    let full_secret = Secret::new(
        password("clave-completa-ñ"),
        Some(password("Línea uno\nLínea dos")),
    )
    .unwrap();
    vault.add(full, full_secret).unwrap();

    let minimal = |service: &str, key: &str| {
        (
            SearchData::new(service, None, None, &[]).unwrap(),
            Secret::new(password(key), None).unwrap(),
        )
    };
    let (search, secret) = minimal("Banco", "clave-minima");
    vault.add(search, secret).unwrap();
    let (search, secret) = minimal("Nube", "clave-nube");
    let used = vault.add(search, secret).unwrap();
    let (search, secret) = minimal("Eliminada", "clave-eliminada");
    let deleted = vault.add(search, secret).unwrap();

    // `reveal` escribe `last_used_at`; `delete` deja la lápida.
    vault.reveal(used).unwrap();
    vault.delete(deleted).unwrap();
}

/// Crea la bóveda de ejemplo `name` en `core/tests/fixtures/` con
/// `KdfParams::MIN`. Si el archivo ya existe, falla a propósito con
/// `AlreadyExists` sin tocarlo.
fn generate_fixture(name: &str, fixture_password: &str) -> Vault {
    // Ruta absoluta, como exigen las reglas de rutas (`SQLITE_USE_URI`).
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    // Sin borrar nada antes: si el archivo existe, `AlreadyExists`.
    let mut vault =
        Vault::create_with_params(&path, &password(fixture_password), KdfParams::MIN).unwrap();
    fill_fixture(&mut vault);
    vault
}

/// Generó `core/tests/fixtures/vault-v1.db`, la bóveda de ejemplo del
/// formato 1, que las pruebas de `core/tests/vault.rs` deben abrir siempre.
///
/// Desde el formato 2, `create_with_params` crea bóvedas de la versión 2,
/// así que este generador ya no reproduce `vault-v1.db`: se conserva como
/// registro de cómo se generó. Nunca lo ejecutes.
///
/// Cada versión de formato nueva lleva un archivo nuevo y su propio
/// generador; los archivos existentes no se modifican ni se borran.
/// Contenido y contraseña: `core/tests/fixtures/README.md`.
#[test]
#[ignore = "registro: generó vault-v1.db con el formato 1; ya no lo reproduce"]
fn generate_fixture_v1() {
    drop(generate_fixture("vault-v1.db", FIXTURE_V1_PASSWORD));
}

/// Genera `core/tests/fixtures/vault-v2.db`, la bóveda de ejemplo del
/// formato 2, con el mismo contenido que la del formato 1.
///
/// Se ejecutó una sola vez:
/// `cargo test -p vault-core --lib -- --ignored --exact vault::tests::generate_fixture_v2`
///
/// Nunca con `--ignored` sin filtro: también correría los generadores, que
/// fallarían con `AlreadyExists` (o crearían una bóveda distinta si falta
/// el archivo).
#[test]
#[ignore = "solo al introducir una versión de formato nueva"]
fn generate_fixture_v2() {
    drop(generate_fixture("vault-v2.db", FIXTURE_V2_PASSWORD));
}

/// Genera `core/tests/fixtures/vault-v3.db`, la bóveda de ejemplo del
/// formato 3: el mismo contenido que las anteriores, una identidad fija
/// (`[7u8; 32]`) y dos PCs emparejadas.
///
/// Se ejecutó una sola vez:
/// `cargo test -p vault-core --lib -- --ignored --exact vault::tests::generate_fixture_v3`
///
/// Nunca con `--ignored` sin filtro: también correría los demás generadores.
#[test]
#[ignore = "solo al introducir una versión de formato nueva"]
fn generate_fixture_v3() {
    let mut vault = generate_fixture("vault-v3.db", FIXTURE_V3_PASSWORD);
    vault.set_identity_for_test([0x07; 32]).unwrap();

    // Una PC con IP (y por eso con fecha de la última vez) y otra sin IP.
    let desktop = vault
        .add_paired_pc(NewPairedPc {
            public_key: [0x11; 32],
            name: "Escritorio".to_owned(),
            last_ip: None,
        })
        .unwrap();
    vault
        .update_paired_pc(
            desktop,
            PairedPcUpdate {
                name: None,
                last_ip: Some("192.168.1.50".parse().unwrap()),
            },
        )
        .unwrap();
    vault
        .add_paired_pc(NewPairedPc {
            public_key: [0x22; 32],
            name: "Portátil".to_owned(),
            last_ip: None,
        })
        .unwrap();
}

// --- Migración del formato 1 al 2 (#34) ----------------------------------

#[test]
fn unlock_migrates_v1_to_v3_with_one_derivation() {
    let (_dir, path) = fixture_v1_copy();
    let mut vault = Vault::open(&path).unwrap();
    assert_eq!(vault.storage.read_header().unwrap().format_version(), 1);

    DERIVE_CALLS.with(|c| c.set(0));
    let report = vault.unlock(&password(FIXTURE_V1_PASSWORD)).unwrap();
    assert_eq!(DERIVE_CALLS.with(|c| c.get()), 1);

    assert!(report.corrupt.is_empty());
    assert_eq!(report.unidentified_corrupt, 0);
    assert_eq!(vault.entries().unwrap().len(), 3);
    let header = vault.storage.read_header().unwrap();
    assert_eq!(header.format_version(), 3);
    assert_eq!(header.verify_key(&vault.unlocked().unwrap().key), Ok(()));
    drop(vault);
    assert_eq!(pairing_row_counts(&path), (0, 0));
}

#[test]
fn migrated_schema_equals_created_schema_from_v1_and_from_v2() {
    let (_dir2, created) = temp_path();
    drop(Vault::create_with_params(&created, &password("p-de-prueba-12"), KdfParams::MIN).unwrap());

    let (_dir1, from_v1) = fixture_v1_copy();
    let (_dir_2, from_v2) = fixture_v2_copy();
    for (path, pw) in [
        (&from_v1, FIXTURE_V1_PASSWORD),
        (&from_v2, FIXTURE_V2_PASSWORD),
    ] {
        let mut vault = Vault::open(path).unwrap();
        vault.unlock(&password(pw)).unwrap();
        drop(vault);

        assert_eq!(schema_of(path), schema_of(&created));
        let integrity: String = raw(path)
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .unwrap();
        assert_eq!(integrity, "ok");
    }
}

#[test]
fn failed_migration_keeps_v1_byte_for_byte() {
    let (_dir, path) = fixture_v1_copy();
    let mut vault = Vault::open(&path).unwrap();
    let before = fs::read(&path).unwrap();
    vault.storage.deny_writes_for_test();

    let err = vault.unlock(&password(FIXTURE_V1_PASSWORD)).unwrap_err();
    assert!(matches!(err, Error::Storage(_)), "{err:?}");
    assert!(vault.is_locked());
    drop(vault);
    assert_eq!(fs::read(&path).unwrap(), before);
}

/// Una migración que falla a mitad de la transacción se revierte entera
/// (#53). `deny_writes_for_test` la hace fallar en su primer paso, antes de
/// escribir nada, así que no distingue una migración con transacción de una
/// sin ella. Aquí la vista `v` apunta a `header`: con `legacy_alter_table`
/// apagado, el `ALTER TABLE header_v2 RENAME TO header` falla, y eso ocurre
/// después del `CREATE`, el `INSERT` y el `DROP`. Sin la transacción, el
/// archivo quedaría sin la tabla `header`.
#[test]
fn failed_migration_rolls_back_after_dropping_the_old_header() {
    let (_dir, path) = fixture_v1_copy();
    let conn = raw(&path);
    conn.execute_batch("CREATE VIEW v AS SELECT * FROM header")
        .unwrap();
    // Guardia: la prueba depende de que SQLite deje borrar `header` pero no
    // renombrar otra tabla a ese nombre mientras la vista la mencione. Si una
    // versión nueva de SQLite lo cambiara, falla aquí y no más abajo, con un
    // `unwrap_err` confuso. Se revierte: el archivo no cambia.
    conn.execute_batch("BEGIN; CREATE TABLE t2 (x); DROP TABLE header")
        .expect("DROP TABLE header debe poder ejecutarse");
    assert!(
        conn.execute_batch("ALTER TABLE t2 RENAME TO header")
            .is_err(),
        "con la vista v, el RENAME debe fallar"
    );
    conn.execute_batch("ROLLBACK").unwrap();
    drop(conn);

    let mut vault = Vault::open(&path).unwrap();
    let before = fs::read(&path).unwrap();

    let err = vault.unlock(&password(FIXTURE_V1_PASSWORD)).unwrap_err();
    assert!(matches!(err, Error::Storage(_)), "{err:?}");
    assert!(vault.is_locked());
    drop(vault);

    assert_eq!(fs::read(&path).unwrap(), before);
    let (version, header_rows): (i64, i64) = raw(&path)
        .query_row(
            "SELECT format_version, (SELECT count(*) FROM header) FROM header WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!((version, header_rows), (1, 1));
    let integrity: String = raw(&path)
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .unwrap();
    assert_eq!(integrity, "ok");
}

#[test]
fn wrong_password_on_v1_writes_nothing() {
    let (_dir, path) = fixture_v1_copy();
    let mut vault = Vault::open(&path).unwrap();
    let before = fs::read(&path).unwrap();

    assert_eq!(
        vault.unlock(&password("incorrecta")).err(),
        Some(Error::WrongPassword)
    );
    assert!(vault.is_locked());
    drop(vault);
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn v3_unlock_writes_nothing() {
    let (_dir, path, mut vault) = new_vault();
    assert_eq!(vault.storage.read_header().unwrap().format_version(), 3);
    vault.lock();
    let before = fs::read(&path).unwrap();
    relock(&mut vault);
    drop(vault);
    assert_eq!(fs::read(&path).unwrap(), before);
}

// --- Migración del formato 2 al 3 (#93) ----------------------------------

/// Llave de bóveda de `vault-v2.db`, sacada de una copia desbloqueada con
/// contraseña (lo que el Keystore habría guardado).
fn fixture_v2_vault_key() -> SecretBox<[u8; 32]> {
    let (_dir, path) = fixture_v2_copy();
    let mut vault = Vault::open(&path).unwrap();
    vault.unlock(&password(FIXTURE_V2_PASSWORD)).unwrap();
    vault.export_vault_key().unwrap()
}

#[test]
fn unlock_migrates_v2_to_v3_without_identity() {
    let (_dir, path) = fixture_v2_copy();
    let mut vault = Vault::open(&path).unwrap();
    assert_eq!(vault.storage.read_header().unwrap().format_version(), 2);

    DERIVE_CALLS.with(|c| c.set(0));
    let report = vault.unlock(&password(FIXTURE_V2_PASSWORD)).unwrap();
    assert_eq!(DERIVE_CALLS.with(|c| c.get()), 1);

    assert!(report.corrupt.is_empty());
    assert_eq!(vault.entries().unwrap().len(), 3);
    let header = vault.storage.read_header().unwrap();
    let key = &vault.unlocked().unwrap().key;
    assert_eq!(header.format_version(), 3);
    assert_eq!(header.verify_key(key), Ok(()));
    // `key_check` quedó regenerado para la versión 3: con la 2 no se autentica.
    assert_eq!(
        crate::crypto::keys::verify_key_check(key, 2, header.key_check().unwrap()),
        Err(Error::WrongVaultKey)
    );
    drop(vault);
    assert_eq!(pairing_row_counts(&path), (0, 0));
}

#[test]
fn unlock_with_key_migrates_v2_to_v3_and_keystore_copy_still_works() {
    let key = fixture_v2_vault_key();
    let (_dir, path) = fixture_v2_copy();
    let mut vault = Vault::open(&path).unwrap();
    assert_eq!(stored_version_of(&path), 2);

    let report = vault.unlock_with_key(key).unwrap();
    assert!(report.corrupt.is_empty());
    assert_eq!(vault.entries().unwrap().len(), 3);
    assert_eq!(stored_version_of(&path), 3);
    assert_eq!(pairing_row_counts(&path), (0, 0));

    // El `key_check` guardado es el de la versión 3 y corresponde a la llave,
    // comprobado en el archivo SIN pasar por `export_vault_key`, que repara en
    // silencio un verificador dañado y taparía uno mal escrito.
    let header = vault.storage.read_header().unwrap();
    let in_memory = &vault.unlocked().unwrap().key;
    assert_eq!(header.verify_key(in_memory), Ok(()));
    assert_eq!(
        crate::crypto::keys::verify_key_check(in_memory, 2, header.key_check().unwrap()),
        Err(Error::WrongVaultKey)
    );

    // La copia del Keystore sigue valiendo: la MISMA llave original (la que
    // guardó el Keystore antes de migrar), sin pedirla de nuevo a la bóveda.
    vault.lock();
    assert!(vault.unlock_with_key(fixture_v2_vault_key()).is_ok());
    // Y la contraseña sigue abriendo.
    vault.lock();
    assert!(vault.unlock(&password(FIXTURE_V2_PASSWORD)).is_ok());
}

#[test]
fn unlock_with_key_on_v1_is_wrong_vault_key_and_writes_nothing() {
    let (_dir, path) = fixture_v1_copy();
    let mut vault = Vault::open(&path).unwrap();
    let before = fs::read(&path).unwrap();
    assert_eq!(
        vault.unlock_with_key(fixture_v2_vault_key()).err(),
        Some(Error::WrongVaultKey)
    );
    assert!(vault.is_locked());
    drop(vault);
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn wrong_vault_key_on_v2_writes_nothing() {
    let (_dir, path) = fixture_v2_copy();
    let mut vault = Vault::open(&path).unwrap();
    let before = fs::read(&path).unwrap();
    assert_eq!(
        vault.unlock_with_key(random_key()).err(),
        Some(Error::WrongVaultKey)
    );
    assert!(vault.is_locked());
    drop(vault);
    assert_eq!(fs::read(&path).unwrap(), before);
    assert_eq!(stored_version_of(&path), 2);
}

#[test]
fn wrong_password_on_v2_writes_nothing() {
    let (_dir, path) = fixture_v2_copy();
    let mut vault = Vault::open(&path).unwrap();
    let before = fs::read(&path).unwrap();
    assert_eq!(
        vault.unlock(&password("incorrecta")).err(),
        Some(Error::WrongPassword)
    );
    drop(vault);
    assert_eq!(fs::read(&path).unwrap(), before);
}

/// Si la v1→v2 se confirma y la v2→v3 falla, el archivo queda en una v2
/// válida y la bóveda sigue bloqueada. Aquí la v2→v3 falla porque ya existe
/// una tabla `identity` (el mismo caso que una v3 degradada a mano).
#[test]
fn v1_to_v2_committed_then_v2_to_v3_failed_leaves_valid_v2_locked() {
    let (_dir, path) = fixture_v1_copy();
    raw(&path)
        .execute_batch("CREATE TABLE identity (x)")
        .unwrap();
    let mut vault = Vault::open(&path).unwrap();

    let err = vault.unlock(&password(FIXTURE_V1_PASSWORD)).unwrap_err();
    assert_eq!(err, Error::Storage(StorageErrorKind::Corrupt));
    assert!(vault.is_locked());
    drop(vault);

    assert_eq!(stored_version_of(&path), 2);
    let integrity: String = raw(&path)
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .unwrap();
    assert_eq!(integrity, "ok");
    // Sigue siendo una v2 válida que se puede abrir.
    let storage = Storage::open_read_only(&path).unwrap();
    assert_eq!(storage.read_header().unwrap().format_version(), 2);
}

// Degradación: una v3 con `format_version` bajado a 2 a mano.
//
// - Con la contraseña abre (`unlock` no lee `key_check` y el dato autenticado
//   de `encrypted_key` lleva la versión de la cabecera, que no cambió); la
//   migración v2→v3 encuentra las tablas ya creadas y falla limpio.
// - Con la llave la rechaza `key_check`, que autentica la versión 3. Ese caso
//   lo cubre `key_check`, no `encrypted_key`.

/// Una v3 con `format_version = 2` y la llave de bóveda.
fn downgraded_v3() -> (tempfile::TempDir, PathBuf, SecretBox<[u8; 32]>) {
    let (dir, path, mut vault) = new_vault();
    let key = vault.export_vault_key().unwrap();
    drop(vault);
    raw(&path)
        .execute("UPDATE header SET format_version = 2", [])
        .unwrap();
    (dir, path, key)
}

#[test]
fn downgraded_v3_to_v2_with_password_fails_cleanly() {
    let (_dir, path, _key) = downgraded_v3();
    let before = fs::read(&path).unwrap();
    let mut vault = Vault::open(&path).unwrap();

    let err = vault.unlock(&password("correcta-larga")).unwrap_err();
    assert_eq!(err, Error::Storage(StorageErrorKind::Corrupt));
    assert!(vault.is_locked());
    drop(vault);
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn downgraded_v3_to_v2_with_key_is_wrong_vault_key() {
    let (_dir, path, key) = downgraded_v3();
    let before = fs::read(&path).unwrap();
    let mut vault = Vault::open(&path).unwrap();

    assert_eq!(vault.unlock_with_key(key).err(), Some(Error::WrongVaultKey));
    assert!(vault.is_locked());
    drop(vault);
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn open_and_read_only_open_never_modify_any_version() {
    let (_dir1, v1) = fixture_v1_copy();
    let (_dir2, v2) = fixture_v2_copy();
    let (_dir3, v3, vault) = new_vault();
    drop(vault);
    let (_dir4, v3_fixture) = fixture_v3_copy();
    for path in [&v1, &v2, &v3, &v3_fixture] {
        let before = fs::read(path).unwrap();
        drop(Vault::open(path).unwrap());
        assert_eq!(fs::read(path).unwrap(), before, "open {path:?}");
        Storage::open_read_only(path).unwrap().close().unwrap();
        assert_eq!(fs::read(path).unwrap(), before, "solo lectura {path:?}");
    }
}

// --- Desbloqueo con la llave de bóveda (#34) -----------------------------

/// Llave aleatoria que no es la de ninguna bóveda.
fn random_key() -> SecretBox<[u8; 32]> {
    crate::crypto::keys::generate_vault_key().unwrap()
}

/// `key_check` tal como está en el archivo.
fn stored_key_check(path: &Path) -> Vec<u8> {
    raw(path)
        .query_row("SELECT key_check FROM header WHERE id = 1", [], |row| {
            row.get(0)
        })
        .unwrap()
}

/// Invierte un bit de `key_check` en el archivo.
fn damage_key_check(path: &Path) {
    let mut check = stored_key_check(path);
    check[30] ^= 0x01;
    raw(path)
        .execute("UPDATE header SET key_check = ?1 WHERE id = 1", [check])
        .unwrap();
}

fn revealed_password(vault: &mut Vault, id: EntryId) -> String {
    vault
        .reveal(id)
        .unwrap()
        .password()
        .expose_secret()
        .to_owned()
}

#[test]
fn export_lock_unlock_with_key_round_trip() {
    let (_dir, _path, mut vault) = new_vault();
    let a = add_sample(&mut vault, "a");
    add_sample(&mut vault, "b");
    let password_a = revealed_password(&mut vault, a);
    let before = snapshot(&vault);
    let used_before = find(&vault, a).last_used_at();

    let key = vault.export_vault_key().unwrap();
    vault.lock();
    let report = vault.unlock_with_key(key).unwrap();

    assert!(report.corrupt.is_empty());
    assert_eq!(report.unidentified_corrupt, 0);
    assert_eq!(snapshot(&vault), before);
    assert_eq!(find(&vault, a).last_used_at(), used_before);
    assert_eq!(revealed_password(&mut vault, a), password_a);
}

#[test]
fn unlock_with_wrong_key_stays_locked() {
    let (_dir, _path, mut vault) = new_vault();
    add_sample(&mut vault, "a");
    vault.lock();
    assert_eq!(
        vault.unlock_with_key(random_key()).err(),
        Some(Error::WrongVaultKey)
    );
    assert!(vault.is_locked());
}

#[test]
fn empty_vault_rejects_wrong_key() {
    let (_dir, _path, mut vault) = new_vault();
    vault.lock();
    assert_eq!(
        vault.unlock_with_key(random_key()).err(),
        Some(Error::WrongVaultKey)
    );
    assert!(vault.is_locked());
    assert_eq!(vault.entries().err(), Some(Error::Locked));
}

#[test]
fn unlock_with_key_of_other_vault() {
    let (_dir, _path, mut vault) = new_vault();
    let (_dir2, _path2, mut other) = new_vault();
    let other_key = other.export_vault_key().unwrap();
    vault.lock();
    assert_eq!(
        vault.unlock_with_key(other_key).err(),
        Some(Error::WrongVaultKey)
    );
    assert!(vault.is_locked());
}

#[test]
fn export_vault_key_locked_is_locked() {
    let (_dir, _path, mut vault) = new_vault();
    vault.lock();
    assert_eq!(vault.export_vault_key().err(), Some(Error::Locked));
}

#[test]
fn unlock_with_key_when_unlocked_is_already_unlocked() {
    let (_dir, _path, mut vault) = new_vault();
    let key = vault.export_vault_key().unwrap();
    assert_eq!(
        vault.unlock_with_key(key).err(),
        Some(Error::AlreadyUnlocked)
    );
}

#[test]
fn old_key_survives_change_master_password() {
    let (_dir, path, mut vault) = new_vault();
    let key = vault.export_vault_key().unwrap();
    let check_before = stored_key_check(&path);

    vault
        .change_master_password_with_params(
            &password("correcta-larga"),
            &password("nueva-maestra"),
            KdfParams::MIN,
        )
        .unwrap();
    assert_ne!(stored_key_check(&path), check_before);

    vault.lock();
    vault.unlock_with_key(key).unwrap();
}

#[test]
fn damaged_key_check_is_repaired_by_export() {
    let (_dir, path, mut vault) = new_vault();
    add_sample(&mut vault, "a");
    let key = vault.export_vault_key().unwrap();
    let key_bytes = *key.expose_secret();
    vault.lock();
    damage_key_check(&path);

    assert_eq!(vault.unlock_with_key(key).err(), Some(Error::WrongVaultKey));
    // La contraseña nunca depende del verificador.
    vault.unlock(&password("correcta-larga")).unwrap();
    let repaired = vault.export_vault_key().unwrap();
    assert_eq!(*repaired.expose_secret(), key_bytes);

    vault.lock();
    vault.unlock_with_key(repaired).unwrap();
    assert_eq!(vault.entries().unwrap().len(), 1);
}

#[test]
fn export_fails_without_key_when_repair_cannot_write() {
    let (_dir, path, mut vault) = new_vault();
    vault.lock();
    damage_key_check(&path);
    vault.unlock(&password("correcta-larga")).unwrap();
    vault.storage.deny_writes_for_test();

    let err = vault.export_vault_key().err();
    assert!(matches!(err, Some(Error::Storage(_))), "{err:?}");
}

#[test]
fn export_with_valid_check_writes_nothing() {
    let (_dir, path, mut vault) = new_vault();
    let before = fs::read(&path).unwrap();
    let first = vault.export_vault_key().unwrap();
    let second = vault.export_vault_key().unwrap();
    assert_eq!(first.expose_secret(), second.expose_secret());
    drop(vault);
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn failed_unlock_with_key_then_password_works() {
    let (_dir, _path, mut vault) = new_vault();
    add_sample(&mut vault, "a");
    vault.lock();
    assert_eq!(
        vault.unlock_with_key(random_key()).err(),
        Some(Error::WrongVaultKey)
    );
    vault.unlock(&password("correcta-larga")).unwrap();
    assert_eq!(vault.entries().unwrap().len(), 1);
}

// --- Migración concurrente (#34, hallazgo de la revisión) -----------------

/// Ejecuta la primera mitad de `unlock` en `vault` (leer la cabecera y
/// derivar), como si estuviera a mitad de Argon2id. Devuelve también la
/// versión de formato que leyó.
fn derive_for_unlock(vault: &Vault, pw: &str) -> (VaultKey, u16, Option<VaultHeader>) {
    let header = vault.storage.read_header().unwrap();
    let version = header.format_version();
    let (key, upgraded) = header.unlock_upgrading(&password(pw)).unwrap();
    (key, version, upgraded)
}

#[test]
fn concurrent_migration_does_not_revert_password_change() {
    let (_dir, path) = fixture_v1_copy();
    let mut a = Vault::open(&path).unwrap();
    let mut b = Vault::open(&path).unwrap();

    // La A derivó con la cabecera v1; antes de que migre, la B migra y
    // cambia la contraseña.
    let (key, version, upgraded) = derive_for_unlock(&a, FIXTURE_V1_PASSWORD);
    assert!(upgraded.is_some());
    b.unlock(&password(FIXTURE_V1_PASSWORD)).unwrap();
    b.change_master_password_with_params(
        &password(FIXTURE_V1_PASSWORD),
        &password("nueva-maestra"),
        KdfParams::MIN,
    )
    .unwrap();
    drop(b);

    // La A termina: no sobrescribe la cabecera de la B.
    assert_eq!(
        a.finish_unlock(&password(FIXTURE_V1_PASSWORD), key, version, upgraded)
            .err(),
        Some(Error::WrongPassword)
    );
    assert!(a.is_locked());

    // La contraseña nueva sigue abriendo y la vieja no.
    a.unlock(&password("nueva-maestra")).unwrap();
    assert_eq!(a.entries().unwrap().len(), 3);
    a.lock();
    assert_eq!(
        a.unlock(&password(FIXTURE_V1_PASSWORD)).err(),
        Some(Error::WrongPassword)
    );
}

#[test]
fn concurrent_migration_without_password_change_unlocks() {
    let (_dir, path) = fixture_v1_copy();
    let mut a = Vault::open(&path).unwrap();
    let mut b = Vault::open(&path).unwrap();

    let (key, version, upgraded) = derive_for_unlock(&a, FIXTURE_V1_PASSWORD);
    b.unlock(&password(FIXTURE_V1_PASSWORD)).unwrap();
    let check_from_b = b.storage.read_header().unwrap().key_check().copied();
    drop(b);

    let report = a
        .finish_unlock(&password(FIXTURE_V1_PASSWORD), key, version, upgraded)
        .unwrap();
    assert!(report.corrupt.is_empty());
    assert_eq!(a.entries().unwrap().len(), 3);
    // La cabecera es la que escribió la B: la A no migró otra vez.
    assert_eq!(
        a.storage.read_header().unwrap().key_check().copied(),
        check_from_b
    );
}

#[test]
fn concurrent_v2_to_v3_migration_does_not_revert_password_change() {
    let (_dir, path) = fixture_v2_copy();
    let mut a = Vault::open(&path).unwrap();
    let mut b = Vault::open(&path).unwrap();

    // La A derivó con la cabecera v2; antes de que migre, la B migra a v3 y
    // cambia la contraseña.
    let (key, version, upgraded) = derive_for_unlock(&a, FIXTURE_V2_PASSWORD);
    assert_eq!((version, upgraded.is_none()), (2, true));
    b.unlock(&password(FIXTURE_V2_PASSWORD)).unwrap();
    b.change_master_password_with_params(
        &password(FIXTURE_V2_PASSWORD),
        &password("nueva-maestra"),
        KdfParams::MIN,
    )
    .unwrap();
    drop(b);

    // La A termina: no migra otra vez ni revierte la contraseña nueva.
    assert_eq!(
        a.finish_unlock(&password(FIXTURE_V2_PASSWORD), key, version, upgraded)
            .err(),
        Some(Error::WrongPassword)
    );
    assert!(a.is_locked());

    a.unlock(&password("nueva-maestra")).unwrap();
    assert_eq!(a.entries().unwrap().len(), 3);
    a.lock();
    assert_eq!(
        a.unlock(&password(FIXTURE_V2_PASSWORD)).err(),
        Some(Error::WrongPassword)
    );
}

#[test]
fn concurrent_v2_to_v3_migration_without_password_change_unlocks() {
    let (_dir, path) = fixture_v2_copy();
    let mut a = Vault::open(&path).unwrap();
    let mut b = Vault::open(&path).unwrap();

    let (key, version, upgraded) = derive_for_unlock(&a, FIXTURE_V2_PASSWORD);
    b.unlock(&password(FIXTURE_V2_PASSWORD)).unwrap();
    let check_from_b = b.storage.read_header().unwrap().key_check().copied();
    drop(b);

    let report = a
        .finish_unlock(&password(FIXTURE_V2_PASSWORD), key, version, upgraded)
        .unwrap();
    assert!(report.corrupt.is_empty());
    assert_eq!(a.entries().unwrap().len(), 3);
    // La cabecera es la que escribió la B: la A no migró otra vez.
    assert_eq!(
        a.storage.read_header().unwrap().key_check().copied(),
        check_from_b
    );
}

// --- longitud mínima de la contraseña maestra (#36) ---------------------

/// Contraseña válida para las pruebas de esta sección (16 caracteres).
const LONG_PASSWORD: &str = "maestra-de-16-ch";

/// Justo debajo del mínimo (11 caracteres) y justo en el mínimo (12).
const ELEVEN_CHARS: &str = "corta-11-ch";
const TWELVE_CHARS: &str = "justa-12-chr";

#[test]
fn boundary_constants_have_their_length() {
    assert_eq!(ELEVEN_CHARS.chars().count(), 11);
    assert_eq!(TWELVE_CHARS.chars().count(), 12);
}

#[test]
fn create_accepts_exactly_twelve() {
    let (_dir, path) = temp_path();
    let mut vault =
        Vault::create_with_params(&path, &password(TWELVE_CHARS), KdfParams::MIN).unwrap();
    vault.lock();
    vault.unlock(&password(TWELVE_CHARS)).unwrap();
}

#[test]
fn change_master_password_accepts_exactly_twelve() {
    let (_dir, path) = temp_path();
    let mut vault =
        Vault::create_with_params(&path, &password(LONG_PASSWORD), KdfParams::MIN).unwrap();
    vault
        .change_master_password_with_params(
            &password(LONG_PASSWORD),
            &password(TWELVE_CHARS),
            KdfParams::MIN,
        )
        .unwrap();
    vault.lock();
    vault.unlock(&password(TWELVE_CHARS)).unwrap();
}

#[test]
fn create_rejects_short_password_before_touching_disk() {
    let (_dir, path) = temp_path();
    // `Vault::create` real: debe regresar antes de calibrar.
    DERIVE_CALLS.with(|c| c.set(0));
    assert_eq!(
        Vault::create(&path, &password(ELEVEN_CHARS)).err(),
        Some(Error::PasswordTooShort)
    );
    assert_eq!(DERIVE_CALLS.with(|c| c.get()), 0);
    assert!(!path.exists());
}

#[test]
fn create_with_params_rejects_short_password() {
    let (_dir, path) = temp_path();
    assert_eq!(
        Vault::create_with_params(&path, &password(ELEVEN_CHARS), KdfParams::MIN).err(),
        Some(Error::PasswordTooShort)
    );
    assert!(!path.exists());
}

#[test]
fn change_master_password_rejects_short_before_deriving() {
    let (_dir, path) = temp_path();
    let mut vault =
        Vault::create_with_params(&path, &password(LONG_PASSWORD), KdfParams::MIN).unwrap();
    let before = header_row(&path);

    DERIVE_CALLS.with(|c| c.set(0));
    assert_eq!(
        vault
            .change_master_password_with_params(
                &password(LONG_PASSWORD),
                &password(ELEVEN_CHARS),
                KdfParams::MIN,
            )
            .err(),
        Some(Error::PasswordTooShort)
    );
    // Con la actual incorrecta, la longitud se comprueba primero.
    assert_eq!(
        vault
            .change_master_password_with_params(
                &password("incorrecta-y-larga"),
                &password(ELEVEN_CHARS),
                KdfParams::MIN,
            )
            .err(),
        Some(Error::PasswordTooShort)
    );
    assert_eq!(DERIVE_CALLS.with(|c| c.get()), 0);
    assert_eq!(header_row(&path), before);

    vault.lock();
    vault.unlock(&password(LONG_PASSWORD)).unwrap();
}

/// Una bóveda con contraseña corta, armada sin pasar por `create`, como las
/// que existían antes de la regla.
fn short_password_vault(path: &Path, pw: &str) {
    let (header, _key) = VaultHeader::create(&password(pw), KdfParams::MIN).unwrap();
    drop(Storage::create(path, &header).unwrap());
}

#[test]
fn unlock_and_import_ignore_the_minimum() {
    let dir = tempfile::tempdir().unwrap();
    let original = dir.path().join("corta.db");
    short_password_vault(&original, "corta");

    let mut vault = Vault::open(&original).unwrap();
    vault.unlock(&password("corta")).unwrap();

    let backup = dir.path().join("respaldo.db");
    vault.export(&backup).unwrap();
    drop(vault);

    let dest = dir.path().join("importada.db");
    let (imported, _report) = Vault::import(&backup, &dest, &password("corta")).unwrap();
    assert!(!imported.is_locked());
}

// --- entry -------------------------------------------------------------

#[test]
fn entry_matches_entries() {
    let (_dir, _path, mut vault) = new_vault();
    add_sample(&mut vault, "gmail");
    add_sample(&mut vault, "banco");
    for expected in vault.entries().unwrap() {
        let found = vault.entry(expected.id()).unwrap();
        assert_eq!(found.id(), expected.id());
        assert!(found.search() == expected.search());
        assert_eq!(found.modified_at(), expected.modified_at());
        assert_eq!(found.last_used_at(), expected.last_used_at());
    }
}

#[test]
fn entry_reflects_edit_and_reveal() {
    let (_dir, _path, mut vault) = new_vault();
    let id = add_sample(&mut vault, "gmail");
    assert_eq!(vault.entry(id).unwrap().last_used_at(), None);

    vault
        .edit(id, search_with_tags("correo", &["x"]), SecretChange::Keep)
        .unwrap();
    assert_eq!(vault.entry(id).unwrap().search().service(), "correo");

    vault.reveal(id).unwrap();
    assert!(vault.entry(id).unwrap().last_used_at().is_some());
}

#[test]
fn entry_not_found_cases() {
    let (_dir, path, mut vault) = new_vault();
    let deleted = add_sample(&mut vault, "borrada");
    let damaged = add_sample(&mut vault, "dañada");
    vault.delete(deleted).unwrap();
    tamper(&path, damaged, "search");
    assert_eq!(relock(&mut vault).corrupt, vec![damaged]);

    for id in [EntryId::from_bytes([7; 16]), deleted, damaged] {
        assert_eq!(vault.entry(id).err(), Some(Error::EntryNotFound));
    }
}

#[test]
fn entry_with_corrupt_secret_is_returned() {
    let (_dir, path, mut vault) = new_vault();
    let id = add_sample(&mut vault, "gmail");
    tamper(&path, id, "secret");
    relock(&mut vault);
    assert_eq!(vault.entry(id).unwrap().search().service(), "gmail");
}

#[test]
fn entry_locked() {
    let (_dir, _path, mut vault) = new_vault();
    let id = add_sample(&mut vault, "gmail");
    vault.lock();
    assert_eq!(vault.entry(id).err(), Some(Error::Locked));
}

// --- Restos de otra bóveda junto a una ruta nueva (#58) -----------------

/// Crea una bóveda "vieja" con una entrada, deja junto a `boveda.db` los
/// restos reales que pide cada caso (diario caliente, `-wal` válido con su
/// `-shm`) y borra la base. Devuelve el directorio y la ruta de `boveda.db`.
fn orphans_next_to_new_path(journal: bool, wal: bool) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let old = dir.path().join("vieja.db");
    let mut vault =
        Vault::create_with_params(&old, &password("clave-vieja-larga"), KdfParams::MIN).unwrap();
    let (search, secret) = sample("vieja");
    vault.add(search, secret).unwrap();
    drop(vault);

    let dest = dir.path().join("boveda.db");
    if journal {
        make_hot_journal_copy(&old, &dest);
    }
    if wal {
        copy_live_wal(&old, &dest);
        fs::write(sibling(&dest, "-shm"), [0xBB_u8; 1024]).unwrap();
    }
    fs::remove_file(&dest).ok();
    (dir, dest)
}

/// SQLite abre una base de 0 bytes (la que deja `create_new`) y descarta el
/// diario y el WAL que encuentre al lado. Si una versión nueva de SQLite lo
/// cambiara, estas pruebas fallan y hay que volver a la #58 (borrar los
/// restos en `Storage::create`, como `settle_dest` en `import`).
fn assert_create_ignores_orphans(dest: &Path) {
    let vault =
        Vault::create_with_params(dest, &password("clave-nueva-larga"), KdfParams::MIN).unwrap();
    assert!(vault.entries().unwrap().is_empty());
    drop(vault);

    // SQLite los borró: no quedan para aplicarse sobre otra base más tarde.
    // El `-shm` no se comprueba: puede sobrevivir y no hace daño (la bóveda
    // no usa WAL).
    assert!(!sibling(dest, "-journal").exists());
    assert!(!sibling(dest, "-wal").exists());

    assert_eq!(Storage::open(dest).unwrap().check_integrity(), Ok(()));
    let mut reopened = Vault::open(dest).unwrap();
    assert_eq!(
        reopened.unlock(&password("clave-vieja-larga")).err(),
        Some(Error::WrongPassword)
    );
    let report = reopened.unlock(&password("clave-nueva-larga")).unwrap();
    assert!(report.corrupt.is_empty());
    assert!(reopened.entries().unwrap().is_empty());
}

#[test]
fn create_next_to_orphan_journal() {
    let (_dir, dest) = orphans_next_to_new_path(true, false);
    assert!(sibling(&dest, "-journal").exists());
    assert_create_ignores_orphans(&dest);
}

#[test]
fn create_next_to_orphan_wal_and_shm() {
    let (_dir, dest) = orphans_next_to_new_path(false, true);
    assert!(sibling(&dest, "-wal").exists());
    assert_create_ignores_orphans(&dest);
}

/// Un diario y un WAL no conviven en una misma base, pero sí pueden quedar
/// juntos junto a una ruta que se reutiliza (restos de dos instalaciones o de
/// un cambio de modo): se prueba el peor caso, con los tres archivos.
#[test]
fn create_next_to_all_orphans() {
    let (_dir, dest) = orphans_next_to_new_path(true, true);
    assert!(sibling(&dest, "-journal").exists() && sibling(&dest, "-wal").exists());
    assert_create_ignores_orphans(&dest);
}

// --- Identidad perezosa (#93) ---------------------------------------------

/// `data` de la fila de `identity`, tal como está en el archivo.
fn identity_data(path: &Path) -> Vec<u8> {
    raw(path)
        .query_row("SELECT data FROM identity WHERE id = 1", [], |row| {
            row.get(0)
        })
        .unwrap()
}

/// Invierte un bit del byte `index` de `identity.data`.
fn flip_identity_byte(path: &Path, index: usize) {
    let mut data = identity_data(path);
    data[index] ^= 0x01;
    raw(path)
        .execute("UPDATE identity SET data = ?1 WHERE id = 1", [data])
        .unwrap();
}

fn key_bytes(identity: &Identity) -> [u8; 32] {
    *identity.private_key().expose_secret()
}

#[test]
fn identity_is_none_on_a_new_vault() {
    let (_dir, path, vault) = new_vault();
    assert!(vault.identity().unwrap().is_none());
    // Ausente no es un error y leerla no crea nada.
    assert_eq!(pairing_row_counts(&path), (0, 0));
}

#[test]
fn ensure_identity_creates_the_first_when_absent() {
    let (_dir, path, mut vault) = new_vault();
    let created = vault.ensure_identity().unwrap();
    assert_eq!(pairing_row_counts(&path), (1, 0));
    let read = vault.identity().unwrap().unwrap();
    assert_eq!(key_bytes(&read), key_bytes(&created));
}

#[test]
fn ensure_identity_creates_once_and_returns_the_same() {
    let (_dir, path, mut vault) = new_vault();
    let first = vault.ensure_identity().unwrap();
    let data = identity_data(&path);
    let second = vault.ensure_identity().unwrap();
    assert_eq!(key_bytes(&first), key_bytes(&second));
    // La segunda llamada no volvió a escribir.
    assert_eq!(identity_data(&path), data);
}

#[test]
fn identity_survives_lock_and_reopen() {
    let (_dir, path, mut vault) = new_vault();
    let created = vault.ensure_identity().unwrap();
    vault.lock();
    drop(vault);

    let mut reopened = Vault::open(&path).unwrap();
    reopened.unlock(&password("correcta-larga")).unwrap();
    let read = reopened.identity().unwrap().unwrap();
    assert_eq!(key_bytes(&read), key_bytes(&created));
}

#[test]
fn identity_requires_the_vault_unlocked() {
    let (_dir, _path, mut vault) = new_vault();
    vault.ensure_identity().unwrap();
    vault.lock();
    assert_eq!(vault.identity().err(), Some(Error::Locked));
    assert_eq!(vault.ensure_identity().err(), Some(Error::Locked));
}

#[test]
fn absent_and_damaged_identity_are_distinct() {
    let (_dir, path, mut vault) = new_vault();
    assert!(matches!(vault.identity(), Ok(None)));
    vault.ensure_identity().unwrap();
    assert!(matches!(vault.identity(), Ok(Some(_))));
    flip_identity_byte(&path, 30);
    assert_eq!(vault.identity().err(), Some(Error::CorruptIdentity));
}

#[test]
fn flipped_byte_in_identity_data_is_corrupt() {
    let (_dir, path, mut vault) = new_vault();
    vault.ensure_identity().unwrap();
    let len = identity_data(&path).len();
    assert_eq!(len, 72);
    for index in [0, 23, 24, 40, 55, 56, 71] {
        flip_identity_byte(&path, index);
        assert_eq!(
            vault.identity().err(),
            Some(Error::CorruptIdentity),
            "byte {index}"
        );
        flip_identity_byte(&path, index);
        assert!(vault.identity().unwrap().is_some(), "restaurado {index}");
    }
}

#[test]
fn changed_identity_schema_version_is_corrupt() {
    let (_dir, path, mut vault) = new_vault();
    vault.ensure_identity().unwrap();
    for version in ["2", "0", "70000", "-1"] {
        raw(&path)
            .execute(
                &format!("UPDATE identity SET identity_schema_version = {version}"),
                [],
            )
            .unwrap();
        assert_eq!(
            vault.identity().err(),
            Some(Error::CorruptIdentity),
            "versión {version}"
        );
    }
}

#[test]
fn identity_data_of_the_wrong_length_is_corrupt() {
    // El `CHECK` solo se aplica al escribir: un archivo alterado puede traer
    // otra longitud. Se simula desactivando el `CHECK` en la conexión de la
    // prueba.
    let (_dir, path, mut vault) = new_vault();
    vault.ensure_identity().unwrap();
    let conn = raw(&path);
    conn.execute_batch("PRAGMA ignore_check_constraints = ON")
        .unwrap();
    conn.execute("UPDATE identity SET data = zeroblob(10)", [])
        .unwrap();
    assert_eq!(vault.identity().err(), Some(Error::CorruptIdentity));
}

#[test]
fn damaged_identity_does_not_block_unlock_or_entries() {
    let (_dir, path, mut vault) = new_vault();
    let id = add_sample(&mut vault, "banco");
    vault.ensure_identity().unwrap();
    flip_identity_byte(&path, 30);
    vault.lock();
    drop(vault);

    let mut reopened = Vault::open(&path).unwrap();
    let report = reopened.unlock(&password("correcta-larga")).unwrap();
    assert!(report.corrupt.is_empty());
    assert_eq!(reopened.entries().unwrap().len(), 1);
    assert_eq!(revealed_password(&mut reopened, id), "clave-banco");
    // Y con la llave de bóveda también.
    let key = reopened.export_vault_key().unwrap();
    reopened.lock();
    assert!(reopened.unlock_with_key(key).is_ok());
}

#[test]
fn ensure_identity_on_a_damaged_one_never_overwrites_it() {
    let (_dir, path, mut vault) = new_vault();
    vault.ensure_identity().unwrap();
    flip_identity_byte(&path, 30);
    let before = (
        identity_data(&path),
        raw(&path)
            .query_row("SELECT identity_schema_version FROM identity", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap(),
    );

    assert_eq!(vault.ensure_identity().err(), Some(Error::CorruptIdentity));
    assert_eq!(vault.ensure_identity().err(), Some(Error::CorruptIdentity));
    vault.lock();
    drop(vault);

    // También tras reabrir.
    let mut reopened = Vault::open(&path).unwrap();
    reopened.unlock(&password("correcta-larga")).unwrap();
    assert_eq!(
        reopened.ensure_identity().err(),
        Some(Error::CorruptIdentity)
    );
    let after = (
        identity_data(&path),
        raw(&path)
            .query_row("SELECT identity_schema_version FROM identity", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap(),
    );
    assert_eq!(after, before);
    assert_eq!(pairing_row_counts(&path).0, 1);
}

#[test]
fn identity_is_not_plaintext_in_the_file() {
    let (_dir, path, mut vault) = new_vault();
    let identity = vault.ensure_identity().unwrap();
    vault.lock();
    drop(vault);
    let bytes = fs::read(&path).unwrap();
    assert!(!contains(&bytes, &key_bytes(&identity)));
}

#[test]
fn identity_is_bound_to_the_vault_key() {
    // La misma fila en otra bóveda (otra llave) no se autentica.
    let (_dir, path, mut vault) = new_vault();
    vault.ensure_identity().unwrap();
    let data = identity_data(&path);

    let (_dir2, other_path, mut other) = new_vault();
    other.ensure_identity().unwrap();
    raw(&other_path)
        .execute("UPDATE identity SET data = ?1", [data])
        .unwrap();
    assert_eq!(other.identity().err(), Some(Error::CorruptIdentity));
}

#[test]
fn set_identity_for_test_stores_the_given_key() {
    let (_dir, path, mut vault) = new_vault();
    vault.set_identity_for_test([7u8; 32]).unwrap();
    assert_eq!(key_bytes(&vault.identity().unwrap().unwrap()), [7u8; 32]);
    assert_eq!(pairing_row_counts(&path), (1, 0));
}

// --- PCs emparejadas (#93) ------------------------------------------------

fn new_pc(byte: u8, name: &str) -> NewPairedPc {
    NewPairedPc {
        public_key: [byte; 32],
        name: name.to_owned(),
        last_ip: None,
    }
}

fn find_pc(vault: &Vault, id: PairedPcId) -> PairedPc {
    vault
        .paired_pcs()
        .unwrap()
        .valid
        .into_iter()
        .find(|pc| pc.id() == id)
        .unwrap()
}

fn ip(text: &str) -> IpAddr {
    text.parse().unwrap()
}

/// `data` de la PC `id`, tal como está en el archivo.
fn pc_data(path: &Path, id: PairedPcId) -> Vec<u8> {
    raw(path)
        .query_row(
            "SELECT data FROM paired_pcs WHERE id = ?1",
            [id.as_bytes().as_slice()],
            |row| row.get(0),
        )
        .unwrap()
}

fn flip_pc_byte(path: &Path, id: PairedPcId, index: usize) {
    let mut data = pc_data(path, id);
    data[index] ^= 0x01;
    raw(path)
        .execute(
            "UPDATE paired_pcs SET data = ?1 WHERE id = ?2",
            rusqlite::params![data, id.as_bytes().as_slice()],
        )
        .unwrap();
}

#[test]
fn add_list_update_remove_round_trip() {
    let (_dir, path, mut vault) = new_vault();
    assert!(vault.paired_pcs().unwrap().valid.is_empty());

    let casa = vault.add_paired_pc(new_pc(1, "Casa")).unwrap();
    let mut trabajo = new_pc(2, "Trabajo");
    trabajo.last_ip = Some(ip("192.168.1.20"));
    let trabajo = vault.add_paired_pc(trabajo).unwrap();

    let listed = vault.paired_pcs().unwrap();
    assert_eq!(listed.valid.len(), 2);
    assert!(listed.corrupt.is_empty());
    assert_eq!(listed.unidentified_corrupt, 0);
    let casa_pc = find_pc(&vault, casa);
    assert_eq!(casa_pc.public_key(), &[1; 32]);
    assert_eq!(casa_pc.name(), "Casa");
    assert_eq!(casa_pc.last_ip(), None);
    assert!(casa_pc.paired_at() > 1_767_225_600_000);
    assert_eq!(casa_pc.last_seen_at(), None);
    assert_eq!(find_pc(&vault, trabajo).last_ip(), Some(ip("192.168.1.20")));

    vault
        .update_paired_pc(
            casa,
            PairedPcUpdate {
                name: Some("Casa nueva".to_owned()),
                last_ip: None,
            },
        )
        .unwrap();
    assert_eq!(find_pc(&vault, casa).name(), "Casa nueva");

    vault.remove_paired_pc(casa).unwrap();
    let left = vault.paired_pcs().unwrap().valid;
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].id(), trabajo);
    assert_eq!(pairing_row_counts(&path), (0, 1));
}

#[test]
fn public_key_already_paired_is_rejected() {
    let (_dir, path, mut vault) = new_vault();
    vault.add_paired_pc(new_pc(1, "Casa")).unwrap();
    let before = pc_row_snapshot(&path);
    assert_eq!(
        vault.add_paired_pc(new_pc(1, "Otro nombre")).err(),
        Some(Error::PairedPcExists)
    );
    assert_eq!(pc_row_snapshot(&path), before);
    assert_eq!(pairing_row_counts(&path), (0, 1));
}

/// Ids y datos de todas las filas de `paired_pcs`, ordenados.
fn pc_row_snapshot(path: &Path) -> Vec<(Vec<u8>, Vec<u8>)> {
    let conn = raw(path);
    let mut stmt = conn
        .prepare("SELECT id, data FROM paired_pcs ORDER BY id")
        .unwrap();
    stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

#[test]
fn empty_blank_or_long_names_are_invalid() {
    let (_dir, path, mut vault) = new_vault();
    for name in ["", "   "] {
        assert_eq!(
            vault.add_paired_pc(new_pc(1, name)).err(),
            Some(Error::InvalidPairedPc(InvalidPairedPcField::NameEmpty))
        );
    }
    assert_eq!(
        vault.add_paired_pc(new_pc(1, &"a".repeat(129))).err(),
        Some(Error::InvalidPairedPc(InvalidPairedPcField::NameTooLong))
    );
    assert_eq!(pairing_row_counts(&path), (0, 0));
    // 128 bytes se aceptan, y el nombre se recorta.
    let id = vault.add_paired_pc(new_pc(1, &"a".repeat(128))).unwrap();
    assert_eq!(find_pc(&vault, id).name().len(), 128);
    let id = vault.add_paired_pc(new_pc(2, "  Casa  ")).unwrap();
    assert_eq!(find_pc(&vault, id).name(), "Casa");
}

#[test]
fn update_ip_sets_last_seen_at_and_keeps_the_rest() {
    let (_dir, _path, mut vault) = new_vault();
    let id = vault.add_paired_pc(new_pc(4, "Casa")).unwrap();
    let before = find_pc(&vault, id);
    assert_eq!(before.last_seen_at(), None);

    vault
        .update_paired_pc(
            id,
            PairedPcUpdate {
                name: None,
                last_ip: Some(ip("192.168.1.77")),
            },
        )
        .unwrap();
    let after = find_pc(&vault, id);
    assert_eq!(after.last_ip(), Some(ip("192.168.1.77")));
    assert!(after.last_seen_at().unwrap() >= before.paired_at());
    assert_eq!(after.name(), "Casa");
    assert_eq!(after.public_key(), before.public_key());
    assert_eq!(after.paired_at(), before.paired_at());
}

#[test]
fn update_without_ip_keeps_last_ip_and_last_seen_at() {
    let (_dir, _path, mut vault) = new_vault();
    let id = vault.add_paired_pc(new_pc(4, "Casa")).unwrap();
    vault
        .update_paired_pc(
            id,
            PairedPcUpdate {
                name: None,
                last_ip: Some(ip("10.0.0.5")),
            },
        )
        .unwrap();
    let seen = find_pc(&vault, id).last_seen_at();
    vault
        .update_paired_pc(
            id,
            PairedPcUpdate {
                name: Some("Otro".to_owned()),
                last_ip: None,
            },
        )
        .unwrap();
    let after = find_pc(&vault, id);
    assert_eq!(after.last_ip(), Some(ip("10.0.0.5")));
    assert_eq!(after.last_seen_at(), seen);
    assert_eq!(after.name(), "Otro");
}

#[test]
fn update_with_an_invalid_name_writes_nothing() {
    let (_dir, path, mut vault) = new_vault();
    let id = vault.add_paired_pc(new_pc(4, "Casa")).unwrap();
    let before = pc_row_snapshot(&path);
    assert_eq!(
        vault
            .update_paired_pc(
                id,
                PairedPcUpdate {
                    name: Some("  ".to_owned()),
                    last_ip: Some(ip("10.0.0.5")),
                },
            )
            .err(),
        Some(Error::InvalidPairedPc(InvalidPairedPcField::NameEmpty))
    );
    assert_eq!(pc_row_snapshot(&path), before);
}

#[test]
fn update_and_remove_of_an_unknown_id_are_not_found() {
    let (_dir, _path, mut vault) = new_vault();
    let unknown = PairedPcId::from_bytes([9; 16]);
    let update = PairedPcUpdate {
        name: Some("x".to_owned()),
        last_ip: None,
    };
    assert_eq!(
        vault.update_paired_pc(unknown, update).err(),
        Some(Error::PairedPcNotFound)
    );
    assert_eq!(
        vault.remove_paired_pc(unknown).err(),
        Some(Error::PairedPcNotFound)
    );
}

#[test]
fn paired_pcs_require_the_vault_unlocked() {
    let (_dir, _path, mut vault) = new_vault();
    let id = vault.add_paired_pc(new_pc(1, "Casa")).unwrap();
    vault.lock();
    assert_eq!(vault.paired_pcs().err(), Some(Error::Locked));
    assert_eq!(
        vault.add_paired_pc(new_pc(2, "x")).err(),
        Some(Error::Locked)
    );
    assert_eq!(
        vault
            .update_paired_pc(
                id,
                PairedPcUpdate {
                    name: None,
                    last_ip: None
                }
            )
            .err(),
        Some(Error::Locked)
    );
    assert_eq!(vault.remove_paired_pc(id).err(), Some(Error::Locked));
}

#[test]
fn paired_pcs_survive_lock_and_reopen() {
    let (_dir, path, mut vault) = new_vault();
    let id = vault.add_paired_pc(new_pc(1, "Casa")).unwrap();
    vault.lock();
    drop(vault);

    let mut reopened = Vault::open(&path).unwrap();
    reopened.unlock(&password("correcta-larga")).unwrap();
    assert_eq!(find_pc(&reopened, id).name(), "Casa");
}

#[test]
fn name_and_ip_are_not_plaintext_in_the_file() {
    let (_dir, path, mut vault) = new_vault();
    let mut pc = new_pc(1, "equipo-canario-xyz");
    pc.last_ip = Some(ip("203.0.113.77"));
    vault.add_paired_pc(pc).unwrap();
    vault.lock();
    drop(vault);
    let bytes = fs::read(&path).unwrap();
    assert!(!contains(&bytes, b"equipo-canario-xyz"));
    assert!(!contains(&bytes, b"203.0.113.77"));
    assert!(!contains(&bytes, &[203, 0, 113, 77]));
}

/// Cada daño deja la PC como dañada y no impide listar las demás, ni
/// agregar, ni desbloquear.
#[test]
fn damaged_pc_rows_are_reported_and_do_not_hide_the_valid_ones() {
    /// Cómo dañar la fila de la PC `bad` (la segunda es la buena).
    type Damage = fn(&Path, PairedPcId, PairedPcId);
    let damages: [(&str, Damage); 4] = [
        ("un byte alterado", |path, bad, _| {
            flip_pc_byte(path, bad, 30)
        }),
        ("versión fuera de u16", |path, bad, _| {
            raw(path)
                .execute(
                    "UPDATE paired_pcs SET pc_schema_version = 70000 WHERE id = ?1",
                    [bad.as_bytes().as_slice()],
                )
                .unwrap();
        }),
        ("movida a otro id", |path, bad, _| {
            raw(path)
                .execute(
                    "UPDATE paired_pcs SET id = ?1 WHERE id = ?2",
                    rusqlite::params![[0xEE_u8; 16].as_slice(), bad.as_bytes().as_slice()],
                )
                .unwrap();
        }),
        ("datos de la identidad", |path, bad, _| {
            // Intercambiada con la de la identidad: otro dato autenticado.
            let conn = raw(path);
            let identity: Vec<u8> = conn
                .query_row("SELECT data FROM identity", [], |row| row.get(0))
                .unwrap();
            conn.execute(
                "UPDATE paired_pcs SET data = ?1 WHERE id = ?2",
                rusqlite::params![identity, bad.as_bytes().as_slice()],
            )
            .unwrap();
        }),
    ];

    for (label, damage) in damages {
        let (_dir, path, mut vault) = new_vault();
        vault.ensure_identity().unwrap();
        let good = vault.add_paired_pc(new_pc(1, "Buena")).unwrap();
        let bad = vault.add_paired_pc(new_pc(2, "Mala")).unwrap();
        damage(&path, bad, good);

        let listed = vault.paired_pcs().unwrap();
        let ids: Vec<_> = listed.valid.iter().map(PairedPc::id).collect();
        assert_eq!(ids, vec![good], "{label}");
        assert_eq!(
            listed.corrupt.len() + listed.unidentified_corrupt,
            1,
            "{label}"
        );
        // Se puede seguir usando la lista y la bóveda.
        vault.add_paired_pc(new_pc(3, "Nueva")).unwrap();
        assert_eq!(vault.paired_pcs().unwrap().valid.len(), 2, "{label}");
        vault.lock();
        assert!(vault.unlock(&password("correcta-larga")).is_ok(), "{label}");
    }
}

#[test]
fn pc_with_an_id_of_15_bytes_is_unidentified_corrupt() {
    let (_dir, path, mut vault) = new_vault();
    vault.add_paired_pc(new_pc(1, "Buena")).unwrap();
    let conn = raw(&path);
    conn.execute_batch("PRAGMA ignore_check_constraints = ON")
        .unwrap();
    conn.execute(
        "INSERT INTO paired_pcs (id, pc_schema_version, data) VALUES (zeroblob(15), 1, zeroblob(60))",
        [],
    )
    .unwrap();
    let listed = vault.paired_pcs().unwrap();
    assert_eq!(listed.valid.len(), 1);
    assert!(listed.corrupt.is_empty());
    assert_eq!(listed.unidentified_corrupt, 1);
}

#[test]
fn remove_paired_pc_deletes_a_corrupt_row() {
    let (_dir, path, mut vault) = new_vault();
    let bad = vault.add_paired_pc(new_pc(2, "Mala")).unwrap();
    flip_pc_byte(&path, bad, 30);
    assert_eq!(vault.paired_pcs().unwrap().corrupt, vec![bad]);
    vault.remove_paired_pc(bad).unwrap();
    assert_eq!(pairing_row_counts(&path), (0, 0));
    assert!(vault.paired_pcs().unwrap().corrupt.is_empty());
}

#[test]
fn update_of_a_corrupt_pc_is_corrupt_and_writes_nothing() {
    let (_dir, path, mut vault) = new_vault();
    let bad = vault.add_paired_pc(new_pc(2, "Mala")).unwrap();
    flip_pc_byte(&path, bad, 30);
    let before = pc_row_snapshot(&path);
    let update = PairedPcUpdate {
        name: Some("Nueva".to_owned()),
        last_ip: None,
    };
    assert_eq!(
        vault.update_paired_pc(bad, update).err(),
        Some(Error::CorruptPairedPc)
    );
    assert_eq!(pc_row_snapshot(&path), before);
}

#[test]
fn corrupt_pc_does_not_block_unlock_or_entries() {
    let (_dir, path, mut vault) = new_vault();
    let entry = add_sample(&mut vault, "banco");
    let bad = vault.add_paired_pc(new_pc(2, "Mala")).unwrap();
    flip_pc_byte(&path, bad, 30);
    vault.lock();
    drop(vault);

    let mut reopened = Vault::open(&path).unwrap();
    let report = reopened.unlock(&password("correcta-larga")).unwrap();
    assert!(report.corrupt.is_empty());
    assert_eq!(revealed_password(&mut reopened, entry), "clave-banco");
}

#[test]
fn identity_swapped_with_a_pc_row_is_corrupt() {
    let (_dir, path, mut vault) = new_vault();
    vault.ensure_identity().unwrap();
    let pc = vault.add_paired_pc(new_pc(1, "Casa")).unwrap();
    let pc_blob = pc_data(&path, pc);
    let conn = raw(&path);
    // El `CHECK` de longitud de `identity` no deja guardar un blob de otra
    // longitud: se desactiva en la conexión de la prueba.
    conn.execute_batch("PRAGMA ignore_check_constraints = ON")
        .unwrap();
    conn.execute("UPDATE identity SET data = ?1", [pc_blob])
        .unwrap();
    assert_eq!(vault.identity().err(), Some(Error::CorruptIdentity));
}

// --- Menores de la revisión final de la #93 ------------------------------

/// Otra conexión migró la v2 a v3 entre la lectura de la cabecera y la
/// migración de `unlock_with_key`: no se migra otra vez ni se escribe nada, y
/// se desbloquea con la cabecera actual.
#[test]
fn concurrent_v2_to_v3_migration_with_key_unlocks_without_migrating_again() {
    let key = fixture_v2_vault_key();
    let (_dir, path) = fixture_v2_copy();
    let mut a = Vault::open(&path).unwrap();
    let mut b = Vault::open(&path).unwrap();

    // La A leyó la cabecera v2; antes de migrar, la B migra a v3.
    let header = a.storage.read_header().unwrap();
    assert_eq!(header.format_version(), 2);
    b.unlock(&password(FIXTURE_V2_PASSWORD)).unwrap();
    let check_from_b = b.storage.read_header().unwrap().key_check().copied();
    drop(b);

    let report = a.finish_unlock_with_key(key, &header).unwrap();
    assert!(report.corrupt.is_empty());
    assert_eq!(a.entries().unwrap().len(), 3);
    assert_eq!(stored_version_of(&path), 3);
    // La cabecera es la que escribió la B: la A no migró otra vez.
    assert_eq!(
        a.storage.read_header().unwrap().key_check().copied(),
        check_from_b
    );
}

/// Dos conexiones crean la identidad a la vez: la que pierde la carrera
/// devuelve la de la ganadora y no la sobrescribe.
#[test]
fn store_identity_returns_the_existing_one_when_another_connection_created_it() {
    let (_dir, path, mut a) = new_vault();
    let mut b = Vault::open(&path).unwrap();
    b.unlock(&password("correcta-larga")).unwrap();

    // La A ya comprobó que no había identidad; antes de escribir, la B la crea.
    let winner = b.ensure_identity().unwrap();
    let data_after_b = identity_data(&path);

    let result = a.store_identity(Identity::from_bytes([1u8; 32])).unwrap();
    assert_eq!(key_bytes(&result), key_bytes(&winner));
    assert_ne!(key_bytes(&result), [1u8; 32]);
    assert_eq!(identity_data(&path), data_after_b);
}

#[test]
fn pc_row_with_data_shorter_than_nonce_and_tag_is_corrupt() {
    // El `CHECK` de longitud solo se aplica al escribir: un archivo alterado
    // puede traer una PC con menos de 40 bytes.
    let (_dir, path, mut vault) = new_vault();
    vault.add_paired_pc(new_pc(1, "Buena")).unwrap();
    let short_id = [0x5A_u8; 16];
    let conn = raw(&path);
    conn.execute_batch("PRAGMA ignore_check_constraints = ON")
        .unwrap();
    conn.execute(
        "INSERT INTO paired_pcs (id, pc_schema_version, data) VALUES (?1, 1, zeroblob(10))",
        [short_id.as_slice()],
    )
    .unwrap();

    let listed = vault.paired_pcs().unwrap();
    assert_eq!(listed.valid.len(), 1);
    assert_eq!(listed.corrupt, vec![PairedPcId::from_bytes(short_id)]);
    assert_eq!(listed.unidentified_corrupt, 0);
    // Se puede quitar.
    vault
        .remove_paired_pc(PairedPcId::from_bytes(short_id))
        .unwrap();
    assert!(vault.paired_pcs().unwrap().corrupt.is_empty());
}
