//! Pruebas unitarias de `vault/header.rs`.

use secrecy::ExposeSecret;

use super::*;

fn password(text: &str) -> SecretString {
    SecretString::from(text)
}

fn new_header() -> (VaultHeader, VaultKey) {
    VaultHeader::create(&password("correcta"), KdfParams::MIN).unwrap()
}

#[test]
fn associated_data_has_fixed_little_endian_layout() {
    let (mut header, _) = new_header();
    header.salt = core::array::from_fn(|i| i as u8);

    let expected: [u8; ASSOCIATED_DATA_LEN] = [
        0x02, 0x00, // versión 2
        0x01, // algoritmo 1 = Argon2id v0x13
        0x00, 0x00, 0x01, 0x00, // memoria 65 536 KiB
        0x03, 0x00, 0x00, 0x00, // 3 iteraciones
        0x01, 0x00, 0x00, 0x00, // paralelismo 1
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, // sal
        0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
    ];
    assert_eq!(header.associated_data(), expected);
}

#[test]
fn create_and_unlock_return_the_same_vault_key() {
    let (header, vault_key) = new_header();
    assert_eq!(header.format_version(), FORMAT_VERSION);
    assert_eq!(header.algorithm(), KdfAlgorithm::Argon2idV13);
    assert_eq!(header.params(), KdfParams::MIN);

    let unlocked = header.unlock(&password("correcta")).unwrap();
    assert_eq!(unlocked.expose_secret(), vault_key.expose_secret());
}

#[test]
fn wrong_password_returns_wrong_password_error() {
    let (header, _) = new_header();
    let result = header.unlock(&password("incorrecta"));
    assert_eq!(result.err(), Some(Error::WrongPassword));
}

#[test]
fn create_rejects_params_below_minimum() {
    let weak = KdfParams {
        iterations: 1,
        ..KdfParams::MIN
    };
    let result = VaultHeader::create(&password("correcta"), weak);
    assert_eq!(result.err(), Some(Error::InvalidKdfParams));
}

#[test]
fn two_vaults_with_the_same_password_differ() {
    let (first, first_key) = new_header();
    let (second, second_key) = new_header();
    assert_ne!(first.salt(), second.salt());
    assert_ne!(first_key.expose_secret(), second_key.expose_secret());
    assert!(first.encrypted_key() != second.encrypted_key());
}

#[test]
fn altering_any_byte_of_associated_data_fails() {
    // Con la llave derivada correcta, solo el dato autenticado cambia:
    // así se comprueba que cada byte de la cabecera está autenticado.
    let (header, _) = new_header();
    let derived = kdf::derive_key(&password("correcta"), header.salt(), &header.params()).unwrap();
    let original = header.associated_data();
    assert!(keys::decrypt_vault_key(header.encrypted_key(), &derived, &original).is_ok());

    for i in 0..ASSOCIATED_DATA_LEN {
        let mut altered = original;
        altered[i] ^= 0x01;
        let result = keys::decrypt_vault_key(header.encrypted_key(), &derived, &altered);
        assert_eq!(result.err(), Some(Error::WrongPassword), "byte {i}");
    }
}

#[test]
fn altering_header_fields_makes_unlock_fail() {
    let (header, _) = new_header();
    let mut altered_headers = Vec::new();

    let mut version = header.clone();
    version.format_version += 1;
    altered_headers.push(("versión", version));

    let mut memory = header.clone();
    memory.params.memory_kib += 1024;
    altered_headers.push(("memoria", memory));

    let mut iterations = header.clone();
    iterations.params.iterations += 1;
    altered_headers.push(("iteraciones", iterations));

    let mut salt = header.clone();
    salt.salt[0] ^= 0x01;
    altered_headers.push(("sal", salt));

    for (field, altered) in altered_headers {
        let result = altered.unlock(&password("correcta"));
        assert_eq!(result.err(), Some(Error::WrongPassword), "{field}");
    }

    // Paralelismo distinto de 1 se rechaza antes de derivar.
    let mut parallelism = header.clone();
    parallelism.params.parallelism = 2;
    let result = parallelism.unlock(&password("correcta"));
    assert_eq!(result.err(), Some(Error::InvalidKdfParams));
}

#[test]
fn from_parts_rebuilds_a_header_that_unlocks() {
    let (header, vault_key) = new_header();
    let rebuilt = VaultHeader::from_parts(
        header.format_version(),
        header.algorithm() as u8,
        header.params(),
        *header.salt(),
        header.encrypted_key().clone(),
        header.key_check().copied(),
    )
    .unwrap();
    assert_eq!(rebuilt.associated_data(), header.associated_data());
    let unlocked = rebuilt.unlock(&password("correcta")).unwrap();
    assert_eq!(unlocked.expose_secret(), vault_key.expose_secret());
}

#[test]
fn from_parts_rejects_unknown_version_algorithm_or_params() {
    let (header, _) = new_header();
    let rebuild = |version: u16, algorithm: u8, params: KdfParams| {
        VaultHeader::from_parts(
            version,
            algorithm,
            params,
            *header.salt(),
            header.encrypted_key().clone(),
            header.key_check().copied(),
        )
        .err()
    };

    assert_eq!(
        rebuild(3, 1, KdfParams::MIN),
        Some(Error::UnsupportedFormatVersion(3))
    );
    assert_eq!(
        rebuild(2, 0, KdfParams::MIN),
        Some(Error::UnsupportedAlgorithm(0))
    );
    assert_eq!(
        rebuild(2, 2, KdfParams::MIN),
        Some(Error::UnsupportedAlgorithm(2))
    );
    let too_much_memory = KdfParams {
        memory_kib: KdfParams::MAX_MEMORY_KIB + 1,
        ..KdfParams::MIN
    };
    assert_eq!(
        rebuild(2, 1, too_much_memory),
        Some(Error::InvalidKdfParams)
    );
}

#[test]
fn rekey_keeps_vault_key() {
    let (old, vault_key) = new_header();
    let new = VaultHeader::rekey(&vault_key, &password("nueva"), KdfParams::MIN).unwrap();

    let unlocked = new.unlock(&password("nueva")).unwrap();
    assert_eq!(unlocked.expose_secret(), vault_key.expose_secret());
    assert!(matches!(
        new.unlock(&password("correcta")),
        Err(Error::WrongPassword)
    ));
    assert_ne!(new.salt(), old.salt());
    assert_ne!(new.encrypted_key().nonce, old.encrypted_key().nonce);
}

#[test]
fn new_header_is_v2_with_valid_key_check() {
    let (header, key) = new_header();
    assert_eq!(header.format_version(), 2);
    assert!(header.key_check().is_some());
    assert_eq!(header.verify_key(&key), Ok(()));
}

#[test]
fn verify_key_rejects_other_key() {
    let (header, _) = new_header();
    let other = keys::generate_vault_key().unwrap();
    assert_eq!(header.verify_key(&other), Err(Error::WrongVaultKey));
}

#[test]
fn rekey_creates_new_key_check_for_same_key() {
    let (old, vault_key) = new_header();
    let new = VaultHeader::rekey(&vault_key, &password("nueva"), KdfParams::MIN).unwrap();
    assert_ne!(old.key_check(), new.key_check());
    assert_eq!(new.verify_key(&vault_key), Ok(()));
}

#[test]
fn from_parts_versions_and_key_check_presence() {
    let (header, key) = new_header();
    let check = *header.key_check().unwrap();
    let rebuild = |version: u16, key_check: Option<[u8; KEY_CHECK_LEN]>| {
        VaultHeader::from_parts(
            version,
            1,
            header.params(),
            *header.salt(),
            header.encrypted_key().clone(),
            key_check,
        )
    };

    assert!(rebuild(2, Some(check)).is_ok());
    let v1 = rebuild(1, None).unwrap();
    assert_eq!(v1.format_version(), 1);
    assert_eq!(v1.key_check(), None);
    assert_eq!(v1.verify_key(&key), Err(Error::WrongVaultKey));
    assert_eq!(
        rebuild(1, Some(check)).err(),
        Some(Error::Storage(StorageErrorKind::Corrupt))
    );
    assert_eq!(
        rebuild(2, None).err(),
        Some(Error::Storage(StorageErrorKind::Corrupt))
    );
    assert_eq!(
        rebuild(3, Some(check)).err(),
        Some(Error::UnsupportedFormatVersion(3))
    );
}

/// Cabecera de la versión 1 con la llave `vault_key` y la contraseña
/// `password`: dato autenticado con la versión 1 y sin `key_check`.
fn v1_header(vault_key: &VaultKey, password_text: &str) -> VaultHeader {
    let params = KdfParams::MIN;
    let salt = kdf::generate_salt().unwrap();
    let ad = encode_associated_data(FORMAT_VERSION_V1, KdfAlgorithm::Argon2idV13, &params, &salt);
    let encrypted =
        keys::seal_vault_key(vault_key, &password(password_text), &salt, &params, &ad).unwrap();
    VaultHeader::from_parts(FORMAT_VERSION_V1, 1, params, salt, encrypted, None).unwrap()
}

#[test]
fn unlock_upgrading_v2_returns_no_new_header() {
    let (header, vault_key) = new_header();
    let (key, upgraded) = header.unlock_upgrading(&password("correcta")).unwrap();
    assert_eq!(key.expose_secret(), vault_key.expose_secret());
    assert!(upgraded.is_none());
}

#[test]
fn unlock_upgrading_v1_builds_v2_header_for_same_key() {
    let vault_key = keys::generate_vault_key().unwrap();
    let old = v1_header(&vault_key, "correcta");

    let (key, upgraded) = old.unlock_upgrading(&password("correcta")).unwrap();
    assert_eq!(key.expose_secret(), vault_key.expose_secret());
    let new = upgraded.unwrap();
    assert_eq!(new.format_version(), 2);
    assert_eq!(new.salt(), old.salt());
    assert_eq!(new.params(), old.params());
    assert!(new.encrypted_key().nonce != old.encrypted_key().nonce);
    assert_eq!(new.verify_key(&vault_key), Ok(()));
    let reopened = new.unlock(&password("correcta")).unwrap();
    assert_eq!(reopened.expose_secret(), vault_key.expose_secret());
}

#[test]
fn unlock_upgrading_v1_wrong_password() {
    let vault_key = keys::generate_vault_key().unwrap();
    let old = v1_header(&vault_key, "correcta");
    assert!(matches!(
        old.unlock_upgrading(&password("otra")),
        Err(Error::WrongPassword)
    ));
}
