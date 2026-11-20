//! Envío de una contraseña a la PC con Noise IK (#96).
//!
//! Máquinas de estado por tipos, sin sockets, y los conductores síncronos que
//! las unen con [`TimedStream`](super::TimedStream). El celular conoce la llave
//! de la PC y la PC aprende la del celular en la primera trama; la PC solo
//! contesta si esa llave está emparejada. El `SendMessage` viaja únicamente
//! después de completar el apretón de manos.
//!
//! # Plazos
//!
//! Hay dos plazos independientes. Los [`SEND_TTL`] (30 s) los cuenta la PC con
//! `Instant` desde que acepta la conexión y limitan la recepción **completa y
//! autenticada** del mensaje: no demuestran cuándo se generó la contraseña. La
//! espera del `Ack` por el celular ([`SendTimeouts::ack_wait`]) es aparte y no
//! amplía esa ventana. Los 30 s del portapapeles empiezan cuando Windows copia
//! la contraseña y son de la #98.

use std::io::ErrorKind;
use std::time::Duration;

use super::error::ProtocolError;
use super::pairing::PairingError;

/// Patrón Noise del envío: el celular conoce la llave de la PC y la PC aprende
/// la del celular en la primera trama.
pub const SEND_PATTERN: &str = "Noise_IK_25519_ChaChaPoly_BLAKE2s";

/// Prólogo de Noise: separa este intercambio de cualquier otro.
pub const SEND_PROLOGUE: &[u8] = b"boveda/envio/v1";

/// Tiempo que tiene la PC, desde que acepta la conexión, para tener el mensaje
/// completo y autenticado.
pub const SEND_TTL: Duration = Duration::from_secs(30);

/// Longitud de la primera trama IK: `e` (32), `s` cifrada (48) y la etiqueta de
/// la carga vacía (16).
pub const SEND_FIRST_FRAME_LEN: usize = 96;

/// Longitud de la segunda trama IK: `e` (32) y la etiqueta de la carga vacía
/// (16).
pub const SEND_SECOND_FRAME_LEN: usize = 48;

/// Error del envío. Ninguna variante lleva datos del mensaje, secretos ni texto
/// de `snow`.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SendError {
    /// La llave del celular no está emparejada (**solo lo ve la app de la
    /// PC**: por la conexión se ve igual que un fallo).
    #[error("el dispositivo no está emparejado")]
    NotPaired,
    /// Pasaron los 30 s antes de tener el mensaje completo y autenticado.
    #[error("el mensaje caducó")]
    Expired,
    /// La respuesta de la PC no se autentica: no es la PC conocida.
    #[error("la PC no se pudo autenticar")]
    PeerNotAuthentic,
    /// Se agotó el tiempo de un paso.
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
    Io(ErrorKind),
    /// Una trama pasa del máximo.
    #[error("la trama es demasiado grande")]
    FrameTooLarge,
    /// Falló el apretón de manos Noise o el descifrado.
    #[error("falló el apretón de manos")]
    Handshake,
    /// Un mensaje no cumple el protocolo.
    #[error("mensaje no válido: {0}")]
    Protocol(ProtocolError),
}

impl From<ProtocolError> for SendError {
    fn from(error: ProtocolError) -> Self {
        Self::Protocol(error)
    }
}

impl From<PairingError> for SendError {
    /// Los errores de tramas y plazos pasan igual; lo propio del
    /// emparejamiento no tiene sentido aquí y cuenta como `Handshake`.
    fn from(error: PairingError) -> Self {
        match error {
            PairingError::Timeout => Self::Timeout,
            PairingError::Cancelled => Self::Cancelled,
            PairingError::Closed => Self::Closed,
            PairingError::Io(kind) => Self::Io(kind),
            PairingError::FrameTooLarge => Self::FrameTooLarge,
            PairingError::PeerNotAuthentic => Self::PeerNotAuthentic,
            PairingError::Protocol(error) => Self::Protocol(error),
            _ => Self::Handshake,
        }
    }
}

/// Plazos del envío.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SendTimeouts {
    /// Máximo por trama leída o escrita.
    pub step: Duration,
    /// Máximo, contado por la PC desde que acepta la conexión, hasta tener el
    /// mensaje completo y autenticado.
    pub total: Duration,
    /// Tiempo que espera el celular el `Ack`. Es independiente de `total` y no
    /// lo amplía.
    pub ack_wait: Duration,
}

impl Default for SendTimeouts {
    fn default() -> Self {
        Self {
            step: Duration::from_secs(10),
            total: SEND_TTL,
            ack_wait: Duration::from_secs(10),
        }
    }
}

#[cfg(test)]
mod tests;
