//! Lógica de una sesión de emparejamiento (#101), sin hilos ni reloj propio:
//! todo recibe `now`, así que los plazos y las carreras se prueban sin esperar.
//!
//! Rust es la única fuente de verdad: el webview solo recibe una
//! [`PairingView`] y envía órdenes sin datos. Cada sesión tiene un
//! [`SessionId`]; cualquier cosa que llegue con el id de otra sesión se descarta.

use std::collections::BTreeSet;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

use secrecy::SecretBox;
use vault_core::protocol::{
    CancelToken, PAIRING_CODE_TTL, PairingCode, PairingOffer, PairingSession,
};
use zeroize::Zeroizing;

use super::{EndReason, IdentitySource, PairingView, choose_address};
use crate::paired_devices::PairedDeviceStore;
use crate::qr;
use crate::server::NetworkSnapshot;

/// Identifica una sesión; solo existe en Rust y crece con cada sesión nueva.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SessionId(u64);

/// Lo que un hilo trabajador necesita para atender una conexión de la sesión
/// vigente: la sesión del núcleo (el código de un solo uso) y su cancelación.
pub(crate) struct Ticket {
    pub(crate) id: SessionId,
    pub(crate) session: Arc<Mutex<PairingSession>>,
    pub(crate) cancel: CancelToken,
}

/// Una sesión viva: el código, su plazo y la oferta que se publicó.
struct Live {
    id: SessionId,
    session: Arc<Mutex<PairingSession>>,
    cancel: CancelToken,
    /// Generación del servidor para la que se hizo la oferta.
    generation: u64,
    port: u16,
    ip: Ipv4Addr,
    expires_at: Instant,
}

impl Live {
    /// Invalida el código y corta lo que esté en curso.
    fn shutdown(&self) {
        self.session
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .cancel();
        self.cancel.cancel();
    }
}

enum Phase {
    Closed,
    NoNetwork,
    /// `svg` contiene el código de un solo uso: se borra al soltarse.
    Waiting {
        live: Live,
        svg: Zeroizing<String>,
    },
    Ended(EndReason),
}

pub(crate) struct PairingCore {
    identity: Arc<dyn IdentitySource>,
    phase: Phase,
    next_id: u64,
    /// ¿Cambió la vista desde la última vez que se preguntó con `changed`?
    dirty: bool,
    #[cfg(test)]
    last_offer_text: Option<String>,
}

impl PairingCore {
    pub(crate) fn new(
        identity: Arc<dyn IdentitySource>,
        _paired: Arc<Mutex<PairedDeviceStore>>,
    ) -> Self {
        Self {
            identity,
            phase: Phase::Closed,
            next_id: 1,
            dirty: false,
            #[cfg(test)]
            last_offer_text: None,
        }
    }

    /// «Emparejar celular»: crea la sesión si no hay una viva. Devuelve `true`
    /// si empezó una (con una ya abierta, la ventana solo se enfoca).
    pub(crate) fn open(&mut self, net: Option<&NetworkSnapshot>, now: Instant) -> bool {
        if matches!(self.phase, Phase::Waiting { .. }) {
            return false;
        }
        self.start_session(net, now);
        true
    }

    /// «Generar otro código»: termina la sesión actual (si la hay) y crea otra
    /// distinta. Nunca se reutiliza una sesión.
    pub(crate) fn regenerate(&mut self, net: Option<&NetworkSnapshot>, now: Instant) {
        self.finish(Phase::Closed);
        self.start_session(net, now);
    }

    /// Cancelar, Esc o cerrar la ventana: invalida la sesión y cierra.
    pub(crate) fn cancel(&mut self) {
        self.finish(Phase::Closed);
    }

    /// Revisión periódica: caducidad y cambios de red.
    pub(crate) fn tick(&mut self, net: Option<&NetworkSnapshot>, now: Instant) {
        match &self.phase {
            Phase::Waiting { live, .. } => {
                if now >= live.expires_at {
                    self.finish(Phase::Ended(EndReason::Expired));
                } else if network_changed(live, net) {
                    self.finish(Phase::Ended(EndReason::NetworkChanged));
                }
            }
            Phase::NoNetwork => {
                if net.is_some_and(|net| pick(net).is_some()) {
                    self.start_session(net, now);
                }
            }
            Phase::Closed | Phase::Ended(_) => {}
        }
    }

    /// Un hilo trabajador pide atender una conexión que aceptó la generación
    /// `generation`. `None` si no hay sesión vigente, si ya caducó (gana el
    /// plazo de Rust) o si la conexión es de otra generación.
    pub(crate) fn begin_connection(&mut self, generation: u64, now: Instant) -> Option<Ticket> {
        let Phase::Waiting { live, .. } = &self.phase else {
            return None;
        };
        if now >= live.expires_at {
            self.finish(Phase::Ended(EndReason::Expired));
            return None;
        }
        if generation != live.generation {
            return None;
        }
        Some(Ticket {
            id: live.id,
            session: Arc::clone(&live.session),
            cancel: live.cancel.clone(),
        })
    }

    /// Lo que debe mostrar la ventana ahora.
    pub(crate) fn view(&self, now: Instant) -> PairingView {
        match &self.phase {
            Phase::Closed => PairingView::Closed,
            Phase::NoNetwork => PairingView::NoNetwork,
            Phase::Waiting { live, svg } => PairingView::Waiting {
                qr: qr::data_uri(svg),
                remaining_ms: millis_until(live.expires_at, now),
            },
            Phase::Ended(reason) => PairingView::Ended {
                reason: *reason,
                placeholder: qr::data_uri(qr::PLACEHOLDER_SVG),
            },
        }
    }

    /// ¿Cambió la vista desde la última pregunta? Quien notifica a la ventana
    /// lo usa para no repetir eventos.
    pub(crate) fn changed(&mut self) -> bool {
        std::mem::take(&mut self.dirty)
    }

    /// La sesión viva, si la hay.
    pub(crate) fn session_id(&self) -> Option<SessionId> {
        match &self.phase {
            Phase::Waiting { live, .. } => Some(live.id),
            _ => None,
        }
    }

    /// El texto del último QR generado (solo en pruebas).
    #[cfg(test)]
    pub(crate) fn last_offer_text(&self) -> Option<String> {
        self.last_offer_text.clone()
    }

    /// Cambia de fase invalidando antes la sesión viva y descartando la imagen.
    fn finish(&mut self, next: Phase) {
        if let Phase::Waiting { live, .. } = std::mem::replace(&mut self.phase, next) {
            live.shutdown();
        }
        self.dirty = true;
        #[cfg(test)]
        {
            self.last_offer_text = None;
        }
    }

    /// Crea una sesión nueva con la dirección elegida y deja el QR listo.
    fn start_session(&mut self, net: Option<&NetworkSnapshot>, now: Instant) {
        let Some((net, ip)) = net.and_then(|net| pick(net).map(|ip| (net, ip))) else {
            self.finish(Phase::NoNetwork);
            return;
        };
        match self.build_session(net, ip, now) {
            Ok((live, svg, _text)) => {
                #[cfg(test)]
                {
                    self.last_offer_text = Some(_text.to_string());
                }
                self.finish_to(Phase::Waiting { live, svg });
            }
            Err(reason) => self.finish(Phase::Ended(reason)),
        }
    }

    /// Como `finish`, pero conserva el texto de prueba ya fijado.
    fn finish_to(&mut self, next: Phase) {
        #[cfg(test)]
        let keep = self.last_offer_text.take();
        self.finish(next);
        #[cfg(test)]
        {
            self.last_offer_text = keep;
        }
    }

    fn build_session(
        &mut self,
        net: &NetworkSnapshot,
        ip: Ipv4Addr,
        now: Instant,
    ) -> Result<(Live, Zeroizing<String>, Zeroizing<String>), EndReason> {
        let identity = self
            .identity
            .load()
            .map_err(|_| EndReason::IdentityUnavailable)?;
        let code = PairingCode::generate().map_err(|_| EndReason::CodeUnavailable)?;
        let offer = PairingOffer::new(
            identity.public(),
            SocketAddr::from((ip, net.port)),
            duplicate(&code),
        )
        .map_err(|_| EndReason::CodeUnavailable)?;
        let text = qr::offer_text(&offer).map_err(|_| EndReason::CodeUnavailable)?;
        let svg = qr::render_svg(&text).map_err(|_| EndReason::CodeUnavailable)?;
        let id = SessionId(self.next_id);
        self.next_id += 1;
        let live = Live {
            id,
            session: Arc::new(Mutex::new(PairingSession::new(identity, code, now))),
            cancel: CancelToken::new(),
            generation: net.generation,
            port: net.port,
            ip,
            expires_at: now + PAIRING_CODE_TTL,
        };
        Ok((live, svg, text))
    }
}

/// La dirección elegida entre lo que el servidor escucha ahora.
fn pick(net: &NetworkSnapshot) -> Option<Ipv4Addr> {
    let listening: BTreeSet<Ipv4Addr> = net.candidates.iter().map(|c| c.ip).collect();
    choose_address(&net.candidates, &listening)
}

/// ¿Dejó de servir la oferta? Cambió la generación o el puerto, o la dirección
/// del QR ya no se escucha. Un cambio de métrica con la dirección activa no la
/// invalida: el QR sigue siendo alcanzable.
fn network_changed(live: &Live, net: Option<&NetworkSnapshot>) -> bool {
    match net {
        None => true,
        Some(net) => {
            net.generation != live.generation
                || net.port != live.port
                || !net.candidates.iter().any(|c| c.ip == live.ip)
        }
    }
}

fn millis_until(deadline: Instant, now: Instant) -> u64 {
    u64::try_from(deadline.saturating_duration_since(now).as_millis()).unwrap_or(u64::MAX)
}

/// Una segunda copia del código: la oferta la consume y la sesión del núcleo
/// necesita la suya. Los 32 bytes pasan por la pila (excepción documentada de
/// `PairingCode`: secreto de 2 minutos y un uso que además se muestra en el QR).
fn duplicate(code: &PairingCode) -> PairingCode {
    PairingCode::from_bytes(SecretBox::new(Box::new(*code.expose_bytes())))
}
