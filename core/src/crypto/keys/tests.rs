//! Pruebas unitarias de `crypto/keys.rs`.

use zeroize::Zeroizing;

use super::*;

const AAD: &[u8] = b"cabecera de prueba";

/// Llave derivada fija: estas pruebas no necesitan pasar por Argon2id.
fn derived(byte: u8) -> DerivedKey {
    Zeroizing::new([byte; 32])
}

#[test]
fn generated_vault_keys_are_different() {
    let first = generate_vault_key().unwrap();
    let second = generate_vault_key().unwrap();
    assert_ne!(first.expose_secret(), second.expose_secret());
}

#[test]
fn decrypt_returns_the_same_vault_key() {
    let vault_key = generate_vault_key().unwrap();
    let encrypted = encrypt_vault_key(&vault_key, &derived(1), AAD).unwrap();
    let decrypted = decrypt_vault_key(&encrypted, &derived(1), AAD).unwrap();
    assert_eq!(decrypted.expose_secret(), vault_key.expose_secret());
}

#[test]
fn ciphertext_does_not_contain_the_vault_key() {
    let vault_key = generate_vault_key().unwrap();
    let encrypted = encrypt_vault_key(&vault_key, &derived(1), AAD).unwrap();
    assert_ne!(
        &encrypted.ciphertext[..VAULT_KEY_LEN],
        vault_key.expose_secret()
    );
}

#[test]
fn wrong_derived_key_fails() {
    let vault_key = generate_vault_key().unwrap();
    let encrypted = encrypt_vault_key(&vault_key, &derived(1), AAD).unwrap();
    let result = decrypt_vault_key(&encrypted, &derived(2), AAD);
    assert_eq!(result.err(), Some(Error::WrongPassword));
}

#[test]
fn different_associated_data_fails() {
    let vault_key = generate_vault_key().unwrap();
    let encrypted = encrypt_vault_key(&vault_key, &derived(1), AAD).unwrap();
    let result = decrypt_vault_key(&encrypted, &derived(1), b"otra cabecera");
    assert_eq!(result.err(), Some(Error::WrongPassword));
}

#[test]
fn any_altered_byte_of_ciphertext_or_nonce_fails() {
    let vault_key = generate_vault_key().unwrap();
    let encrypted = encrypt_vault_key(&vault_key, &derived(1), AAD).unwrap();

    for i in 0..ENCRYPTED_KEY_LEN {
        let mut altered = encrypted.clone();
        altered.ciphertext[i] ^= 0x01;
        let result = decrypt_vault_key(&altered, &derived(1), AAD);
        assert_eq!(
            result.err(),
            Some(Error::WrongPassword),
            "texto cifrado, byte {i}"
        );
    }
    for i in 0..NONCE_LEN {
        let mut altered = encrypted.clone();
        altered.nonce[i] ^= 0x01;
        let result = decrypt_vault_key(&altered, &derived(1), AAD);
        assert_eq!(result.err(), Some(Error::WrongPassword), "nonce, byte {i}");
    }
}

#[test]
fn encrypting_twice_uses_a_new_nonce() {
    let vault_key = generate_vault_key().unwrap();
    let first = encrypt_vault_key(&vault_key, &derived(1), AAD).unwrap();
    let second = encrypt_vault_key(&vault_key, &derived(1), AAD).unwrap();
    assert_ne!(first.nonce, second.nonce);
    assert_ne!(first.ciphertext, second.ciphertext);
}

#[test]
fn seal_and_open_with_the_master_password() {
    let salt = [3u8; SALT_LEN];
    let password = SecretString::from("correcta");
    let vault_key = generate_vault_key().unwrap();
    let sealed = seal_vault_key(&vault_key, &password, &salt, &KdfParams::MIN, AAD).unwrap();

    let opened = open_vault_key(&sealed, &password, &salt, &KdfParams::MIN, AAD).unwrap();
    assert_eq!(opened.expose_secret(), vault_key.expose_secret());

    let wrong = SecretString::from("incorrecta");
    let result = open_vault_key(&sealed, &wrong, &salt, &KdfParams::MIN, AAD);
    assert_eq!(result.err(), Some(Error::WrongPassword));
}

#[test]
fn key_check_associated_data_layout() {
    let ad = key_check_associated_data(0x0102);
    assert_eq!(ad.len(), KEY_CHECK_AAD_LEN);
    assert_eq!(KEY_CHECK_AAD_LEN, 18);
    assert_eq!(&ad[..16], b"boveda-key-check");
    assert_eq!(&ad[16..], &[0x02, 0x01]);
}

#[test]
fn key_check_round_trip() {
    let key = generate_vault_key().unwrap();
    let check = seal_key_check(&key, 2).unwrap();
    assert_eq!(check.len(), KEY_CHECK_LEN);
    assert_eq!(KEY_CHECK_LEN, 40);
    assert_eq!(verify_key_check(&key, 2, &check), Ok(()));
}

#[test]
fn key_check_rejects_other_key_version_or_tamper() {
    let key = generate_vault_key().unwrap();
    let other = generate_vault_key().unwrap();
    let check = seal_key_check(&key, 2).unwrap();
    assert_eq!(
        verify_key_check(&other, 2, &check),
        Err(Error::WrongVaultKey)
    );
    assert_eq!(verify_key_check(&key, 3, &check), Err(Error::WrongVaultKey));
    for i in [0, 23, 24, 39] {
        let mut bad = check;
        bad[i] ^= 0x01;
        assert_eq!(verify_key_check(&key, 2, &bad), Err(Error::WrongVaultKey));
    }
}

#[test]
fn key_check_uses_fresh_nonce() {
    let key = generate_vault_key().unwrap();
    let a = seal_key_check(&key, 2).unwrap();
    let b = seal_key_check(&key, 2).unwrap();
    assert_ne!(a[..NONCE_LEN], b[..NONCE_LEN]);
}

#[test]
fn open_keeping_derived_matches_open() {
    let password = SecretString::from("contraseña de prueba");
    let salt = kdf::generate_salt().unwrap();
    let params = KdfParams::MIN;
    let vault_key = generate_vault_key().unwrap();
    let encrypted = seal_vault_key(&vault_key, &password, &salt, &params, AAD).unwrap();

    let (opened, derived) =
        open_vault_key_keeping_derived(&encrypted, &password, &salt, &params, AAD).unwrap();
    assert_eq!(opened.expose_secret(), vault_key.expose_secret());
    let expected = kdf::derive_key(&password, &salt, &params).unwrap();
    assert_eq!(*derived, *expected);

    let wrong = SecretString::from("otra");
    assert!(matches!(
        open_vault_key_keeping_derived(&encrypted, &wrong, &salt, &params, AAD),
        Err(Error::WrongPassword)
    ));
}
