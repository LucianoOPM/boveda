//! Cabecera de la bóveda: versión del formato, parámetros de derivación, sal
//! y la llave de bóveda cifrada.
//!
//! Los campos (sin la llave cifrada) van como dato autenticado al cifrar la
//! llave de bóveda, así que alterar cualquiera hace fallar el desbloqueo.
//! Guardarla en SQLite corresponde al módulo de almacenamiento.

use secrecy::SecretString;

use crate::crypto::kdf::{self, KdfParams, SALT_LEN};
use crate::crypto::keys::{self, EncryptedKey, VaultKey};
use crate::error::{Error, Result};

/// Versión actual del formato de la bóveda.
pub(crate) const FORMAT_VERSION: u16 = 1;

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
}

impl VaultHeader {
    /// Crea la cabecera de una bóveda nueva: genera la sal y la llave de
    /// bóveda, y la cifra con la contraseña maestra.
    ///
    /// Devuelve también la llave de bóveda en claro para usarla de inmediato.
    pub(crate) fn create(password: &SecretString, params: KdfParams) -> Result<(Self, VaultKey)> {
        params.validate()?;
        let salt = kdf::generate_salt()?;
        let vault_key = keys::generate_vault_key()?;

        // La llave cifrada no forma parte del dato autenticado, así que se
        // puede calcular antes de tenerla.
        let associated_data =
            encode_associated_data(FORMAT_VERSION, KdfAlgorithm::Argon2idV13, &params, &salt);
        let encrypted_key =
            keys::seal_vault_key(&vault_key, password, &salt, &params, &associated_data)?;

        let header = Self {
            format_version: FORMAT_VERSION,
            algorithm: KdfAlgorithm::Argon2idV13,
            params,
            salt,
            encrypted_key,
        };
        Ok((header, vault_key))
    }

    /// Reconstruye una cabecera leída del almacenamiento y valida la versión
    /// del formato, el algoritmo y los parámetros.
    pub(crate) fn from_parts(
        format_version: u16,
        algorithm: u8,
        params: KdfParams,
        salt: [u8; SALT_LEN],
        encrypted_key: EncryptedKey,
    ) -> Result<Self> {
        if format_version != FORMAT_VERSION {
            return Err(Error::UnsupportedFormatVersion(format_version));
        }
        let algorithm = KdfAlgorithm::try_from(algorithm)?;
        params.validate()?;
        Ok(Self {
            format_version,
            algorithm,
            params,
            salt,
            encrypted_key,
        })
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
mod tests {
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
            0x01, 0x00, // versión 1
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
        let derived =
            kdf::derive_key(&password("correcta"), header.salt(), &header.params()).unwrap();
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
            )
            .err()
        };

        assert_eq!(
            rebuild(2, 1, KdfParams::MIN),
            Some(Error::UnsupportedFormatVersion(2))
        );
        assert_eq!(
            rebuild(1, 0, KdfParams::MIN),
            Some(Error::UnsupportedAlgorithm(0))
        );
        assert_eq!(
            rebuild(1, 2, KdfParams::MIN),
            Some(Error::UnsupportedAlgorithm(2))
        );
        let too_much_memory = KdfParams {
            memory_kib: KdfParams::MAX_MEMORY_KIB + 1,
            ..KdfParams::MIN
        };
        assert_eq!(
            rebuild(1, 1, too_much_memory),
            Some(Error::InvalidKdfParams)
        );
    }
}
