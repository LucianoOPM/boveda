//! Cabecera de la bóveda: versión del formato, parámetros de derivación, sal
//! y la llave de bóveda cifrada.
//!
//! Los campos (sin la llave cifrada) van como dato autenticado al cifrar la
//! llave de bóveda, así que alterar cualquiera hace fallar el desbloqueo.
//! Guardarla en SQLite corresponde al módulo de almacenamiento.
//!
//! Desde la versión 2 incluye `key_check`, el verificador de la llave de
//! bóveda (ver `crypto::keys::seal_key_check`). Una cabecera de la versión 1
//! leída del disco no lo tiene.

use secrecy::SecretString;

use crate::crypto::kdf::{self, KdfParams, SALT_LEN};
use crate::crypto::keys::{self, EncryptedKey, KEY_CHECK_LEN, VaultKey};
use crate::error::{Error, Result, StorageErrorKind};

/// Versión actual del formato de la bóveda.
pub(crate) const FORMAT_VERSION: u16 = 2;

/// Versión 1 del formato: sin verificador de la llave. Solo se lee, para
/// migrarla.
pub(crate) const FORMAT_VERSION_V1: u16 = 1;

/// Longitud del dato autenticado:
/// versión (2) | algoritmo (1) | memoria (4) | iteraciones (4) | paralelismo (4) | sal (16).
pub(crate) const ASSOCIATED_DATA_LEN: usize = 2 + 1 + 4 + 4 + 4 + SALT_LEN;

/// Algoritmo de derivación de la llave, guardado como `u8` en la cabecera.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum KdfAlgorithm {
    /// Argon2id versión 0x13.
    Argon2idV13 = 1,
}

impl TryFrom<u8> for KdfAlgorithm {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::Argon2idV13),
            other => Err(Error::UnsupportedAlgorithm(other)),
        }
    }
}

/// Cabecera de la bóveda.
///
/// No deriva `Debug`: contiene la llave de bóveda cifrada.
#[derive(Clone)]
pub(crate) struct VaultHeader {
    format_version: u16,
    algorithm: KdfAlgorithm,
    params: KdfParams,
    salt: [u8; SALT_LEN],
    encrypted_key: EncryptedKey,
    /// Verificador de la llave de bóveda. `None` solo en una versión 1.
    key_check: Option<[u8; KEY_CHECK_LEN]>,
}

impl VaultHeader {
    /// Crea la cabecera de una bóveda nueva: genera la sal y la llave de
    /// bóveda, y la cifra con la contraseña maestra.
    ///
    /// Devuelve también la llave de bóveda en claro para usarla de inmediato.
    pub(crate) fn create(password: &SecretString, params: KdfParams) -> Result<(Self, VaultKey)> {
        params.validate()?;
        let vault_key = keys::generate_vault_key()?;
        let header = Self::rekey(&vault_key, password, params)?;
        Ok((header, vault_key))
    }

    /// Crea una cabecera para una llave de bóveda existente, con sal y nonce
    /// nuevos. La usa el cambio de contraseña maestra: las entradas no se
    /// vuelven a cifrar.
    pub(crate) fn rekey(
        vault_key: &VaultKey,
        password: &SecretString,
        params: KdfParams,
    ) -> Result<Self> {
        params.validate()?;
        let salt = kdf::generate_salt()?;

        // La llave cifrada no forma parte del dato autenticado, así que se
        // puede calcular antes de tenerla.
        let associated_data =
            encode_associated_data(FORMAT_VERSION, KdfAlgorithm::Argon2idV13, &params, &salt);
        let encrypted_key =
            keys::seal_vault_key(vault_key, password, &salt, &params, &associated_data)?;
        let key_check = keys::seal_key_check(vault_key, FORMAT_VERSION)?;

        Ok(Self {
            format_version: FORMAT_VERSION,
            algorithm: KdfAlgorithm::Argon2idV13,
            params,
            salt,
            encrypted_key,
            key_check: Some(key_check),
        })
    }

    /// Reconstruye una cabecera leída del almacenamiento y valida la versión
    /// del formato, el algoritmo y los parámetros.
    ///
    /// Acepta las versiones 1 (sin `key_check`) y 2 (con él); otra versión
    /// da [`Error::UnsupportedFormatVersion`]. Una versión conocida con la
    /// presencia equivocada de `key_check` da `Storage(Corrupt)`.
    pub(crate) fn from_parts(
        format_version: u16,
        algorithm: u8,
        params: KdfParams,
        salt: [u8; SALT_LEN],
        encrypted_key: EncryptedKey,
        key_check: Option<[u8; KEY_CHECK_LEN]>,
    ) -> Result<Self> {
        match (format_version, key_check.is_some()) {
            (FORMAT_VERSION_V1, false) | (FORMAT_VERSION, true) => {}
            (FORMAT_VERSION_V1 | FORMAT_VERSION, _) => {
                return Err(Error::Storage(StorageErrorKind::Corrupt));
            }
            (other, _) => return Err(Error::UnsupportedFormatVersion(other)),
        }
        let algorithm = KdfAlgorithm::try_from(algorithm)?;
        params.validate()?;
        Ok(Self {
            format_version,
            algorithm,
            params,
            salt,
            encrypted_key,
            key_check,
        })
    }

    /// Comprueba que `key` es la llave de esta bóveda con `key_check`.
    ///
    /// Una cabecera de la versión 1 (sin verificador), otra llave o un
    /// verificador dañado dan [`Error::WrongVaultKey`].
    pub(crate) fn verify_key(&self, key: &VaultKey) -> Result<()> {
        let key_check = self.key_check.as_ref().ok_or(Error::WrongVaultKey)?;
        keys::verify_key_check(key, self.format_version, key_check)
    }

    pub(crate) fn key_check(&self) -> Option<&[u8; KEY_CHECK_LEN]> {
        self.key_check.as_ref()
    }

    /// Descifra la llave de bóveda con la contraseña maestra.
    ///
    /// Una contraseña incorrecta o una cabecera alterada devuelven
    /// [`Error::WrongPassword`].
    pub(crate) fn unlock(&self, password: &SecretString) -> Result<VaultKey> {
        keys::open_vault_key(
            &self.encrypted_key,
            password,
            &self.salt,
            &self.params,
            &self.associated_data(),
        )
    }

    /// Descifra la llave de bóveda con la contraseña maestra y, si la
    /// cabecera es de la versión 1, arma también la de la versión 2.
    ///
    /// Argon2id corre una sola vez: la llave derivada que descifró la llave
    /// de bóveda la vuelve a cifrar con el dato autenticado de la versión 2,
    /// con la misma sal y los mismos parámetros y un nonce nuevo, y se crea
    /// `key_check`. En una versión 2 devuelve `(llave, None)`. Los errores
    /// son los de [`VaultHeader::unlock`].
    pub(crate) fn unlock_upgrading(
        &self,
        password: &SecretString,
    ) -> Result<(VaultKey, Option<VaultHeader>)> {
        let (vault_key, derived) = keys::open_vault_key_keeping_derived(
            &self.encrypted_key,
            password,
            &self.salt,
            &self.params,
            &self.associated_data(),
        )?;
        if self.format_version == FORMAT_VERSION {
            return Ok((vault_key, None));
        }

        let associated_data =
            encode_associated_data(FORMAT_VERSION, self.algorithm, &self.params, &self.salt);
        let encrypted_key = keys::encrypt_vault_key(&vault_key, &derived, &associated_data)?;
        let key_check = keys::seal_key_check(&vault_key, FORMAT_VERSION)?;
        let upgraded = Self {
            format_version: FORMAT_VERSION,
            algorithm: self.algorithm,
            params: self.params,
            salt: self.salt,
            encrypted_key,
            key_check: Some(key_check),
        };
        Ok((vault_key, Some(upgraded)))
    }

    /// Codifica los campos de la cabecera como dato autenticado: binario de
    /// ancho fijo y little-endian.
    pub(crate) fn associated_data(&self) -> [u8; ASSOCIATED_DATA_LEN] {
        encode_associated_data(
            self.format_version,
            self.algorithm,
            &self.params,
            &self.salt,
        )
    }

    pub(crate) fn format_version(&self) -> u16 {
        self.format_version
    }

    pub(crate) fn algorithm(&self) -> KdfAlgorithm {
        self.algorithm
    }

    pub(crate) fn params(&self) -> KdfParams {
        self.params
    }

    pub(crate) fn salt(&self) -> &[u8; SALT_LEN] {
        &self.salt
    }

    pub(crate) fn encrypted_key(&self) -> &EncryptedKey {
        &self.encrypted_key
    }
}

/// `versión (u16) | algoritmo (u8) | memoria en KiB (u32) | iteraciones (u32)
/// | paralelismo (u32) | sal (16 bytes)`, little-endian y sin separadores.
fn encode_associated_data(
    format_version: u16,
    algorithm: KdfAlgorithm,
    params: &KdfParams,
    salt: &[u8; SALT_LEN],
) -> [u8; ASSOCIATED_DATA_LEN] {
    let mut out = [0u8; ASSOCIATED_DATA_LEN];
    out[0..2].copy_from_slice(&format_version.to_le_bytes());
    out[2] = algorithm as u8;
    out[3..7].copy_from_slice(&params.memory_kib.to_le_bytes());
    out[7..11].copy_from_slice(&params.iterations.to_le_bytes());
    out[11..15].copy_from_slice(&params.parallelism.to_le_bytes());
    out[15..].copy_from_slice(salt);
    out
}

#[cfg(test)]
mod tests;
