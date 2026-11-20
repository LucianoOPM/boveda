//! Emparejamiento Noise entre el celular y la PC (#95).
//!
//! Máquinas de estado por tipos, sin sockets: cada paso consume el estado
//! anterior y devuelve el siguiente. Completar el apretón de manos **no
//! autoriza nada**: el celular solo obtiene un [`PairedPc`] con una
//! aceptación autenticada de la PC, y la PC solo obtiene un [`PairedDevice`]
//! cuando el usuario acepta. El núcleo nunca persiste la relación de confianza.
//!
//! # Borrado de memoria
//!
//! `snow` 0.10.0 no depende de `zeroize` y no borra ningún secreto: quedan
//! en su memoria la llave privada estática y la efímera, el código de un solo
//! uso (copia en `psks`) y lo que se deriva de él, el estado simétrico y las
//! llaves de cifrado de los dos sentidos. Se pasan como `&[u8]` desde su
//! `SecretBox` y los estados se sueltan en cuanto dejan de servir, lo que
//! reduce copias y su vida, pero **no es borrado seguro**. Es una excepción
//! aceptada (ver `ARCHITECTURE.md`).

use std::net::SocketAddr;
use std::time::Duration;

use snow::params::NoiseParams;

use super::error::ProtocolError;
use super::identity::PublicKey;
use super::message::DeviceName;
use crate::vault::NewPairedPc;

/// Patrón Noise del emparejamiento: el celular conoce la llave de la PC (QR)
/// y el código de un solo uso entra como PSK en la posición 0.
pub const PAIRING_PATTERN: &str = "Noise_XKpsk0_25519_ChaChaPoly_BLAKE2s";

/// Prólogo de Noise: separa este intercambio de cualquier otro.
pub const PAIRING_PROLOGUE: &[u8] = b"boveda/emparejamiento/v1";

/// Vida del código de un solo uso, desde que se crea la sesión.
pub const PAIRING_CODE_TTL: Duration = Duration::from_secs(120);

/// Intentos fallidos que aguanta un código antes de invalidarse.
pub const MAX_FAILED_ATTEMPTS: u8 = 3;

/// Longitud de la primera trama Noise: `e` (32) y la etiqueta de 16 bytes.
pub const FIRST_FRAME_LEN: usize = 48;

/// Longitud máxima de una trama del emparejamiento, sin su prefijo de largo.
pub const MAX_FRAME_LEN: usize = 512;

/// Parámetros de Noise del emparejamiento. Un patrón que no se pueda leer es
/// `Handshake`: no ocurre con la constante, pero la API no hace panic.
// Se usa desde la construcción de los extremos (Task 4).
#[cfg_attr(not(test), allow(dead_code))]
fn noise_params() -> Result<NoiseParams, PairingError> {
    PAIRING_PATTERN.parse().map_err(|_| PairingError::Handshake)
}

/// Una trama del emparejamiento, sin su prefijo de largo.
pub type Frame = Vec<u8>;

/// Por qué la PC rechazó una conexión. **Solo para la app local** (por
/// ejemplo, ofrecer "Generar otro código"): nunca se envía ni cambia lo que
/// ve quien intenta conectarse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// La primera trama no se autentica con el código.
    WrongCode,
    /// La primera trama no mide 48 bytes.
    Malformed,
    /// El código caducó.
    Expired,
    /// El código ya se usó.
    AlreadyUsed,
    /// El código agotó sus intentos fallidos.
    TooManyAttempts,
    /// La app canceló el código.
    Cancelled,
}

/// Error del emparejamiento. Ninguna variante lleva datos del mensaje, secretos
/// ni texto de `snow`.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PairingError {
    /// La PC rechazó la conexión (solo lo ve la app de la PC).
    #[error("conexión rechazada")]
    Refused(Refusal),
    /// La respuesta de la PC no se autentica: no es la PC del QR.
    #[error("la PC no se pudo autenticar")]
    PeerNotAuthentic,
    /// La PC rechazó el emparejamiento.
    #[error("la PC rechazó el emparejamiento")]
    Rejected,
    /// No hubo una decisión a tiempo.
    #[error("se agotó el tiempo de la confirmación")]
    ConfirmationTimeout,
    /// Se agotó el tiempo de un paso o del apretón de manos.
    #[error("se agotó el tiempo de espera")]
    Timeout,
    /// La operación se canceló.
    #[error("operación cancelada")]
    Cancelled,
    /// La otra parte cerró la conexión.
    #[error("la conexión se cerró")]
    Closed,
    /// Error de entrada o salida; solo se conserva su clase.
    #[error("error de entrada o salida")]
    Io(std::io::ErrorKind),
    /// Una trama pasa del máximo.
    #[error("la trama es demasiado grande")]
    FrameTooLarge,
    /// Falló el apretón de manos Noise.
    #[error("falló el apretón de manos")]
    Handshake,
    /// Un mensaje no cumple el protocolo.
    #[error("mensaje no válido: {0}")]
    Protocol(ProtocolError),
}

impl From<ProtocolError> for PairingError {
    fn from(error: ProtocolError) -> Self {
        Self::Protocol(error)
    }
}

/// Lo que obtiene el celular tras una aceptación autenticada de la PC.
///
/// La llave y la dirección salen del QR y ya las autenticó Noise. La PC no
/// envía su nombre en esta versión: la #103 decide cómo lo obtiene el celular.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PairedPc {
    public_key: PublicKey,
    address: SocketAddr,
}

impl PairedPc {
    // Lo construye `AwaitingReply::read_reply` (Task 5).
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn new(public_key: PublicKey, address: SocketAddr) -> Self {
        Self {
            public_key,
            address,
        }
    }

    /// Llave pública de la PC.
    pub fn public_key(&self) -> PublicKey {
        self.public_key
    }

    /// Dirección donde estaba la PC.
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    /// Nombre provisional derivado de la llave, `PC A1B2-C3D4-E5F6-0718`:
    /// cumple el rango de la bóveda sin inventar información.
    pub fn provisional_name(&self) -> String {
        format!("PC {}", self.public_key.fingerprint())
    }

    /// Datos para guardar la PC en la bóveda. Sin `name`, usa
    /// [`provisional_name`](Self::provisional_name).
    pub fn into_new_paired_pc(self, name: Option<String>) -> NewPairedPc {
        NewPairedPc {
            public_key: *self.public_key.as_bytes(),
            name: name.unwrap_or_else(|| self.provisional_name()),
            last_ip: Some(self.address.ip().to_canonical()),
        }
    }
}

/// Lo que obtiene la PC cuando el usuario acepta: la llave del celular y su
/// nombre (solo informativo). La identidad es la llave. Su `Debug` solo
/// muestra la huella.
#[derive(Clone, PartialEq, Eq)]
pub struct PairedDevice {
    public_key: PublicKey,
    name: DeviceName,
}

impl PairedDevice {
    // Lo construye `PendingPairing::accept` (Task 5).
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn new(public_key: PublicKey, name: DeviceName) -> Self {
        Self { public_key, name }
    }

    /// Llave pública del celular.
    pub fn public_key(&self) -> PublicKey {
        self.public_key
    }

    /// Nombre que el celular dio.
    pub fn name(&self) -> &DeviceName {
        &self.name
    }
}

impl std::fmt::Debug for PairedDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PairedDevice({})", self.public_key.fingerprint())
    }
}

#[cfg(test)]
mod tests;
