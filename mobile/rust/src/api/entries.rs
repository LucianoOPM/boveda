//! Entradas: búsqueda, lectura por id, alta, edición, borrado y revelado.
//!
//! El id cruza como 32 caracteres hexadecimales en minúsculas: en Dart, un
//! `Uint8List` se compara por referencia y no serviría en `==`, `Set`, `Map`
//! ni en las `Key` de los widgets. El id no es secreto.

use secrecy::ExposeSecret;
use vault_core::vault::{self, EntryId, SearchData, Secret, SecretChange};

use super::error::{FacadeResult, VaultError};
use super::session::{secret_from, VaultSession};

/// Datos de búsqueda de una entrada válida, con sus fechas. Sin secretos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntrySummary {
    pub id: String,
    pub service: String,
    pub url: Option<String>,
    pub username: Option<String>,
    pub tags: Vec<String>,
    /// Milisegundos Unix.
    pub modified_at: i64,
    /// Milisegundos Unix; `None` si nunca se usó.
    pub last_used_at: Option<i64>,
}

/// Datos de búsqueda para agregar o editar. El núcleo los normaliza y valida.
pub struct EntryInput {
    pub service: String,
    pub url: Option<String>,
    pub username: Option<String>,
    pub tags: Vec<String>,
}

/// Contraseña y notas de una entrada nueva (`add`). No deriva `Debug`:
/// lleva el secreto.
pub struct NewSecret {
    pub password: String,
    pub notes: Option<String>,
}

impl VaultSession {
    /// Entradas que coinciden con `query`, la mejor primero. Sin texto,
    /// todas: las usadas más recientemente primero.
    pub fn search(&self, query: String) -> FacadeResult<Vec<EntrySummary>> {
        self.with_vault(|vault| {
            Ok(vault
                .search(&query)?
                .into_iter()
                .map(EntrySummary::from)
                .collect())
        })
    }

    /// Agrega una entrada y devuelve su id.
    pub fn add(&self, input: EntryInput, secret: NewSecret) -> FacadeResult<String> {
        // El secreto primero: así el `String` recibido se borra también
        // cuando falla una validación posterior.
        let secret = secret_data(secret)?;
        let search = search_data(&input)?;
        self.with_vault(|vault| vault.add(search, secret))
            .map(entry_id_to_hex)
    }

    /// Reemplaza los datos de búsqueda. Con `new_password` y `new_notes` en
    /// `None` conserva el secreto, que no sale del núcleo. Con uno solo,
    /// reemplaza esa parte y la otra no sale del núcleo. Con los dos,
    /// reemplaza el secreto completo (repara una entrada dañada). Unas notas
    /// en blanco las quitan.
    pub fn edit(
        &self,
        id: String,
        input: EntryInput,
        new_password: Option<String>,
        new_notes: Option<String>,
    ) -> FacadeResult<()> {
        // El secreto primero, como en `add`: los `String` recibidos se
        // borran antes de cualquier salida con error.
        let new_password = new_password.map(secret_from);
        let new_notes = new_notes.map(secret_from);
        let change = match (new_password, new_notes) {
            (None, None) => SecretChange::Keep,
            (Some(password), None) => SecretChange::ReplacePassword(password),
            (None, Some(notes)) => SecretChange::ReplaceNotes(Some(notes)),
            (Some(password), Some(notes)) => {
                SecretChange::Replace(Secret::new(password, Some(notes))?)
            }
        };
        let id = entry_id_from_hex(&id)?;
        let search = search_data(&input)?;
        self.with_vault(|vault| vault.edit(id, search, change))
    }

    /// Datos de búsqueda y fechas de una entrada, desde la caché del núcleo:
    /// no toca el disco ni el secreto. Un id que no está en la lista da
    /// `EntryNotFound`.
    pub fn entry(&self, id: String) -> FacadeResult<EntrySummary> {
        let id = entry_id_from_hex(&id)?;
        self.with_vault(|vault| vault.entry(id).map(EntrySummary::from))
    }

    pub fn delete(&self, id: String) -> FacadeResult<()> {
        let id = entry_id_from_hex(&id)?;
        self.with_vault(|vault| vault.delete(id))
    }

    /// Contraseña de una entrada, para mostrarla o copiarla. Las notas se
    /// descartan sin salir del núcleo.
    pub fn reveal_password(&self, id: String) -> FacadeResult<String> {
        let id = entry_id_from_hex(&id)?;
        let secret = self.with_vault(|vault| vault.reveal(id))?;
        Ok(secret.password().expose_secret().to_owned())
    }

    /// Notas de una entrada, para mostrarlas. La contraseña se descarta sin
    /// salir del núcleo.
    pub fn reveal_notes(&self, id: String) -> FacadeResult<Option<String>> {
        let id = entry_id_from_hex(&id)?;
        let secret = self.with_vault(|vault| vault.reveal(id))?;
        Ok(secret.notes().map(|n| n.expose_secret().to_owned()))
    }
}

impl From<&vault::EntrySummary> for EntrySummary {
    fn from(entry: &vault::EntrySummary) -> Self {
        let search = entry.search();
        Self {
            id: entry_id_to_hex(entry.id()),
            service: search.service().to_owned(),
            url: search.url().map(str::to_owned),
            username: search.username().map(str::to_owned),
            tags: search.tags().to_vec(),
            modified_at: entry.modified_at(),
            last_used_at: entry.last_used_at(),
        }
    }
}

fn search_data(input: &EntryInput) -> FacadeResult<SearchData> {
    let tags: Vec<&str> = input.tags.iter().map(String::as_str).collect();
    Ok(SearchData::new(
        &input.service,
        input.url.as_deref(),
        input.username.as_deref(),
        &tags,
    )?)
}

fn secret_data(secret: NewSecret) -> FacadeResult<Secret> {
    Ok(Secret::new(
        secret_from(secret.password),
        secret.notes.map(secret_from),
    )?)
}

/// Id de entrada en 32 caracteres hexadecimales en minúsculas.
pub(crate) fn entry_id_to_hex(id: EntryId) -> String {
    id.as_bytes().iter().map(|b| format!("{b:02x}")).collect()
}

/// Lee un id de 32 caracteres hexadecimales en minúsculas. Cualquier otra
/// cosa da `InvalidEntryId`.
pub(crate) fn entry_id_from_hex(hex: &str) -> FacadeResult<EntryId> {
    // Se valida antes de cortar: así un carácter multibyte nunca cae a la
    // mitad de un par.
    let valid = hex.len() == 32 && hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
    if !valid {
        return Err(VaultError::InvalidEntryId);
    }
    let mut bytes = [0u8; 16];
    for (byte, pair) in bytes.iter_mut().zip(hex.as_bytes().as_chunks::<2>().0) {
        *byte = (hex_value(pair[0]) << 4) | hex_value(pair[1]);
    }
    Ok(EntryId::from_bytes(bytes))
}

/// Valor de un dígito hexadecimal ya validado (`0-9a-f`).
fn hex_value(digit: u8) -> u8 {
    match digit {
        b'0'..=b'9' => digit - b'0',
        _ => digit - b'a' + 10,
    }
}

#[cfg(test)]
mod tests;
