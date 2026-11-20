//! Una generación del servidor: los listeners de un conjunto de IPv4 privadas,
//! su hilo de `accept` y sus trabajadores, con un `CancelToken` propio (#100).
//!
//! La autorización la hace `receive_send` del núcleo: aquí solo se le da la
//! lista de emparejados y se atiende lo que devuelve.

use std::collections::BTreeSet;
use std::io::ErrorKind;
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use vault_core::protocol::{CancelToken, PublicKey, SendTimeouts, receive_send};

use super::bind::{BindError, bind_all};
use super::{Config, Deps};
use crate::paired_devices::PairedDeviceStore;

#[cfg(test)]
mod tests;

/// Ranuras de conexiones simultáneas. Una ranura se ocupa con [`Slots::acquire`]
/// y se libera sola al soltar su guarda, también si el trabajador falla o entra
/// en pánico.
struct Slots {
    active: Mutex<usize>,
    changed: Condvar,
    max: usize,
}

impl Slots {
    fn new(max: usize) -> Arc<Self> {
        Arc::new(Self {
            active: Mutex::new(0),
            changed: Condvar::new(),
            max,
        })
    }

    /// Ocupa una ranura, o `None` si están todas ocupadas.
    fn acquire(self: &Arc<Self>) -> Option<SlotGuard> {
        let mut active = self.active.lock().unwrap_or_else(PoisonError::into_inner);
        if *active >= self.max {
            return None;
        }
        *active += 1;
        Some(SlotGuard(Arc::clone(self)))
    }

    fn active(&self) -> usize {
        *self.active.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Espera hasta `within` a que no quede ninguna ranura ocupada.
    fn wait_idle(&self, within: Duration) -> bool {
        let active = self.active.lock().unwrap_or_else(PoisonError::into_inner);
        let (active, _) = self
            .changed
            .wait_timeout_while(active, within, |active| *active > 0)
            .unwrap_or_else(PoisonError::into_inner);
        *active == 0
    }
}

struct SlotGuard(Arc<Slots>);

impl Drop for SlotGuard {
    fn drop(&mut self) {
        let mut active = self.0.active.lock().unwrap_or_else(PoisonError::into_inner);
        *active = active.saturating_sub(1);
        self.0.changed.notify_all();
    }
}

/// Lo que comparten el hilo de `accept` y los trabajadores.
struct Shared {
    deps: Deps,
    timeouts: SendTimeouts,
    token: CancelToken,
    slots: Arc<Slots>,
}

/// Servidor de un conjunto de IPv4 privadas, con un único puerto.
pub(crate) struct Generation {
    port: u16,
    token: CancelToken,
    alive: Arc<AtomicBool>,
    accept: Option<JoinHandle<()>>,
    shared: Arc<Shared>,
    stop_wait: Duration,
}

impl Generation {
    /// Enlaza todas las `ips` al mismo puerto y empieza a aceptar conexiones.
    /// Si algo falla no queda nada abierto.
    pub(crate) fn start(
        config: &Config,
        ips: &BTreeSet<Ipv4Addr>,
        deps: &Deps,
    ) -> Result<Self, BindError> {
        let bound = bind_all(ips, config.preferred_port, config.bind_attempts)?;
        let token = CancelToken::new();
        let shared = Arc::new(Shared {
            deps: deps.clone(),
            timeouts: config.timeouts,
            token: token.clone(),
            slots: Slots::new(config.max_connections),
        });
        let alive = Arc::new(AtomicBool::new(true));
        let accept = {
            let shared = Arc::clone(&shared);
            let alive = Arc::clone(&alive);
            let poll = config.accept_poll;
            thread::Builder::new()
                .name("boveda-accept".to_owned())
                .spawn(move || {
                    accept_loop(&bound.listeners, &shared, poll);
                    alive.store(false, Ordering::SeqCst);
                })
                .map_err(|error| BindError::Io(error.kind()))?
        };
        Ok(Self {
            port: bound.port,
            token,
            alive,
            accept: Some(accept),
            shared,
            stop_wait: config.stop_wait,
        })
    }

    /// El único puerto de esta generación.
    pub(crate) fn port(&self) -> u16 {
        self.port
    }

    /// `false` si el hilo de `accept` terminó (error no recuperable).
    pub(crate) fn is_alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }

    /// Conexiones que se están atendiendo ahora.
    pub(crate) fn active_connections(&self) -> usize {
        self.shared.slots.active()
    }

    /// El token propio de esta generación.
    pub(crate) fn token(&self) -> &CancelToken {
        &self.token
    }

    /// Baja la generación: cancela su token, cierra los listeners y espera a los
    /// trabajadores como máximo `stop_wait`. Un trabajador atascado (por ejemplo
    /// en el portapapeles) no retiene el cierre: se deja terminar solo.
    pub(crate) fn stop(mut self) {
        self.token.cancel();
        if let Some(accept) = self.accept.take() {
            let _ = accept.join();
        }
        self.shared.slots.wait_idle(self.stop_wait);
    }
}

impl Drop for Generation {
    fn drop(&mut self) {
        // Soltarla sin `stop` también cancela: nada queda atendiendo.
        self.token.cancel();
    }
}

/// Acepta conexiones de todos los listeners hasta que se cancele el token o
/// falle un `accept` de forma no recuperable.
fn accept_loop(listeners: &[TcpListener], shared: &Arc<Shared>, poll: Duration) {
    while !shared.token.is_cancelled() {
        let mut accepted = false;
        for listener in listeners {
            match listener.accept() {
                Ok((stream, _)) => {
                    accepted = true;
                    handle(stream, shared);
                }
                Err(error) if error.kind() == ErrorKind::WouldBlock => {}
                Err(error) if transient(error.kind()) => {}
                Err(error) => {
                    eprintln!(
                        "servidor: falló la aceptación de conexiones ({:?})",
                        error.kind()
                    );
                    return;
                }
            }
        }
        if !accepted {
            thread::sleep(poll);
        }
    }
}

/// Errores de `accept` por una conexión que se perdió antes de aceptarla.
fn transient(kind: ErrorKind) -> bool {
    matches!(
        kind,
        ErrorKind::Interrupted | ErrorKind::ConnectionAborted | ErrorKind::ConnectionReset
    )
}

/// Da una ranura a la conexión y la atiende en su propio hilo. Sin ranura, o si
/// no se puede preparar, se cierra al instante sin leer ni escribir.
fn handle(stream: TcpStream, shared: &Arc<Shared>) {
    let Some(guard) = shared.slots.acquire() else {
        return;
    };
    // Los plazos de `receive_send` necesitan un socket bloqueante.
    if stream.set_nonblocking(false).is_err() {
        return;
    }
    let shared = Arc::clone(shared);
    let spawned = thread::Builder::new()
        .name("boveda-conn".to_owned())
        .spawn(move || {
            let _guard = guard;
            run_connection(stream, &shared);
        });
    if let Err(error) = spawned {
        eprintln!(
            "servidor: no se pudo atender una conexión ({:?})",
            error.kind()
        );
    }
}

/// Atiende una conexión: `receive_send` autentica y autoriza; con un envío
/// válido se entrega la contraseña y se confirma según el resultado.
fn run_connection(stream: TcpStream, shared: &Shared) {
    let deps = &shared.deps;
    let result = receive_send(
        stream,
        &deps.identity,
        |peer| is_paired(&deps.paired, peer),
        &shared.token,
        &shared.timeouts,
    );
    let incoming = match result {
        Ok(incoming) => incoming,
        Err(error) => {
            eprintln!("servidor: conexión terminada: {error}");
            return;
        }
    };
    // `copied` solo si el portapapeles quedó escrito; si no, `failed`.
    let answered = if deps.sink.deliver(incoming.message().password()).is_ok() {
        incoming.copied()
    } else {
        incoming.failed()
    };
    if let Err(error) = answered {
        eprintln!("servidor: no se pudo confirmar al celular: {error}");
    }
}

/// ¿Está esa llave en la lista? Un candado envenenado es duda: no se autoriza.
fn is_paired(paired: &Mutex<PairedDeviceStore>, peer: &PublicKey) -> bool {
    match paired.lock() {
        Ok(store) => store
            .list()
            .iter()
            .any(|device| &device.public_key == peer.as_bytes()),
        Err(_) => false,
    }
}
