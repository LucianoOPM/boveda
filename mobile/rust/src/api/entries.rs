//! Entradas: búsqueda, alta, edición, borrado y revelado.
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

/// Contraseña y notas nuevas. No deriva `Debug`: lleva el secreto.
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
        let search = search_data(&input)?;
        let secret = secret_data(secret)?;
        self.with_vault(|vault| vault.add(search, secret))
            .map(entry_id_to_hex)
    }

    /// Reemplaza los datos de búsqueda. Con `new_secret` en `None` conserva
    /// el secreto (no sale del núcleo); con `Some`, lo reemplaza.
    pub fn edit(
        &self,
        id: String,
        input: EntryInput,
        new_secret: Option<NewSecret>,
    ) -> FacadeResult<()> {
        let id = entry_id_from_hex(&id)?;
        let search = search_data(&input)?;
        let change = match new_secret {
            None => SecretChange::Keep,
            Some(secret) => SecretChange::Replace(secret_data(secret)?),
        };
        self.with_vault(|vault| vault.edit(id, search, change))
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
    for (byte, pair) in bytes.iter_mut().zip(hex.as_bytes().chunks_exact(2)) {
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
