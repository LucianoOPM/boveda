//! Modelo de entrada: identificador, datos de búsqueda y secreto.
//!
//! Cada entrada se divide en dos partes que se cifran por separado: los datos
//! de búsqueda (se descifran al desbloquear) y el secreto (solo al copiar,
//! mostrar o enviar). Aquí se definen los tipos validados y su serialización
//! a texto plano; el cifrado vive en otro módulo.

use std::fmt;

use secrecy::{ExposeSecret, SecretString};
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use crate::error::{Error, InvalidEntryField, Result};

mod flavor;
mod raw;

use flavor::ZeroizingVec;
use raw::{RawSearchDataV1, RawSecretV1};

/// Versión de estructura con la que se escriben las entradas nuevas.
///
/// Se guarda en claro junto a cada entrada; no confundir con la versión de
/// formato de la cabecera (`format_version`).
pub(crate) const ENTRY_SCHEMA_VERSION: u16 = 1;

/// Capacidad inicial del búfer de serialización; crece si hace falta.
const ENCODE_INITIAL_CAPACITY: usize = 256;

// Límites de la v1, en bytes UTF-8 sobre el valor ya normalizado.
pub(crate) const MAX_SERVICE_LEN: usize = 256;
pub(crate) const MAX_URL_LEN: usize = 2048;
pub(crate) const MAX_USERNAME_LEN: usize = 256;
pub(crate) const MAX_PASSWORD_LEN: usize = 1024;
pub(crate) const MAX_NOTES_LEN: usize = 16384;
pub(crate) const MAX_TAGS: usize = 32;
pub(crate) const MAX_TAG_LEN: usize = 64;

/// Longitud del identificador de entrada.
pub(crate) const ENTRY_ID_LEN: usize = 16;

/// Identificador de una entrada: 16 bytes aleatorios del generador del sistema.
///
/// No es secreto, así que deriva `Debug`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct EntryId([u8; ENTRY_ID_LEN]);

impl EntryId {
    /// Genera un identificador nuevo con el generador del sistema.
    pub(crate) fn generate() -> Result<Self> {
        let mut bytes = [0u8; ENTRY_ID_LEN];
        getrandom::fill(&mut bytes).map_err(|_| Error::Random)?;
        Ok(Self(bytes))
    }

    /// Reconstruye un identificador leído del almacenamiento.
    pub(crate) fn from_bytes(bytes: [u8; ENTRY_ID_LEN]) -> Self {
        Self(bytes)
    }

    pub(crate) fn as_bytes(&self) -> &[u8; ENTRY_ID_LEN] {
        &self.0
    }
}

/// Datos de búsqueda de una entrada: servicio, URL, usuario y etiquetas.
///
/// Solo se construye con [`SearchData::new`] o al decodificar, así que
/// siempre cumple las reglas de su versión. Se borra de memoria al soltarse y
/// su `Debug` no muestra los valores.
#[derive(Clone, PartialEq, Eq, Zeroize, ZeroizeOnDrop)]
pub(crate) struct SearchData {
    service: String,
    url: Option<String>,
    username: Option<String>,
    tags: Vec<String>,
}

impl SearchData {
    /// Crea los datos de búsqueda normalizándolos y validándolos con las
    /// reglas de la versión de estructura actual.
    pub(crate) fn new(
        service: &str,
        url: Option<&str>,
        username: Option<&str>,
        tags: &[&str],
    ) -> Result<Self> {
        build_search_data_v1(service, url, username, tags).map_err(Error::InvalidEntry)
    }

    /// Serializa con la versión de estructura actual.
    pub(crate) fn encode(&self) -> Result<Zeroizing<Vec<u8>>> {
        let raw = RawSearchDataV1 {
            service: &self.service,
            url: self.url.as_deref(),
            username: self.username.as_deref(),
            // Solo punteros al texto: no es una copia de los datos.
            tags: self.tags.iter().map(String::as_str).collect(),
        };
        encode_with_zeroizing(&raw)
    }

    /// Deserializa unos bytes escritos con `entry_schema_version`, aplicando
    /// las reglas de esa versión.
    ///
    /// Una versión desconocida, bytes mal formados o sobrantes, o un contenido
    /// que viola las reglas devuelven [`Error::CorruptEntry`].
    pub(crate) fn decode(entry_schema_version: u16, bytes: &[u8]) -> Result<Self> {
        match entry_schema_version {
            1 => {
                let raw: RawSearchDataV1 = take_all(bytes)?;
                build_search_data_v1(raw.service, raw.url, raw.username, &raw.tags)
                    .map_err(|_| Error::CorruptEntry)
            }
            _ => Err(Error::CorruptEntry),
        }
    }

    pub(crate) fn service(&self) -> &str {
        &self.service
    }

    pub(crate) fn url(&self) -> Option<&str> {
        self.url.as_deref()
    }

    pub(crate) fn username(&self) -> Option<&str> {
        self.username.as_deref()
    }

    pub(crate) fn tags(&self) -> &[String] {
        &self.tags
    }
}

/// Muestra qué campos hay y cuántas etiquetas, nunca su contenido.
impl fmt::Debug for SearchData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const HIDDEN: &str = "[oculto]";
        f.debug_struct("SearchData")
            .field("service", &HIDDEN)
            .field("url", &self.url.as_ref().map(|_| HIDDEN))
            .field("username", &self.username.as_ref().map(|_| HIDDEN))
            .field("tags", &self.tags.len())
            .finish()
    }
}

/// Serializa con postcard dentro de un búfer que se borra al crecer y al
/// soltarse.
fn encode_with_zeroizing<T: serde::Serialize>(raw: &T) -> Result<Zeroizing<Vec<u8>>> {
    // Solo falla si el tamaño desborda `usize`; no ocurre con los límites v1.
    postcard::serialize_with_flavor(raw, ZeroizingVec::with_capacity(ENCODE_INITIAL_CAPACITY))
        .map_err(|_| Error::CorruptEntry)
}

/// Deserializa `bytes` completo: un error o bytes sobrantes son
/// [`Error::CorruptEntry`].
fn take_all<'a, T: serde::Deserialize<'a>>(bytes: &'a [u8]) -> Result<T> {
    match postcard::take_from_bytes(bytes) {
        Ok((value, [])) => Ok(value),
        _ => Err(Error::CorruptEntry),
    }
}

/// Recorta un campo opcional: vacío o solo espacios cuenta como ausente.
///
/// Devuelve `Err(too_long)` si el valor recortado supera `max` bytes.
fn optional_field_v1(
    value: Option<&str>,
    max: usize,
    too_long: InvalidEntryField,
) -> std::result::Result<Option<String>, InvalidEntryField> {
    match value.map(str::trim) {
        None | Some("") => Ok(None),
        Some(v) if v.len() > max => Err(too_long),
        // `to_owned` reserva exactamente la longitud del texto.
        Some(v) => Ok(Some(v.to_owned())),
    }
}

/// Reglas de los datos de búsqueda en la versión de estructura 1.
///
/// Las comparten `new()` y la decodificación de entradas v1: una versión
/// nueva agrega su propia función sin tocar esta.
fn build_search_data_v1(
    service: &str,
    url: Option<&str>,
    username: Option<&str>,
    tags: &[&str],
) -> std::result::Result<SearchData, InvalidEntryField> {
    let service = service.trim();
    if service.is_empty() {
        return Err(InvalidEntryField::ServiceEmpty);
    }
    if service.len() > MAX_SERVICE_LEN {
        return Err(InvalidEntryField::ServiceTooLong);
    }
    let url = optional_field_v1(url, MAX_URL_LEN, InvalidEntryField::UrlTooLong)?;
    let username = optional_field_v1(
        username,
        MAX_USERNAME_LEN,
        InvalidEntryField::UsernameTooLong,
    )?;

    // Duplicados sin distinguir mayúsculas: se conserva la primera forma
    // escrita y el orden. Las claves en minúsculas también son datos del
    // usuario, así que se borran al soltarse.
    let mut seen: Vec<Zeroizing<String>> = Vec::new();
    let mut kept: Vec<String> = Vec::new();
    for tag in tags.iter().map(|t| t.trim()) {
        if tag.is_empty() {
            continue;
        }
        if tag.len() > MAX_TAG_LEN {
            return Err(InvalidEntryField::TagTooLong);
        }
        let key = Zeroizing::new(tag.to_lowercase());
        if seen.contains(&key) {
            continue;
        }
        if kept.len() == MAX_TAGS {
            return Err(InvalidEntryField::TooManyTags);
        }
        seen.push(key);
        kept.push(tag.to_owned());
    }

    Ok(SearchData {
        service: service.to_owned(),
        url,
        username,
        tags: kept,
    })
}

/// Secreto de una entrada: contraseña y notas.
///
/// No deriva `Clone` ni `PartialEq` para no copiar ni comparar el secreto, y
/// su `Debug` no muestra los valores. Solo se lee con `expose_secret()`.
pub(crate) struct Secret {
    password: SecretString,
    notes: Option<SecretString>,
}

// La garantía la dan los campos: `SecretString` borra su contenido al
// soltarse. Este marcador solo lo hace visible en el tipo.
impl ZeroizeOnDrop for Secret {}

impl Secret {
    /// Crea el secreto validándolo con las reglas de la versión de
    /// estructura actual.
    pub(crate) fn new(password: SecretString, notes: Option<SecretString>) -> Result<Self> {
        build_secret_v1(password, notes).map_err(Error::InvalidEntry)
    }

    /// Serializa con la versión de estructura actual.
    pub(crate) fn encode(&self) -> Result<Zeroizing<Vec<u8>>> {
        let raw = RawSecretV1 {
            password: self.password.expose_secret(),
            notes: self.notes.as_ref().map(|n| n.expose_secret()),
        };
        encode_with_zeroizing(&raw)
    }

    /// Deserializa unos bytes escritos con `entry_schema_version`, aplicando
    /// las reglas de esa versión. Los errores son [`Error::CorruptEntry`].
    pub(crate) fn decode(entry_schema_version: u16, bytes: &[u8]) -> Result<Self> {
        match entry_schema_version {
            1 => {
                let raw: RawSecretV1 = take_all(bytes)?;
                // `SecretString::from(&str)` usa `String::from`, que reserva
                // exactamente la longitud: su `into_boxed_str` no reasigna y no
                // deja una copia sin borrar.
                let password = SecretString::from(raw.password);
                let notes = raw.notes.map(SecretString::from);
                build_secret_v1(password, notes).map_err(|_| Error::CorruptEntry)
            }
            _ => Err(Error::CorruptEntry),
        }
    }

    pub(crate) fn password(&self) -> &SecretString {
        &self.password
    }

    pub(crate) fn notes(&self) -> Option<&SecretString> {
        self.notes.as_ref()
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const HIDDEN: &str = "[oculto]";
        f.debug_struct("Secret")
            .field("password", &HIDDEN)
            .field("notes", &self.notes.as_ref().map(|_| HIDDEN))
            .finish()
    }
}

/// Reglas del secreto en la versión de estructura 1.
///
/// La contraseña y el contenido de las notas nunca se recortan: los espacios
/// pueden ser parte del secreto. Unas notas en blanco cuentan como ausentes.
fn build_secret_v1(
    password: SecretString,
    notes: Option<SecretString>,
) -> std::result::Result<Secret, InvalidEntryField> {
    let len = password.expose_secret().len();
    if len == 0 {
        return Err(InvalidEntryField::PasswordEmpty);
    }
    if len > MAX_PASSWORD_LEN {
        return Err(InvalidEntryField::PasswordTooLong);
    }
    let notes = notes.filter(|n| !n.expose_secret().trim().is_empty());
    if notes
        .as_ref()
        .is_some_and(|n| n.expose_secret().len() > MAX_NOTES_LEN)
    {
        return Err(InvalidEntryField::NotesTooLong);
    }
    Ok(Secret { password, notes })
}

#[cfg(test)]
mod tests;
