//! Mensajes del protocolo (#94) y su codificación binaria estricta.
//!
//! Cada mensaje viaja como `versión (u8) | tipo (u8) | cuerpo`, con el cuerpo
//! serializado con `serde` y `postcard`. Decodificar es estricto: comprueba el
//! tamaño, la versión y el tipo antes de leer el cuerpo, y rechaza los bytes
//! que sobren (`postcard::from_bytes` los ignoraría).

// Hasta que los mensajes (oferta, envío y confirmación) usen estas piezas.
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use super::error::{ProtocolError, Result};
use crate::vault::entry::flavor::ZeroizingVec;

/// Versión del protocolo que escribe y acepta esta versión del núcleo.
pub const PROTOCOL_VERSION: u8 = 1;

/// Tamaño máximo de un mensaje, contando la versión y el tipo.
pub const MAX_MESSAGE_LEN: usize = 4096;

/// Tiempo de vida de una contraseña enviada, en segundos.
pub const SEND_TTL_SECS: u8 = 30;

/// Longitud del código de emparejamiento de un solo uso.
pub const PAIRING_CODE_LEN: usize = 32;

/// Longitud máxima de la contraseña enviada, en bytes UTF-8 (la misma que
/// el límite de una entrada).
pub const MAX_PASSWORD_BYTES: usize = 1024;

/// Bytes de la cabecera de un mensaje: versión y tipo.
const HEADER_LEN: usize = 2;

/// Tipo de mensaje, el segundo byte de la cabecera.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
enum Kind {
    PairingOffer = 1,
    Send = 2,
    Ack = 3,
}

/// Codifica `body` como `versión | tipo | cuerpo`.
///
/// Calcula el tamaño exacto antes y reserva un solo búfer `Zeroizing`: no hay
/// reubicaciones que dejen copias sin borrar del contenido (la contraseña).
fn encode_body<T: Serialize>(kind: Kind, body: &T) -> Result<Zeroizing<Vec<u8>>> {
    let size = postcard::experimental::serialized_size(body).map_err(|_| ProtocolError::Encode)?;
    let total = size
        .checked_add(HEADER_LEN)
        .filter(|total| *total <= MAX_MESSAGE_LEN)
        .ok_or(ProtocolError::TooLarge)?;
    let mut flavor = ZeroizingVec::with_capacity(total);
    {
        use postcard::ser_flavors::Flavor;
        flavor
            .try_push(PROTOCOL_VERSION)
            .map_err(|_| ProtocolError::Encode)?;
        flavor
            .try_push(kind as u8)
            .map_err(|_| ProtocolError::Encode)?;
    }
    postcard::serialize_with_flavor(body, flavor).map_err(|_| ProtocolError::Encode)
}

/// Decodifica un mensaje del tipo `kind`, en este orden: tamaño, versión,
/// longitud mínima, tipo, cuerpo y bytes sobrantes. La validación de los
/// campos la hace cada mensaje después.
///
/// El error de `postcard` nunca se reenvía: un final inesperado es
/// [`ProtocolError::Truncated`] y cualquier otro es [`ProtocolError::Malformed`].
fn decode_body<'a, T: Deserialize<'a>>(kind: Kind, bytes: &'a [u8]) -> Result<T> {
    if bytes.len() > MAX_MESSAGE_LEN {
        return Err(ProtocolError::TooLarge);
    }
    let Some(&version) = bytes.first() else {
        return Err(ProtocolError::Truncated);
    };
    if version != PROTOCOL_VERSION {
        return Err(ProtocolError::UnsupportedVersion(version));
    }
    let Some(&found) = bytes.get(1) else {
        return Err(ProtocolError::Truncated);
    };
    if found != kind as u8 {
        return Err(ProtocolError::UnexpectedKind(found));
    }
    let (body, rest) = postcard::take_from_bytes::<T>(&bytes[HEADER_LEN..]).map_err(|e| {
        if e == postcard::Error::DeserializeUnexpectedEnd {
            ProtocolError::Truncated
        } else {
            ProtocolError::Malformed
        }
    })?;
    if !rest.is_empty() {
        return Err(ProtocolError::TrailingBytes);
    }
    Ok(body)
}

#[cfg(test)]
mod tests;
