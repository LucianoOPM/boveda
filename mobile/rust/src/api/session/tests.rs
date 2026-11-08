use super::*;
use crate::test_support::{fixture_copy, path_string, temp_path, FIXTURE_PASSWORD};

fn opened_fixture() -> (tempfile::TempDir, VaultSession) {
    let (dir, path) = fixture_copy();
    let session = VaultSession::new(path_string(&path));
    session.open().unwrap();
    (dir, session)
}

#[test]
fn without_vault_is_locked_and_operations_fail() {
    let (_dir, path) = temp_path();
    let session = VaultSession::new(path_string(&path));
    assert_eq!(session.is_locked(), Ok(true));
    assert_eq!(
        session.unlock(FIXTURE_PASSWORD.into()),
        Err(VaultError::NoVault)
    );
    assert_eq!(session.lock(), Ok(()));
}

#[test]
fn open_leaves_vault_locked() {
    let (_dir, session) = opened_fixture();
    assert_eq!(session.is_locked(), Ok(true));
}

#[test]
fn unlock_with_wrong_password_stays_locked() {
    let (_dir, session) = opened_fixture();
    assert_eq!(
        session.unlock("incorrecta".into()),
        Err(VaultError::WrongPassword)
    );
    assert_eq!(session.is_locked(), Ok(true));
}

#[test]
fn unlock_with_fixture_password() {
    let (_dir, session) = opened_fixture();
    assert_eq!(
        session.unlock(FIXTURE_PASSWORD.into()),
        Ok(UnlockReport {
            corrupt: vec![],
            unidentified_corrupt: 0
        })
    );
    assert_eq!(session.is_locked(), Ok(false));
    assert_eq!(
        session.unlock(FIXTURE_PASSWORD.into()),
        Err(VaultError::AlreadyUnlocked)
    );
}

#[test]
fn open_or_create_with_vault_loaded_fails() {
    let (_dir, session) = opened_fixture();
    assert_eq!(session.open(), Err(VaultError::VaultAlreadyOpen));
    assert_eq!(
        session.create(FIXTURE_PASSWORD.into()),
        Err(VaultError::VaultAlreadyOpen)
    );
}

#[test]
fn lock_after_unlock_locks() {
    let (_dir, session) = opened_fixture();
    session.unlock(FIXTURE_PASSWORD.into()).unwrap();
    assert_eq!(session.lock(), Ok(()));
    assert_eq!(session.is_locked(), Ok(true));
    assert!(session.unlock(FIXTURE_PASSWORD.into()).is_ok());
}

#[test]
fn poisoned_mutex_gives_session_poisoned() {
    let (_dir, session) = opened_fixture();
    std::thread::scope(|scope| {
        let handle = scope.spawn(|| {
            let _guard = session.vault.lock().unwrap();
            panic!("envenenar el candado");
        });
        assert!(handle.join().is_err());
    });
    assert_eq!(session.is_locked(), Err(VaultError::SessionPoisoned));
    assert_eq!(session.lock(), Err(VaultError::SessionPoisoned));
}

#[test]
fn create_on_new_path_is_unlocked_and_empty() {
    let (_dir, path) = temp_path();
    let session = VaultSession::new(path_string(&path));
    assert_eq!(session.create("contraseña nueva".into()), Ok(()));
    assert_eq!(session.is_locked(), Ok(false));
}
