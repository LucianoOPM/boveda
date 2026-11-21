//! Supervisor del servidor (#100): sube y baja una generación según el conjunto
//! de IPv4 privadas que informa el detector de redes.
//!
//! `Supervisor` es la lógica, sin hilos ni reloj (`tick(now)`), para probarla
//! sin esperas. `ServerSupervisor` la corre en un hilo propio cada
//! `Config::check_interval`.

use std::collections::BTreeSet;
use std::net::Ipv4Addr;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::Instant;

use super::generation::Generation;
use super::{Config, Deps, NetworkSnapshot, NetworkView};
use crate::backoff::Backoff;
use crate::network::{Candidate, NetworkProbe};

#[cfg(test)]
mod tests;

/// La lógica del supervisor.
pub(crate) struct Supervisor {
    probe: Box<dyn NetworkProbe>,
    deps: Deps,
    config: Config,
    /// El conjunto de IPv4 privadas para el que hay (o debería haber) servidor.
    current: BTreeSet<Ipv4Addr>,
    /// Los candidatos de la última consulta: lo que sabe el supervisor de cada
    /// interfaz, que puede cambiar (por ejemplo la métrica) sin cambiar `current`.
    candidates: Vec<Candidate>,
    generation: Option<Generation>,
    started: u32,
    /// Espera para volver a subir tras un bind fallido o un listener caído.
    retry: Backoff,
}

impl Supervisor {
    pub(crate) fn new(probe: Box<dyn NetworkProbe>, deps: Deps, config: Config) -> Self {
        let retry = Backoff::new(config.retry_base, config.retry_max);
        Self {
            probe,
            deps,
            config,
            current: BTreeSet::new(),
            candidates: Vec::new(),
            generation: None,
            started: 0,
            retry,
        }
    }

    /// Una revisión. Cualquier error o duda del detector es «ninguna red
    /// privada»: se baja todo (falla cerrando).
    pub(crate) fn tick(&mut self, now: Instant) {
        let candidates = match self.probe.private_candidates() {
            Ok(candidates) => candidates,
            Err(error) => {
                eprintln!("servidor: {error}; se trata como red no privada");
                Vec::new()
            }
        };
        let wanted: BTreeSet<Ipv4Addr> = candidates.iter().map(|candidate| candidate.ip).collect();
        self.candidates = candidates;
        if wanted != self.current {
            self.stop_generation();
            self.current = wanted;
            self.retry.reset();
            if !self.current.is_empty() {
                self.start_generation(now);
            }
            return;
        }
        match &mut self.generation {
            Some(generation) if generation.is_alive() => generation.poll_announce(now),
            Some(_) => {
                // El hilo de `accept` murió: se baja y se vuelve a intentar tras
                // la espera, sin cambiar de conjunto.
                eprintln!("servidor: el listener cayó; se reconstruirá");
                self.stop_generation();
                self.retry.fail(now);
            }
            None if !self.current.is_empty() && self.retry.ready(now) => {
                self.start_generation(now);
            }
            None => {}
        }
    }

    fn start_generation(&mut self, now: Instant) {
        let id = u64::from(self.started) + 1;
        match Generation::start(&self.config, &self.current, &self.deps, id) {
            Ok(generation) => {
                self.generation = Some(generation);
                self.started += 1;
                self.retry.reset();
            }
            Err(error) => {
                eprintln!("servidor: no se pudo escuchar ({error:?}); se reintentará");
                self.retry.fail(now);
            }
        }
    }

    fn stop_generation(&mut self) {
        if let Some(generation) = self.generation.take() {
            generation.stop();
        }
    }

    /// Baja todo y olvida el conjunto.
    pub(crate) fn shutdown(&mut self) {
        self.stop_generation();
        self.current.clear();
    }

    /// Generaciones que se han subido desde el principio.
    #[cfg(test)]
    pub(crate) fn generations_started(&self) -> u32 {
        self.started
    }

    /// El puerto de la generación vigente, si la hay.
    #[cfg(test)]
    pub(crate) fn current_port(&self) -> Option<u16> {
        self.generation.as_ref().map(Generation::port)
    }

    /// Lo que se escucha ahora: la generación, su puerto y los candidatos de las
    /// IP que escucha. `None` si no hay generación.
    pub(crate) fn snapshot(&self) -> Option<NetworkSnapshot> {
        self.generation.as_ref().map(|generation| NetworkSnapshot {
            generation: generation.id(),
            port: generation.port(),
            candidates: self
                .candidates
                .iter()
                .filter(|candidate| self.current.contains(&candidate.ip))
                .copied()
                .collect(),
        })
    }

    /// El token de la generación vigente, si la hay.
    #[cfg(test)]
    pub(crate) fn current_token(&self) -> Option<vault_core::protocol::CancelToken> {
        self.generation
            .as_ref()
            .map(|generation| generation.token().clone())
    }

    /// ¿Sigue vivo el hilo de `accept` de la generación vigente?
    #[cfg(test)]
    pub(crate) fn current_is_alive(&self) -> bool {
        self.generation.as_ref().is_some_and(Generation::is_alive)
    }
}

/// El supervisor corriendo en su propio hilo. Estado de la app de Tauri.
pub struct ServerSupervisor {
    snapshot: Arc<Mutex<Option<NetworkSnapshot>>>,
    stop: Mutex<Option<Sender<()>>>,
    handle: Mutex<Option<JoinHandle<()>>>,
}

impl ServerSupervisor {
    /// Arranca el hilo. La sonda se mueve a él antes de usarse (COM se
    /// inicializa en ese hilo). Revisa de inmediato y luego cada
    /// `config.check_interval`.
    pub fn start(probe: impl NetworkProbe, deps: Deps, config: Config) -> Self {
        let snapshot = Arc::new(Mutex::new(None));
        let (stop, signal) = mpsc::channel::<()>();
        let interval = config.check_interval;
        let shared_snapshot = Arc::clone(&snapshot);
        let handle = thread::Builder::new()
            .name("boveda-server".to_owned())
            .spawn(move || {
                let mut supervisor = Supervisor::new(Box::new(probe), deps, config);
                loop {
                    supervisor.tick(Instant::now());
                    *shared_snapshot
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner) = supervisor.snapshot();
                    match signal.recv_timeout(interval) {
                        Err(RecvTimeoutError::Timeout) => {}
                        Ok(()) | Err(RecvTimeoutError::Disconnected) => break,
                    }
                }
                supervisor.shutdown();
                *shared_snapshot
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner) = None;
            })
            .ok();
        Self {
            snapshot,
            stop: Mutex::new(Some(stop)),
            handle: Mutex::new(handle),
        }
    }

    /// El puerto de la generación vigente; `None` si no hay servidor.
    pub fn local_port(&self) -> Option<u16> {
        self.snapshot().map(|snapshot| snapshot.port)
    }

    /// Detiene el servidor y espera a que baje. Se puede llamar varias veces.
    pub fn stop(&self) {
        if let Some(stop) = self
            .stop
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
        {
            let _ = stop.send(());
        }
        if let Some(handle) = self
            .handle
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
        {
            let _ = handle.join();
        }
    }
}

impl NetworkView for ServerSupervisor {
    fn snapshot(&self) -> Option<NetworkSnapshot> {
        self.snapshot
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl Drop for ServerSupervisor {
    fn drop(&mut self) {
        self.stop();
    }
}
