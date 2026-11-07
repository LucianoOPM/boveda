//! Bóveda: cabecera, entradas, almacenamiento y la API pública `Vault`.
//! Los respaldos llegan más adelante.

use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{Error, Result};

pub(crate) mod entry;
pub(crate) mod header;
pub(crate) mod storage;

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
