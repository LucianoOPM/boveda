//! Bóveda: cabecera, entradas, almacenamiento y la API pública `Vault`.
//! Los respaldos llegan más adelante.
//!
//! `Vault` guarda la conexión al archivo y, mientras está desbloqueada, la
//! llave de bóveda junto con los datos de búsqueda descifrados. Ambos viven
//! en un solo `Option<Unlocked>`: `lock()` lo pone en `None` y los borra de
//! memoria a la vez.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use secrecy::SecretString;
use zeroize::ZeroizeOnDrop;

use crate::crypto::kdf::{self, KdfParams};
use crate::crypto::keys::VaultKey;
use crate::error::{Error, Result};

pub(crate) mod entry;
pub(crate) mod header;
pub(crate) mod storage;

use entry::sealed::SealedEntry;
pub use entry::{EntryId, SearchData, Secret};
use header::VaultHeader;
use storage::Storage;

/// Bóveda abierta sobre un archivo. No deriva `Debug`.
///
/// `rusqlite::Connection` no es `Sync`: quien la comparta entre hilos la
/// envuelve en un `Mutex` (la fachada de `mobile/rust`).
pub struct Vault {
    storage: Storage,
    unlocked: Option<Unlocked>,
}

/// Estado desbloqueado: la llave y la caché, que se borran juntas.
struct Unlocked {
    key: VaultKey,
    entries: Vec<EntrySummary>,
}

/// Datos de búsqueda descifrados de una entrada válida, con sus fechas.
///
/// Su `Debug` no muestra los valores (lo hereda de [`SearchData`]).
#[derive(Debug)]
pub struct EntrySummary {
    id: EntryId,
    search: SearchData,
    modified_at: i64,
    last_used_at: Option<i64>,
}

// La garantía la da `SearchData`, que se borra al soltarse. Este marcador
// solo lo hace visible en el tipo.
impl ZeroizeOnDrop for EntrySummary {}

impl EntrySummary {
    pub fn id(&self) -> EntryId {
        self.id
    }

    pub fn search(&self) -> &SearchData {
        &self.search
    }

    /// Fecha de la última modificación, en milisegundos Unix.
    pub fn modified_at(&self) -> i64 {
        self.modified_at
    }

    /// Fecha del último uso (`reveal`), en milisegundos Unix. `None` si
    /// nunca se usó.
    pub fn last_used_at(&self) -> Option<i64> {
        self.last_used_at
    }
}

/// Resultado de desbloquear, sin texto plano. Las entradas válidas se leen
/// con [`Vault::entries`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlockReport {
    /// Entradas que no se pudieron descifrar.
    pub corrupt: Vec<EntryId>,
    /// Filas dañadas sin un id de 16 bytes: no hay un id que reportar.
    pub unidentified_corrupt: usize,
}

impl Vault {
    /// Crea una bóveda nueva en `path` y la devuelve desbloqueada y vacía.
    ///
    /// Calibra Argon2id en este dispositivo. Una ruta que ya existe da
    /// [`Error::AlreadyExists`] sin tocarla.
    // Temporal: se quita al hacer público el módulo `vault`.
    #[cfg_attr(test, allow(dead_code))]
    pub fn create(path: &Path, password: &SecretString) -> Result<Self> {
        let params = kdf::calibrate()?;
        Self::create_with_params(path, password, params)
    }

    /// Como [`Vault::create`], con parámetros de Argon2id fijos.
    pub(crate) fn create_with_params(
        path: &Path,
        password: &SecretString,
        params: KdfParams,
    ) -> Result<Self> {
        // Todo lo que puede fallar en memoria ocurre antes de crear el archivo.
        let (header, key) = VaultHeader::create(password, params)?;
        let storage = Storage::create(path, &header)?;
        Ok(Self {
            storage,
            unlocked: Some(Unlocked {
                key,
                entries: Vec::new(),
            }),
        })
    }

    /// Abre una bóveda existente. La devuelve bloqueada.
    pub fn open(path: &Path) -> Result<Self> {
        Ok(Self {
            storage: Storage::open(path)?,
            unlocked: None,
        })
    }

    /// Desbloquea con la contraseña maestra y descifra los datos de búsqueda.
    ///
    /// Las entradas que no se pueden descifrar no impiden abrir las demás: se
    /// reportan en [`UnlockReport`]. Si falla, la bóveda sigue bloqueada.
    pub fn unlock(&mut self, password: &SecretString) -> Result<UnlockReport> {
        if self.unlocked.is_some() {
            return Err(Error::AlreadyUnlocked);
        }
        let key = self.storage.read_header()?.unlock(password)?;
        let listing = self.storage.list_entries()?;

        let mut corrupt = listing.corrupt;
        let mut entries = Vec::with_capacity(listing.entries.len());
        for stored in listing.entries {
            match stored.sealed.open_search(&key) {
                Ok(search) => entries.push(EntrySummary {
                    id: stored.sealed.id(),
                    search,
                    modified_at: stored.modified_at,
                    last_used_at: stored.last_used_at,
                }),
                Err(_) => corrupt.push(stored.sealed.id()),
            }
        }

        self.unlocked = Some(Unlocked { key, entries });
        Ok(UnlockReport {
            corrupt,
            unidentified_corrupt: listing.unidentified_corrupt,
        })
    }

    /// Bloquea: borra de memoria la llave de bóveda y los datos de búsqueda.
    /// Sobre una bóveda ya bloqueada no hace nada.
    pub fn lock(&mut self) {
        self.unlocked = None;
    }

    pub fn is_locked(&self) -> bool {
        self.unlocked.is_none()
    }

    /// Entradas válidas, con sus datos de búsqueda descifrados.
    pub fn entries(&self) -> Result<&[EntrySummary]> {
        Ok(&self.unlocked()?.entries)
    }

    /// Agrega una entrada y devuelve su identificador nuevo.
    pub fn add(&mut self, search: SearchData, secret: Secret) -> Result<EntryId> {
        let unlocked = self.unlocked.as_mut().ok_or(Error::Locked)?;
        let now = now_millis()?;
        let id = EntryId::generate()?;
        let sealed = SealedEntry::seal(&unlocked.key, id, &search, &secret)?;
        self.storage.insert_entry(&sealed, now)?;
        unlocked.entries.push(EntrySummary {
            id,
            search,
            modified_at: now,
            last_used_at: None,
        });
        Ok(id)
    }

    fn unlocked(&self) -> Result<&Unlocked> {
        self.unlocked.as_ref().ok_or(Error::Locked)
    }
}

/// Hora actual en milisegundos Unix. Es el único punto del núcleo que lee el
/// reloj del sistema.
fn now_millis() -> Result<i64> {
    millis_since_epoch(SystemTime::now())
}

/// Convierte una hora a milisegundos Unix. Una fecha anterior a 1970 o que
/// no cabe en `i64` devuelve [`Error::Clock`].
fn millis_since_epoch(time: SystemTime) -> Result<i64> {
    let elapsed = time.duration_since(UNIX_EPOCH).map_err(|_| Error::Clock)?;
    i64::try_from(elapsed.as_millis()).map_err(|_| Error::Clock)
}

#[cfg(test)]
mod tests;
