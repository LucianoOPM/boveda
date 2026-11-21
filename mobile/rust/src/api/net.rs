//! Estado de las operaciones de red de la sesión (#102).
//!
//! Emparejar y enviar son operaciones de red largas (hasta 65 s y 30 s) que
//! no pueden retener el candado de la bóveda: el bloqueo tiene prioridad
//! absoluta. Cada operación toma de la bóveda, bajo su candado y un instante,
//! lo que necesita; lo suelta; y solo entonces conecta. Para que un bloqueo
//! (o una importación) alcance a una operación que ya salió de la bóveda, la
//! sesión lleva una **época** y un `CancelToken`:
//!
//! - `lock()` e `import_backup` suben la época, cancelan el token vigente,
//!   abren uno nuevo y liberan el estado «ocupado», **con el candado de la
//!   bóveda tomado**, así que no existe un instante en que la bóveda esté
//!   desbloqueada con la época ya nueva.
//! - Cada operación captura la época al empezar (`begin_op`) y la comprueba
//!   (`OpGuard::check`) tras conectar, antes de guardar y antes de devolver.
//! - Solo hay una operación de red a la vez (`Busy`).
//!
//! **Orden de candados: siempre `vault` y después `net`.** Nadie toma `vault`
//! mientras conserva `net`. `begin_op` y el `Drop` del guard solo toman `net`,
//! un instante.

// Temporal: lo usan `pair` y `send_to_pc` (Tareas 5 y 6). Se quita en la Tarea 6.
#![cfg_attr(not(test), allow(dead_code))]

use std::net::{SocketAddr, TcpStream};
use std::sync::MutexGuard;
use std::time::Duration;

use vault_core::protocol::CancelToken;

use super::error::{FacadeResult, VaultError};
use super::session::VaultSession;

/// Tiempo máximo para abrir la conexión con la PC.
pub(crate) const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// Época, token y operación activa de la sesión.
pub(crate) struct NetState {
    epoch: u64,
    token: CancelToken,
    /// Id de la operación de red en curso.
    active: Option<u64>,
    next_op: u64,
}

impl NetState {
    pub(crate) fn new() -> Self {
        Self {
            epoch: 0,
            token: CancelToken::new(),
            active: None,
            next_op: 0,
        }
    }

    /// Invalida lo que corre: sube la época, cancela el token vigente, abre
    /// uno nuevo y libera el estado «ocupado».
    pub(crate) fn invalidate(&mut self) {
        self.epoch += 1;
        self.token.cancel();
        self.token = CancelToken::new();
        self.active = None;
    }
}

/// Una operación de red en curso. Al soltarse libera el estado «ocupado»,
/// pero solo si todavía es el suyo: una operación vieja nunca borra el de una
/// más reciente.
pub(crate) struct OpGuard<'a> {
    session: &'a VaultSession,
    op: u64,
    epoch: u64,
    token: CancelToken,
}

impl OpGuard<'_> {
    /// Token de la época en que empezó la operación.
    pub(crate) fn token(&self) -> &CancelToken {
        &self.token
    }

    /// `Cancelled` si el token se canceló o la época cambió desde que empezó.
    ///
    /// Llamada con el candado de la bóveda tomado (`vault` y luego `net`),
    /// responde sobre ese mismo instante.
    pub(crate) fn check(&self) -> FacadeResult<()> {
        if self.token.is_cancelled() || self.session.net_epoch() != self.epoch {
            return Err(VaultError::Cancelled);
        }
        Ok(())
    }
}

impl Drop for OpGuard<'_> {
    fn drop(&mut self) {
        let mut net = self.session.net();
        if net.active == Some(self.op) {
            net.active = None;
        }
    }
}

impl VaultSession {
    /// Empieza una operación de red: `Busy` si ya hay una. Toma solo `net`,
    /// un instante.
    pub(crate) fn begin_op(&self) -> FacadeResult<OpGuard<'_>> {
        let mut net = self.net();
        if net.active.is_some() {
            return Err(VaultError::Busy);
        }
        let op = net.next_op;
        net.next_op += 1;
        net.active = Some(op);
        Ok(OpGuard {
            session: self,
            op,
            epoch: net.epoch,
            token: net.token.clone(),
        })
    }

    /// Época vigente. Se lee con el candado de la bóveda tomado cuando la
    /// respuesta debe coincidir con el estado de la bóveda.
    pub(crate) fn net_epoch(&self) -> u64 {
        self.net().epoch
    }

    /// Invalida las operaciones de red en curso. Se llama con el candado de
    /// la bóveda ya tomado (orden `vault` y luego `net`).
    pub(crate) fn invalidate_net(&self) {
        self.net().invalidate();
    }

    /// Toma `net` aunque un panic lo haya envenenado: su contenido es
    /// siempre consistente (se modifica en un paso).
    pub(crate) fn net(&self) -> MutexGuard<'_, NetState> {
        self.net
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// Abre la conexión con la PC. No se puede interrumpir un `connect` en
/// curso: al volver se comprueba el token y la época **antes** de seguir, y
/// una operación cancelada no inicia Noise ni escribe un solo byte.
pub(crate) fn connect(op: &OpGuard<'_>, address: SocketAddr) -> FacadeResult<TcpStream> {
    let stream = TcpStream::connect_timeout(&address, CONNECT_TIMEOUT)
        .map_err(|_| VaultError::ConnectFailed)?;
    op.check()?;
    Ok(stream)
}

#[cfg(test)]
mod tests;
