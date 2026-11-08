//! Bóveda: cabecera, entradas, almacenamiento, respaldos y la API pública
//! `Vault`.
//!
//! `Vault` guarda la conexión al archivo y, mientras está desbloqueada, la
//! llave de bóveda junto con los datos de búsqueda descifrados. Ambos viven
//! en un solo `Option<Unlocked>`: `lock()` lo pone en `None` y los borra de
//! memoria a la vez.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use secrecy::{ExposeSecret, ExposeSecretMut, SecretBox, SecretString};
use zeroize::ZeroizeOnDrop;

use crate::crypto::kdf::{self, KdfParams};
use crate::crypto::keys::{self, VAULT_KEY_LEN, VaultKey};

mod backup;
pub(crate) mod entry;
pub(crate) mod header;
mod password;
pub(crate) mod storage;
#[cfg(test)]
pub(crate) mod test_support;

pub use crate::error::{Error, InvalidEntryField, Result, StorageErrorKind};
use entry::sealed::SealedEntry;
pub use entry::{EntryId, SearchData, Secret};
use header::VaultHeader;
use password::require_master_password;
pub use password::{
    MIN_MASTER_PASSWORD_CHARS, PasswordStrength, STRONG_MASTER_PASSWORD_CHARS, password_strength,
};
use storage::{Migration, Storage};

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
    /// Ids de las entradas con la fila ilegible o con los datos de búsqueda dañados.
    /// Los secretos no se descifran al desbloquear: un secreto dañado no
    /// aparece aquí, sino al usar [`Vault::reveal`], con
    /// [`Error::CorruptEntry`].
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
    /// [`Error::AlreadyExists`] sin tocarla. `path` debe ser absoluta (si
    /// no, `Storage(Io)`): el SQLite empaquetado interpretaría como URI una
    /// ruta que empiece con `file:`. Una contraseña de menos de
    /// [`MIN_MASTER_PASSWORD_CHARS`] caracteres da
    /// [`Error::PasswordTooShort`] antes de tocar el disco.
    pub fn create(path: &Path, password: &SecretString) -> Result<Self> {
        // Las protecciones reales están en `create_with_params` y en
        // `Storage::create`; estas comprobaciones solo sirven para fallar
        // antes de calibrar (medio segundo y al menos 64 MiB) con una
        // contraseña o una ruta que se van a rechazar igual.
        require_master_password(password)?;
        storage::require_absolute(path)?;
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
        require_master_password(password)?;
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

    /// Abre una bóveda existente. La devuelve bloqueada. `path` debe ser
    /// absoluta (si no, `Storage(Io)`), como en [`Vault::create`].
    pub fn open(path: &Path) -> Result<Self> {
        Ok(Self {
            storage: Storage::open(path)?,
            unlocked: None,
        })
    }

    /// Desbloquea con la contraseña maestra y descifra los datos de búsqueda.
    ///
    /// Las entradas con la fila ilegible o con los datos de búsqueda dañados
    /// no impiden abrir las demás: se reportan en [`UnlockReport`]. Los
    /// secretos no se descifran aquí; uno dañado aparece al usar
    /// [`Vault::reveal`]. Si falla, la bóveda sigue bloqueada.
    ///
    /// Sobre una bóveda de la versión de formato 1, la migra a la 2 antes de
    /// cargar las entradas, con una sola derivación de Argon2id y en una
    /// transacción: abrir no modifica el archivo, pero desbloquear una
    /// versión 1 sí. Si la migración falla, devuelve el error de
    /// almacenamiento, la bóveda sigue bloqueada y el archivo sigue siendo
    /// de la versión 1. Sobre una versión 2 no escribe nada.
    ///
    /// Si otra conexión migró la cabecera mientras esta derivaba, no se
    /// sobrescribe: se vuelve a derivar contra la cabecera actual (una
    /// segunda derivación), y si esa conexión cambió la contraseña el
    /// resultado es [`Error::WrongPassword`], con el archivo ya en la
    /// versión 2.
    pub fn unlock(&mut self, password: &SecretString) -> Result<UnlockReport> {
        if self.unlocked.is_some() {
            return Err(Error::AlreadyUnlocked);
        }
        let (key, upgraded) = self.storage.read_header()?.unlock_upgrading(password)?;
        self.finish_unlock(password, key, upgraded)
    }

    /// Segunda mitad de [`Vault::unlock`], después de derivar: migra si
    /// hace falta y carga las entradas. Separada para que las pruebas
    /// puedan intercalar otra `Vault` entre la derivación y la migración.
    ///
    /// Si otra conexión migró la cabecera mientras esta derivaba
    /// ([`Migration::NotNeeded`]), no se sobrescribe nada: se vuelve a leer
    /// la cabecera y se verifica `password` contra ella. Si esa otra
    /// conexión cambió la contraseña, el resultado es
    /// [`Error::WrongPassword`] y la contraseña nueva sigue valiendo.
    fn finish_unlock(
        &mut self,
        password: &SecretString,
        key: VaultKey,
        upgraded: Option<VaultHeader>,
    ) -> Result<UnlockReport> {
        let key = match upgraded {
            Some(header) => match self.storage.migrate_header(&header)? {
                Migration::Done => key,
                // La llave derivada de la cabecera vieja se suelta (y se
                // borra); vale la que abre la cabecera actual.
                Migration::NotNeeded => self.storage.read_header()?.unlock(password)?,
            },
            None => key,
        };
        let (unlocked, report) = Self::load_entries(&self.storage, key)?;
        self.unlocked = Some(unlocked);
        Ok(report)
    }

    /// Lista las entradas vivas de `storage` y descifra sus datos de
    /// búsqueda con `key` (los secretos no). Las filas ilegibles y las de
    /// datos de búsqueda dañados van al reporte, sin fallar. La usan
    /// `unlock` y la importación de respaldos.
    fn load_entries(storage: &Storage, key: VaultKey) -> Result<(Unlocked, UnlockReport)> {
        let listing = storage.list_entries()?;

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

        let report = UnlockReport {
            corrupt,
            unidentified_corrupt: listing.unidentified_corrupt,
        };
        Ok((Unlocked { key, entries }, report))
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
    /// [`SecretChange::Replace`] repara una entrada dañada, incluida una fila
    /// que no se puede leer (por ejemplo, con la versión de estructura fuera
    /// de rango), y conserva su `last_used_at`. Con
    /// [`SecretChange::Keep`] y el secreto dañado devuelve
    /// [`Error::CorruptEntry`] sin tocar el archivo.
    pub fn edit(&mut self, id: EntryId, search: SearchData, change: SecretChange) -> Result<()> {
        let unlocked = self.unlocked.as_mut().ok_or(Error::Locked)?;
        let now = now_millis()?;
        // Los metadatos vienen de la fila, no de la caché: así funciona igual
        // para una entrada que estaba dañada.
        let (secret, last_used_at) = match (self.storage.get_entry(id), change) {
            (Ok(stored), SecretChange::Keep) => (
                stored.sealed.open_secret(&unlocked.key)?,
                stored.last_used_at,
            ),
            (Ok(stored), SecretChange::Replace(secret)) => (secret, stored.last_used_at),
            // Una fila viva que no se pudo leer (p. ej. con la versión de
            // estructura fuera de rango) se reescribe completa y conserva su
            // último uso, que se lee aparte. Un id inexistente o una lápida
            // ya dieron `EntryNotFound` en `get_entry`.
            (Err(Error::CorruptEntry), SecretChange::Replace(secret)) => {
                (secret, self.storage.get_last_used_at(id)?)
            }
            (Err(e), _) => return Err(e),
        };
        let sealed = SealedEntry::seal(&unlocked.key, id, &search, &secret)?;
        self.storage.update_entry(&sealed, now)?;

        let summary = EntrySummary {
            id,
            search,
            modified_at: now,
            last_used_at,
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

    /// Exporta un respaldo: una copia consistente de la bóveda en `dest`,
    /// con su cabecera, que abre con la misma contraseña maestra.
    ///
    /// Exige la bóveda desbloqueada ([`Error::Locked`]), para que nadie
    /// exporte sin que el usuario esté presente. `dest` debe ser absoluta
    /// (si no, `Storage(Io)`) y no existir ([`Error::AlreadyExists`]). Las
    /// partes cifradas de las entradas eliminadas no viajan.
    pub fn export(&self, dest: &Path) -> Result<()> {
        self.unlocked()?;
        backup::export(&self.storage, dest)
    }

    /// Importa el respaldo `backup` sobre `dest`, exista o no (un celular
    /// nuevo no tiene bóveda), y devuelve la bóveda importada desbloqueada
    /// con su [`UnlockReport`].
    ///
    /// `password` es la contraseña maestra del respaldo, que puede ser
    /// distinta de la actual. Antes de reemplazar nada, se valida una copia
    /// temporal junto a `dest`: se abre en solo lectura (versión de
    /// formato), se revisa su integridad (`PRAGMA integrity_check`), se
    /// desbloquea con `password` y se descifran sus datos de búsqueda. Si
    /// algo falla, `dest` queda intacta.
    /// Las entradas con la fila ilegible o con los datos de búsqueda dañados
    /// no impiden importar: se reportan como en [`Vault::unlock`].
    ///
    /// Los secretos (contraseña y notas) **no** se descifran al importar: un
    /// secreto dañado no aparece en el [`UnlockReport`] y se descubre al
    /// usar [`Vault::reveal`], que devuelve [`Error::CorruptEntry`]. Es
    /// intencional: un secreto se descifra solo al copiarlo, mostrarlo o
    /// enviarlo, y validarlos aquí pondría todas las contraseñas en memoria
    /// a la vez.
    ///
    /// Quien llama debe soltar antes cualquier `Vault` abierta sobre `dest`
    /// y, si la importación falla, volver a abrirla con [`Vault::open`].
    /// Ambas rutas deben ser absolutas (si no, `Storage(Io)`).
    ///
    /// Único caso sin vuelta atrás: si falla la apertura final, `dest` ya
    /// es el respaldo validado; se puede reintentar con [`Vault::open`].
    pub fn import(
        backup: &Path,
        dest: &Path,
        password: &SecretString,
    ) -> Result<(Self, UnlockReport)> {
        backup::import(backup, dest, password)
    }

    /// Cambia la contraseña maestra: verifica `current`, calibra Argon2id y
    /// vuelve a cifrar solo la llave de bóveda, con sal y nonce nuevos.
    ///
    /// No toca la llave de bóveda ni las entradas. Una `new` de menos de
    /// [`MIN_MASTER_PASSWORD_CHARS`] caracteres da
    /// [`Error::PasswordTooShort`] antes de verificar `current` (que corre
    /// Argon2id). Con `current` incorrecta devuelve [`Error::WrongPassword`]
    /// sin calibrar ni cambiar nada.
    pub fn change_master_password(
        &mut self,
        current: &SecretString,
        new: &SecretString,
    ) -> Result<()> {
        self.check_password_change(current, new)?;
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
        self.check_password_change(current, new)?;
        self.rekey_with_params(new, params)
    }

    /// Lo que comparten ambas variantes del cambio de contraseña antes de
    /// cifrar: primero la longitud de la nueva (barata), después la actual.
    fn check_password_change(&self, current: &SecretString, new: &SecretString) -> Result<()> {
        require_master_password(new)?;
        self.verify_current(current)
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

    /// Entrega una copia de la llave de bóveda, para que la app la guarde
    /// protegida con huella (en Android, en el Keystore). Exige la bóveda
    /// desbloqueada ([`Error::Locked`]).
    ///
    /// Antes comprueba el verificador `key_check` con la llave en memoria.
    /// Si no se autentica, lo regenera con un nonce nuevo y lo escribe; si
    /// esa escritura falla, devuelve el error sin entregar la llave, para no
    /// activar una huella que no va a funcionar. Regenerarlo es seguro: la
    /// llave en memoria ya se autenticó al desbloquear (con `encrypted_key`
    /// o con `key_check`), así que un `key_check` que no se autentica solo
    /// puede estar dañado; nunca significa que la llave en memoria sea la
    /// equivocada. Con el verificador válido no escribe nada.
    ///
    /// La llave sigue valiendo después de
    /// [`Vault::change_master_password`], que no cambia la llave de bóveda.
    pub fn export_vault_key(&mut self) -> Result<SecretBox<[u8; VAULT_KEY_LEN]>> {
        let unlocked = self.unlocked.as_ref().ok_or(Error::Locked)?;
        let header = self.storage.read_header()?;
        match header.verify_key(&unlocked.key) {
            Ok(()) => {}
            Err(Error::WrongVaultKey) => {
                let key_check = keys::seal_key_check(&unlocked.key, header.format_version())?;
                self.storage.replace_key_check(&key_check)?;
            }
            Err(e) => return Err(e),
        }

        // La copia se escribe directamente dentro del `SecretBox` nuevo,
        // sin pasar por la pila.
        let mut copy = VaultKey::default();
        copy.expose_secret_mut()
            .copy_from_slice(unlocked.key.expose_secret());
        Ok(copy)
    }

    /// Desbloquea con la llave de bóveda que entregó
    /// [`Vault::export_vault_key`] y descifra los datos de búsqueda, como
    /// [`Vault::unlock`]: el mismo [`UnlockReport`] y la misma caché.
    ///
    /// Sobre una bóveda desbloqueada devuelve [`Error::AlreadyUnlocked`].
    /// Una llave de otra bóveda, alterada, un verificador dañado o una
    /// bóveda de la versión de formato 1 dan [`Error::WrongVaultKey`], y la
    /// bóveda sigue bloqueada.
    ///
    /// Recibe la llave por valor: si falla, se descarta y se borra de
    /// memoria al soltarse. Quien llama no la recupera: para reintentar,
    /// debe pedirla de nuevo al Keystore.
    pub fn unlock_with_key(&mut self, key: SecretBox<[u8; VAULT_KEY_LEN]>) -> Result<UnlockReport> {
        if self.unlocked.is_some() {
            return Err(Error::AlreadyUnlocked);
        }
        self.storage.read_header()?.verify_key(&key)?;
        let (unlocked, report) = Self::load_entries(&self.storage, key)?;
        self.unlocked = Some(unlocked);
        Ok(report)
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
