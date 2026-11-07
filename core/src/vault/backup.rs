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
use std::path::Path;

use super::storage::Storage;
use crate::error::{Error, Result, StorageErrorKind};

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
