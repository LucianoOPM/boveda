//! Identidades X25519 del protocolo (#94): la llave pública con su huella
//! corta y el par de llaves.
//!
//! La llave privada son **32 bytes tal cual**, sin recortar ni transformar:
//! cualquier secuencia de 32 bytes sirve, porque el recorte (clamping) lo hace
//! la multiplicación al derivar la llave pública. Así la identidad que la
//! bóveda guarda (`vault::Identity`, #93) y la que usa el protocolo son los
//! mismos bytes.
//!
//! # Borrado de memoria
//!
//! `StaticSecret::from([u8; 32])` recibe la llave por valor, y un arreglo de 32
//! bytes es `Copy`: queda una copia en la pila de quien llama que
//! `x25519-dalek` no borra, y `curve25519-dalek` deja el escalar recortado en
//! registros y pila durante la multiplicación. Es una excepción aceptada y
//! documentada (ver "Borrado de memoria" en `ARCHITECTURE.md`): solo se cierra
//! con una primitiva propia, que `core/CLAUDE.md` prohíbe. Se mitiga derivando
//! justo después de leer la llave del `SecretBox`, sin retenerla.

use std::fmt;

use secrecy::{ExposeSecret, ExposeSecretMut, SecretBox};
use serde::{Deserialize, Serialize};
use x25519_dalek::{PublicKey as DalekPublicKey, StaticSecret};
use zeroize::ZeroizeOnDrop;

use super::error::{ProtocolError, Result};
use crate::vault::PUBLIC_KEY_LEN;

/// Longitud de la huella, en bytes.
const FINGERPRINT_LEN: usize = 8;

/// Llave pública X25519. No es un secreto. En el cable son sus 32 bytes, sin
/// longitud.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PublicKey([u8; PUBLIC_KEY_LEN]);

impl PublicKey {
    /// Construye la llave con esos 32 bytes. Cualquier valor de 32 bytes es
    /// una llave pública válida a este nivel; que sirva lo comprueba el
    /// apretón de manos.
    pub fn from_bytes(bytes: [u8; PUBLIC_KEY_LEN]) -> Self {
        Self(bytes)
    }

    /// Los 32 bytes de la llave.
    pub fn as_bytes(&self) -> &[u8; PUBLIC_KEY_LEN] {
        &self.0
    }

    /// Deriva la llave pública de una llave privada de 32 bytes, sin
    /// recortarla ni cambiarla.
    pub fn from_private(private: &SecretBox<[u8; PUBLIC_KEY_LEN]>) -> Self {
        // Se copia a la pila justo antes de derivar y no se retiene: ver la
        // excepción de borrado en la documentación del módulo.
        let secret = StaticSecret::from(*private.expose_secret());
        Self(DalekPublicKey::from(&secret).to_bytes())
    }

    /// Huella corta para mostrarla al usuario.
    pub fn fingerprint(&self) -> Fingerprint {
        let mut bytes = [0u8; FINGERPRINT_LEN];
        bytes.copy_from_slice(&self.0[..FINGERPRINT_LEN]);
        Fingerprint(bytes)
    }
}

impl fmt::Debug for PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PublicKey({})", self.fingerprint())
    }
}

/// Huella corta de una llave pública: sus primeros 8 bytes, en hexadecimal
/// con mayúsculas y agrupados de a dos bytes (`A1B2-C3D4-E5F6-0718`).
///
/// Es un identificador visual corto para que una persona compare dos
/// pantallas. **No autentica nada y no sustituye a Noise**: con 64 bits, un
/// atacante podría fabricar una llave con la misma huella, así que nunca se
/// usa para decidir en qué confiar.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Fingerprint([u8; FINGERPRINT_LEN]);

impl Fingerprint {
    /// Los 8 bytes de la huella.
    pub fn as_bytes(&self) -> &[u8; FINGERPRINT_LEN] {
        &self.0
    }
}

impl fmt::Display for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let b = &self.0;
        write!(
            f,
            "{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}",
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]
        )
    }
}

/// Par de llaves X25519 de un dispositivo.
///
/// La llave privada vive en un `SecretBox` (se borra al soltarse) y no se
/// puede clonar. Su `Debug` muestra solo la huella de la llave pública.
pub struct KeyPair {
    private: SecretBox<[u8; PUBLIC_KEY_LEN]>,
    public: PublicKey,
}

// La garantía la da `SecretBox`, que se borra al soltarse. Este marcador solo
// la hace visible en el tipo.
impl ZeroizeOnDrop for KeyPair {}

impl KeyPair {
    /// Genera un par nuevo: 32 bytes del generador del sistema, escritos
    /// directamente en el `SecretBox`. Un fallo del generador da
    /// [`ProtocolError::Random`].
    pub fn generate() -> Result<Self> {
        let mut private = SecretBox::<[u8; PUBLIC_KEY_LEN]>::default();
        getrandom::fill(private.expose_secret_mut()).map_err(|_| ProtocolError::Random)?;
        Ok(Self::from_private(private))
    }

    /// Construye el par con esa llave privada, que se conserva tal cual.
    pub fn from_private(private: SecretBox<[u8; PUBLIC_KEY_LEN]>) -> Self {
        let public = PublicKey::from_private(&private);
        Self { private, public }
    }

    /// Llave privada: los mismos 32 bytes con los que se construyó.
    pub fn private(&self) -> &SecretBox<[u8; PUBLIC_KEY_LEN]> {
        &self.private
    }

    /// Llave pública derivada de la privada.
    pub fn public(&self) -> PublicKey {
        self.public
    }
}

impl fmt::Debug for KeyPair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KeyPair")
            .field(
                "fingerprint",
                &format_args!("{}", self.public.fingerprint()),
            )
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests;
