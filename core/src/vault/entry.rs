//! Modelo de entrada: identificador, datos de búsqueda y secreto.
//!
//! Cada entrada se divide en dos partes que se cifran por separado: los datos
//! de búsqueda (se descifran al desbloquear) y el secreto (solo al copiar,
//! mostrar o enviar). Aquí se definen los tipos validados y su serialización
//! a texto plano; el cifrado vive en otro módulo.

use std::fmt;

use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use crate::error::{Error, InvalidEntryField, Result};

// Límites de la v1, en bytes UTF-8 sobre el valor ya normalizado.
pub(crate) const MAX_SERVICE_LEN: usize = 256;
pub(crate) const MAX_URL_LEN: usize = 2048;
pub(crate) const MAX_USERNAME_LEN: usize = 256;
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

#[cfg(test)]
mod tests;
