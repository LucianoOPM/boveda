//! Emparejamiento de la PC con un celular (#101): sesión del QR, confirmación
//! del usuario y guardado en la lista de emparejados.
//!
//! Diseño: `docs/superpowers/specs/2026-11-20-ventana-qr-design.md`.

mod address;
#[cfg_attr(not(test), allow(dead_code))] // Lo arranca lib.rs en la Tarea 8 de #101.
mod controller;
#[cfg_attr(not(test), allow(dead_code))] // Lo arranca lib.rs en la Tarea 8 de #101.
mod core;
mod view;

#[cfg(test)]
mod tests;

pub use address::choose_address;
pub use controller::{LateNetworkView, Notifier, PairingController};
pub use view::{EndReason, PairingView};

use vault_core::protocol::KeyPair;

/// No se pudo leer la identidad de la PC (archivo dañado, de otra cuenta o
/// ausente). Sin datos: nunca se regenera una identidad que no se pudo leer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdentityUnavailable;

/// De dónde sale la identidad de la PC para cada sesión. Es un trait para
/// probar sin DPAPI.
pub trait IdentitySource: Send + Sync + 'static {
    /// Una copia propia del par de llaves de la PC.
    fn load(&self) -> Result<KeyPair, IdentityUnavailable>;
}

impl IdentitySource for crate::identity::IdentityStore {
    fn load(&self) -> Result<KeyPair, IdentityUnavailable> {
        match crate::identity::IdentityStore::load(self) {
            Ok(Some(identity)) => Ok(identity),
            Ok(None) | Err(_) => Err(IdentityUnavailable),
        }
    }
}
