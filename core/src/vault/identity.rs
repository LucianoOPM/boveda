//! Identidad del celular: la llave privada X25519 que la bóveda guarda cifrada
//! con la llave de bóveda (formato v3, #93).
//!
//! El texto en claro son los **32 bytes de la llave privada, sin serializar**.
//! Cualquier secuencia de 32 bytes aleatorios sirve como llave privada X25519,
//! porque se recorta al usarla: quien la lea (la #94) no debe validarla ni
//! transformarla. La llave pública no se guarda: se deriva de la privada.
//!
//! Se cifra con el mismo XChaCha20-Poly1305 que las partes de una entrada,
//! pero con su propio dato autenticado, de 17 bytes y sin separadores:
//!
//! | Offset | Campo                 | Tamaño   | Valor                    |
//! | ------ | --------------------- | -------- | ------------------------ |
//! | 0–14   | Etiqueta              | 15 bytes | ASCII `boveda-identity`  |
//! | 15–16  | Versión de estructura | 2 bytes  | `u16` little-endian      |
//!
//! La etiqueta no se confunde con `boveda-v1` (entradas), `boveda-pc` (PCs
//! emparejadas) ni `boveda-key-check`. Guardada: `nonce (24) | llave (32) |
//! etiqueta (16)`, 72 bytes.

use std::fmt;

use secrecy::{ExposeSecret, ExposeSecretMut, SecretBox};
use zeroize::ZeroizeOnDrop;

use crate::crypto::aead::{self, EncryptedPart};
use crate::crypto::keys::VaultKey;
use crate::error::{Error, Result};

/// Longitud de la llave privada.
pub const IDENTITY_KEY_LEN: usize = 32;

/// Versión de estructura con la que se escribe la identidad. Se guarda en
/// claro junto a la fila y forma parte del dato autenticado.
pub(crate) const IDENTITY_SCHEMA_VERSION: u16 = 1;

/// Etiqueta fija al inicio del dato autenticado.
const AAD_LABEL: &[u8; 15] = b"boveda-identity";

/// Longitud del dato autenticado.
const AAD_LEN: usize = 17;

/// Identidad del celular. No deriva `Debug` con el valor: su `Debug` no
/// muestra la llave. La llave se borra de memoria al soltarla.
pub struct Identity {
    private_key: SecretBox<[u8; IDENTITY_KEY_LEN]>,
}

// La garantía la da `SecretBox`, que se borra al soltarse. Este marcador solo
// la hace visible en el tipo.
impl ZeroizeOnDrop for Identity {}

impl Identity {
    /// Llave privada X25519: 32 bytes sin transformar.
    pub fn private_key(&self) -> &SecretBox<[u8; IDENTITY_KEY_LEN]> {
        &self.private_key
    }

    /// Construye la identidad con esos 32 bytes, tal cual. Solo para pruebas
    /// y para generar la bóveda de ejemplo: `open` y `generate` escriben
    /// directamente en el `SecretBox`, sin pasar por un arreglo en la pila.
    #[cfg(test)]
    pub(crate) fn from_bytes(bytes: [u8; IDENTITY_KEY_LEN]) -> Self {
        let bytes = zeroize::Zeroizing::new(bytes);
        let mut private_key = SecretBox::<[u8; IDENTITY_KEY_LEN]>::default();
        private_key.expose_secret_mut().copy_from_slice(&*bytes);
        Self { private_key }
    }
}

impl fmt::Debug for Identity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Identity").finish_non_exhaustive()
    }
}

/// Dato autenticado de la identidad para esa versión de estructura.
fn associated_data(version: u16) -> [u8; AAD_LEN] {
    let mut ad = [0u8; AAD_LEN];
    let (label, rest) = ad.split_at_mut(AAD_LABEL.len());
    label.copy_from_slice(AAD_LABEL);
    rest.copy_from_slice(&version.to_le_bytes());
    ad
}

/// Genera una identidad nueva: 32 bytes del generador del sistema, escritos
/// directamente en el `SecretBox`. Un fallo del generador da [`Error::Random`].
pub(crate) fn generate() -> Result<Identity> {
    let mut private_key = SecretBox::<[u8; IDENTITY_KEY_LEN]>::default();
    getrandom::fill(private_key.expose_secret_mut()).map_err(|_| Error::Random)?;
    Ok(Identity { private_key })
}

/// Cifra la identidad con la llave de bóveda y la versión de estructura
/// actual.
pub(crate) fn seal(key: &VaultKey, identity: &Identity) -> Result<EncryptedPart> {
    aead::seal_with_ad(
        key,
        &associated_data(IDENTITY_SCHEMA_VERSION),
        identity.private_key.expose_secret(),
    )
}

/// Descifra una identidad guardada con `version`.
///
/// Una versión de estructura desconocida, una pieza que no se autentica
/// (otra llave, otra versión o un byte alterado) o un texto que no mide 32
/// bytes dan [`Error::CorruptIdentity`]. Los 32 bytes se devuelven sin
/// validarlos ni transformarlos.
pub(crate) fn open(key: &VaultKey, version: u16, sealed: &EncryptedPart) -> Result<Identity> {
    if version != IDENTITY_SCHEMA_VERSION {
        return Err(Error::CorruptIdentity);
    }
    let plain = aead::open_with_ad(key, &associated_data(version), sealed)
        .map_err(|_| Error::CorruptIdentity)?;
    if plain.len() != IDENTITY_KEY_LEN {
        return Err(Error::CorruptIdentity);
    }
    // Se copia directamente dentro del `SecretBox`: un `[u8; 32]` intermedio
    // (es `Copy`) dejaría una copia de la llave en la pila sin borrar.
    let mut private_key = SecretBox::<[u8; IDENTITY_KEY_LEN]>::default();
    private_key.expose_secret_mut().copy_from_slice(&plain);
    Ok(Identity { private_key })
}

#[cfg(test)]
mod tests;
