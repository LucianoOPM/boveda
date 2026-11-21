//! Identidad del celular y lista de PCs emparejadas (#102).
//!
//! Nada de aquí cruza una llave: la identidad sale como huella y cada PC como
//! nombre, fechas y la huella de su llave pública. Ninguna función toca la
//! red.

use std::fmt;

use secrecy::{ExposeSecret, ExposeSecretMut, SecretBox};
use vault_core::protocol::{KeyPair, PublicKey};
use vault_core::vault::{self, PairedPcId};

use super::entries::{bytes16_to_hex, hex_to_bytes16};
use super::error::{FacadeResult, VaultError};
use super::session::VaultSession;

/// La identidad del celular, sin su llave privada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityInfo {
    /// Huella corta de la llave pública, `XXXX-XXXX-XXXX-XXXX`. No autentica
    /// nada: sirve para que una persona compare dos pantallas.
    pub fingerprint: String,
}

/// Una PC emparejada, sin su llave pública.
///
/// Su `Debug` solo muestra el id: el nombre y la IP revelan con qué equipos
/// se usa la bóveda (igual que en el núcleo).
pub struct PairedPcSummary {
    /// 32 caracteres hexadecimales en minúsculas.
    pub id: String,
    pub name: String,
    /// Huella corta de la llave pública de la PC.
    pub fingerprint: String,
    /// Última IP conocida, en texto.
    pub last_ip: Option<String>,
    /// Milisegundos Unix.
    pub paired_at: i64,
    /// Milisegundos Unix; `None` si nunca se guardó una IP.
    pub last_seen_at: Option<i64>,
}

impl fmt::Debug for PairedPcSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PairedPcSummary")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

/// Las PCs emparejadas, con las filas dañadas aparte.
#[derive(Debug)]
pub struct PairedPcList {
    pub pcs: Vec<PairedPcSummary>,
    /// Ids (en hexadecimal) de las filas ilegibles; se pueden quitar.
    pub corrupt: Vec<String>,
    /// Filas dañadas sin un id válido. `u32` y no `usize`: frb genera
    /// `usize` como `BigInt` en Dart. Satura en `u32::MAX`.
    pub unidentified_corrupt: u32,
}

impl VaultSession {
    /// La identidad del celular; la crea si todavía no hay ninguna. Una
    /// identidad que existe pero no se puede leer da `CorruptIdentity` y no
    /// se sobrescribe (las PCs emparejadas conocen esa llave).
    pub fn ensure_identity(&self) -> FacadeResult<IdentityInfo> {
        self.with_vault(|vault| vault.ensure_identity())
            .map(|identity| IdentityInfo {
                fingerprint: PublicKey::from_private(identity.private_key())
                    .fingerprint()
                    .to_string(),
            })
    }

    /// Las PCs emparejadas.
    pub fn paired_pcs(&self) -> FacadeResult<PairedPcList> {
        self.with_vault(|vault| vault.paired_pcs())
            .map(|list| PairedPcList {
                pcs: list.valid.iter().map(pc_summary).collect(),
                corrupt: list
                    .corrupt
                    .iter()
                    .map(|id| bytes16_to_hex(*id.as_bytes()))
                    .collect(),
                unidentified_corrupt: u32::try_from(list.unidentified_corrupt).unwrap_or(u32::MAX),
            })
    }

    /// Quita una PC emparejada, también una fila dañada.
    pub fn remove_paired_pc(&self, id: String) -> FacadeResult<()> {
        let id = pc_id_from_hex(&id)?;
        self.with_vault(|vault| vault.remove_paired_pc(id))
    }
}

/// Resumen de una PC, sin su llave pública.
pub(crate) fn pc_summary(pc: &vault::PairedPc) -> PairedPcSummary {
    PairedPcSummary {
        id: bytes16_to_hex(*pc.id().as_bytes()),
        name: pc.name().to_owned(),
        fingerprint: PublicKey::from_bytes(*pc.public_key())
            .fingerprint()
            .to_string(),
        last_ip: pc.last_ip().map(|ip| ip.to_string()),
        paired_at: pc.paired_at(),
        last_seen_at: pc.last_seen_at(),
    }
}

/// Lee el id de una PC: 32 caracteres hexadecimales en minúsculas.
pub(crate) fn pc_id_from_hex(hex: &str) -> FacadeResult<PairedPcId> {
    hex_to_bytes16(hex)
        .map(PairedPcId::from_bytes)
        .map_err(|_| VaultError::InvalidPairedPcId)
}

#[cfg(test)]
mod tests;

/// Copia la llave privada de la identidad a un `KeyPair` de vida corta. Se
/// copia directamente dentro del `SecretBox` (sin pasar por la pila), como
/// `unlock_with_key`; `snow` no borra sus copias internas (excepción (5) de
/// `ARCHITECTURE.md`).
pub(crate) fn key_pair_from(private: &SecretBox<[u8; 32]>) -> KeyPair {
    let mut copy = SecretBox::<[u8; 32]>::default();
    copy.expose_secret_mut()
        .copy_from_slice(private.expose_secret());
    KeyPair::from_private(copy)
}
