//! Pruebas unitarias de `vault/backup.rs`.

use std::fs;
use std::path::PathBuf;

use secrecy::ExposeSecret;

use super::*;
use crate::crypto::kdf::KdfParams;
use crate::vault::test_support::*;
use crate::vault::{EntryId, Error, SearchData, Secret, StorageErrorKind, UnlockReport, Vault};

/// Bóveda nueva y desbloqueada con contraseña `correcta`, en `boveda.db`.
fn new_vault() -> (tempfile::TempDir, PathBuf, Vault) {
    let (dir, path) = temp_path();
    let vault = Vault::create_with_params(&path, &password("correcta"), KdfParams::MIN).unwrap();
    (dir, path, vault)
}

fn add_sample(vault: &mut Vault, service: &str) -> EntryId {
    let (search, secret) = sample(service);
    vault.add(search, secret).unwrap()
}

/// Ids y servicios de las entradas, ordenados por id.
fn ids_and_services(vault: &Vault) -> Vec<(EntryId, String)> {
    let mut rows: Vec<_> = vault
        .entries()
        .unwrap()
        .iter()
        .map(|e| (e.id(), e.search().service().to_owned()))
        .collect();
    rows.sort_by_key(|r| *r.0.as_bytes());
    rows
}

// --- Exportar ----------------------------------------------------------

#[test]
fn export_creates_openable_backup() {
    let (dir, _path, mut vault) = new_vault();
    add_sample(&mut vault, "gmail");
    add_sample(&mut vault, "banco");
    let dest = dir.path().join("respaldo.db");

    vault.export(&dest).unwrap();

    Storage::open_read_only(&dest).unwrap().close().unwrap();
    let mut copy = Vault::open(&dest).unwrap();
    copy.unlock(&password("correcta")).unwrap();
    assert_eq!(ids_and_services(&copy), ids_and_services(&vault));
}

#[test]
fn export_contains_no_known_plaintext() {
    let (dir, path) = temp_path();
    let mut vault =
        Vault::create_with_params(&path, &password("canario-maestra"), KdfParams::MIN).unwrap();
    for n in 1..=2 {
        let search = SearchData::new(
            &format!("canario-servicio-{n}"),
            Some(&format!("https://canario-url-{n}.example")),
            Some(&format!("canario-usuario-{n}")),
            &[&format!("canario-etiqueta-{n}")],
        )
        .unwrap();
        let secret = Secret::new(
            password(&format!("canario-pass-{n}")),
            Some(password(&format!("canario-notas-{n}"))),
        )
        .unwrap();
        vault.add(search, secret).unwrap();
    }
    let dest = dir.path().join("respaldo.db");

    vault.export(&dest).unwrap();

    let bytes = fs::read(&dest).unwrap();
    assert!(!contains(&bytes, b"canario-"));
}

#[test]
fn export_purges_deleted_entry_parts() {
    let (dir, path, mut vault) = new_vault();
    let keep = add_sample(&mut vault, "gmail");
    let gone = add_sample(&mut vault, "banco");
    let (search, secret): (Vec<u8>, Vec<u8>) = raw(&path)
        .query_row(
            "SELECT search, secret FROM entries WHERE id = ?1",
            [gone.as_bytes().as_slice()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    vault.delete(gone).unwrap();
    let dest = dir.path().join("respaldo.db");

    vault.export(&dest).unwrap();

    let bytes = fs::read(&dest).unwrap();
    assert!(!contains(&bytes, &search));
    assert!(!contains(&bytes, &secret));
    // Control: la entrada viva sí viaja.
    let mut copy = Vault::open(&dest).unwrap();
    copy.unlock(&password("correcta")).unwrap();
    assert_eq!(copy.entries().unwrap().len(), 1);
    assert_eq!(copy.entries().unwrap()[0].id(), keep);
}

#[test]
fn export_locked_is_locked() {
    let (dir, _path, mut vault) = new_vault();
    vault.lock();
    let dest = dir.path().join("respaldo.db");

    assert_eq!(vault.export(&dest), Err(Error::Locked));
    assert!(!dest.exists());
}

#[test]
fn export_existing_dest_is_already_exists() {
    let (dir, _path, vault) = new_vault();
    let dest = dir.path().join("respaldo.db");
    fs::write(&dest, b"previo").unwrap();

    assert_eq!(vault.export(&dest), Err(Error::AlreadyExists));
    assert_eq!(fs::read(&dest).unwrap(), b"previo");
}

#[test]
fn export_twice_keeps_first_backup() {
    let (dir, _path, mut vault) = new_vault();
    add_sample(&mut vault, "gmail");
    let dest = dir.path().join("respaldo.db");
    vault.export(&dest).unwrap();
    let first = fs::read(&dest).unwrap();

    assert_eq!(vault.export(&dest), Err(Error::AlreadyExists));
    assert_eq!(fs::read(&dest).unwrap(), first);
}

#[test]
fn export_relative_path_is_io() {
    let (_dir, _path, vault) = new_vault();
    let dest = Path::new("respaldo-relativo-no-debe-existir.db");

    assert_eq!(
        vault.export(dest),
        Err(Error::Storage(StorageErrorKind::Io))
    );
    assert!(!dest.exists());
}

#[test]
fn export_missing_directory_is_io() {
    let (dir, _path, vault) = new_vault();
    let dest = dir.path().join("no-existe").join("respaldo.db");

    assert_eq!(
        vault.export(&dest),
        Err(Error::Storage(StorageErrorKind::Io))
    );
    assert!(!dest.exists());
}

#[test]
fn export_non_ascii_path() {
    let (dir, _path, mut vault) = new_vault();
    add_sample(&mut vault, "gmail");
    let dest = dir.path().join("bóveda ñ 2026.db");

    vault.export(&dest).unwrap();

    let mut copy = Vault::open(&dest).unwrap();
    copy.unlock(&password("correcta")).unwrap();
    assert_eq!(copy.entries().unwrap().len(), 1);
}

#[cfg(unix)]
#[test]
fn export_non_utf8_path_is_io() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let (dir, _path, vault) = new_vault();
    let dest = dir.path().join(OsStr::from_bytes(b"\xFF.db"));

    assert_eq!(
        vault.export(&dest),
        Err(Error::Storage(StorageErrorKind::Io))
    );
    assert!(!dest.exists());
}

#[test]
fn exported_secrets_match() {
    let (dir, _path, mut vault) = new_vault();
    let id = add_sample(&mut vault, "gmail");
    let dest = dir.path().join("respaldo.db");

    vault.export(&dest).unwrap();

    let mut copy = Vault::open(&dest).unwrap();
    copy.unlock(&password("correcta")).unwrap();
    let secret = copy.reveal(id).unwrap();
    assert_eq!(secret.password().expose_secret(), "clave-gmail");
}

// --- Importar ----------------------------------------------------------

/// Bóveda `origen.db` (contraseña `respaldo`) con gmail y banco, exportada
/// a `respaldo.db`. Devuelve la ruta del respaldo y sus ids y servicios.
fn make_backup(dir: &Path) -> (PathBuf, Vec<(EntryId, String)>) {
    let origin = dir.join("origen.db");
    let mut vault =
        Vault::create_with_params(&origin, &password("respaldo"), KdfParams::MIN).unwrap();
    add_sample(&mut vault, "gmail");
    add_sample(&mut vault, "banco");
    let backup = dir.join("respaldo.db");
    vault.export(&backup).unwrap();
    (backup, ids_and_services(&vault))
}

/// Bóveda actual en `dest` (contraseña `actual`) con una entrada, cerrada.
fn make_current(dest: &Path) {
    let mut vault = Vault::create_with_params(dest, &password("actual"), KdfParams::MIN).unwrap();
    add_sample(&mut vault, "actual");
}

/// Importa esperando `expected`, y comprueba que `dest` quedó igual byte
/// por byte (o sigue sin existir) y que no quedó la copia temporal.
fn assert_import_fails_intact(backup: &Path, dest: &Path, pw: &str, expected: Error) {
    let before = fs::read(dest).ok();
    let err = Vault::import(backup, dest, &password(pw)).err().unwrap();
    assert_eq!(err, expected);
    assert_eq!(fs::read(dest).ok(), before);
    assert!(!sibling(dest, IMPORT_SUFFIX).exists());
}

#[test]
fn import_round_trip() {
    let (dir, path, mut vault) = new_vault();
    let gmail = add_sample(&mut vault, "gmail");
    let banco = add_sample(&mut vault, "banco");
    let gone = add_sample(&mut vault, "borrada");
    vault.delete(gone).unwrap();
    let backup = dir.path().join("respaldo.db");
    vault.export(&backup).unwrap();
    let expected = ids_and_services(&vault);
    drop(vault);
    fs::remove_file(&path).unwrap();

    let (mut imported, report) = Vault::import(&backup, &path, &password("correcta")).unwrap();

    assert_eq!(
        report,
        UnlockReport {
            corrupt: vec![],
            unidentified_corrupt: 0
        }
    );
    assert_eq!(ids_and_services(&imported), expected);
    for (id, service) in [(gmail, "gmail"), (banco, "banco")] {
        let secret = imported.reveal(id).unwrap();
        assert_eq!(
            secret.password().expose_secret(),
            format!("clave-{service}")
        );
        assert_eq!(secret.notes().unwrap().expose_secret(), "notas de prueba");
    }
    assert_eq!(imported.reveal(gone).err(), Some(Error::EntryNotFound));
    assert!(!sibling(&path, IMPORT_SUFFIX).exists());
}

#[test]
fn import_wrong_password_keeps_dest() {
    let dir = tempfile::tempdir().unwrap();
    let (backup, _) = make_backup(dir.path());
    let dest = dir.path().join("boveda.db");
    make_current(&dest);

    assert_import_fails_intact(&backup, &dest, "incorrecta", Error::WrongPassword);
}

#[test]
fn import_future_version_keeps_dest() {
    let dir = tempfile::tempdir().unwrap();
    let (backup, _) = make_backup(dir.path());
    raw(&backup)
        .execute("UPDATE header SET format_version = 2", [])
        .unwrap();
    let dest = dir.path().join("boveda.db");
    make_current(&dest);

    assert_import_fails_intact(
        &backup,
        &dest,
        "respaldo",
        Error::UnsupportedFormatVersion(2),
    );
}

#[test]
fn import_not_a_vault_keeps_dest() {
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("boveda.db");
    make_current(&dest);

    let random = dir.path().join("aleatorio.db");
    fs::write(&random, [0x5A_u8; 4096]).unwrap();
    let empty = dir.path().join("vacio.db");
    fs::write(&empty, b"").unwrap();
    let text = dir.path().join("texto.db");
    fs::write(&text, b"hola").unwrap();
    let foreign = dir.path().join("ajeno.db");
    raw(&foreign)
        .execute_batch("CREATE TABLE t(x); INSERT INTO t VALUES (1);")
        .unwrap();

    for backup in [&random, &empty, &text, &foreign] {
        assert_import_fails_intact(backup, &dest, "respaldo", Error::NotAVault);
    }
}

#[test]
fn import_damaged_backup_is_corrupt() {
    let dir = tempfile::tempdir().unwrap();
    let (backup, _) = make_backup(dir.path());
    damage_entries_root_page(&backup);
    let dest = dir.path().join("boveda.db");
    make_current(&dest);

    assert_import_fails_intact(
        &backup,
        &dest,
        "respaldo",
        Error::Storage(StorageErrorKind::Corrupt),
    );
}

#[test]
fn import_reports_corrupt_entry() {
    let dir = tempfile::tempdir().unwrap();
    let (backup, entries) = make_backup(dir.path());
    let damaged = entries[0].0;
    tamper(&backup, damaged, "search");
    let dest = dir.path().join("boveda.db");

    let (imported, report) = Vault::import(&backup, &dest, &password("respaldo")).unwrap();

    assert_eq!(report.corrupt, vec![damaged]);
    assert_eq!(imported.entries().unwrap().len(), 1);
}

#[test]
fn import_into_missing_dest() {
    let dir = tempfile::tempdir().unwrap();
    let (backup, entries) = make_backup(dir.path());
    let dest = dir.path().join("nueva.db");

    assert_import_fails_intact(&backup, &dest, "incorrecta", Error::WrongPassword);
    assert!(!dest.exists());

    let (imported, _) = Vault::import(&backup, &dest, &password("respaldo")).unwrap();
    assert_eq!(ids_and_services(&imported), entries);
}

#[test]
fn import_uses_backup_password() {
    let dir = tempfile::tempdir().unwrap();
    let (backup, entries) = make_backup(dir.path());
    let dest = dir.path().join("boveda.db");
    make_current(&dest);

    let (imported, _) = Vault::import(&backup, &dest, &password("respaldo")).unwrap();
    assert_eq!(ids_and_services(&imported), entries);
    drop(imported);

    let mut reopened = Vault::open(&dest).unwrap();
    assert_eq!(
        reopened.unlock(&password("actual")).err(),
        Some(Error::WrongPassword)
    );
    reopened.unlock(&password("respaldo")).unwrap();
}

#[test]
fn import_replaces_leftover_temp() {
    let dir = tempfile::tempdir().unwrap();
    let (backup, entries) = make_backup(dir.path());
    let dest = dir.path().join("boveda.db");
    fs::write(sibling(&dest, IMPORT_SUFFIX), b"basura").unwrap();

    let (imported, _) = Vault::import(&backup, &dest, &password("respaldo")).unwrap();

    assert_eq!(ids_and_services(&imported), entries);
    assert!(!sibling(&dest, IMPORT_SUFFIX).exists());
}

/// Marca o desmarca un archivo como de solo lectura.
fn set_readonly(path: &Path, readonly: bool) {
    let mut perms = fs::metadata(path).unwrap().permissions();
    perms.set_readonly(readonly);
    fs::set_permissions(path, perms).unwrap();
}

#[test]
fn import_read_only_backup() {
    let dir = tempfile::tempdir().unwrap();
    let (backup, entries) = make_backup(dir.path());
    set_readonly(&backup, true);
    let dest = dir.path().join("boveda.db");

    let result = Vault::import(&backup, &dest, &password("respaldo"));
    set_readonly(&backup, false);

    let (mut imported, _) = result.unwrap();
    assert_eq!(ids_and_services(&imported), entries);
    // La bóveda importada no hereda el solo lectura del respaldo.
    assert!(!fs::metadata(&dest).unwrap().permissions().readonly());
    add_sample(&mut imported, "nueva");
}

#[test]
fn import_from_temp_path_keeps_backup() {
    let dir = tempfile::tempdir().unwrap();
    let (backup, _) = make_backup(dir.path());
    let dest = dir.path().join("boveda.db");
    let temp = sibling(&dest, IMPORT_SUFFIX);
    fs::rename(&backup, &temp).unwrap();
    let before = fs::read(&temp).unwrap();

    assert_eq!(
        Vault::import(&temp, &dest, &password("respaldo")).err(),
        Some(Error::Storage(StorageErrorKind::Io))
    );
    assert_eq!(fs::read(&temp).unwrap(), before);
    assert!(!dest.exists());
}

#[test]
fn import_relative_paths_are_io() {
    let dir = tempfile::tempdir().unwrap();
    let (backup, _) = make_backup(dir.path());
    let dest = dir.path().join("boveda.db");
    let io = Some(Error::Storage(StorageErrorKind::Io));

    let relative_backup = Path::new("respaldo-relativo.db");
    assert_eq!(
        Vault::import(relative_backup, &dest, &password("respaldo")).err(),
        io
    );
    let relative_dest = Path::new("boveda-relativa-no-debe-existir.db");
    assert_eq!(
        Vault::import(&backup, relative_dest, &password("respaldo")).err(),
        io
    );
    assert!(!dest.exists());
    assert!(!relative_dest.exists());
    assert!(!sibling(relative_dest, IMPORT_SUFFIX).exists());
}

#[test]
fn import_missing_backup_or_directory_is_io() {
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("boveda.db");
    make_current(&dest);
    assert_import_fails_intact(
        &dir.path().join("no-existe.db"),
        &dest,
        "respaldo",
        Error::Storage(StorageErrorKind::Io),
    );

    let (backup, _) = make_backup(dir.path());
    let nested = dir.path().join("no-existe").join("boveda.db");
    assert_import_fails_intact(
        &backup,
        &nested,
        "respaldo",
        Error::Storage(StorageErrorKind::Io),
    );
}

#[test]
fn import_same_path_as_dest() {
    let dir = tempfile::tempdir().unwrap();
    let (backup, entries) = make_backup(dir.path());

    let (imported, _) = Vault::import(&backup, &backup, &password("respaldo")).unwrap();

    assert_eq!(ids_and_services(&imported), entries);
    assert!(!sibling(&backup, IMPORT_SUFFIX).exists());
}

#[test]
fn import_non_ascii_paths() {
    let dir = tempfile::tempdir().unwrap();
    let (backup, entries) = make_backup(dir.path());
    let renamed = dir.path().join("respaldo bóveda ñ.db");
    fs::rename(&backup, &renamed).unwrap();
    let dest = dir.path().join("bóveda ñ 2026.db");

    let (imported, _) = Vault::import(&renamed, &dest, &password("respaldo")).unwrap();

    assert_eq!(ids_and_services(&imported), entries);
}

#[test]
fn imported_vault_is_writable_and_exportable() {
    let dir = tempfile::tempdir().unwrap();
    let (backup, _) = make_backup(dir.path());
    let dest = dir.path().join("boveda.db");
    let (mut imported, _) = Vault::import(&backup, &dest, &password("respaldo")).unwrap();

    let new_id = add_sample(&mut imported, "nueva");
    let second = dir.path().join("segundo.db");
    imported.export(&second).unwrap();
    drop(imported);
    let other = dir.path().join("otra.db");
    let (again, _) = Vault::import(&second, &other, &password("respaldo")).unwrap();

    assert!(again.entries().unwrap().iter().any(|e| e.id() == new_id));
    assert_eq!(again.entries().unwrap().len(), 3);
}

#[test]
fn backup_errors_do_not_leak() {
    let dir = tempfile::tempdir().unwrap();
    let origin = dir.path().join("origen.db");
    let mut vault =
        Vault::create_with_params(&origin, &password("maestra-secreta"), KdfParams::MIN).unwrap();
    add_sample(&mut vault, "servicio-secreto");
    let backup = dir.path().join("respaldo.db");
    vault.export(&backup).unwrap();
    let damaged = dir.path().join("danado.db");
    fs::copy(&backup, &damaged).unwrap();
    damage_entries_root_page(&damaged);
    let text = dir.path().join("texto.db");
    fs::write(&text, b"hola").unwrap();
    let dest = dir.path().join("boveda.db");

    let mut errors = vec![vault.export(&backup).err().unwrap()];
    vault.lock();
    errors.push(vault.export(&dir.path().join("otro.db")).err().unwrap());
    for (file, pw) in [
        (&backup, "maestra-incorrecta"),
        (&text, "maestra-secreta"),
        (&damaged, "maestra-secreta"),
    ] {
        errors.push(Vault::import(file, &dest, &password(pw)).err().unwrap());
    }

    let dir_name = dir.path().file_name().unwrap().to_str().unwrap();
    for err in &errors {
        for text in [format!("{err}"), format!("{err:?}")] {
            for needle in ["maestra", "clave-", "servicio-secreto", dir_name] {
                assert!(!text.contains(needle), "{text:?} contiene {needle:?}");
            }
        }
    }
}

// --- El -journal de la bóveda actual -----------------------------------

#[test]
fn import_rolls_back_hot_journal_of_dest() {
    let dir = tempfile::tempdir().unwrap();
    let (backup, entries) = make_backup(dir.path());
    let old = dir.path().join("vieja.db");
    make_current(&old);
    let dest = dir.path().join("boveda.db");
    make_hot_journal_copy(&old, &dest);

    let (imported, _) = Vault::import(&backup, &dest, &password("respaldo")).unwrap();
    assert_eq!(ids_and_services(&imported), entries);
    drop(imported);

    assert!(!sibling(&dest, "-journal").exists());
    let mut reopened = Vault::open(&dest).unwrap();
    let report = reopened.unlock(&password("respaldo")).unwrap();
    assert_eq!(report.unidentified_corrupt, 0);
    assert!(report.corrupt.is_empty());
    assert_eq!(ids_and_services(&reopened), entries);
    assert_eq!(Storage::open(&dest).unwrap().check_integrity(), Ok(()));
}

#[test]
fn import_keeps_unopenable_dest_with_journal() {
    let dir = tempfile::tempdir().unwrap();
    let (backup, _) = make_backup(dir.path());
    let dest = dir.path().join("boveda.db");
    fs::write(&dest, "no soy bóveda").unwrap();
    let journal = sibling(&dest, "-journal");
    fs::write(&journal, [0xAA_u8; 1024]).unwrap();

    assert_import_fails_intact(&backup, &dest, "respaldo", Error::NotAVault);
    assert_eq!(fs::read(&journal).unwrap(), [0xAA_u8; 1024]);
}

#[test]
fn import_deletes_orphan_journal_without_dest() {
    let dir = tempfile::tempdir().unwrap();
    let (backup, entries) = make_backup(dir.path());
    let old = dir.path().join("vieja.db");
    make_current(&old);
    let dest = dir.path().join("boveda.db");
    // Diario caliente real que queda huérfano: si sobreviviera al
    // `rename`, SQLite lo aplicaría sobre el respaldo.
    make_hot_journal_copy(&old, &dest);
    fs::remove_file(&dest).unwrap();

    let (imported, _) = Vault::import(&backup, &dest, &password("respaldo")).unwrap();
    assert_eq!(ids_and_services(&imported), entries);
    drop(imported);

    assert!(!sibling(&dest, "-journal").exists());
    let mut reopened = Vault::open(&dest).unwrap();
    let report = reopened.unlock(&password("respaldo")).unwrap();
    assert_eq!(report.unidentified_corrupt, 0);
    assert!(report.corrupt.is_empty());
    assert_eq!(ids_and_services(&reopened), entries);
    assert_eq!(Storage::open(&dest).unwrap().check_integrity(), Ok(()));
}

#[test]
fn import_deletes_orphan_wal_and_shm_without_dest() {
    let dir = tempfile::tempdir().unwrap();
    let (backup, entries) = make_backup(dir.path());
    let old = dir.path().join("vieja.db");
    make_current(&old);
    let dest = dir.path().join("boveda.db");
    // `-wal` válido de otra bóveda: si sobreviviera al `rename`, SQLite lo
    // integraría al abrir y cambiaría la cabecera del respaldo.
    copy_live_wal(&old, &dest);
    fs::write(sibling(&dest, "-shm"), [0xBB_u8; 1024]).unwrap();

    let (imported, _) = Vault::import(&backup, &dest, &password("respaldo")).unwrap();
    assert_eq!(ids_and_services(&imported), entries);
    drop(imported);

    assert!(!sibling(&dest, "-wal").exists());
    assert!(!sibling(&dest, "-shm").exists());
    let mut reopened = Vault::open(&dest).unwrap();
    let report = reopened.unlock(&password("respaldo")).unwrap();
    assert!(report.corrupt.is_empty());
    assert_eq!(ids_and_services(&reopened), entries);
    assert_eq!(Storage::open(&dest).unwrap().check_integrity(), Ok(()));
}

/// `dest` es un enlace simbólico roto: existe como enlace, así que su
/// diario no es huérfano y no se borra (`symlink_metadata` no sigue el
/// enlace; `Path::exists` lo daría por ausente).
#[cfg(unix)]
#[test]
fn import_keeps_journal_of_dangling_symlink_dest() {
    let dir = tempfile::tempdir().unwrap();
    let (backup, _) = make_backup(dir.path());
    let dest = dir.path().join("boveda.db");
    std::os::unix::fs::symlink(dir.path().join("no-existe.db"), &dest).unwrap();
    let journal = sibling(&dest, "-journal");
    fs::write(&journal, [0xAA_u8; 1024]).unwrap();

    assert_import_fails_intact(
        &backup,
        &dest,
        "respaldo",
        Error::Storage(StorageErrorKind::Io),
    );
    assert_eq!(fs::read(&journal).unwrap(), [0xAA_u8; 1024]);
}

#[cfg(windows)]
#[test]
fn import_fails_while_dest_is_open() {
    let dir = tempfile::tempdir().unwrap();
    let (backup, _) = make_backup(dir.path());
    let dest = dir.path().join("boveda.db");
    make_current(&dest);
    let before = fs::read(&dest).unwrap();
    let conn = raw(&dest);
    let _: i64 = conn
        .query_row("SELECT count(*) FROM entries", [], |r| r.get(0))
        .unwrap();

    let err = Vault::import(&backup, &dest, &password("respaldo"))
        .err()
        .unwrap();
    drop(conn);

    assert!(matches!(
        err,
        Error::Storage(StorageErrorKind::Io | StorageErrorKind::Busy)
    ));
    assert_eq!(fs::read(&dest).unwrap(), before);
    assert!(!sibling(&dest, IMPORT_SUFFIX).exists());
}
