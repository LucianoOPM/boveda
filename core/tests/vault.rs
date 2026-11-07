//! Pruebas de punta a punta de la API pública `vault_core::vault`, con la
//! calibración real de Argon2id. Tardan algunos segundos.

use secrecy::{ExposeSecret, SecretString};
use vault_core::vault::{Error, SearchData, Secret, Vault};

fn password(text: &str) -> SecretString {
    SecretString::from(text)
}

fn temp_path() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("boveda.db");
    (dir, path)
}

#[test]
fn create_lock_unlock_with_calibration() {
    let (_dir, path) = temp_path();
    let mut vault = Vault::create(&path, &password("maestra")).unwrap();
    let search = SearchData::new("gmail", None, Some("yo@example.com"), &["correo"]).unwrap();
    let secret = Secret::new(password("clave-gmail"), None).unwrap();
    let id = vault.add(search, secret).unwrap();

    vault.lock();
    let report = vault.unlock(&password("maestra")).unwrap();
    assert!(report.corrupt.is_empty());
    let entries = vault.entries().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].id(), id);
    assert_eq!(entries[0].search().service(), "gmail");
    assert_eq!(
        vault.reveal(id).unwrap().password().expose_secret(),
        "clave-gmail"
    );

    // Parámetros calibrados dentro de los límites, sin fijar valores exactos.
    let (memory, iterations, parallelism): (i64, i64, i64) = rusqlite::Connection::open(&path)
        .unwrap()
        .query_row(
            "SELECT kdf_memory_kib, kdf_iterations, kdf_parallelism FROM header",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert!((65_536..=131_072).contains(&memory), "memoria: {memory}");
    assert!(iterations >= 3, "iteraciones: {iterations}");
    assert_eq!(parallelism, 1);
}

#[test]
fn change_password_with_calibration() {
    let (_dir, path) = temp_path();
    let mut vault = Vault::create(&path, &password("vieja")).unwrap();
    vault
        .change_master_password(&password("vieja"), &password("nueva"))
        .unwrap();
    vault.lock();
    assert_eq!(
        vault.unlock(&password("vieja")).err(),
        Some(Error::WrongPassword)
    );
    assert!(vault.unlock(&password("nueva")).is_ok());
}
