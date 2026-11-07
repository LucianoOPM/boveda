//! Respaldos: exportar la bóveda e importar un respaldo.
//!
//! Un respaldo es una bóveda completa, con su cabecera: abre con la misma
//! contraseña maestra con la que se exportó.
//!
//! - Exportar usa `VACUUM INTO`, que escribe una copia consistente y
//!   compacta: "all deleted content is purged from the backup, leaving
//!   behind no forensic traces" (`lang_vacuum.html`). Tampoco hace falta
//!   sincronizarla a mano: con `synchronous` en `NORMAL` o `FULL` (aquí,
//!   siempre `FULL`), SQLite hace el fsync del archivo generado.
//! - Las rutas deben ser absolutas: el SQLite empaquetado se compila con
//!   `SQLITE_USE_URI`, y una ruta que empiece con `file:` se interpretaría
//!   como URI (con parámetros como `?mode=` o `?vfs=`). Una ruta absoluta
//!   no puede empezar así.

use std::fs::{self, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

use secrecy::SecretString;

use super::storage::Storage;
use super::{UnlockReport, Unlocked, Vault};
use crate::error::{Error, Result, StorageErrorKind};

/// Sufijo de la copia temporal de un respaldo, junto a `dest`: así el
/// renombrado final queda en el mismo volumen, como exige `fs::rename`.
pub(crate) const IMPORT_SUFFIX: &str = ".import";

/// Exporta la bóveda abierta en `storage` a `dest`, que debe ser absoluta y
/// no existir. Quien llama ya comprobó que la bóveda está desbloqueada.
///
/// `dest` se crea vacío con `create_new`, que falla de forma atómica si ya
/// existe; `VACUUM INTO` acepta un destino vacío. Si algo falla después,
/// se borra `dest`: solo puede ser el archivo creado en esta llamada.
pub(super) fn export(storage: &Storage, dest: &Path) -> Result<()> {
    require_absolute(dest)?;
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dest)
        .map_err(|e| match e.kind() {
            io::ErrorKind::AlreadyExists => Error::AlreadyExists,
            _ => Error::Storage(StorageErrorKind::Io),
        })?;

    if let Err(e) = storage.vacuum_into(dest) {
        let _ = fs::remove_file(dest);
        return Err(e);
    }
    Ok(())
}

/// Importa `backup` sobre `dest` (exista o no) y devuelve la bóveda
/// importada, desbloqueada con `password` (la del respaldo).
///
/// Se valida una copia temporal, no el original: así se instala
/// exactamente lo que se validó, y el respaldo nunca se abre con SQLite.
/// Hasta el `rename`, `dest` no se toca; si algo falla antes, la copia se
/// borra y `dest` queda igual.
pub(super) fn import(
    backup: &Path,
    dest: &Path,
    password: &SecretString,
) -> Result<(Vault, UnlockReport)> {
    require_absolute(backup)?;
    require_absolute(dest)?;

    // El guardián se declara antes de cualquier conexión sobre la copia,
    // para soltarse después de ella: en Windows no se borra un archivo
    // abierto.
    let temp = TempFile(with_suffix(dest, IMPORT_SUFFIX));
    fs::copy(backup, &temp.0).map_err(io_error)?;
    // En Windows, `FlushFileBuffers` exige permiso de escritura.
    OpenOptions::new()
        .write(true)
        .open(&temp.0)
        .and_then(|file| file.sync_all())
        .map_err(io_error)?;

    let (unlocked, report) = validate(&temp.0, password)?;

    fs::rename(&temp.0, dest).map_err(io_error)?;
    sync_parent(dest);

    // Único paso sin vuelta atrás: si esta apertura falla, `dest` ya es el
    // respaldo validado y la app puede reintentar con `Vault::open`.
    let storage = Storage::open(dest)?;
    Ok((
        Vault {
            storage,
            unlocked: Some(unlocked),
        },
        report,
    ))
}

/// Abre la copia en solo lectura (rechaza un archivo ajeno, en WAL o de una
/// versión desconocida), revisa su integridad, la desbloquea y descifra los
/// datos de búsqueda. Cierra la conexión antes de volver.
fn validate(path: &Path, password: &SecretString) -> Result<(Unlocked, UnlockReport)> {
    let storage = Storage::open_read_only(path)?;
    storage.check_integrity()?;
    let key = storage.read_header()?.unlock(password)?;
    let loaded = Vault::load_entries(&storage, key)?;
    storage.close()?;
    Ok(loaded)
}

/// Borra el archivo al soltarse, ignorando el error (tras el `rename` ya no
/// existe).
struct TempFile(PathBuf);

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

/// Ruta con un sufijo agregado al nombre del archivo.
fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

/// Sincroniza el directorio de `path` para que el `rename` sobreviva a un
/// corte de luz. Mejor esfuerzo; en Windows no aplica.
#[cfg(unix)]
fn sync_parent(path: &Path) {
    if let Some(parent) = path.parent() {
        let _ = fs::File::open(parent).and_then(|dir| dir.sync_all());
    }
}

#[cfg(not(unix))]
fn sync_parent(_path: &Path) {}

/// Los errores de E/S se descartan: pueden traer rutas.
fn io_error(_: io::Error) -> Error {
    Error::Storage(StorageErrorKind::Io)
}

/// Rechaza una ruta relativa con `Storage(Io)` (ver el comentario del
/// módulo sobre `SQLITE_USE_URI`).
fn require_absolute(path: &Path) -> Result<()> {
    if path.is_absolute() {
        Ok(())
    } else {
        Err(Error::Storage(StorageErrorKind::Io))
    }
}

#[cfg(test)]
mod tests;
