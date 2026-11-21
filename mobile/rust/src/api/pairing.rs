//! Emparejar con el QR de una PC (#102).
//!
//! `pair` es solo orquestación: decodifica el texto del QR, toma de la
//! bóveda lo que necesita (bajo su candado y un instante), conecta **sin**
//! ese candado y llama a `pair_with_pc` del núcleo; con una aceptación
//! autenticada vuelve a tomar el candado para guardar la PC, y solo si la
//! bóveda sigue desbloqueada y la época es la misma.
//!
//! La dirección sale de la oferta del QR y **no autentica a la PC**: solo
//! dice adónde conectar. La PC se autentica con Noise (`XKpsk0`: la llave
//! pública de la oferta y el código de un solo uso); una respuesta que no se
//! autentica da `PeerNotAuthentic` y el celular no manda su llave ni su
//! nombre.

use vault_core::protocol::{
    self, pair_with_pc, DeviceName, KeyPair, PairingOffer, PairingTimeouts,
};
use vault_core::vault::PairedPcUpdate;
use zeroize::Zeroize;

use super::error::{FacadeResult, VaultError};
use super::net::{connect, OpGuard};
use super::pcs::{key_pair_from, pc_summary, PairedPcSummary};
use super::session::VaultSession;

impl VaultSession {
    /// Empareja con la PC del QR y la guarda en la bóveda.
    ///
    /// `offer_text` es el texto del QR tal cual (base64url sin relleno; no se
    /// recorta nada). `device_name` es el nombre que verá la PC. La PC se
    /// guarda con un nombre provisional derivado de su llave; si ya estaba en
    /// la lista, conserva su id, nombre y fecha y solo actualiza la IP.
    ///
    /// Una sola operación de red a la vez (`Busy`). Un bloqueo de la bóveda
    /// mientras corre la cancela (`Cancelled`) y, si llega justo después de
    /// la aceptación y antes de guardar, da `Locked` y no guarda nada.
    pub fn pair(&self, offer_text: String, device_name: String) -> FacadeResult<PairedPcSummary> {
        self.pair_with(offer_text, device_name, &PairingTimeouts::default())
    }

    /// `pair` con plazos explícitos (las pruebas usan plazos cortos).
    pub(crate) fn pair_with(
        &self,
        mut offer_text: String,
        device_name: String,
        timeouts: &PairingTimeouts,
    ) -> FacadeResult<PairedPcSummary> {
        // El texto lleva el código de un solo uso: se borra el `String`
        // recibido en todos los caminos (mejor esfuerzo; ver ARCHITECTURE.md).
        let offer = PairingOffer::decode_qr_text(&offer_text);
        offer_text.zeroize();
        let offer = offer.map_err(|_| VaultError::InvalidPairingOffer)?;
        let name = DeviceName::new(&device_name).map_err(|_| VaultError::InvalidDeviceName)?;

        let op = self.begin_op()?;
        let keys = self.pairing_prepare(&op)?;
        let stream = connect(&op, offer.address())?;
        let paired = pair_with_pc(stream, &keys, &offer, name, op.token(), timeouts)?;
        drop(keys);
        let summary = self.pairing_persist(&op, &paired)?;
        // Nada de éxitos tardíos: si la época cambió, no se presenta como vigente.
        op.check()?;
        Ok(summary)
    }

    /// Bajo el candado de la bóveda: comprueba la época, crea la identidad si
    /// falta y copia su llave privada a un `KeyPair` de vida corta.
    pub(crate) fn pairing_prepare(&self, op: &OpGuard<'_>) -> FacadeResult<KeyPair> {
        let mut slot = self.slot()?;
        let vault = slot.as_mut().ok_or(VaultError::NoVault)?;
        op.check()?;
        let identity = vault.ensure_identity()?;
        Ok(key_pair_from(identity.private_key()))
    }

    /// Bajo el candado de la bóveda: guarda la PC emparejada solo si la
    /// bóveda sigue desbloqueada y la época es la de la operación; si no,
    /// `Locked` y no se guarda nada. Una PC con esa llave ya en la lista solo
    /// actualiza su IP.
    pub(crate) fn pairing_persist(
        &self,
        op: &OpGuard<'_>,
        paired: &protocol::PairedPc,
    ) -> FacadeResult<PairedPcSummary> {
        let mut slot = self.slot()?;
        let vault = slot.as_mut().ok_or(VaultError::NoVault)?;
        if vault.is_locked() || op.check().is_err() {
            return Err(VaultError::Locked);
        }
        let key = *paired.public_key().as_bytes();
        let existing = vault
            .paired_pcs()?
            .valid
            .iter()
            .find(|pc| pc.public_key() == &key)
            .map(|pc| pc.id());
        let id = match existing {
            Some(id) => {
                vault.update_paired_pc(
                    id,
                    PairedPcUpdate {
                        name: None,
                        last_ip: Some(paired.address().ip().to_canonical()),
                    },
                )?;
                id
            }
            None => vault.add_paired_pc((*paired).into_new_paired_pc(None))?,
        };
        vault
            .paired_pcs()?
            .valid
            .iter()
            .find(|pc| pc.id() == id)
            .map(pc_summary)
            .ok_or(VaultError::PairedPcNotFound)
    }
}

#[cfg(test)]
mod tests;
