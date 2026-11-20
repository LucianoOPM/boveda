//! Mensajes del protocolo (#94) y su codificación binaria estricta.
//!
//! Cada mensaje viaja como `versión (u8) | tipo (u8) | cuerpo`, con el cuerpo
//! serializado con `serde` y `postcard`. Decodificar es estricto: comprueba el
//! tamaño, la versión y el tipo antes de leer el cuerpo, y rechaza los bytes
//! que sobren (`postcard::from_bytes` los ignoraría).

use std::fmt;
use std::net::SocketAddr;

use secrecy::{ExposeSecret, ExposeSecretMut, SecretBox, SecretString};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use zeroize::{ZeroizeOnDrop, Zeroizing};

use super::error::{InvalidField, ProtocolError, Result};
use super::identity::PublicKey;
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
fn decode_body<'a, T: Deserialize<'a> + Serialize>(kind: Kind, bytes: &'a [u8]) -> Result<T> {
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
    // `postcard` acepta varints con ceros de relleno (`0x80 0x00` por `0x00`):
    // si el cuerpo vuelve a medir distinto de lo recibido, no está en su forma
    // canónica y el mismo mensaje tendría varias codificaciones.
    let canonical =
        postcard::experimental::serialized_size(&body).map_err(|_| ProtocolError::Malformed)?;
    if canonical != bytes.len() - HEADER_LEN {
        return Err(ProtocolError::Malformed);
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

/// Tope del contenido del QR de emparejamiento, en bytes: una IPv6 con un
/// puerto de 3 bytes (versión, tipo, llave, dirección y código). La #94 no
/// fija una versión de QR; la #101 mide el tamaño real con su biblioteca.
pub const MAX_PAIRING_OFFER_LEN: usize = 86;

/// Contenido del QR de emparejamiento: la llave pública de la PC, la
/// dirección donde escucha y el código de un solo uso.
///
/// No lleva fecha de caducidad: los relojes de la PC y del celular no
/// coinciden, y los 2 minutos los cuenta la PC desde que muestra el QR. Una
/// IPv6 se codifica sin zona (`scope id`): una dirección link-local sin zona no
/// sirve para conectar, y qué dirección anuncia la PC lo decide la #100.
///
/// Su `Debug` solo muestra la huella de la llave: ni el código ni la
/// dirección, que revela dónde está la PC.
#[derive(Serialize, Deserialize)]
pub struct PairingOffer {
    public_key: PublicKey,
    address: SocketAddr,
    code: PairingCode,
}

impl PairingOffer {
    /// Arma una oferta. Puerto 0 da `InvalidField::Port`; una dirección no
    /// especificada o multicast, `InvalidField::Address`. Loopback y las
    /// direcciones privadas se aceptan.
    pub fn new(public_key: PublicKey, address: SocketAddr, code: PairingCode) -> Result<Self> {
        validate_address(&address)?;
        Ok(Self {
            public_key,
            address,
            code,
        })
    }

    /// Llave pública de la PC.
    pub fn public_key(&self) -> PublicKey {
        self.public_key
    }

    /// Dirección donde escucha la PC.
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    /// Código de un solo uso.
    pub fn code(&self) -> &PairingCode {
        &self.code
    }

    /// Codifica la oferta (`versión | tipo | cuerpo`). El resultado contiene
    /// el código: va en un búfer que se borra al soltarse.
    pub fn encode(&self) -> Result<Zeroizing<Vec<u8>>> {
        encode_body(Kind::PairingOffer, self)
    }

    /// Decodifica y valida una oferta.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let offer: Self = decode_body(Kind::PairingOffer, bytes)?;
        validate_address(&offer.address)?;
        Ok(offer)
    }
}

impl fmt::Debug for PairingOffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PairingOffer")
            .field("public_key", &self.public_key)
            .finish_non_exhaustive()
    }
}

/// Puerto distinto de 0 y dirección que no sea no especificada ni multicast.
fn validate_address(address: &SocketAddr) -> Result<()> {
    if address.port() == 0 {
        return Err(ProtocolError::InvalidField(InvalidField::Port));
    }
    // Una IPv6 que envuelve una IPv4 (`::ffff:a.b.c.d`) se juzga como la IPv4.
    let ip = address.ip().to_canonical();
    if ip.is_unspecified() || ip.is_multicast() {
        return Err(ProtocolError::InvalidField(InvalidField::Address));
    }
    Ok(())
}

/// Envío de una contraseña a la PC.
///
/// El tiempo de vida es **relativo**: 30 s desde que la PC recibe el mensaje,
/// sin hora, porque los relojes de los dos equipos no coinciden. Quien borra
/// el portapapeles a los 30 s es la PC (#98); este campo solo lo declara.
///
/// Los bytes de la contraseña no se modifican nunca: sin recortar, sin
/// normalizar a NFC, sin quitar caracteres de control. Su `Debug` solo muestra
/// el tiempo de vida.
#[derive(Serialize, Deserialize)]
pub struct SendMessage {
    ttl_secs: u8,
    #[serde(with = "secret_str")]
    password: SecretString,
}

// La garantía la da `SecretString`, que se borra al soltarse. Este marcador
// solo la hace visible en el tipo.
impl ZeroizeOnDrop for SendMessage {}

impl SendMessage {
    /// Arma el envío con el tiempo de vida de 30 s. La contraseña va **por
    /// valor**, sin copiarla; debe medir de 1 a 1024 bytes UTF-8.
    pub fn new(password: SecretString) -> Result<Self> {
        let message = Self {
            ttl_secs: SEND_TTL_SECS,
            password,
        };
        message.validate()?;
        Ok(message)
    }

    /// Tiempo de vida en segundos (siempre 30 en la versión 1).
    pub fn ttl_secs(&self) -> u8 {
        self.ttl_secs
    }

    /// La contraseña.
    pub fn password(&self) -> &SecretString {
        &self.password
    }

    /// Codifica el mensaje (`versión | tipo | cuerpo`) en un búfer de
    /// capacidad exacta que se borra al soltarse.
    pub fn encode(&self) -> Result<Zeroizing<Vec<u8>>> {
        encode_body(Kind::Send, self)
    }

    /// Decodifica y valida un envío. Si algo falla, la contraseña ya
    /// decodificada se suelta y se borra.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let message: Self = decode_body(Kind::Send, bytes)?;
        message.validate()?;
        Ok(message)
    }

    fn validate(&self) -> Result<()> {
        if self.ttl_secs != SEND_TTL_SECS {
            return Err(ProtocolError::InvalidField(InvalidField::Ttl));
        }
        match self.password.expose_secret().len() {
            0 => Err(ProtocolError::InvalidField(InvalidField::PasswordEmpty)),
            len if len > MAX_PASSWORD_BYTES => {
                Err(ProtocolError::InvalidField(InvalidField::PasswordTooLong))
            }
            _ => Ok(()),
        }
    }
}

impl fmt::Debug for SendMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SendMessage")
            .field("ttl_secs", &self.ttl_secs)
            .finish_non_exhaustive()
    }
}

/// Resultado de un envío, según la PC.
///
/// El rechazo de un dispositivo no emparejado ocurre en el apretón de manos
/// (#96), no en un mensaje.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AckOutcome {
    /// La contraseña quedó en el portapapeles.
    Copied,
    /// La PC no pudo escribir en el portapapeles.
    Failed,
}

/// Confirmación que la PC devuelve tras un envío.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ack {
    outcome: AckOutcome,
}

impl Ack {
    /// Arma una confirmación.
    pub fn new(outcome: AckOutcome) -> Self {
        Self { outcome }
    }

    /// Resultado del envío.
    pub fn outcome(&self) -> AckOutcome {
        self.outcome
    }

    /// Codifica la confirmación (`versión | tipo | resultado`).
    pub fn encode(&self) -> Result<Zeroizing<Vec<u8>>> {
        encode_body(Kind::Ack, self)
    }

    /// Decodifica una confirmación.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        decode_body(Kind::Ack, bytes)
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
