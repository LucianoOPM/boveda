use crate::api::entries::{EntryInput, NewSecret};
use crate::api::error::VaultError;
use crate::api::session::{UnlockReport, VaultSession};
use crate::test_support::{fixture_copy, path_string, temp_path, FIXTURE_PASSWORD};

fn unlocked_fixture() -> (tempfile::TempDir, VaultSession) {
    let (dir, path) = fixture_copy();
    let session = VaultSession::new(path_string(&path));
    session.open().unwrap();
    session.unlock(FIXTURE_PASSWORD.into()).unwrap();
    (dir, session)
}

/// Exporta la copia del fixture a `<dir>/respaldo.db` y devuelve su ruta.
fn exported_backup() -> (tempfile::TempDir, String) {
    let (_fixture_dir, session) = unlocked_fixture();
    let dir = tempfile::tempdir().unwrap();
    let backup = path_string(&dir.path().join("respaldo.db"));
    session.export(backup.clone()).unwrap();
    (dir, backup)
}

fn services(session: &VaultSession) -> Vec<String> {
    let mut names: Vec<String> = session
        .search(String::new())
        .unwrap()
        .into_iter()
        .map(|e| e.service)
        .collect();
    names.sort();
    names
}

fn fixture_services() -> Vec<String> {
    vec!["Banco".into(), "Correo Electrónico".into(), "Nube".into()]
}

const EMPTY_REPORT: UnlockReport = UnlockReport {
    corrupt: Vec::new(),
    unidentified_corrupt: 0,
};

#[test]
fn export_then_import_into_new_session_recovers_entries() {
    let (_backup_dir, backup) = exported_backup();
    let (_dir, path) = temp_path();
    let session = VaultSession::new(path_string(&path));
    assert_eq!(
        session.import_backup(backup, FIXTURE_PASSWORD.into()),
        Ok(EMPTY_REPORT)
    );
    assert_eq!(session.is_locked(), Ok(false));
    assert_eq!(services(&session), fixture_services());
}

#[test]
fn import_over_existing_replaces_it() {
    let (_backup_dir, backup) = exported_backup();
    let (_dir, session) = unlocked_fixture();
    let input = EntryInput {
        service: "Extra".into(),
        url: None,
        username: None,
        tags: vec![],
    };
    let secret = NewSecret {
        password: "clave".into(),
        notes: None,
    };
    session.add(input, secret).unwrap();
    session
        .import_backup(backup, FIXTURE_PASSWORD.into())
        .unwrap();
    assert_eq!(services(&session), fixture_services());
}

#[test]
fn failed_import_reopens_previous_locked() {
    let (_backup_dir, backup) = exported_backup();
    let (_dir, session) = unlocked_fixture();
    assert_eq!(
        session.import_backup(backup, "incorrecta".into()),
        Err(VaultError::WrongPassword)
    );
    assert_eq!(session.is_locked(), Ok(true));
    session.unlock(FIXTURE_PASSWORD.into()).unwrap();
    assert_eq!(services(&session), fixture_services());
}

#[test]
fn import_of_non_vault_reopens_previous() {
    let (_dir, session) = unlocked_fixture();
    let other = tempfile::tempdir().unwrap();
    let junk = other.path().join("basura.db");
    std::fs::write(&junk, "no soy una bóveda").unwrap();
    assert_eq!(
        session.import_backup(path_string(&junk), FIXTURE_PASSWORD.into()),
        Err(VaultError::NotAVault)
    );
    assert_eq!(session.is_locked(), Ok(true));
    session.unlock(FIXTURE_PASSWORD.into()).unwrap();
    assert_eq!(services(&session), fixture_services());
}

#[test]
fn failed_import_without_vault_stays_empty() {
    let (_backup_dir, backup) = exported_backup();
    let (_dir, path) = temp_path();
    let session = VaultSession::new(path_string(&path));
    assert_eq!(
        session.import_backup(backup, "incorrecta".into()),
        Err(VaultError::WrongPassword)
    );
    assert_eq!(session.is_locked(), Ok(true));
    assert_eq!(session.search(String::new()), Err(VaultError::NoVault));
}

#[test]
fn export_requires_unlocked_and_new_absolute_dest() {
    let (_dir, session) = unlocked_fixture();
    let other = tempfile::tempdir().unwrap();
    let existing = other.path().join("existe.db");
    std::fs::write(&existing, "ocupado").unwrap();

    assert_eq!(
        session.export(path_string(&existing)),
        Err(VaultError::AlreadyExists)
    );
    assert_eq!(
        session.export("respaldo.db".into()),
        Err(VaultError::StorageIo)
    );
    assert_eq!(services(&session), fixture_services());

    session.lock().unwrap();
    let dest = path_string(&other.path().join("nuevo.db"));
    assert_eq!(session.export(dest), Err(VaultError::Locked));
    session.unlock(FIXTURE_PASSWORD.into()).unwrap();
    assert_eq!(services(&session), fixture_services());
}
