//! Llave de bóveda: se genera al azar, se cifra con la llave derivada de la
//! contraseña maestra y se descifra al desbloquear.
//!
//! La llave de bóveda es la que cifra las entradas; cambiar la contraseña
//! maestra solo vuelve a cifrar esta llave, sin tocar las entradas.

use chacha20poly1305::aead::{Generate, Tag};
use chacha20poly1305::{AeadInOut, KeyInit, XChaCha20Poly1305, XNonce};
use secrecy::{ExposeSecret, ExposeSecretMut, SecretBox, SecretString};

use super::kdf::{self, DerivedKey, KdfParams, SALT_LEN};
use crate::error::{Error, Result};

/// Longitud de la llave de bóveda: 256 bits.
pub(crate) const VAULT_KEY_LEN: usize = 32;

/// Longitud del nonce de XChaCha20-Poly1305.
pub(crate) const NONCE_LEN: usize = 24;

/// Longitud de la etiqueta de autenticación de Poly1305.
const TAG_LEN: usize = 16;

/// Longitud de la llave de bóveda cifrada: llave más etiqueta.
pub(crate) const ENCRYPTED_KEY_LEN: usize = VAULT_KEY_LEN + TAG_LEN;

/// Llave de bóveda en claro. Su `Debug` no muestra el valor y se borra de
/// memoria al soltarla.
pub(crate) type VaultKey = SecretBox<[u8; VAULT_KEY_LEN]>;

/// Llave de bóveda cifrada, tal como se guarda en la cabecera.
///
/// No deriva `Debug`: aunque está cifrada, es material de llave.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct EncryptedKey {
    /// Nonce aleatorio, nuevo en cada cifrado.
    pub(crate) nonce: [u8; NONCE_LEN],
    /// Llave cifrada seguida de la etiqueta de autenticación.
    pub(crate) ciphertext: [u8; ENCRYPTED_KEY_LEN],
}

/// Genera una llave de bóveda nueva con el generador del sistema.
///
/// Se llena directamente dentro del `SecretBox` para no dejar copias.
pub(crate) fn generate_vault_key() -> Result<VaultKey> {
    let mut vault_key = VaultKey::default();
    getrandom::fill(vault_key.expose_secret_mut()).map_err(|_| Error::Random)?;
    Ok(vault_key)
}

/// Crea el cifrador a partir de la llave derivada (se borra al soltarlo).
fn cipher(derived: &DerivedKey) -> Result<XChaCha20Poly1305> {
    // La longitud es fija (32 bytes), así que este error no ocurre en la práctica.
    XChaCha20Poly1305::new_from_slice(derived.as_ref()).map_err(|_| Error::WrongPassword)
}

/// Cifra la llave de bóveda con la llave derivada y `associated_data`.
///
/// Usa un nonce aleatorio nuevo en cada llamada.
pub(crate) fn encrypt_vault_key(
    vault_key: &VaultKey,
    derived: &DerivedKey,
    associated_data: &[u8],
) -> Result<EncryptedKey> {
    let nonce = XNonce::try_generate().map_err(|_| Error::Random)?;

    // Se cifra en el mismo búfer de tamaño fijo: al terminar ya solo contiene
    // texto cifrado y no queda ninguna copia de la llave en claro.
    let mut ciphertext = [0u8; ENCRYPTED_KEY_LEN];
    let (body, tag_out) = ciphertext.split_at_mut(VAULT_KEY_LEN);
    body.copy_from_slice(vault_key.expose_secret());
    let tag = cipher(derived)?
        .encrypt_inout_detached(&nonce, associated_data, body.into())
        .map_err(|_| Error::MalformedEncryptedKey)?;
    tag_out.copy_from_slice(&tag);

    Ok(EncryptedKey {
        nonce: nonce.into(),
        ciphertext,
    })
}

/// Descifra la llave de bóveda. Cualquier fallo de autenticación (llave
/// derivada incorrecta, dato autenticado distinto o byte alterado) devuelve
/// [`Error::WrongPassword`].
pub(crate) fn decrypt_vault_key(
    encrypted: &EncryptedKey,
    derived: &DerivedKey,
    associated_data: &[u8],
) -> Result<VaultKey> {
    let (body, tag) = encrypted.ciphertext.split_at(VAULT_KEY_LEN);
    let tag = Tag::<XChaCha20Poly1305>::try_from(tag).map_err(|_| Error::MalformedEncryptedKey)?;
    let nonce = XNonce::from(encrypted.nonce);

    // Se descifra directamente dentro del `SecretBox`; si la autenticación
    // falla, el búfer se borra al soltarlo.
    let mut vault_key = VaultKey::default();
    vault_key.expose_secret_mut().copy_from_slice(body);
    cipher(derived)?
        .decrypt_inout_detached(
            &nonce,
            associated_data,
            vault_key.expose_secret_mut().as_mut_slice().into(),
            &tag,
        )
        .map_err(|_| Error::WrongPassword)?;
    Ok(vault_key)
}

/// Deriva la llave de la contraseña maestra y cifra con ella la llave de bóveda.
pub(crate) fn seal_vault_key(
    vault_key: &VaultKey,
    password: &SecretString,
    salt: &[u8; SALT_LEN],
    params: &KdfParams,
    associated_data: &[u8],
) -> Result<EncryptedKey> {
    let derived = kdf::derive_key(password, salt, params)?;
    encrypt_vault_key(vault_key, &derived, associated_data)
}

/// Deriva la llave de la contraseña maestra y descifra con ella la llave de bóveda.
pub(crate) fn open_vault_key(
    encrypted: &EncryptedKey,
    password: &SecretString,
    salt: &[u8; SALT_LEN],
    params: &KdfParams,
    associated_data: &[u8],
) -> Result<VaultKey> {
    let derived = kdf::derive_key(password, salt, params)?;
    decrypt_vault_key(encrypted, &derived, associated_data)
}

#[cfg(test)]
mod tests {
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
}
