//! Enviar una entrada a una PC emparejada (#102).
//!
//! La contraseña **nunca cruza a Dart**: `send_to_pc` recibe el id de la
//! entrada, Rust la revela, arma el `SendMessage` y la entrega directo al
//! conductor del núcleo. Como en `pair`, la red corre sin el candado de la
//! bóveda y con la época capturada al empezar.

use std::net::SocketAddr;

use secrecy::{ExposeSecret, SecretString};
use vault_core::protocol::{self, AckOutcome, KeyPair, PublicKey, SendMessage, SendTimeouts};
use vault_core::vault::{EntryId, PairedPcId};

use super::entries::entry_id_from_hex;
use super::error::{FacadeResult, VaultError};
use super::net::{connect, OpGuard};
use super::pcs::{key_pair_from, pc_id_from_hex};
use super::session::VaultSession;

/// Resultado de un envío que llegó a la PC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendOutcome {
    /// La PC dejó la contraseña en su portapapeles.
    Copied,
    /// La PC recibió la contraseña pero no pudo copiarla. No es un error del
    /// protocolo.
    Failed,
}

impl From<AckOutcome> for SendOutcome {
    fn from(outcome: AckOutcome) -> Self {
        match outcome {
            AckOutcome::Copied => Self::Copied,
            AckOutcome::Failed => Self::Failed,
        }
    }
}

/// Lo que se extrae de la bóveda para un envío; se suelta (y se borra) al
/// terminar.
pub(crate) struct SendJob {
    keys: KeyPair,
    pc_key: PublicKey,
    message: SendMessage,
}

impl VaultSession {
    /// Envía la contraseña de la entrada `entry_id` a la PC `pc_id`, que
    /// escucha en `address` (`ip:puerto`).
    ///
    /// Una sola operación de red a la vez (`Busy`). Un bloqueo de la bóveda
    /// mientras corre la cancela (`Cancelled`; en ese caso la contraseña pudo
    /// haber llegado ya a la PC). Revelar la entrada actualiza su
    /// `last_used_at`, como cualquier revelado. Cómo se obtiene la dirección
    /// (última IP, mDNS, IP manual) lo decide la #104.
    pub fn send_to_pc(
        &self,
        pc_id: String,
        entry_id: String,
        address: String,
    ) -> FacadeResult<SendOutcome> {
        self.send_with(pc_id, entry_id, address, &SendTimeouts::default())
    }

    /// `send_to_pc` con plazos explícitos (las pruebas usan plazos cortos).
    pub(crate) fn send_with(
        &self,
        pc_id: String,
        entry_id: String,
        address: String,
        timeouts: &SendTimeouts,
    ) -> FacadeResult<SendOutcome> {
        self.send_after(pc_id, entry_id, address, timeouts, || {})
    }

    /// El envío completo. `after_network` corre después de la red y antes de
    /// la última comprobación de la época: sirve a las pruebas para provocar
    /// un bloqueo justo ahí; en producción es un cierre vacío.
    pub(crate) fn send_after(
        &self,
        pc_id: String,
        entry_id: String,
        address: String,
        timeouts: &SendTimeouts,
        after_network: impl FnOnce(),
    ) -> FacadeResult<SendOutcome> {
        let pc = pc_id_from_hex(&pc_id)?;
        let entry = entry_id_from_hex(&entry_id)?;
        let address = parse_address(&address)?;

        let op = self.begin_op()?;
        let job = self.send_prepare(&op, pc, entry)?;
        let stream = connect(&op, address)?;
        let outcome = protocol::send_to_pc(
            stream,
            &job.keys,
            &job.pc_key,
            &job.message,
            op.token(),
            timeouts,
        )
        .map_err(|error| op.cancelled_or(error.into()))?;
        drop(job);
        after_network();
        // Nada de éxitos tardíos: si la época cambió, no se presenta como vigente.
        op.check()?;
        Ok(outcome.into())
    }

    /// Bajo el candado de la bóveda: comprueba la época, busca la PC, revela
    /// la contraseña (las notas se descartan) y copia la llave privada de la
    /// identidad a un `KeyPair` de vida corta.
    pub(crate) fn send_prepare(
        &self,
        op: &OpGuard<'_>,
        pc: PairedPcId,
        entry: EntryId,
    ) -> FacadeResult<SendJob> {
        let mut slot = self.slot()?;
        let vault = slot.as_mut().ok_or(VaultError::NoVault)?;
        op.check()?;
        let list = vault.paired_pcs()?;
        let pc_key = match list.valid.iter().find(|candidate| candidate.id() == pc) {
            Some(found) => PublicKey::from_bytes(*found.public_key()),
            None if list.corrupt.contains(&pc) => return Err(VaultError::CorruptPairedPc),
            None => return Err(VaultError::PairedPcNotFound),
        };
        // Antes de revelar: con la identidad dañada el envío falla sin marcar
        // la entrada como usada (`reveal` escribe `last_used_at`).
        let identity = vault.ensure_identity()?;
        let secret = vault.reveal(entry)?;
        // Copia de largo exacto de la contraseña; las notas no salen.
        let password = SecretString::from(secret.password().expose_secret());
        drop(secret);
        let message = SendMessage::new(password).map_err(|_| VaultError::ProtocolFailure)?;
        Ok(SendJob {
            keys: key_pair_from(identity.private_key()),
            pc_key,
            message,
        })
    }
}

/// `ip:puerto` de la PC. Rechaza lo que no se puede usar para conectar:
/// puerto 0 y direcciones sin especificar o multicast.
fn parse_address(text: &str) -> FacadeResult<SocketAddr> {
    let address: SocketAddr = text.parse().map_err(|_| VaultError::InvalidAddress)?;
    if address.port() == 0 || address.ip().is_unspecified() || address.ip().is_multicast() {
        return Err(VaultError::InvalidAddress);
    }
    Ok(address)
}

#[cfg(test)]
mod tests;
