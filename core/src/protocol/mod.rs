//! Protocolo entre el celular y la PC: identidades X25519 y mensajes (#94).
//!
//! Solo tipos y codificación binaria; sin red. El emparejamiento y el
//! transporte con Noise llegan en las issues siguientes de la etapa 3.

mod error;
mod identity;
mod message;

pub use error::{InvalidField, ProtocolError, Result};
pub use identity::{Fingerprint, KeyPair, PublicKey};
pub use message::{
    Ack, AckDecision, AckOutcome, DeviceName, MAX_DEVICE_NAME_BYTES, MAX_MESSAGE_LEN,
    MAX_PAIRING_OFFER_LEN, MAX_PASSWORD_BYTES, PAIRING_CODE_LEN, PROTOCOL_VERSION, PairingCode,
    PairingOffer, PairingReply, PairingRequest, SEND_TTL_SECS, SendMessage,
};
