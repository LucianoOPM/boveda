//! Servidor de red local de la PC (#100): escucha solo en las IPv4 de redes
//! privadas y rechaza en Noise a los celulares no emparejados.
//!
//! Diseño: `docs/superpowers/specs/2026-11-20-servidor-windows-design.md`.

mod bind;
mod generation;
mod supervisor;
mod tauri_sink;

#[cfg(test)]
pub(crate) mod test_support;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use secrecy::SecretString;
use vault_core::protocol::{KeyPair, SendTimeouts};

use crate::announce::AnnouncerFactory;
use crate::paired_devices::PairedDeviceStore;

pub use supervisor::ServerSupervisor;
pub use tauri_sink::TauriSink;

/// Dónde acaba una contraseña recibida. El servidor no sabe de portapapeles ni
/// de avisos: la app le pasa una implementación (y las pruebas, un doble).
pub trait PasswordSink: Send + Sync + 'static {
    /// Entrega la contraseña. `Ok` solo si quedó escrita en el portapapeles.
    fn deliver(&self, password: &SecretString) -> Result<(), DeliverFailed>;
}

/// No se pudo escribir la contraseña. Sin datos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeliverFailed;

/// Ajustes del servidor.
#[derive(Debug, Clone)]
pub struct Config {
    /// Puerto que se prefiere; si no sirve, uno común que asigna el sistema.
    pub preferred_port: u16,
    /// Conexiones simultáneas; la que sobra se cierra sin leer ni escribir.
    pub max_connections: usize,
    /// Cada cuánto el hilo de `accept` mira si hay conexión o cancelación.
    pub accept_poll: Duration,
    /// Plazos de cada envío.
    pub timeouts: SendTimeouts,
    /// Intentos de encontrar un puerto común si el preferido no sirve.
    pub bind_attempts: u32,
    /// Lo máximo que `stop` espera a los trabajadores.
    pub stop_wait: Duration,
    /// Espera inicial y máxima entre reintentos (anuncio mDNS y listener caído).
    pub retry_base: Duration,
    pub retry_max: Duration,
    /// Cada cuánto el supervisor revisa las redes.
    pub check_interval: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            preferred_port: 47615,
            max_connections: 4,
            accept_poll: Duration::from_millis(100),
            timeouts: SendTimeouts::default(),
            bind_attempts: 5,
            stop_wait: Duration::from_secs(2),
            retry_base: Duration::from_secs(5),
            retry_max: Duration::from_secs(60),
            check_interval: Duration::from_secs(5),
        }
    }
}

/// Lo que el servidor necesita de la app. Se clona barato.
#[derive(Clone)]
pub struct Deps {
    /// La identidad de la PC (el tipo ya oculta el secreto en `Debug`).
    pub identity: Arc<KeyPair>,
    /// Lista de emparejados, compartida con quien empareja (#101).
    pub paired: Arc<Mutex<PairedDeviceStore>>,
    /// Destino de las contraseñas recibidas.
    pub sink: Arc<dyn PasswordSink>,
    /// Fábrica del anunciante mDNS (uno por generación).
    pub announcer: AnnouncerFactory,
}
