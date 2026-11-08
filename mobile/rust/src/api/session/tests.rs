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

#[test]
fn create_with_short_password_fails_and_leaves_no_vault() {
    let (_dir, path) = temp_path();
    let session = VaultSession::new(path_string(&path));
    assert_eq!(
        session.create("corta".into()),
        Err(VaultError::PasswordTooShort)
    );
    assert_eq!(session.is_locked(), Ok(true));
    assert_eq!(
        session.search(String::new()).err(),
        Some(VaultError::NoVault)
    );
    assert!(!path.exists());
}

/// Convierte un id de la fachada (32 caracteres hexadecimales) a sus bytes.
fn id_bytes(id: &str) -> Vec<u8> {
    (0..id.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&id[i..i + 2], 16).unwrap())
        .collect()
}

#[test]
fn unlock_reports_corrupt_entry_and_rest_still_opens() {
    let (_dir, path) = fixture_copy();

    // Ids de las entradas vivas, leídos con una sesión que después se suelta.
    let entries = {
        let session = VaultSession::new(path_string(&path));
        session.open().unwrap();
        session.unlock(FIXTURE_PASSWORD.into()).unwrap();
        session.search(String::new()).unwrap()
    };
    assert!(entries.len() >= 2, "hacen falta dos entradas vivas");
    let banco_id = entries
        .iter()
        .find(|e| e.service == "Banco")
        .expect("el fixture debe tener la entrada Banco")
        .id
        .clone();

    // Altera un bit de los datos de búsqueda de `Banco`, solo en la copia.
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        let id = id_bytes(&banco_id);
        let mut blob: Vec<u8> = conn
            .query_row("SELECT search FROM entries WHERE id = ?1", [&id], |r| {
                r.get(0)
            })
            .unwrap();
        blob[30] ^= 1;
        conn.execute(
            "UPDATE entries SET search = ?1 WHERE id = ?2",
            rusqlite::params![blob, id],
        )
        .unwrap();
    }

    let session = VaultSession::new(path_string(&path));
    session.open().unwrap();
    assert_eq!(
        session.unlock(FIXTURE_PASSWORD.into()),
        Ok(UnlockReport {
            corrupt: vec![banco_id],
            unidentified_corrupt: 0
        })
    );
    let services: Vec<String> = session
        .search(String::new())
        .unwrap()
        .into_iter()
        .map(|e| e.service)
        .collect();
    assert_eq!(services.len(), 2);
    assert!(services.contains(&"Correo Electrónico".to_owned()));
    assert!(services.contains(&"Nube".to_owned()));
}

#[test]
fn vault_key_round_trip() {
    let (_dir, session) = opened_fixture();
    session.unlock(FIXTURE_PASSWORD.into()).unwrap();
    let key = session.export_vault_key().unwrap();
    assert_eq!(key.len(), 32);
    session.lock().unwrap();
    assert_eq!(
        session.unlock_with_key(key),
        Ok(UnlockReport {
            corrupt: vec![],
            unidentified_corrupt: 0
        })
    );
    assert_eq!(session.search(String::new()).unwrap().len(), 3);
}

#[test]
fn unlock_with_key_of_wrong_length_stays_locked() {
    for len in [31, 33] {
        let (_dir, session) = opened_fixture();
        assert_eq!(
            session.unlock_with_key(vec![7; len]),
            Err(VaultError::WrongVaultKey)
        );
        assert_eq!(session.is_locked(), Ok(true));
    }
}

#[test]
fn unlock_with_zeroed_key_stays_locked() {
    // Una llave del largo correcto que no viene de ninguna bóveda.
    let (_dir, session) = opened_fixture();
    assert_eq!(
        session.unlock_with_key(vec![0; 32]),
        Err(VaultError::WrongVaultKey)
    );
    assert_eq!(session.is_locked(), Ok(true));
}

#[test]
fn unlock_with_key_of_other_vault_fails() {
    let (_other_dir, other_path) = temp_path();
    let other = VaultSession::new(path_string(&other_path));
    other.create("otra-contraseña-larga".into()).unwrap();
    let other_key = other.export_vault_key().unwrap();

    let (_dir, session) = opened_fixture();
    assert_eq!(
        session.unlock_with_key(other_key),
        Err(VaultError::WrongVaultKey)
    );
    assert_eq!(session.is_locked(), Ok(true));
}

#[test]
fn export_vault_key_locked_is_locked() {
    let (_dir, session) = opened_fixture();
    assert_eq!(session.export_vault_key(), Err(VaultError::Locked));
}

#[test]
fn unlock_with_key_when_unlocked_is_already_unlocked() {
    let (_dir, session) = opened_fixture();
    session.unlock(FIXTURE_PASSWORD.into()).unwrap();
    let key = session.export_vault_key().unwrap();
    assert_eq!(
        session.unlock_with_key(key),
        Err(VaultError::AlreadyUnlocked)
    );
}

#[test]
fn vault_key_without_vault_is_no_vault() {
    let (_dir, path) = temp_path();
    let session = VaultSession::new(path_string(&path));
    assert_eq!(session.export_vault_key(), Err(VaultError::NoVault));
    assert_eq!(
        session.unlock_with_key(vec![0; 32]),
        Err(VaultError::NoVault)
    );
}
