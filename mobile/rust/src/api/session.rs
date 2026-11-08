//! Sesión de la bóveda: el objeto opaco que Dart guarda y por el que pasan
//! todas las operaciones.
//!
//! `rusqlite::Connection` no es `Sync`, así que la `Vault` vive dentro de un
//! `Mutex` (la especificación lo pone en la fachada, no en el núcleo). El
//! `Option` representa una sesión sin bóveda y permite soltarla para
//! importar un respaldo.
//!
//! Ninguna función que toque la bóveda es `#[frb(sync)]`: si una síncrona
//! esperara el `Mutex` mientras otra corre Argon2id, congelaría la interfaz.

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

use flutter_rust_bridge::frb;
use secrecy::SecretString;
use vault_core::vault::{self, EntryId, Vault};
use zeroize::Zeroize;

use super::error::{FacadeResult, VaultError};

/// Sesión sobre la bóveda de una ruta fija. Empieza sin bóveda.
#[frb(opaque)]
pub struct VaultSession {
    path: PathBuf,
    vault: Mutex<Option<Vault>>,
}

/// Resultado de desbloquear o importar, sin texto plano.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlockReport {
    /// Ids (en hexadecimal) de las entradas con la fila ilegible o con los
    /// datos de búsqueda dañados.
    pub corrupt: Vec<String>,
    /// Filas dañadas sin un id válido. `u32` y no `usize`: frb genera
    /// `usize` como `BigInt` en Dart. Satura en `u32::MAX`, inalcanzable.
    pub unidentified_corrupt: u32,
}

impl From<vault::UnlockReport> for UnlockReport {
    fn from(report: vault::UnlockReport) -> Self {
        Self {
            corrupt: report.corrupt.into_iter().map(entry_id_to_hex).collect(),
            unidentified_corrupt: u32::try_from(report.unidentified_corrupt).unwrap_or(u32::MAX),
        }
    }
}

impl VaultSession {
    /// Crea una sesión sin bóveda sobre `path`. No toca el disco: el núcleo
    /// rechaza una ruta relativa al crear, abrir o importar.
    #[frb(sync)]
    pub fn new(path: String) -> VaultSession {
        Self {
            path: PathBuf::from(path),
            vault: Mutex::new(None),
        }
    }

    /// Crea la bóveda y la deja desbloqueada. Calibra Argon2id.
    pub fn create(&self, password: String) -> FacadeResult<()> {
        let mut slot = self.slot()?;
        if slot.is_some() {
            return Err(VaultError::VaultAlreadyOpen);
        }
        *slot = Some(Vault::create(&self.path, &secret_from(password))?);
        Ok(())
    }

    /// Abre la bóveda existente; queda bloqueada.
    pub fn open(&self) -> FacadeResult<()> {
        let mut slot = self.slot()?;
        if slot.is_some() {
            return Err(VaultError::VaultAlreadyOpen);
        }
        *slot = Some(Vault::open(&self.path)?);
        Ok(())
    }

    pub fn unlock(&self, password: String) -> FacadeResult<UnlockReport> {
        let password = secret_from(password);
        self.with_vault(|vault| vault.unlock(&password))
            .map(UnlockReport::from)
    }

    /// Bloquea. Sin bóveda, o con la bóveda ya bloqueada, no hace nada.
    pub fn lock(&self) -> FacadeResult<()> {
        if let Some(vault) = self.slot()?.as_mut() {
            vault.lock();
        }
        Ok(())
    }

    /// `true` también sin bóveda.
    pub fn is_locked(&self) -> FacadeResult<bool> {
        Ok(self.slot()?.as_ref().is_none_or(Vault::is_locked))
    }

    /// Toma el candado de la sesión.
    pub(crate) fn slot(&self) -> FacadeResult<MutexGuard<'_, Option<Vault>>> {
        self.vault.lock().map_err(|_| VaultError::SessionPoisoned)
    }

    /// Ejecuta `f` sobre la bóveda cargada (`NoVault` si no hay).
    pub(crate) fn with_vault<T>(
        &self,
        f: impl FnOnce(&mut Vault) -> vault::Result<T>,
    ) -> FacadeResult<T> {
        let mut slot = self.slot()?;
        let vault = slot.as_mut().ok_or(VaultError::NoVault)?;
        Ok(f(vault)?)
    }
}

/// Pasa un texto recibido de Dart a `SecretString` y borra el original.
///
/// Copia con `SecretString::from(&str)`, que reserva exactamente el largo:
/// `SecretString::from(String)` reasignaría (y dejaría una copia sin borrar)
/// si la capacidad supera el largo.
pub(crate) fn secret_from(mut text: String) -> SecretString {
    let secret = SecretString::from(text.as_str());
    text.zeroize();
    secret
}

/// Id de entrada en 32 caracteres hexadecimales en minúsculas.
pub(crate) fn entry_id_to_hex(id: EntryId) -> String {
    id.as_bytes().iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests;
