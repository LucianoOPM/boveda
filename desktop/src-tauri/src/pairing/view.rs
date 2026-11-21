//! Lo que el webview recibe de la sesión de emparejamiento (#101).
//!
//! Es lo único que cruza el IPC: la imagen del QR (que contiene el código de un
//! solo uso, así que es sensible) y los datos para dibujar el estado. Nunca la
//! oferta decodificada, el código ni las llaves.

use std::fmt;

use serde::Serialize;

/// Por qué terminó una sesión sin emparejar. La interfaz muestra el motivo y
/// ofrece «Generar otro código».
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EndReason {
    /// Pasaron los 2 minutos del código.
    Expired,
    /// El usuario rechazó el emparejamiento.
    Rejected,
    /// El usuario no decidió a tiempo.
    ConfirmationTimeout,
    /// Cambió la red del servidor: el QR ya no sirve.
    NetworkChanged,
    /// El celular se desconectó o la conexión falló después de usar el código.
    PhoneDisconnected,
    /// El código agotó sus intentos con un código incorrecto.
    TooManyAttempts,
    /// El celular se emparejó pero no se pudo guardar en la lista.
    SaveFailed,
    /// No se pudo leer la identidad de la PC.
    IdentityUnavailable,
    /// No se pudo generar el código o el QR.
    CodeUnavailable,
}

/// El estado de la ventana. `qr` y `placeholder` son `data:` URI de SVG.
#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "phase",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum PairingView {
    /// Sin sesión: la ventana se cierra.
    Closed,
    /// No hay una red privada elegible: no hay QR.
    NoNetwork,
    /// Esperando al celular. `qr` contiene el código de un solo uso.
    Waiting { qr: String, remaining_ms: u64 },
    /// Un celular completó el apretón de manos y espera la decisión.
    Confirming {
        name: String,
        fingerprint: String,
        remaining_ms: u64,
    },
    /// El usuario aceptó: se cierra el protocolo y se guarda.
    Saving { name: String },
    /// Emparejado y guardado.
    Done { name: String },
    /// Terminó sin emparejar. `placeholder` es el marcador fijo, no un QR.
    Ended {
        reason: EndReason,
        placeholder: String,
    },
}

// Solo la fase: ni la imagen ni el nombre ni la huella salen por `Debug`.
impl fmt::Debug for PairingView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let phase = match self {
            Self::Closed => "Closed",
            Self::NoNetwork => "NoNetwork",
            Self::Waiting { .. } => "Waiting",
            Self::Confirming { .. } => "Confirming",
            Self::Saving { .. } => "Saving",
            Self::Done { .. } => "Done",
            Self::Ended { reason, .. } => return write!(f, "PairingView::Ended({reason:?})"),
        };
        write!(f, "PairingView::{phase}")
    }
}
