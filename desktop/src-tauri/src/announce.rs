//! Anuncio mDNS de la PC con reintento independiente del listener (#100).
//!
//! El servicio nunca lleva el nombre de la PC: instancia y `host_name` son
//! aleatorios por generación. Un fallo del anuncio se reintenta aquí, con su
//! propia espera, sin tocar el listener.

use std::collections::BTreeSet;
use std::collections::hash_map::RandomState;
use std::fmt;
use std::hash::{BuildHasher, Hasher};
use std::net::Ipv4Addr;
use std::sync::Arc;
use std::time::Instant;

use vault_core::protocol::Fingerprint;

use crate::backoff::Backoff;

mod mdns;
pub use mdns::MdnsAnnouncer;

#[cfg(test)]
mod tests;

/// Tipo de servicio mDNS de Bóveda.
pub const SERVICE_TYPE: &str = "_boveda._tcp.local.";

/// Qué se anuncia en una generación.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Service {
    /// Nombre de instancia: `boveda-<8 hex aleatorios>`.
    pub instance: String,
    /// Nombre de host: `<instancia>.local.`, nunca el de la PC.
    pub host: String,
    /// Las IPv4 privadas de la generación; son las únicas que se publican.
    pub addrs: BTreeSet<Ipv4Addr>,
    /// El puerto de la generación.
    pub port: u16,
    /// Huella corta de la llave pública de la PC, para que el celular elija
    /// entre varias PCs.
    pub id: String,
}

impl Service {
    pub fn new(addrs: BTreeSet<Ipv4Addr>, port: u16, fingerprint: &Fingerprint) -> Self {
        let instance = format!("boveda-{}", random_hex());
        Self {
            host: format!("{instance}.local."),
            instance,
            addrs,
            port,
            id: fingerprint.to_string(),
        }
    }

    /// Los registros TXT: versión del anuncio e id de la PC.
    pub fn txt(&self) -> [(&'static str, String); 2] {
        [("v", "1".to_owned()), ("id", self.id.clone())]
    }
}

/// 8 dígitos hexadecimales aleatorios. Solo sirven para que los nombres no se
/// repitan ni delaten la PC; no son un secreto.
fn random_hex() -> String {
    format!("{:08x}", RandomState::new().build_hasher().finish() as u32)
}

/// No se pudo anunciar. Sin datos: solo se registra que falló.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnnounceError;

impl fmt::Display for AnnounceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("no se pudo anunciar el servicio por mDNS")
    }
}

impl std::error::Error for AnnounceError {}

/// Quien publica el servicio en la red. Un trait para probar sin multidifusión.
pub trait Announcer: Send + 'static {
    /// Publica el servicio. Puede llamarse otra vez tras un fallo.
    fn announce(&mut self, service: &Service) -> Result<(), AnnounceError>;

    /// Retira el anuncio y libera lo que haya abierto. Idempotente, acotado en
    /// el tiempo y sin errores: se llama también tras un anuncio fallido.
    fn withdraw(&mut self);

    /// ¿Sigue sano tras un `announce` exitoso? `register` solo encola el trabajo:
    /// los fallos del daemon llegan después. `false` hace que el anuncio se retire
    /// y se reintente. Por defecto, sano.
    fn healthy(&mut self) -> bool {
        true
    }
}

/// Crea un anunciante nuevo por generación.
pub type AnnouncerFactory = Arc<dyn Fn() -> Box<dyn Announcer> + Send + Sync>;

/// El anuncio de una generación: lo publica y, si falla, lo reintenta con
/// espera creciente cada vez que se le pregunta (`poll`).
pub(crate) struct AnnounceSlot {
    announcer: Box<dyn Announcer>,
    service: Service,
    backoff: Backoff,
    announced: bool,
}

impl AnnounceSlot {
    pub(crate) fn new(announcer: Box<dyn Announcer>, service: Service, backoff: Backoff) -> Self {
        Self {
            announcer,
            service,
            backoff,
            announced: false,
        }
    }

    /// Intenta anunciar si aún no está anunciado y ya toca.
    pub(crate) fn poll(&mut self, now: Instant) {
        if self.announced {
            if !self.announcer.healthy() {
                eprintln!("mDNS: el anuncio falló después de publicarse; se reintentará");
                self.withdraw();
                self.backoff.fail(now);
            }
            return;
        }
        if !self.backoff.ready(now) {
            return;
        }
        match self.announcer.announce(&self.service) {
            Ok(()) => {
                self.announced = true;
                self.backoff.reset();
            }
            Err(error) => {
                self.backoff.fail(now);
                eprintln!(
                    "mDNS: {error} (intento {}); se reintentará",
                    self.backoff.failures()
                );
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn is_announced(&self) -> bool {
        self.announced
    }

    /// Retira el anuncio (y limpia lo que haya quedado de un intento fallido).
    pub(crate) fn withdraw(&mut self) {
        self.announcer.withdraw();
        self.announced = false;
    }
}
