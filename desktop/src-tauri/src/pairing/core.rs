//! Lógica de una sesión de emparejamiento (#101), sin hilos ni reloj propio:
//! todo recibe `now`, así que los plazos y las carreras se prueban sin esperar.
//!
//! Rust es la única fuente de verdad: el webview solo recibe una
//! [`PairingView`] y envía órdenes sin datos. Cada sesión tiene un
//! [`SessionId`]; cualquier cosa que llegue con el id de otra sesión se descarta.

use std::collections::BTreeSet;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use secrecy::SecretBox;
use vault_core::protocol::{
    CancelToken, KeyPair, MAX_FAILED_ATTEMPTS, PAIRING_CODE_TTL, PairingCode, PairingError,
    PairingOffer, PairingSession, Refusal,
};
use zeroize::Zeroizing;

use super::{EndReason, IdentitySource, PairingView, choose_address};
use crate::paired_devices::{PairedDevice, PairedDeviceStore, PairedDevicesError};
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

/// Lo que el usuario (o el controlador) decide sobre una conexión pendiente.
/// El hilo trabajador que tiene la conexión lo recibe y actúa.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UserDecision {
    /// Aceptar: el hilo llama a `confirm()`.
    Accept,
    /// Rechazar: el hilo llama a `reject()`.
    Reject,
    /// Cancelar: la sesión terminó (cancelada, caducada o con otra red); el
    /// hilo rechaza o cierra la conexión sin emparejar.
    Cancel,
}

/// El celular que el protocolo terminó de emparejar. Sin `Debug`: ni el nombre
/// ni la llave deben salir en logs.
pub(crate) struct NewDevice {
    pub(crate) name: String,
    pub(crate) public_key: [u8; 32],
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
    /// Tramas 1 malas que ya contó el núcleo contra el código (lo agota a las
    /// `MAX_FAILED_ATTEMPTS`, sin avisar: aquí se lleva la misma cuenta).
    failed: u32,
}

impl Live {
    /// Invalida el código (un código ya gastado sigue gastado).
    fn invalidate_code(&self) {
        self.session
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .cancel();
    }

    /// Invalida el código y corta lo que esté en curso, también las lecturas y
    /// escrituras bloqueadas de la conexión.
    fn shutdown(&self) {
        self.invalidate_code();
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
    /// Un celular espera la decisión; `decide` llega al hilo que tiene su conexión.
    Confirming {
        live: Live,
        name: String,
        fingerprint: String,
        deadline: Instant,
        decide: Sender<UserDecision>,
    },
    /// El usuario aceptó: el hilo cierra el protocolo y la app guarda.
    Saving {
        live: Live,
        name: String,
    },
    Done {
        name: String,
    },
    Ended(EndReason),
}

pub(crate) struct PairingCore {
    identity: Arc<dyn IdentitySource>,
    paired: Arc<Mutex<PairedDeviceStore>>,
    phase: Phase,
    next_id: u64,
    /// ¿Cambió la vista desde la última vez que se preguntó con `changed`?
    dirty: bool,
    /// Milisegundos Unix para `paired_at`: un solo punto, cambiable en pruebas.
    clock: fn() -> i64,
    #[cfg(test)]
    last_offer_text: Option<String>,
}

impl PairingCore {
    pub(crate) fn new(
        identity: Arc<dyn IdentitySource>,
        paired: Arc<Mutex<PairedDeviceStore>>,
    ) -> Self {
        Self {
            identity,
            paired,
            phase: Phase::Closed,
            next_id: 1,
            dirty: false,
            clock: unix_millis,
            #[cfg(test)]
            last_offer_text: None,
        }
    }

    /// «Emparejar celular»: crea la sesión si no hay una viva. Devuelve `true`
    /// si empezó una (con una ya abierta, la ventana solo se enfoca).
    pub(crate) fn open(&mut self, net: Option<&NetworkSnapshot>, now: Instant) -> bool {
        if self.session_id().is_some() {
            return false;
        }
        self.start_session(net, now);
        true
    }

    /// «Generar otro código»: termina la sesión actual (si la hay) y crea otra
    /// distinta. Nunca se reutiliza una sesión. Con el usuario ya aceptado
    /// (`Saving`) no hace nada: el cierre del protocolo es corto y acotado.
    pub(crate) fn regenerate(&mut self, net: Option<&NetworkSnapshot>, now: Instant) {
        if matches!(self.phase, Phase::Saving { .. }) {
            return;
        }
        self.finish(Phase::Closed);
        self.start_session(net, now);
    }

    /// Cancelar, Esc o cerrar la ventana: invalida la sesión y cierra. Igual
    /// que `regenerate`, no interrumpe un guardado que el usuario ya aceptó.
    pub(crate) fn cancel(&mut self) {
        if matches!(self.phase, Phase::Saving { .. }) {
            return;
        }
        self.finish(Phase::Closed);
    }

    /// Revisión periódica: caducidad, plazo de la confirmación y cambios de red.
    pub(crate) fn tick(&mut self, net: Option<&NetworkSnapshot>, now: Instant) {
        match &self.phase {
            Phase::Waiting { live, .. } => {
                if now >= live.expires_at {
                    self.finish(Phase::Ended(EndReason::Expired));
                } else if network_changed(live, net) {
                    self.finish(Phase::Ended(EndReason::NetworkChanged));
                }
            }
            Phase::Confirming { live, deadline, .. } => {
                if now >= *deadline {
                    self.finish(Phase::Ended(EndReason::ConfirmationTimeout));
                } else if network_changed(live, net) {
                    self.finish(Phase::Ended(EndReason::NetworkChanged));
                }
            }
            Phase::NoNetwork => {
                if net.is_some_and(|net| pick(net).is_some()) {
                    self.start_session(net, now);
                }
            }
            Phase::Closed | Phase::Saving { .. } | Phase::Done { .. } | Phase::Ended(_) => {}
        }
    }

    /// Un hilo trabajador pide atender una conexión que aceptó la generación
    /// `generation`. `None` si no hay una sesión esperando al celular (también
    /// con otro celular ya pendiente), si ya caducó (gana el plazo de Rust) o si
    /// la conexión es de otra generación.
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

    /// El apretón de manos de un celular terminó: queda a la espera del usuario.
    /// `false` si la sesión ya no es la vigente o caducó: el llamador cierra la
    /// conexión y nadie decide nada. `decide` recibe la decisión del usuario.
    pub(crate) fn offer_pending(
        &mut self,
        id: SessionId,
        name: &str,
        fingerprint: String,
        deadline: Instant,
        decide: Sender<UserDecision>,
        now: Instant,
    ) -> bool {
        let Phase::Waiting { live, .. } = &self.phase else {
            return false;
        };
        if live.id != id {
            return false;
        }
        if now >= live.expires_at {
            self.finish(Phase::Ended(EndReason::Expired));
            return false;
        }
        // La imagen del QR ya no hace falta: se suelta (y se borra) al reemplazar.
        let Phase::Waiting { live, .. } = std::mem::replace(&mut self.phase, Phase::Closed) else {
            return false;
        };
        self.phase = Phase::Confirming {
            live,
            name: name.to_owned(),
            fingerprint,
            deadline,
            decide,
        };
        self.mark_changed();
        true
    }

    /// El usuario acepta. Solo cuenta con la fase «confirmando» y antes del
    /// plazo, medido por Rust: una interfaz atrasada no puede aceptar tarde.
    pub(crate) fn accept(&mut self, now: Instant) {
        let Phase::Confirming { deadline, .. } = &self.phase else {
            return;
        };
        if now >= *deadline {
            self.finish(Phase::Ended(EndReason::ConfirmationTimeout));
            return;
        }
        let Phase::Confirming {
            live, name, decide, ..
        } = std::mem::replace(&mut self.phase, Phase::Closed)
        else {
            return;
        };
        if decide.send(UserDecision::Accept).is_err() {
            // El hilo de la conexión ya no está: no hay nada que confirmar.
            live.shutdown();
            self.phase = Phase::Ended(EndReason::PhoneDisconnected);
        } else {
            self.phase = Phase::Saving { live, name };
        }
        self.mark_changed();
    }

    /// El usuario rechaza: el celular recibe el rechazo.
    pub(crate) fn reject(&mut self) {
        if matches!(self.phase, Phase::Confirming { .. }) {
            self.end_with(UserDecision::Reject, Phase::Ended(EndReason::Rejected));
        }
    }

    /// El hilo de la conexión terminó `confirm()`. Solo se guarda si la sesión
    /// sigue siendo la vigente y el usuario ya aceptó (`Saving`); cualquier
    /// respuesta tardía de otra sesión se descarta sin guardar nada.
    pub(crate) fn confirmed(
        &mut self,
        id: SessionId,
        result: Result<NewDevice, PairingError>,
        _now: Instant,
    ) {
        let Phase::Saving { live, .. } = &self.phase else {
            return;
        };
        if live.id != id {
            return;
        }
        match result {
            Ok(device) => {
                let name = device.name.clone();
                if self.save(device) {
                    self.finish(Phase::Done { name });
                } else {
                    self.finish(Phase::Ended(EndReason::SaveFailed));
                }
            }
            Err(PairingError::ConfirmationTimeout) => {
                self.finish(Phase::Ended(EndReason::ConfirmationTimeout));
            }
            Err(_) => self.finish(Phase::Ended(EndReason::PhoneDisconnected)),
        }
    }

    /// Una conexión de la sesión `id` falló antes de la confirmación. El núcleo
    /// no deja consultar el estado del código, así que se falla cerrando: salvo
    /// los rechazos que dejan el código intacto, la sesión termina y el usuario
    /// genera otro. Un id que no es el vigente se ignora.
    pub(crate) fn connection_failed(&mut self, id: SessionId, error: PairingError) {
        let Phase::Waiting { live, .. } = &mut self.phase else {
            return;
        };
        if live.id != id {
            return;
        }
        let reason = match error {
            PairingError::Refused(Refusal::TooManyAttempts) => EndReason::TooManyAttempts,
            PairingError::Refused(Refusal::Expired) => EndReason::Expired,
            // Una trama 1 mala: el núcleo la cuenta y al tercer intento agota el
            // código sin avisar, así que aquí se lleva la misma cuenta.
            PairingError::Refused(Refusal::WrongCode | Refusal::Malformed) => {
                live.failed += 1;
                if live.failed < u32::from(MAX_FAILED_ATTEMPTS) {
                    return;
                }
                EndReason::TooManyAttempts
            }
            // El código sigue sirviendo (o ya lo cancelamos nosotros).
            PairingError::Refused(_) | PairingError::Cancelled => return,
            _ => EndReason::PhoneDisconnected,
        };
        self.finish(Phase::Ended(reason));
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
            Phase::Confirming {
                name,
                fingerprint,
                deadline,
                ..
            } => PairingView::Confirming {
                name: name.clone(),
                fingerprint: fingerprint.clone(),
                remaining_ms: millis_until(*deadline, now),
            },
            Phase::Saving { name, .. } => PairingView::Saving { name: name.clone() },
            Phase::Done { name } => PairingView::Done { name: name.clone() },
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

    /// La sesión viva, si la hay (esperando, confirmando o guardando).
    pub(crate) fn session_id(&self) -> Option<SessionId> {
        match &self.phase {
            Phase::Waiting { live, .. }
            | Phase::Confirming { live, .. }
            | Phase::Saving { live, .. } => Some(live.id),
            _ => None,
        }
    }

    /// El texto del último QR generado (solo en pruebas).
    #[cfg(test)]
    pub(crate) fn last_offer_text(&self) -> Option<String> {
        self.last_offer_text.clone()
    }

    /// Fija el reloj de `paired_at` (solo en pruebas).
    #[cfg(test)]
    pub(crate) fn set_clock(&mut self, clock: fn() -> i64) {
        self.clock = clock;
    }

    fn mark_changed(&mut self) {
        self.dirty = true;
        #[cfg(test)]
        {
            self.last_offer_text = None;
        }
    }

    /// Guarda al celular. `AlreadyPaired` es éxito (la PC trata una llave ya
    /// existente como actualización y conserva el nombre guardado); un candado
    /// envenenado o cualquier otro error es un fallo.
    fn save(&self, device: NewDevice) -> bool {
        let Ok(mut store) = self.paired.lock() else {
            return false;
        };
        let device = PairedDevice {
            name: device.name,
            public_key: device.public_key,
            paired_at: (self.clock)(),
        };
        matches!(
            store.add(device),
            Ok(()) | Err(PairedDevicesError::AlreadyPaired)
        )
    }

    /// Cambia de fase invalidando antes la sesión viva y descartando la imagen.
    fn finish(&mut self, next: Phase) {
        self.end_with(UserDecision::Cancel, next);
    }

    /// Como `finish`, avisando al hilo de una conexión pendiente con `decision`.
    fn end_with(&mut self, decision: UserDecision, next: Phase) {
        match std::mem::replace(&mut self.phase, next) {
            Phase::Waiting { live, .. } | Phase::Saving { live, .. } => live.shutdown(),
            Phase::Confirming { live, decide, .. } => {
                // Si el hilo ya no escucha, no hay a quién avisar. El token de la
                // conexión **no** se cancela: el hilo no está leyendo, espera la
                // decisión, y necesita escribir el rechazo al celular.
                let _ = decide.send(decision);
                live.invalidate_code();
            }
            Phase::Closed | Phase::NoNetwork | Phase::Done { .. } | Phase::Ended(_) => {}
        }
        self.mark_changed();
    }

    /// Crea una sesión nueva con la dirección elegida y deja el QR listo.
    fn start_session(&mut self, net: Option<&NetworkSnapshot>, now: Instant) {
        // Primero la identidad: si no se puede leer, ese es el problema que el
        // usuario debe ver, haya red o no.
        let identity = match self.identity.load() {
            Ok(identity) => identity,
            Err(_) => {
                self.finish(Phase::Ended(EndReason::IdentityUnavailable));
                return;
            }
        };
        let Some((net, ip)) = net.and_then(|net| pick(net).map(|ip| (net, ip))) else {
            self.finish(Phase::NoNetwork);
            return;
        };
        match self.build_session(identity, net, ip, now) {
            Ok((live, svg, _text)) => {
                self.finish(Phase::Waiting { live, svg });
                #[cfg(test)]
                {
                    self.last_offer_text = Some(_text.to_string());
                }
            }
            Err(reason) => self.finish(Phase::Ended(reason)),
        }
    }

    fn build_session(
        &mut self,
        identity: KeyPair,
        net: &NetworkSnapshot,
        ip: Ipv4Addr,
        now: Instant,
    ) -> Result<(Live, Zeroizing<String>, Zeroizing<String>), EndReason> {
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
            failed: 0,
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

fn unix_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|elapsed| i64::try_from(elapsed.as_millis()).ok())
        .unwrap_or(0)
}

/// Una segunda copia del código: la oferta la consume y la sesión del núcleo
/// necesita la suya. Los 32 bytes pasan por la pila (excepción documentada de
/// `PairingCode`: secreto de 2 minutos y un uso que además se muestra en el QR).
fn duplicate(code: &PairingCode) -> PairingCode {
    PairingCode::from_bytes(SecretBox::new(Box::new(*code.expose_bytes())))
}
