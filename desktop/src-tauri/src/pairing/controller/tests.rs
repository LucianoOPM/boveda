//! Pruebas de punta a punta: el servidor real (`ServerSupervisor`) con el
//! controlador y `pair_with_pc` como celular, por `127.0.0.1`.

use std::collections::BTreeSet;
use std::net::{Ipv4Addr, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use secrecy::{ExposeSecret, SecretBox};
use vault_core::protocol::{
    AckOutcome, CancelToken, DeviceName, KeyPair, PairedPc, PairingCode, PairingError,
    PairingOffer, PairingTimeouts,
};

use super::{LateNetworkView, Notifier, PairingController};
use crate::network::FakeProbe;
use crate::pairing::core::PairingCore;
use crate::pairing::{EndReason, IdentitySource, IdentityUnavailable, PairingView};
use crate::server::test_support::{Fixture, config, wait_until};
use crate::server::{NetworkView, PairingHandler, ServerSupervisor};

const LOCAL: Ipv4Addr = Ipv4Addr::new(127, 0, 0, 1);
const SECOND: Duration = Duration::from_secs(1);

/// La identidad de la PC de la prueba, con una copia propia por sesión.
struct PcIdentity(Arc<KeyPair>);

impl IdentitySource for PcIdentity {
    fn load(&self) -> Result<KeyPair, IdentityUnavailable> {
        Ok(KeyPair::from_private(SecretBox::new(Box::new(
            *self.0.private().expose_secret(),
        ))))
    }
}

/// Anota cada vista que el controlador notifica.
#[derive(Default)]
struct Recorder {
    views: Mutex<Vec<PairingView>>,
    closed: AtomicBool,
}

impl Notifier for Recorder {
    fn changed(&self, view: &PairingView) {
        if matches!(view, PairingView::Closed) {
            self.closed.store(true, Ordering::SeqCst);
        }
        self.views
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(view.clone());
    }
}

impl Recorder {
    /// Espera una vista que cumpla `condition`; la devuelve.
    fn wait_for(&self, condition: impl Fn(&PairingView) -> bool) -> PairingView {
        let found = wait_until(
            || self.views.lock().expect("candado").iter().any(&condition),
            SECOND * 4,
        );
        let views = self.views.lock().expect("candado");
        assert!(found, "no llegó la vista esperada; llegaron {views:?}");
        views
            .iter()
            .rev()
            .find(|v| condition(v))
            .expect("vista")
            .clone()
    }

    fn count(&self, condition: impl Fn(&PairingView) -> bool) -> usize {
        self.views
            .lock()
            .expect("candado")
            .iter()
            .filter(|v| condition(v))
            .count()
    }
}

struct Harness {
    fixture: Fixture,
    controller: Arc<PairingController>,
    recorder: Arc<Recorder>,
    probe: FakeProbe,
    supervisor: Arc<ServerSupervisor>,
}

fn timeouts(confirmation: Duration) -> PairingTimeouts {
    PairingTimeouts {
        step: Duration::from_secs(2),
        handshake: Duration::from_secs(5),
        confirmation,
        reply_wait: Duration::from_secs(5),
    }
}

fn harness(confirmation: Duration) -> Harness {
    let fixture = Fixture::new();
    let recorder = Arc::new(Recorder::default());
    let late = Arc::new(LateNetworkView::default());
    let core = PairingCore::new(
        Arc::new(PcIdentity(Arc::clone(&fixture.identity))),
        Arc::clone(&fixture.paired),
    );
    let controller = Arc::new(PairingController::start(
        core,
        Arc::clone(&late) as Arc<dyn NetworkView>,
        Arc::clone(&recorder) as Arc<dyn Notifier>,
        timeouts(confirmation),
        Duration::from_millis(20),
    ));
    let mut deps = fixture.deps();
    deps.pairing = Arc::clone(&controller) as Arc<dyn PairingHandler>;
    let probe = FakeProbe::new();
    probe.set(Ok(BTreeSet::from([LOCAL])));
    let supervisor = Arc::new(ServerSupervisor::start(probe.clone(), deps, config()));
    late.set(Arc::clone(&supervisor) as Arc<dyn NetworkView>);
    assert!(wait_until(|| supervisor.snapshot().is_some(), SECOND * 3));
    Harness {
        fixture,
        controller,
        recorder,
        probe,
        supervisor,
    }
}

fn unbase64url(text: &str) -> Vec<u8> {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let (mut out, mut buffer, mut bits) = (Vec::new(), 0u32, 0u32);
    for ch in text.bytes() {
        let value = ALPHABET.iter().position(|c| *c == ch).expect("base64url") as u32;
        buffer = (buffer << 6) | value;
        bits += 6;
        if bits >= 8 {
            out.push(((buffer >> (bits - 8)) & 0xff) as u8);
            bits -= 8;
        }
    }
    out
}

impl Harness {
    /// Abre la sesión y devuelve la oferta del QR (decodificada del texto).
    fn open(&self) -> PairingOffer {
        self.controller.open();
        self.recorder
            .wait_for(|v| matches!(v, PairingView::Waiting { .. }));
        self.current_offer()
    }

    fn current_offer(&self) -> PairingOffer {
        let text = self
            .controller
            .inner
            .core
            .lock()
            .expect("candado")
            .last_offer_text()
            .expect("texto del QR");
        PairingOffer::decode(&unbase64url(&text)).expect("oferta")
    }

    /// El celular empieza el emparejamiento con esa oferta en otro hilo.
    fn phone(&self, offer: PairingOffer, name: &str) -> JoinHandle<Result<PairedPc, PairingError>> {
        let name = name.to_owned();
        thread::spawn(move || {
            let identity = KeyPair::generate().expect("llave del celular");
            let stream = TcpStream::connect_timeout(&offer.address(), SECOND * 2)
                .map_err(|error| PairingError::Io(error.kind()))?;
            pair_with_pc_as(stream, &identity, &offer, &name)
        })
    }

    fn paired_count(&self) -> usize {
        self.fixture.paired.lock().expect("candado").list().len()
    }
}

fn pair_with_pc_as(
    stream: TcpStream,
    identity: &KeyPair,
    offer: &PairingOffer,
    name: &str,
) -> Result<PairedPc, PairingError> {
    vault_core::protocol::pair_with_pc(
        stream,
        identity,
        offer,
        DeviceName::new(name).expect("nombre"),
        &CancelToken::new(),
        &timeouts(Duration::from_secs(2)),
    )
}

fn is_confirming(view: &PairingView) -> bool {
    matches!(view, PairingView::Confirming { .. })
}

fn is_ended(reason: EndReason) -> impl Fn(&PairingView) -> bool {
    move |view| matches!(view, PairingView::Ended { reason: r, .. } if *r == reason)
}

impl Drop for Harness {
    fn drop(&mut self) {
        self.controller.shutdown();
        self.supervisor.stop();
    }
}

#[test]
fn right_code_and_accept_pairs_and_saves() {
    let h = harness(Duration::from_secs(2));
    let before = h.paired_count();
    let offer = h.open();
    let pc_key = offer.public_key();
    let phone = h.phone(offer, "Pixel de Ana");
    let view = h.recorder.wait_for(is_confirming);
    match view {
        PairingView::Confirming {
            name, fingerprint, ..
        } => {
            assert_eq!(name, "Pixel de Ana");
            assert_eq!(fingerprint.len(), "A1B2-C3D4-E5F6-0718".len());
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(h.paired_count(), before, "nada se guarda sin confirmar");
    h.controller.accept();
    h.recorder
        .wait_for(|v| matches!(v, PairingView::Done { .. }));
    let paired = phone
        .join()
        .expect("hilo")
        .expect("el celular debe emparejarse");
    assert_eq!(paired.public_key(), pc_key);
    assert_eq!(h.paired_count(), before + 1);
    assert!(
        h.recorder
            .count(|v| matches!(v, PairingView::Saving { .. }))
            >= 1
    );
}

#[test]
fn phone_without_the_code_cannot_pair_and_nothing_is_saved() {
    let h = harness(Duration::from_secs(2));
    let before = h.paired_count();
    let real = h.open();
    // Misma PC y dirección, pero otro código: el celular no escaneó el QR.
    let forged = PairingOffer::new(
        real.public_key(),
        real.address(),
        PairingCode::generate().expect("código"),
    )
    .expect("oferta falsa");
    let result = h.phone(forged, "Intruso").join().expect("hilo");
    assert!(result.is_err());
    assert_eq!(h.paired_count(), before);
    assert_eq!(
        h.recorder.count(is_confirming),
        0,
        "ni siquiera hay diálogo"
    );
    // Con el código correcto la sesión sigue sirviendo.
    assert!(matches!(
        h.controller.snapshot(),
        PairingView::Waiting { .. }
    ));
}

#[test]
fn three_wrong_codes_end_the_session() {
    let h = harness(Duration::from_secs(2));
    let real = h.open();
    for _ in 0..3 {
        let forged = PairingOffer::new(
            real.public_key(),
            real.address(),
            PairingCode::generate().expect("código"),
        )
        .expect("oferta falsa");
        assert!(h.phone(forged, "Intruso").join().expect("hilo").is_err());
    }
    h.recorder.wait_for(is_ended(EndReason::TooManyAttempts));
    assert_eq!(h.recorder.count(is_confirming), 0);
}

#[test]
fn reject_does_not_save_and_phone_sees_rejected() {
    let h = harness(Duration::from_secs(2));
    let before = h.paired_count();
    let phone = h.phone(h.open(), "Pixel de Ana");
    h.recorder.wait_for(is_confirming);
    h.controller.reject();
    h.recorder.wait_for(is_ended(EndReason::Rejected));
    assert_eq!(
        phone.join().expect("hilo").err(),
        Some(PairingError::Rejected)
    );
    assert_eq!(h.paired_count(), before);
}

#[test]
fn confirmation_timeout_does_not_save() {
    let h = harness(Duration::from_millis(400));
    let before = h.paired_count();
    let phone = h.phone(h.open(), "Pixel de Ana");
    h.recorder.wait_for(is_confirming);
    h.recorder
        .wait_for(is_ended(EndReason::ConfirmationTimeout));
    assert!(phone.join().expect("hilo").is_err());
    assert_eq!(h.paired_count(), before);
}

#[test]
fn closing_the_session_during_confirmation_does_not_save() {
    let h = harness(Duration::from_secs(2));
    let before = h.paired_count();
    let phone = h.phone(h.open(), "Pixel de Ana");
    h.recorder.wait_for(is_confirming);
    h.controller.cancel();
    assert!(phone.join().expect("hilo").is_err());
    assert_eq!(h.controller.snapshot(), PairingView::Closed);
    assert_eq!(h.paired_count(), before);
}

#[test]
fn reusing_the_code_after_a_pairing_is_refused() {
    let h = harness(Duration::from_secs(2));
    let offer = h.open();
    let phone = h.phone(
        PairingOffer::decode(&offer.encode().expect("bytes")).expect("copia"),
        "Pixel de Ana",
    );
    h.recorder.wait_for(is_confirming);
    h.controller.accept();
    phone.join().expect("hilo").expect("emparejado");
    let after_first = h.paired_count();
    let again = h.phone(offer, "Otro celular").join().expect("hilo");
    assert!(again.is_err());
    assert_eq!(h.paired_count(), after_first);
}

#[test]
fn regenerate_invalidates_the_previous_code_for_a_phone_that_arrives_late() {
    let h = harness(Duration::from_secs(2));
    let before = h.paired_count();
    let old = h.open();
    h.controller.regenerate();
    let new = h.current_offer();
    assert_ne!(old.code().expose_bytes(), new.code().expose_bytes());
    assert!(h.phone(old, "Tarde").join().expect("hilo").is_err());
    assert_eq!(h.paired_count(), before);
    assert_eq!(h.recorder.count(is_confirming), 0);
    // El código nuevo sí sirve.
    let phone = h.phone(new, "Pixel de Ana");
    h.recorder.wait_for(is_confirming);
    h.controller.reject();
    assert!(phone.join().expect("hilo").is_err());
}

#[test]
fn send_still_works_on_the_same_port_while_a_session_is_open() {
    let h = harness(Duration::from_secs(2));
    h.open();
    let port = h.supervisor.local_port().expect("puerto");
    let address = std::net::SocketAddr::from((LOCAL, port));
    assert_eq!(h.fixture.send(address), Ok(AckOutcome::Copied));
}

#[test]
fn a_network_change_ends_the_session_and_the_old_code_stops_working() {
    let h = harness(Duration::from_secs(2));
    let offer = h.open();
    h.probe.set(Ok(BTreeSet::new()));
    h.recorder.wait_for(is_ended(EndReason::NetworkChanged));
    assert!(h.phone(offer, "Tarde").join().expect("hilo").is_err());
}

#[test]
fn shutdown_cancels_the_session_and_is_safe_to_repeat() {
    let h = harness(Duration::from_secs(2));
    h.open();
    h.controller.shutdown();
    h.controller.shutdown();
    assert_eq!(h.controller.snapshot(), PairingView::Closed);
}
