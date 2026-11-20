//! Protocolo entre el celular y la PC: identidades X25519 y mensajes (#94).
//!
//! Solo tipos y codificación binaria; sin red. El emparejamiento y el
//! transporte con Noise llegan en las issues siguientes de la etapa 3.

mod error;

pub use error::{InvalidField, ProtocolError, Result};
