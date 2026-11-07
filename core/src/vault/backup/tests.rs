//! Pruebas unitarias de `vault/backup.rs`.

use std::fs;
use std::path::PathBuf;

use secrecy::ExposeSecret;

use super::*;
use crate::crypto::kdf::KdfParams;
use crate::vault::test_support::*;
use crate::vault::{EntryId, Error, SearchData, Secret, StorageErrorKind, Vault};

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
