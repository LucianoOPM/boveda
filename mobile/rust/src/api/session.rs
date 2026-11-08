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
use secrecy::{ExposeSecret, ExposeSecretMut, SecretBox, SecretString};
use vault_core::vault::{self, Vault};
use zeroize::{Zeroize, Zeroizing};

/// Largo de la llave de bóveda, en bytes.
const VAULT_KEY_LEN: usize = 32;

use super::entries::entry_id_to_hex;
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
        // Antes de cualquier salida con error, para que el `String` recibido
        // siempre se borre.
        let password = secret_from(password);
        let mut slot = self.slot()?;
        if slot.is_some() {
            return Err(VaultError::VaultAlreadyOpen);
        }
        *slot = Some(Vault::create(&self.path, &password)?);
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

    /// Entrega la llave de bóveda para la copia del Keystore. Con la bóveda
    /// bloqueada da `Locked`.
    ///
    /// El `Vec` es la copia que sale del núcleo, como al revelar una
    /// contraseña: su borrado es de mejor esfuerzo. En Dart lo pone en ceros
    /// `enableBiometric` después de entregarlo a Kotlin.
    pub fn export_vault_key(&self) -> FacadeResult<Vec<u8>> {
        self.with_vault(Vault::export_vault_key)
            .map(|key| key.expose_secret().to_vec())
    }

    /// Desbloquea con la llave que liberó el Keystore.
    ///
    /// La llave recibida se pone en ceros al soltarse, en todos los caminos.
    /// Un largo distinto de 32 da `WrongVaultKey` sin tocar la bóveda, así
    /// que con la bóveda ya desbloqueada da `WrongVaultKey` y no
    /// `AlreadyUnlocked`; en la práctica no ocurre, porque la pantalla de
    /// desbloqueo solo existe con la bóveda bloqueada.
    pub fn unlock_with_key(&self, key: Vec<u8>) -> FacadeResult<UnlockReport> {
        let key = Zeroizing::new(key);
        if key.len() != VAULT_KEY_LEN {
            return Err(VaultError::WrongVaultKey);
        }
        // Como `Vault::export_vault_key`: la copia se escribe directamente
        // dentro del `SecretBox`, sin pasar por la pila.
        let mut vault_key = SecretBox::<[u8; VAULT_KEY_LEN]>::default();
        vault_key.expose_secret_mut().copy_from_slice(&key);
        self.with_vault(|vault| vault.unlock_with_key(vault_key))
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

    /// Ruta de la bóveda de esta sesión.
    pub(crate) fn path(&self) -> &std::path::Path {
        &self.path
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

#[cfg(test)]
mod tests;
