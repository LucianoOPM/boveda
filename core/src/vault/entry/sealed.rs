//! Entrada cifrada: une los datos de búsqueda y el secreto con el cifrado de
//! `crypto::aead`.
//!
//! Cada parte se cifra por separado y queda ligada a su entrada, a su versión
//! de estructura y a su tipo. Las dos partes se cifran siempre con la misma
//! versión de estructura.

use std::fmt;

use super::{ENTRY_SCHEMA_VERSION, EntryId, SearchData, Secret};
use crate::crypto::aead::{self, EncryptedPart, Part};
use crate::crypto::keys::VaultKey;
use crate::error::{Error, Result};

/// Entrada tal como se guarda: id y versión de estructura en claro, y las
/// dos partes cifradas.
///
/// Su `Debug` muestra el id, la versión y las longitudes, nunca los bytes
/// cifrados.
pub(crate) struct SealedEntry {
    id: EntryId,
    entry_schema_version: u16,
    search: EncryptedPart,
    secret: EncryptedPart,
}

impl SealedEntry {
    /// Cifra ambas partes con la versión de estructura actual.
    ///
    /// Un fallo del generador aleatorio devuelve [`Error::Random`].
    pub(crate) fn seal(
        key: &VaultKey,
        id: EntryId,
        search: &SearchData,
        secret: &Secret,
    ) -> Result<Self> {
        // Los búferes en claro de `encode` son `Zeroizing`: se borran al
        // salir de esta función.
        let search = aead::seal_part(
            key,
            id.as_bytes(),
            ENTRY_SCHEMA_VERSION,
            Part::Search,
            &search.encode()?,
        )?;
        let secret = aead::seal_part(
            key,
            id.as_bytes(),
            ENTRY_SCHEMA_VERSION,
            Part::Secret,
            &secret.encode()?,
        )?;
        Ok(Self {
            id,
            entry_schema_version: ENTRY_SCHEMA_VERSION,
            search,
            secret,
        })
    }

    /// Reconstruye una entrada leída del almacenamiento, sin descifrarla.
    pub(crate) fn from_parts(
        id: EntryId,
        entry_schema_version: u16,
        search: EncryptedPart,
        secret: EncryptedPart,
    ) -> Self {
        Self {
            id,
            entry_schema_version,
            search,
            secret,
        }
    }

    /// Descifra solo los datos de búsqueda (al desbloquear).
    ///
    /// Cualquier fallo devuelve [`Error::CorruptEntry`] para esta entrada.
    pub(crate) fn open_search(&self, key: &VaultKey) -> Result<SearchData> {
        let plain = self.open(key, Part::Search, &self.search)?;
        SearchData::decode(self.entry_schema_version, &plain).map_err(|_| Error::CorruptEntry)
    }

    /// Descifra solo el secreto (al copiar, mostrar o enviar).
    ///
    /// Cualquier fallo devuelve [`Error::CorruptEntry`] para esta entrada.
    pub(crate) fn open_secret(&self, key: &VaultKey) -> Result<Secret> {
        let plain = self.open(key, Part::Secret, &self.secret)?;
        Secret::decode(self.entry_schema_version, &plain).map_err(|_| Error::CorruptEntry)
    }

    fn open(
        &self,
        key: &VaultKey,
        part: Part,
        sealed: &EncryptedPart,
    ) -> Result<zeroize::Zeroizing<Vec<u8>>> {
        aead::open_part(
            key,
            self.id.as_bytes(),
            self.entry_schema_version,
            part,
            sealed,
        )
        .map_err(|_| Error::CorruptEntry)
    }

    pub(crate) fn id(&self) -> EntryId {
        self.id
    }

    pub(crate) fn entry_schema_version(&self) -> u16 {
        self.entry_schema_version
    }

    pub(crate) fn search_bytes(&self) -> &EncryptedPart {
        &self.search
    }

    pub(crate) fn secret_bytes(&self) -> &EncryptedPart {
        &self.secret
    }
}

impl fmt::Debug for SealedEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SealedEntry")
            .field("id", &self.id)
            .field("entry_schema_version", &self.entry_schema_version)
            .field("search_len", &self.search.as_bytes().len())
            .field("secret_len", &self.secret.as_bytes().len())
            .finish()
    }
}

#[cfg(test)]
mod tests;
