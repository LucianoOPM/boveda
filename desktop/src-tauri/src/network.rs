//! Detección de las redes privadas de Windows (#100).

#[cfg(windows)]
mod nlm;

use std::collections::BTreeSet;
use std::fmt;
use std::net::Ipv4Addr;

#[cfg(windows)]
pub use nlm::NlmProbe;

/// No se pudo saber qué redes son privadas. Sin datos: quien llama lo trata
/// como «ninguna red privada» (falla cerrando).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProbeError;

impl fmt::Display for ProbeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("no se pudo determinar el perfil de las redes")
    }
}

impl std::error::Error for ProbeError {}

/// Fuente de las direcciones IPv4 de las interfaces cuyo perfil de red de
/// Windows es `Private`. Es un trait para probar los cambios de red sin red real.
pub trait NetworkProbe: Send + 'static {
    /// Direcciones ya normalizadas (ver [`normalize`]). Cualquier error o duda
    /// es `Err`.
    fn private_addresses(&mut self) -> Result<BTreeSet<Ipv4Addr>, ProbeError>;
}

/// ¿Sirve como dirección donde el celular pueda alcanzar a la PC? Se descartan
/// loopback, link-local, no especificada, multidifusión y difusión.
pub fn is_usable(ip: Ipv4Addr) -> bool {
    !(ip.is_loopback()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_multicast()
        || ip.is_broadcast())
}

/// Conjunto ordenado y sin duplicados de las direcciones que sirven. Dos
/// listas con las mismas direcciones en distinto orden dan el mismo conjunto.
pub fn normalize(ips: impl IntoIterator<Item = Ipv4Addr>) -> BTreeSet<Ipv4Addr> {
    ips.into_iter().filter(|ip| is_usable(*ip)).collect()
}

/// Detector de pruebas: devuelve lo último que fijó el test con `set`. Los
/// clones comparten el valor, así el test lo cambia mientras el supervisor lo usa.
#[cfg(test)]
#[derive(Debug, Clone)]
pub(crate) struct FakeProbe {
    current: std::sync::Arc<std::sync::Mutex<Result<BTreeSet<Ipv4Addr>, ProbeError>>>,
}

#[cfg(test)]
impl FakeProbe {
    /// Empieza sin redes privadas.
    pub(crate) fn new() -> Self {
        Self {
            current: std::sync::Arc::new(std::sync::Mutex::new(Ok(BTreeSet::new()))),
        }
    }

    pub(crate) fn set(&self, result: Result<BTreeSet<Ipv4Addr>, ProbeError>) {
        *self.current.lock().expect("candado") = result;
    }
}

#[cfg(test)]
impl NetworkProbe for FakeProbe {
    fn private_addresses(&mut self) -> Result<BTreeSet<Ipv4Addr>, ProbeError> {
        self.current.lock().expect("candado").clone()
    }
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use super::{FakeProbe, NetworkProbe, ProbeError, is_usable, normalize};

    fn ip(a: u8, b: u8, c: u8, d: u8) -> Ipv4Addr {
        Ipv4Addr::new(a, b, c, d)
    }

    #[test]
    fn is_usable_rejects_addresses_a_phone_cannot_reach() {
        for rejected in [
            ip(127, 0, 0, 1),
            ip(169, 254, 1, 2),
            ip(0, 0, 0, 0),
            ip(224, 0, 0, 251),
            ip(255, 255, 255, 255),
        ] {
            assert!(!is_usable(rejected), "{rejected} no debe servir");
        }
    }

    #[test]
    fn is_usable_accepts_lan_addresses() {
        for accepted in [ip(192, 168, 1, 20), ip(10, 0, 0, 8), ip(172, 16, 0, 5)] {
            assert!(is_usable(accepted), "{accepted} debe servir");
        }
    }

    #[test]
    fn normalize_drops_unusable_and_duplicates() {
        let set = normalize([
            ip(192, 168, 1, 20),
            ip(127, 0, 0, 1),
            ip(192, 168, 1, 20),
            ip(169, 254, 3, 3),
            ip(10, 0, 0, 8),
        ]);
        assert_eq!(set.len(), 2);
        assert!(set.contains(&ip(192, 168, 1, 20)) && set.contains(&ip(10, 0, 0, 8)));
    }

    #[test]
    fn normalize_ignores_order() {
        let (a, b) = (ip(192, 168, 1, 20), ip(10, 0, 0, 8));
        assert_eq!(normalize([a, b]), normalize([b, a]));
    }

    #[test]
    fn probe_error_carries_no_data() {
        assert_eq!(format!("{ProbeError:?}"), "ProbeError");
        assert!(!ProbeError.to_string().is_empty());
    }

    #[test]
    fn fake_probe_returns_what_the_test_sets() {
        let handle = FakeProbe::new();
        let mut probe = handle.clone();
        assert_eq!(probe.private_addresses(), Ok(normalize([])));
        handle.set(Ok(normalize([ip(192, 168, 1, 20)])));
        assert_eq!(
            probe.private_addresses(),
            Ok(normalize([ip(192, 168, 1, 20)]))
        );
        handle.set(Err(ProbeError));
        assert_eq!(probe.private_addresses(), Err(ProbeError));
    }
}
