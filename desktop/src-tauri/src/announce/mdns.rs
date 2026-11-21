//! Anunciante real sobre `mdns-sd` (#100).
//!
//! La configuración de interfaces está verificada en la tarea 1 del plan (ver
//! «Verificación de `mdns-sd`» en el diseño): se deshabilitan todas las
//! interfaces y solo se habilitan las IPv4 privadas del servicio, que son
//! además las únicas direcciones que se publican. Limitación conocida: la crate
//! abre sus sockets UDP 5353 en `0.0.0.0` y `[::]` y no deja cambiarlo.

use std::net::IpAddr;
use std::time::Duration;

use mdns_sd::{IfKind, ServiceDaemon, ServiceInfo};

use super::{AnnounceError, Announcer, SERVICE_TYPE, Service};

/// Cuánto se espera la confirmación de `unregister` y de `shutdown`.
const WITHDRAW_WAIT: Duration = Duration::from_secs(1);

/// Anunciante mDNS: crea su `ServiceDaemon` en el primer `announce` y lo apaga
/// en `withdraw`.
#[derive(Default)]
pub struct MdnsAnnouncer {
    daemon: Option<ServiceDaemon>,
    /// Nombre completo del servicio registrado, si lo hay.
    fullname: Option<String>,
}

impl MdnsAnnouncer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Crea el daemon y lo limita a las IPv4 del servicio.
    fn start_daemon(service: &Service) -> Result<ServiceDaemon, AnnounceError> {
        let daemon = ServiceDaemon::new().map_err(|_| AnnounceError)?;
        let configured = daemon.disable_interface(IfKind::All).and_then(|()| {
            service
                .addrs
                .iter()
                .try_for_each(|ip| daemon.enable_interface(IfKind::Addr(IpAddr::V4(*ip))))
        });
        if configured.is_err() {
            shut_down(&daemon);
            return Err(AnnounceError);
        }
        Ok(daemon)
    }

    /// Descarta el daemon y el registro.
    fn teardown(&mut self) {
        if let Some(daemon) = self.daemon.take() {
            if let Some(fullname) = self.fullname.take()
                && let Ok(status) = daemon.unregister(&fullname)
            {
                let _ = status.recv_timeout(WITHDRAW_WAIT);
            }
            shut_down(&daemon);
        }
        self.fullname = None;
    }
}

/// Apaga el daemon y espera su confirmación un momento.
fn shut_down(daemon: &ServiceDaemon) {
    if let Ok(status) = daemon.shutdown() {
        let _ = status.recv_timeout(WITHDRAW_WAIT);
    }
}

impl Announcer for MdnsAnnouncer {
    fn announce(&mut self, service: &Service) -> Result<(), AnnounceError> {
        if self.fullname.is_some() {
            return Ok(()); // ya registrado
        }
        let daemon = match self.daemon.take() {
            Some(daemon) => daemon,
            None => Self::start_daemon(service)?,
        };
        let addresses = service
            .addrs
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let info = ServiceInfo::new(
            SERVICE_TYPE,
            &service.instance,
            &service.host,
            addresses,
            service.port,
            &service.txt()[..],
        )
        .map(|mut info| {
            info.set_interfaces(
                service
                    .addrs
                    .iter()
                    .map(|ip| IfKind::Addr(IpAddr::V4(*ip)))
                    .collect(),
            );
            info
        });
        let fullname = match info {
            Ok(info) => {
                let fullname = info.get_fullname().to_owned();
                match daemon.register(info) {
                    Ok(()) => Some(fullname),
                    Err(_) => None,
                }
            }
            Err(_) => None,
        };
        match fullname {
            Some(fullname) => {
                self.daemon = Some(daemon);
                self.fullname = Some(fullname);
                Ok(())
            }
            None => {
                shut_down(&daemon);
                Err(AnnounceError)
            }
        }
    }

    fn withdraw(&mut self) {
        self.teardown();
    }
}

impl Drop for MdnsAnnouncer {
    fn drop(&mut self) {
        self.teardown();
    }
}
