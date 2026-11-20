//! Mensajes del protocolo (#94) y su codificación binaria estricta.
//!
//! Cada mensaje viaja como `versión (u8) | tipo (u8) | cuerpo`, con el cuerpo
//! serializado con `serde` y `postcard`. Decodificar es estricto: comprueba el
//! tamaño, la versión y el tipo antes de leer el cuerpo, y rechaza los bytes
//! que sobren (`postcard::from_bytes` los ignoraría).

// Hasta que los mensajes (oferta, envío y confirmación) usen estas piezas.
#![allow(dead_code)]

use std::fmt;

use secrecy::{ExposeSecret, ExposeSecretMut, SecretBox};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use zeroize::{ZeroizeOnDrop, Zeroizing};

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

/// Código de un solo uso del emparejamiento: 32 bytes aleatorios que viajan
/// en el QR y que la #95 usa como secreto previamente compartido (PSK) de
/// Noise. Vive 2 minutos y se usa una vez.
///
/// No se puede clonar, su `Debug` no muestra el valor y se borra al soltarse.
/// Al decodificarlo, los 32 bytes pasan por un arreglo en la pila antes de
/// llegar al `SecretBox`: excepción aceptada (ver "Borrado de memoria" en
/// `ARCHITECTURE.md`), por ser un secreto de vida corta que además se
/// muestra en el QR.
pub struct PairingCode(SecretBox<[u8; PAIRING_CODE_LEN]>);

// La garantía la da `SecretBox`, que se borra al soltarse. Este marcador solo
// la hace visible en el tipo.
impl ZeroizeOnDrop for PairingCode {}

impl PairingCode {
    /// Genera un código nuevo con el generador del sistema, escrito
    /// directamente en el `SecretBox`.
    pub fn generate() -> Result<Self> {
        let mut secret = SecretBox::<[u8; PAIRING_CODE_LEN]>::default();
        getrandom::fill(secret.expose_secret_mut()).map_err(|_| ProtocolError::Random)?;
        Ok(Self(secret))
    }

    /// Construye el código con esos 32 bytes.
    pub fn from_bytes(bytes: SecretBox<[u8; PAIRING_CODE_LEN]>) -> Self {
        Self(bytes)
    }

    /// Los 32 bytes del código.
    pub fn expose_bytes(&self) -> &[u8; PAIRING_CODE_LEN] {
        self.0.expose_secret()
    }
}

impl fmt::Debug for PairingCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PairingCode(..)")
    }
}

impl Serialize for PairingCode {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        self.0.expose_secret().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for PairingCode {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        let bytes = Zeroizing::new(<[u8; PAIRING_CODE_LEN]>::deserialize(deserializer)?);
        let mut secret = SecretBox::<[u8; PAIRING_CODE_LEN]>::default();
        secret.expose_secret_mut().copy_from_slice(&*bytes);
        Ok(Self(secret))
    }
}

/// Serialización de un `SecretString` para `#[serde(with = "secret_str")]`.
///
/// Es la **única** ruta por la que un secreto se serializa en este módulo, y
/// solo se usa en el campo de la contraseña: `secrecy` no implementa
/// `Serialize` para `SecretString` a propósito.
///
/// - Al serializar, `serialize_str` escribe la longitud y los bytes
///   directamente en el búfer de `encode_body`, que tiene la capacidad exacta
///   (sin reubicaciones ni copias intermedias).
/// - Al deserializar, `postcard` (1.1.3) pide una `String` con `visit_str`,
///   que hace una sola asignación de capacidad exacta (`to_owned`);
///   `From<String>` de `SecretString` usa `into_boxed_str`, que no copia si
///   `len == capacity`. Verificado en el código de `postcard` 1.1.3, `serde` y
///   `secrecy` 0.10.3.
mod secret_str {
    use secrecy::{ExposeSecret, SecretString};
    use serde::{Deserialize, Deserializer, Serializer};

    pub(super) fn serialize<S: Serializer>(
        value: &SecretString,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(value.expose_secret())
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<SecretString, D::Error> {
        String::deserialize(deserializer).map(SecretString::from)
    }
}

#[cfg(test)]
mod tests;
