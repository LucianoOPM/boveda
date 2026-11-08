//! Respaldos: exportar e importar.

use std::path::Path;

use vault_core::vault::Vault;

use super::error::FacadeResult;
use super::session::{secret_from, UnlockReport, VaultSession};

impl VaultSession {
    /// Exporta un respaldo a `dest`, que debe ser absoluta y no existir.
    /// Exige la bóveda desbloqueada.
    pub fn export(&self, dest: String) -> FacadeResult<()> {
        self.with_vault(|vault| vault.export(Path::new(&dest)))
    }

    /// Importa el respaldo `backup` sobre la bóveda de la sesión, exista o
    /// no. `password` es la del respaldo.
    ///
    /// Suelta la bóveda actual y llama a `Vault::import`. Si sale bien, la
    /// sesión queda con la importada, desbloqueada. Si falla, vuelve a abrir
    /// la de la ruta: queda bloqueada, o la sesión queda sin bóveda si no se
    /// puede abrir (por ejemplo, en un celular nuevo). En ambos casos
    /// devuelve el error de la importación. El candado se mantiene todo el
    /// ciclo, para que ninguna otra operación se intercale.
    pub fn import_backup(&self, backup: String, password: String) -> FacadeResult<UnlockReport> {
        let password = secret_from(password);
        let mut slot = self.slot()?;
        // La especificación exige soltar la `Vault` sobre la ruta antes de
        // importar; se borra de memoria al soltarse.
        drop(slot.take());
        match Vault::import(Path::new(&backup), self.path(), &password) {
            Ok((vault, report)) => {
                *slot = Some(vault);
                Ok(report.into())
            }
            Err(error) => {
                *slot = Vault::open(self.path()).ok();
                Err(error.into())
            }
        }
    }
}

#[cfg(test)]
mod tests;
