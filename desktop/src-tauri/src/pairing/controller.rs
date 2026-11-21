//! Controlador del emparejamiento (#101): corre el [`PairingCore`] con un hilo
//! de revisión, avisa a la ventana de cada cambio de vista y atiende las
//! conexiones de emparejamiento que le entrega el servidor.
//!
//! El candado del núcleo nunca se mantiene durante entrada o salida de red, y
//! los avisos a la ventana salen **fuera** de él pero en el mismo orden en que
//! cambió el estado. El [`Notifier`] no debe llamar de vuelta al controlador de
//! forma síncrona.

use std::net::TcpStream;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use vault_core::protocol::{PairingTimeouts, serve_pairing};

use super::PairingView;
use super::core::{NewDevice, PairingCore, UserDecision};
use crate::server::{NetworkSnapshot, NetworkView, PairingHandler};

#[cfg(test)]
mod tests;

/// Quién recibe los cambios de vista (la ventana, o un doble en las pruebas).
pub trait Notifier: Send + Sync + 'static {
    fn changed(&self, view: &PairingView);
}

/// Vista de la red que se completa después de crear el servidor: el servidor
/// necesita el controlador (para atender emparejamientos) y el controlador
/// necesita al servidor (para saber qué escucha), así que uno se fija después.
#[derive(Default)]
pub struct LateNetworkView {
    inner: OnceLock<Arc<dyn NetworkView>>,
}

impl LateNetworkView {
    /// Fija la vista real; solo cuenta la primera vez.
    pub fn set(&self, view: Arc<dyn NetworkView>) {
        let _ = self.inner.set(view);
    }
}

impl NetworkView for LateNetworkView {
    fn snapshot(&self) -> Option<NetworkSnapshot> {
        self.inner.get().and_then(|view| view.snapshot())
    }
}

struct Inner {
    core: Mutex<PairingCore>,
    /// Mantiene el orden de los avisos: se toma antes de soltar `core`.
    order: Mutex<()>,
    net: Arc<dyn NetworkView>,
    notifier: Arc<dyn Notifier>,
    timeouts: PairingTimeouts,
}

impl Inner {
    /// Corre `f` sobre el núcleo con `now` y la instantánea de la red, y avisa
    /// si la vista cambió.
    fn run<R>(
        &self,
        f: impl FnOnce(&mut PairingCore, Instant, Option<&NetworkSnapshot>) -> R,
    ) -> R {
        let net = self.net.snapshot();
        let mut core = self.core.lock().unwrap_or_else(PoisonError::into_inner);
        let now = Instant::now();
        let result = f(&mut core, now, net.as_ref());
        let view = core.changed().then(|| core.view(now));
        let Some(view) = view else {
            return result;
        };
        let order = self.order.lock().unwrap_or_else(PoisonError::into_inner);
        drop(core);
        self.notifier.changed(&view);
        drop(order);
        result
    }
}

/// El controlador. Se comparte con `Arc`: el servidor lo usa como
/// [`PairingHandler`] y la ventana, por las órdenes.
pub struct PairingController {
    inner: Arc<Inner>,
    stop: Mutex<Option<Sender<()>>>,
    ticker: Mutex<Option<JoinHandle<()>>>,
}

impl PairingController {
    /// Arranca el hilo de revisión (cada `tick`).
    pub(crate) fn start(
        core: PairingCore,
        net: Arc<dyn NetworkView>,
        notifier: Arc<dyn Notifier>,
        timeouts: PairingTimeouts,
        tick: Duration,
    ) -> Self {
        let inner = Arc::new(Inner {
            core: Mutex::new(core),
            order: Mutex::new(()),
            net,
            notifier,
            timeouts,
        });
        let (stop, signal) = mpsc::channel::<()>();
        let worker = Arc::clone(&inner);
        let ticker = thread::Builder::new()
            .name("boveda-pairing".to_owned())
            .spawn(move || {
                while let Err(RecvTimeoutError::Timeout) = signal.recv_timeout(tick) {
                    worker.run(|core, now, net| core.tick(net, now));
                }
            })
            .ok();
        Self {
            inner,
            stop: Mutex::new(Some(stop)),
            ticker: Mutex::new(ticker),
        }
    }

    /// «Emparejar celular»: crea la sesión si no hay una viva.
    pub fn open(&self) {
        self.inner.run(|core, now, net| core.open(net, now));
    }

    /// «Generar otro código».
    pub fn regenerate(&self) {
        self.inner.run(|core, now, net| core.regenerate(net, now));
    }

    /// Cancelar, Esc o cerrar la ventana.
    pub fn cancel(&self) {
        self.inner.run(|core, _, _| core.cancel());
    }

    /// El usuario acepta (el plazo lo decide Rust).
    pub fn accept(&self) {
        self.inner.run(|core, now, _| core.accept(now));
    }

    /// El usuario rechaza.
    pub fn reject(&self) {
        self.inner.run(|core, _, _| core.reject());
    }

    /// El estado vigente (para una ventana que se abre o se recarga).
    pub fn snapshot(&self) -> PairingView {
        let core = self
            .inner
            .core
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        core.view(Instant::now())
    }

    /// Cancela la sesión y detiene el hilo de revisión. Se puede repetir.
    pub fn shutdown(&self) {
        self.cancel();
        if let Some(stop) = self
            .stop
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
        {
            let _ = stop.send(());
        }
        if let Some(ticker) = self
            .ticker
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
        {
            let _ = ticker.join();
        }
    }
}

impl Drop for PairingController {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl PairingHandler for PairingController {
    /// Atiende una conexión de emparejamiento en el hilo del trabajador del
    /// servidor: apretón de manos, espera de la decisión del usuario y cierre.
    fn serve(&self, stream: TcpStream, generation: u64) {
        let Some(ticket) = self
            .inner
            .run(|core, now, _| core.begin_connection(generation, now))
        else {
            return; // sin sesión (o ya caducó): se cierra sin leer
        };
        let pending = match serve_pairing(
            stream,
            &ticket.session,
            &ticket.cancel,
            &self.inner.timeouts,
        ) {
            Ok(pending) => pending,
            Err(error) => {
                // Solo el `Display` del error: no lleva datos del mensaje.
                eprintln!("emparejamiento: conexión terminada: {error}");
                self.inner
                    .run(|core, _, _| core.connection_failed(ticket.id, error));
                return;
            }
        };
        let name = pending.pending().peer_name().as_str().to_owned();
        let fingerprint = pending.pending().fingerprint().to_string();
        let deadline = pending.pending().deadline();
        let (decide, decision) = mpsc::channel();
        let offered = self.inner.run(|core, now, _| {
            core.offer_pending(ticket.id, &name, fingerprint, deadline, decide, now)
        });
        if !offered {
            let _ = pending.reject();
            return;
        }
        let wait = deadline.saturating_duration_since(Instant::now());
        match decision.recv_timeout(wait).unwrap_or(UserDecision::Cancel) {
            UserDecision::Accept => {
                let result = pending.confirm().map(|device| NewDevice {
                    name: device.name().as_str().to_owned(),
                    public_key: *device.public_key().as_bytes(),
                });
                if let Err(error) = &result {
                    eprintln!("emparejamiento: no se pudo confirmar: {error}");
                }
                self.inner
                    .run(|core, now, _| core.confirmed(ticket.id, result, now));
            }
            UserDecision::Reject | UserDecision::Cancel => {
                // El celular merece enterarse; si no se puede, da igual.
                let _ = pending.reject();
            }
        }
    }
}
