//! Pruebas unitarias de la API `Vault` (`vault/mod.rs`).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use secrecy::ExposeSecret;
use zeroize::ZeroizeOnDrop;

use super::*;
use crate::crypto::kdf::KdfParams;
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
    let vault = Vault::create_with_params(&path, &password("correcta"), KdfParams::MIN).unwrap();
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
    vault.unlock(&password("correcta")).unwrap();
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
        Vault::create_with_params(&path, &password("maestra-Xy7"), KdfParams::MIN).unwrap();
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
    vault.unlock(&password("correcta")).unwrap()
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
            &password("correcta"),
            &password("nueva"),
            KdfParams::MIN,
        )
        .unwrap();
    vault.lock();

    assert_eq!(
        vault.unlock(&password("correcta")).err(),
        Some(Error::WrongPassword)
    );
    vault.unlock(&password("nueva")).unwrap();
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
                &password("nueva"),
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
                &password("correcta"),
                &password("nueva"),
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
            &password("correcta"),
            &password("correcta"),
            KdfParams::MIN,
        )
        .unwrap();
    assert_ne!(header_row(&path).0, before.0);
    vault.lock();
    assert!(vault.unlock(&password("correcta")).is_ok());
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

// --- Bóveda de ejemplo del formato 1 -------------------------------------

/// Contraseña de prueba de `core/tests/fixtures/vault-v1.db`. Se repite en
/// `core/tests/vault.rs`, que no ve el código `cfg(test)` del crate.
const FIXTURE_V1_PASSWORD: &str = "prueba-bóveda-v1";

/// Genera `core/tests/fixtures/vault-v1.db`, la bóveda de ejemplo del
/// formato 1, que las pruebas de `core/tests/vault.rs` deben abrir siempre.
///
/// El número del nombre es la `format_version` de la cabecera. Cada versión
/// de formato nueva lleva un archivo nuevo (`vault-v2.db`, ...) y su propio
/// generador; los archivos existentes no se modifican ni se borran. Si el
/// archivo ya existe, falla a propósito con `AlreadyExists`.
///
/// Se ejecuta una sola vez, al introducir una versión de formato nueva:
/// `cargo test -p vault-core --lib -- --ignored --exact vault::tests::generate_fixture_v1`
///
/// Nunca con `--ignored` sin filtro: también correría este generador, que
/// fallaría con `AlreadyExists` (o crearía una bóveda distinta si falta el
/// archivo).
///
/// Contenido y contraseña: `core/tests/fixtures/README.md`.
#[test]
#[ignore = "solo al introducir una versión de formato nueva"]
fn generate_fixture_v1() {
    // Ruta absoluta, como exigen las reglas de rutas (`SQLITE_USE_URI`).
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/vault-v1.db");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    // Sin borrar nada antes: si el archivo existe, `AlreadyExists`.
    let mut vault =
        Vault::create_with_params(&path, &password(FIXTURE_V1_PASSWORD), KdfParams::MIN).unwrap();

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
