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
pub(crate) const TAG_LEN: usize = 16;

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
    let (vault_key, _derived) =
        open_vault_key_keeping_derived(encrypted, password, salt, params, associated_data)?;
    Ok(vault_key)
}

/// Como [`open_vault_key`], pero devuelve también la llave derivada, para
/// volver a cifrar la llave de bóveda sin un segundo Argon2id (la
/// migración de la versión 1 a la 2). La derivada se borra al soltarse.
pub(crate) fn open_vault_key_keeping_derived(
    encrypted: &EncryptedKey,
    password: &SecretString,
    salt: &[u8; SALT_LEN],
    params: &KdfParams,
    associated_data: &[u8],
) -> Result<(VaultKey, DerivedKey)> {
    let derived = kdf::derive_key(password, salt, params)?;
    let vault_key = decrypt_vault_key(encrypted, &derived, associated_data)?;
    Ok((vault_key, derived))
}

/// Longitud del verificador de la llave: nonce más etiqueta (texto vacío).
pub(crate) const KEY_CHECK_LEN: usize = NONCE_LEN + TAG_LEN;

/// Longitud del dato autenticado del verificador.
pub(crate) const KEY_CHECK_AAD_LEN: usize = 18;

/// Etiqueta fija al inicio del dato autenticado del verificador. No se
/// confunde con `boveda-v1`, la de las entradas.
const KEY_CHECK_LABEL: &[u8; 16] = b"boveda-key-check";

/// Dato autenticado del verificador: `boveda-key-check` (16 bytes) |
/// versión de formato (`u16` little-endian), sin separadores.
pub(crate) fn key_check_associated_data(format_version: u16) -> [u8; KEY_CHECK_AAD_LEN] {
    let mut ad = [0u8; KEY_CHECK_AAD_LEN];
    let (label, version) = ad.split_at_mut(KEY_CHECK_LABEL.len());
    label.copy_from_slice(KEY_CHECK_LABEL);
    version.copy_from_slice(&format_version.to_le_bytes());
    ad
}

/// Cifrador a partir de la llave de bóveda (se borra al soltarlo).
fn vault_key_cipher(key: &VaultKey) -> Result<XChaCha20Poly1305> {
    // La longitud es fija (32 bytes), así que este error no ocurre en la práctica.
    XChaCha20Poly1305::new_from_slice(key.expose_secret()).map_err(|_| Error::WrongVaultKey)
}

/// Crea el verificador de la llave de bóveda: cifra un texto vacío con un
/// nonce aleatorio nuevo y devuelve `nonce (24) | etiqueta (16)`.
///
/// XChaCha20-Poly1305 no compromete la llave: se puede construir un texto
/// que se autentique con dos llaves, pero eso exige conocer ambas. Quien
/// altera el archivo no conoce la llave real, así que no aplica.
pub(crate) fn seal_key_check(key: &VaultKey, format_version: u16) -> Result<[u8; KEY_CHECK_LEN]> {
    let nonce = XNonce::try_generate().map_err(|_| Error::Random)?;
    let ad = key_check_associated_data(format_version);
    let mut body: [u8; 0] = [];
    let tag = vault_key_cipher(key)?
        .encrypt_inout_detached(&nonce, &ad, body.as_mut_slice().into())
        .map_err(|_| Error::Random)?;

    let mut check = [0u8; KEY_CHECK_LEN];
    let (nonce_out, tag_out) = check.split_at_mut(NONCE_LEN);
    nonce_out.copy_from_slice(&nonce);
    tag_out.copy_from_slice(&tag);
    Ok(check)
}

/// Comprueba que `key_check` se autentica con `key` y `format_version`.
/// Cualquier fallo da [`Error::WrongVaultKey`].
pub(crate) fn verify_key_check(
    key: &VaultKey,
    format_version: u16,
    key_check: &[u8; KEY_CHECK_LEN],
) -> Result<()> {
    let (nonce, tag) = key_check.split_at(NONCE_LEN);
    let nonce = XNonce::try_from(nonce).map_err(|_| Error::WrongVaultKey)?;
    let tag = Tag::<XChaCha20Poly1305>::try_from(tag).map_err(|_| Error::WrongVaultKey)?;
    let ad = key_check_associated_data(format_version);
    let mut body: [u8; 0] = [];
    vault_key_cipher(key)?
        .decrypt_inout_detached(&nonce, &ad, body.as_mut_slice().into(), &tag)
        .map_err(|_| Error::WrongVaultKey)
}

#[cfg(test)]
mod tests;
