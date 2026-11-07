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

pub(crate) mod entry;
pub(crate) mod header;
pub(crate) mod storage;

pub use crate::error::{Error, InvalidEntryField, Result, StorageErrorKind};
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

    /// Construye un resumen en memoria, sin bóveda. Solo para pruebas.
    #[cfg(test)]
    pub(crate) fn for_test(
        id: EntryId,
        search: SearchData,
        modified_at: i64,
        last_used_at: Option<i64>,
    ) -> Self {
        Self {
            id,
            search,
            modified_at,
            last_used_at,
        }
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

/// Qué hacer con el secreto al editar una entrada.
#[derive(Debug)]
pub enum SecretChange {
    /// Conservar el secreto actual. No sale del núcleo.
    Keep,
    /// Reemplazar contraseña y notas juntas.
    Replace(Secret),
}

impl Vault {
    /// Crea una bóveda nueva en `path` y la devuelve desbloqueada y vacía.
    ///
    /// Calibra Argon2id en este dispositivo. Una ruta que ya existe da
    /// [`Error::AlreadyExists`] sin tocarla.
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

    /// Entradas que coinciden con `query`, la mejor primero. Sin texto
    /// (vacío o solo espacios), todas: las usadas más recientemente primero.
    ///
    /// Busca en servicio, dominio de la URL, usuario y etiquetas, sobre los
    /// datos de búsqueda ya descifrados; nunca toca el secreto ni el disco.
    pub fn search(&self, query: &str) -> Result<Vec<&EntrySummary>> {
        Ok(crate::search::search(&self.unlocked()?.entries, query))
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

    /// Reemplaza los datos de búsqueda y, según `change`, conserva o
    /// reemplaza el secreto.
    ///
    /// Siempre vuelve a cifrar ambas partes con nonces nuevos y la versión de
    /// estructura actual. Actualiza `modified_at`, no `last_used_at`. Con
    /// [`SecretChange::Replace`] repara una entrada dañada. Con
    /// [`SecretChange::Keep`] y el secreto dañado devuelve
    /// [`Error::CorruptEntry`] sin tocar el archivo.
    pub fn edit(&mut self, id: EntryId, search: SearchData, change: SecretChange) -> Result<()> {
        let unlocked = self.unlocked.as_mut().ok_or(Error::Locked)?;
        let now = now_millis()?;
        // Los metadatos vienen de la fila, no de la caché: así funciona igual
        // para una entrada que estaba dañada.
        let stored = self.storage.get_entry(id)?;
        let secret = match change {
            SecretChange::Keep => stored.sealed.open_secret(&unlocked.key)?,
            SecretChange::Replace(secret) => secret,
        };
        let sealed = SealedEntry::seal(&unlocked.key, id, &search, &secret)?;
        self.storage.update_entry(&sealed, now)?;

        let summary = EntrySummary {
            id,
            search,
            modified_at: now,
            last_used_at: stored.last_used_at,
        };
        match unlocked.entries.iter_mut().find(|e| e.id == id) {
            // El valor anterior se borra al soltarse.
            Some(slot) => *slot = summary,
            None => unlocked.entries.push(summary),
        }
        Ok(())
    }

    /// Elimina una entrada: queda como lápida en el archivo. También funciona
    /// sobre una entrada dañada.
    pub fn delete(&mut self, id: EntryId) -> Result<()> {
        let unlocked = self.unlocked.as_mut().ok_or(Error::Locked)?;
        let now = now_millis()?;
        self.storage.delete_entry(id, now)?;
        unlocked.entries.retain(|e| e.id != id);
        Ok(())
    }

    /// Descifra el secreto de una entrada (al copiar, mostrar o enviar) y
    /// registra su uso.
    ///
    /// Un secreto dañado devuelve [`Error::CorruptEntry`] sin escribir nada.
    ///
    /// `last_used_at` es de mejor esfuerzo: se actualiza en memoria y luego
    /// se intenta escribir en disco. Si la escritura falla (o el reloj no es
    /// válido), el secreto se devuelve igual y el error se descarta. Dentro de
    /// la sesión, el orden en memoria es correcto; si el disco falló, ese uso
    /// se pierde al cerrar. No afecta la integridad de la bóveda.
    pub fn reveal(&mut self, id: EntryId) -> Result<Secret> {
        let unlocked = self.unlocked.as_mut().ok_or(Error::Locked)?;
        let secret = self
            .storage
            .get_entry(id)?
            .sealed
            .open_secret(&unlocked.key)?;
        if let Ok(now) = now_millis() {
            if let Some(entry) = unlocked.entries.iter_mut().find(|e| e.id == id) {
                entry.last_used_at = Some(now);
            }
            let _ = self.storage.touch_entry(id, now);
        }
        Ok(secret)
    }

    /// Cambia la contraseña maestra: verifica `current`, calibra Argon2id y
    /// vuelve a cifrar solo la llave de bóveda, con sal y nonce nuevos.
    ///
    /// No toca la llave de bóveda ni las entradas. Con `current` incorrecta
    /// devuelve [`Error::WrongPassword`] sin calibrar ni cambiar nada.
    pub fn change_master_password(
        &mut self,
        current: &SecretString,
        new: &SecretString,
    ) -> Result<()> {
        self.verify_current(current)?;
        let params = kdf::calibrate()?;
        self.rekey_with_params(new, params)
    }

    /// Como [`Vault::change_master_password`], con parámetros fijos. Solo la
    /// usan las pruebas, para no calibrar.
    #[cfg(test)]
    pub(crate) fn change_master_password_with_params(
        &mut self,
        current: &SecretString,
        new: &SecretString,
        params: KdfParams,
    ) -> Result<()> {
        self.verify_current(current)?;
        self.rekey_with_params(new, params)
    }

    /// Comprueba la contraseña actual con la cabecera guardada. La llave
    /// obtenida se suelta (y se borra) de inmediato.
    fn verify_current(&self, current: &SecretString) -> Result<()> {
        self.unlocked()?;
        self.storage.read_header()?.unlock(current)?;
        Ok(())
    }

    /// Vuelve a cifrar la llave de bóveda en memoria y reemplaza la cabecera
    /// en una sola transacción.
    fn rekey_with_params(&mut self, new: &SecretString, params: KdfParams) -> Result<()> {
        let header = VaultHeader::rekey(&self.unlocked()?.key, new, params)?;
        self.storage.replace_header(&header)
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
