use std::net::{Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use secrecy::SecretBox;
use vault_core::protocol::{KeyPair, PairingError, PairingOffer, Refusal};

use super::core::{NewDevice, PairingCore, SessionId, Ticket, UserDecision};
use super::{EndReason, IdentitySource, IdentityUnavailable, PairingView};
use crate::network::Candidate;
use crate::paired_devices::PairedDeviceStore;
use crate::qr::{PLACEHOLDER_SVG, data_uri};
use crate::server::NetworkSnapshot;

const IP_A: Ipv4Addr = Ipv4Addr::new(192, 168, 1, 20);
const IP_B: Ipv4Addr = Ipv4Addr::new(10, 0, 0, 8);
const TTL: Duration = Duration::from_secs(120);

/// Identidad fija de la PC de prueba: siempre los mismos 32 bytes.
struct FakeIdentity {
    fail: AtomicBool,
}

impl IdentitySource for FakeIdentity {
    fn load(&self) -> Result<KeyPair, IdentityUnavailable> {
        if self.fail.load(Ordering::SeqCst) {
            return Err(IdentityUnavailable);
        }
        Ok(KeyPair::from_private(SecretBox::new(Box::new([5u8; 32]))))
    }
}

fn candidate(ip: Ipv4Addr, has_gateway: bool, metric: u32) -> Candidate {
    Candidate {
        ip,
        eligible: true,
        has_gateway,
        metric,
    }
}

/// Instantánea con las dos IP; la preferida es `IP_A` (con puerta de enlace).
fn net(generation: u64, port: u16) -> NetworkSnapshot {
    NetworkSnapshot {
        generation,
        port,
        candidates: vec![candidate(IP_A, true, 10), candidate(IP_B, false, 50)],
    }
}

struct Rig {
    core: PairingCore,
    identity: Arc<FakeIdentity>,
    paired: Arc<Mutex<PairedDeviceStore>>,
    _dir: tempfile::TempDir,
}

fn rig() -> Rig {
    let dir = tempfile::tempdir().expect("carpeta temporal");
    let paired = Arc::new(Mutex::new(
        PairedDeviceStore::open(dir.path().join("paired-devices.bin")).expect("lista"),
    ));
    let identity = Arc::new(FakeIdentity {
        fail: AtomicBool::new(false),
    });
    let core = PairingCore::new(identity.clone(), paired.clone());
    Rig {
        core,
        identity,
        paired,
        _dir: dir,
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

fn ended(view: PairingView) -> EndReason {
    match view {
        PairingView::Ended { reason, .. } => reason,
        other => panic!("se esperaba Ended y es {other:?}"),
    }
}

#[test]
fn open_without_network_shows_no_network_and_no_session() {
    let mut rig = rig();
    let t0 = Instant::now();
    assert!(rig.core.open(None, t0));
    assert_eq!(rig.core.view(t0), PairingView::NoNetwork);
    assert_eq!(rig.core.session_id(), None);
}

#[test]
fn open_with_only_ineligible_interfaces_shows_no_network() {
    let mut rig = rig();
    let mut snapshot = net(1, 40000);
    for candidate in &mut snapshot.candidates {
        candidate.eligible = false;
    }
    let t0 = Instant::now();
    rig.core.open(Some(&snapshot), t0);
    assert_eq!(rig.core.view(t0), PairingView::NoNetwork);
}

#[test]
fn open_with_network_creates_one_session_with_a_qr() {
    let mut rig = rig();
    let t0 = Instant::now();
    assert!(rig.core.open(Some(&net(1, 40000)), t0));
    match rig.core.view(t0) {
        PairingView::Waiting { qr, .. } => {
            assert!(qr.starts_with("data:image/svg+xml;utf8,"));
        }
        other => panic!("{other:?}"),
    }
    assert!(rig.core.session_id().is_some());
}

#[test]
fn open_twice_keeps_the_same_session() {
    let mut rig = rig();
    let t0 = Instant::now();
    rig.core.open(Some(&net(1, 40000)), t0);
    let id = rig.core.session_id();
    let first = rig.core.view(t0);
    assert!(
        !rig.core
            .open(Some(&net(1, 40000)), t0 + Duration::from_secs(5))
    );
    assert_eq!(rig.core.session_id(), id);
    assert_eq!(rig.core.view(t0), first, "mismo QR: no se creó otra sesión");
}

#[test]
fn qr_encodes_the_chosen_address_port_and_identity() {
    let mut rig = rig();
    rig.core.open(Some(&net(1, 40000)), Instant::now());
    let text = rig.core.last_offer_text().expect("texto del QR");
    let offer = PairingOffer::decode(&unbase64url(&text)).expect("oferta");
    assert_eq!(offer.address(), SocketAddr::from((IP_A, 40000)));
    let expected = KeyPair::from_private(SecretBox::new(Box::new([5u8; 32]))).public();
    assert_eq!(offer.public_key(), expected);
}

#[test]
fn remaining_counts_from_creation_with_the_given_now() {
    let mut rig = rig();
    let t0 = Instant::now();
    rig.core.open(Some(&net(1, 40000)), t0);
    let remaining = |at| match rig.core.view(at) {
        PairingView::Waiting { remaining_ms, .. } => remaining_ms,
        other => panic!("{other:?}"),
    };
    assert_eq!(remaining(t0), 120_000);
    assert_eq!(remaining(t0 + Duration::from_secs(30)), 90_000);
    assert_eq!(remaining(t0 + TTL), 0);
}

#[test]
fn expiry_ends_the_session_and_drops_the_image() {
    let mut rig = rig();
    let t0 = Instant::now();
    rig.core.open(Some(&net(1, 40000)), t0);
    let ticket = rig.core.begin_connection(1, t0).expect("conexión válida");
    rig.core.tick(Some(&net(1, 40000)), t0 + TTL);
    let view = rig.core.view(t0 + TTL);
    assert_eq!(
        view,
        PairingView::Ended {
            reason: EndReason::Expired,
            placeholder: data_uri(PLACEHOLDER_SVG),
        }
    );
    // El código ya no sirve: el token se canceló y la sesión del núcleo también.
    assert!(ticket.cancel.is_cancelled());
    let refusal = ticket
        .session
        .lock()
        .expect("candado")
        .accept_first(t0 + TTL, &[0u8; 48]);
    assert!(matches!(
        refusal,
        Err(Refusal::Cancelled | Refusal::Expired)
    ));
    assert_eq!(rig.core.session_id(), None);
    assert_eq!(
        rig.core.last_offer_text(),
        None,
        "el texto del QR se descarta"
    );
    assert!(!format!("{view:?}").contains("data:image"));
}

#[test]
fn big_time_jump_expires_at_once() {
    let mut rig = rig();
    let t0 = Instant::now();
    rig.core.open(Some(&net(1, 40000)), t0);
    let later = t0 + Duration::from_secs(600);
    rig.core.tick(Some(&net(1, 40000)), later);
    assert_eq!(ended(rig.core.view(later)), EndReason::Expired);
}

#[test]
fn regenerate_creates_a_new_session_and_never_reuses_the_old_one() {
    let mut rig = rig();
    let t0 = Instant::now();
    rig.core.open(Some(&net(1, 40000)), t0);
    let old_id = rig.core.session_id().expect("id");
    let old_text = rig.core.last_offer_text();
    let old_ticket = rig.core.begin_connection(1, t0).expect("ticket");
    rig.core
        .regenerate(Some(&net(1, 40000)), t0 + Duration::from_secs(1));
    assert_ne!(rig.core.session_id(), Some(old_id));
    assert_ne!(rig.core.last_offer_text(), old_text, "otro código, otro QR");
    assert!(old_ticket.cancel.is_cancelled());
    assert!(matches!(
        old_ticket
            .session
            .lock()
            .expect("candado")
            .accept_first(t0, &[0u8; 48]),
        Err(Refusal::Cancelled)
    ));
}

#[test]
fn regenerate_after_expiry_starts_a_fresh_session() {
    let mut rig = rig();
    let t0 = Instant::now();
    rig.core.open(Some(&net(1, 40000)), t0);
    rig.core.tick(Some(&net(1, 40000)), t0 + TTL);
    rig.core.regenerate(Some(&net(1, 40000)), t0 + TTL);
    assert!(matches!(
        rig.core.view(t0 + TTL),
        PairingView::Waiting { .. }
    ));
}

#[test]
fn cancel_closes_and_drops_the_image() {
    let mut rig = rig();
    let t0 = Instant::now();
    rig.core.open(Some(&net(1, 40000)), t0);
    let ticket = rig.core.begin_connection(1, t0).expect("ticket");
    rig.core.cancel();
    assert_eq!(rig.core.view(t0), PairingView::Closed);
    assert!(ticket.cancel.is_cancelled());
    assert_eq!(rig.core.last_offer_text(), None);
}

#[test]
fn identity_unreadable_ends_with_identity_unavailable_and_no_session() {
    let mut rig = rig();
    rig.identity.fail.store(true, Ordering::SeqCst);
    let t0 = Instant::now();
    rig.core.open(Some(&net(1, 40000)), t0);
    assert_eq!(ended(rig.core.view(t0)), EndReason::IdentityUnavailable);
    assert_eq!(rig.core.session_id(), None);
}

#[test]
fn network_changes_end_the_session() {
    let cases: Vec<(&str, Option<NetworkSnapshot>)> = vec![
        ("otra generación", Some(net(2, 40000))),
        ("otro puerto", Some(net(1, 40001))),
        (
            "la dirección ya no se escucha",
            Some(NetworkSnapshot {
                generation: 1,
                port: 40000,
                candidates: vec![candidate(IP_B, true, 5)],
            }),
        ),
        ("sin servidor", None),
    ];
    for (label, changed) in cases {
        let mut rig = rig();
        let t0 = Instant::now();
        rig.core.open(Some(&net(1, 40000)), t0);
        rig.core.tick(changed.as_ref(), t0 + Duration::from_secs(1));
        assert_eq!(
            ended(rig.core.view(t0)),
            EndReason::NetworkChanged,
            "{label}"
        );
    }
}

#[test]
fn a_metric_change_that_keeps_the_address_listening_does_not_end_the_session() {
    let mut rig = rig();
    let t0 = Instant::now();
    rig.core.open(Some(&net(1, 40000)), t0);
    let mut same = net(1, 40000);
    same.candidates = vec![candidate(IP_A, true, 99), candidate(IP_B, true, 1)];
    rig.core.tick(Some(&same), t0 + Duration::from_secs(1));
    assert!(matches!(rig.core.view(t0), PairingView::Waiting { .. }));
}

#[test]
fn no_network_becomes_a_session_when_a_network_appears() {
    let mut rig = rig();
    let t0 = Instant::now();
    rig.core.open(None, t0);
    let later = t0 + Duration::from_secs(2);
    rig.core.tick(Some(&net(1, 40000)), later);
    assert!(matches!(rig.core.view(later), PairingView::Waiting { .. }));
}

#[test]
fn changed_reports_each_change_once() {
    let mut rig = rig();
    let t0 = Instant::now();
    assert!(!rig.core.changed());
    rig.core.open(Some(&net(1, 40000)), t0);
    assert!(rig.core.changed());
    assert!(!rig.core.changed());
    rig.core
        .tick(Some(&net(1, 40000)), t0 + Duration::from_secs(1));
    assert!(!rig.core.changed(), "un tick sin cambios no notifica");
    rig.core.tick(Some(&net(1, 40000)), t0 + TTL);
    assert!(rig.core.changed());
}

#[test]
fn commands_without_a_session_are_noops() {
    let mut rig = rig();
    let t0 = Instant::now();
    rig.core.cancel();
    assert_eq!(rig.core.view(t0), PairingView::Closed);
    assert!(rig.core.begin_connection(1, t0).is_none());
}

#[test]
fn view_debug_never_prints_the_image_names_or_fingerprints() {
    let views = [
        PairingView::Waiting {
            qr: "data:image/svg+xml;utf8,SECRETO".to_owned(),
            remaining_ms: 5,
        },
        PairingView::Confirming {
            name: "Pixel de Ana".to_owned(),
            fingerprint: "A1B2-C3D4-E5F6-0718".to_owned(),
            remaining_ms: 5,
        },
        PairingView::Saving {
            name: "Pixel de Ana".to_owned(),
        },
        PairingView::Done {
            name: "Pixel de Ana".to_owned(),
        },
        PairingView::Ended {
            reason: EndReason::Expired,
            placeholder: "data:image/svg+xml;utf8,SECRETO".to_owned(),
        },
        PairingView::NoNetwork,
        PairingView::Closed,
    ];
    for view in views {
        let text = format!("{view:?}");
        for forbidden in ["SECRETO", "Ana", "A1B2", "data:"] {
            assert!(!text.contains(forbidden), "{text} contiene {forbidden}");
        }
    }
}

#[test]
fn ticket_carries_the_id_of_the_live_session() {
    let mut rig = rig();
    let t0 = Instant::now();
    rig.core.open(Some(&net(1, 40000)), t0);
    let ticket = rig.core.begin_connection(1, t0).expect("ticket");
    assert_eq!(Some(ticket.id), rig.core.session_id());
    assert!(
        rig.core.begin_connection(2, t0).is_none(),
        "otra generación"
    );
}

// ---- Conexión, confirmación, guardado y carreras ---------------------------

const CONFIRMATION: Duration = Duration::from_secs(60);
const NAME: &str = "Pixel de Ana";
const FINGERPRINT: &str = "A1B2-C3D4-E5F6-0718";

fn device(seed: u8, name: &str) -> NewDevice {
    NewDevice {
        name: name.to_owned(),
        public_key: [seed; 32],
    }
}

/// Una sesión abierta con un celular esperando la decisión del usuario.
struct Pending {
    id: SessionId,
    ticket: Ticket,
    decisions: Receiver<UserDecision>,
    t0: Instant,
}

fn pending(rig: &mut Rig) -> Pending {
    let t0 = Instant::now();
    rig.core.set_clock(|| 1234);
    rig.core.open(Some(&net(1, 40000)), t0);
    let ticket = rig.core.begin_connection(1, t0).expect("ticket");
    let (tx, decisions) = channel();
    assert!(rig.core.offer_pending(
        ticket.id,
        NAME,
        FINGERPRINT.to_owned(),
        t0 + CONFIRMATION,
        tx,
        t0
    ));
    Pending {
        id: ticket.id,
        ticket,
        decisions,
        t0,
    }
}

fn list_len(rig: &Rig) -> usize {
    rig.paired.lock().expect("candado").list().len()
}

fn next_decision(pending: &Pending) -> Option<UserDecision> {
    pending.decisions.try_recv().ok()
}

#[test]
fn pending_shows_name_and_fingerprint_with_the_remaining_time() {
    let mut rig = rig();
    let p = pending(&mut rig);
    assert_eq!(
        rig.core.view(p.t0 + Duration::from_secs(10)),
        PairingView::Confirming {
            name: NAME.to_owned(),
            fingerprint: FINGERPRINT.to_owned(),
            remaining_ms: 50_000,
        }
    );
}

#[test]
fn no_ticket_after_expiry_and_the_view_says_expired() {
    let mut rig = rig();
    let t0 = Instant::now();
    rig.core.open(Some(&net(1, 40000)), t0);
    assert!(rig.core.begin_connection(1, t0 + TTL).is_none());
    assert_eq!(ended(rig.core.view(t0 + TTL)), EndReason::Expired);
}

#[test]
fn accept_sends_the_decision_and_moves_to_saving() {
    let mut rig = rig();
    let p = pending(&mut rig);
    rig.core.accept(p.t0 + Duration::from_secs(5));
    assert_eq!(next_decision(&p), Some(UserDecision::Accept));
    assert_eq!(
        rig.core.view(p.t0),
        PairingView::Saving {
            name: NAME.to_owned()
        }
    );
    assert_eq!(list_len(&rig), 0, "todavía no se guarda nada");
}

#[test]
fn accept_after_the_deadline_is_ignored_even_if_the_view_still_says_confirming() {
    let mut rig = rig();
    let p = pending(&mut rig);
    let almost = p.t0 + CONFIRMATION - Duration::from_millis(1);
    assert!(matches!(
        rig.core.view(almost),
        PairingView::Confirming { .. }
    ));
    rig.core
        .accept(p.t0 + CONFIRMATION + Duration::from_millis(1));
    assert_eq!(next_decision(&p), Some(UserDecision::Cancel));
    assert_eq!(ended(rig.core.view(p.t0)), EndReason::ConfirmationTimeout);
    assert_eq!(list_len(&rig), 0);
}

#[test]
fn reject_sends_reject_and_ends_with_rejected() {
    let mut rig = rig();
    let p = pending(&mut rig);
    rig.core.reject();
    assert_eq!(next_decision(&p), Some(UserDecision::Reject));
    assert_eq!(ended(rig.core.view(p.t0)), EndReason::Rejected);
    assert_eq!(list_len(&rig), 0);
}

#[test]
fn confirmation_timeout_ends_the_session_and_sends_cancel() {
    let mut rig = rig();
    let p = pending(&mut rig);
    rig.core.tick(Some(&net(1, 40000)), p.t0 + CONFIRMATION);
    assert_eq!(next_decision(&p), Some(UserDecision::Cancel));
    assert_eq!(ended(rig.core.view(p.t0)), EndReason::ConfirmationTimeout);
}

#[test]
fn saves_only_after_confirmed_ok_and_with_paired_at_in_ms() {
    let mut rig = rig();
    let p = pending(&mut rig);
    rig.core.accept(p.t0);
    assert_eq!(list_len(&rig), 0);
    rig.core.confirmed(p.id, Ok(device(9, NAME)), p.t0);
    assert_eq!(
        rig.core.view(p.t0),
        PairingView::Done {
            name: NAME.to_owned()
        }
    );
    let store = rig.paired.lock().expect("candado");
    assert_eq!(store.list().len(), 1);
    assert_eq!(store.list()[0].name, NAME);
    assert_eq!(store.list()[0].public_key, [9u8; 32]);
    assert_eq!(store.list()[0].paired_at, 1234);
    assert!(
        p.ticket.cancel.is_cancelled(),
        "la sesión terminada se cancela"
    );
}

#[test]
fn already_paired_is_success_and_keeps_the_existing_name_without_duplicates() {
    let mut rig = rig();
    rig.paired
        .lock()
        .expect("candado")
        .add(crate::paired_devices::PairedDevice {
            name: "Viejo".to_owned(),
            public_key: [9u8; 32],
            paired_at: 1,
        })
        .expect("emparejar antes");
    let p = pending(&mut rig);
    rig.core.accept(p.t0);
    rig.core.confirmed(p.id, Ok(device(9, "Nuevo")), p.t0);
    assert!(matches!(rig.core.view(p.t0), PairingView::Done { .. }));
    let store = rig.paired.lock().expect("candado");
    assert_eq!(store.list().len(), 1);
    assert_eq!(store.list()[0].name, "Viejo");
}

#[test]
fn a_full_list_ends_with_save_failed_and_stays_unchanged() {
    let mut rig = rig();
    {
        let mut store = rig.paired.lock().expect("candado");
        for seed in 0..64u8 {
            store
                .add(crate::paired_devices::PairedDevice {
                    name: format!("Celular {seed}"),
                    public_key: [seed; 32],
                    paired_at: 1,
                })
                .expect("llenar la lista");
        }
    }
    let p = pending(&mut rig);
    rig.core.accept(p.t0);
    rig.core.confirmed(p.id, Ok(device(200, NAME)), p.t0);
    assert_eq!(ended(rig.core.view(p.t0)), EndReason::SaveFailed);
    assert_eq!(list_len(&rig), 64);
}

#[test]
fn a_poisoned_list_ends_with_save_failed() {
    let mut rig = rig();
    let poisoned = rig.paired.clone();
    let _ = std::thread::spawn(move || {
        let _guard = poisoned.lock().expect("candado");
        panic!("envenenar el candado");
    })
    .join();
    let p = pending(&mut rig);
    rig.core.accept(p.t0);
    rig.core.confirmed(p.id, Ok(device(9, NAME)), p.t0);
    assert_eq!(ended(rig.core.view(p.t0)), EndReason::SaveFailed);
}

#[test]
fn a_failed_confirm_saves_nothing() {
    for (error, reason) in [
        (PairingError::Closed, EndReason::PhoneDisconnected),
        (PairingError::Timeout, EndReason::PhoneDisconnected),
        (
            PairingError::ConfirmationTimeout,
            EndReason::ConfirmationTimeout,
        ),
    ] {
        let mut rig = rig();
        let p = pending(&mut rig);
        rig.core.accept(p.t0);
        rig.core.confirmed(p.id, Err(error), p.t0);
        assert_eq!(ended(rig.core.view(p.t0)), reason);
        assert_eq!(list_len(&rig), 0);
    }
}

#[test]
fn confirmed_without_the_user_having_accepted_is_ignored() {
    let mut rig = rig();
    let p = pending(&mut rig);
    rig.core.confirmed(p.id, Ok(device(9, NAME)), p.t0);
    assert!(matches!(
        rig.core.view(p.t0),
        PairingView::Confirming { .. }
    ));
    assert_eq!(list_len(&rig), 0);
}

#[test]
fn connection_arrives_at_the_expiry_instant_no_dialog_no_save() {
    let mut rig = rig();
    let t0 = Instant::now();
    rig.core.open(Some(&net(1, 40000)), t0);
    let ticket = rig.core.begin_connection(1, t0).expect("ticket");
    let (tx, decisions) = channel();
    assert!(!rig.core.offer_pending(
        ticket.id,
        NAME,
        FINGERPRINT.to_owned(),
        t0 + TTL + CONFIRMATION,
        tx,
        t0 + TTL
    ));
    assert_eq!(ended(rig.core.view(t0 + TTL)), EndReason::Expired);
    assert_eq!(list_len(&rig), 0);
    assert!(
        decisions.try_recv().is_err(),
        "nadie decide sobre una sesión caducada"
    );
}

#[test]
fn second_connection_with_the_right_code_while_one_is_pending_does_not_replace_it() {
    let mut rig = rig();
    let p = pending(&mut rig);
    assert!(rig.core.begin_connection(1, p.t0).is_none());
    assert!(matches!(
        rig.core.view(p.t0),
        PairingView::Confirming { .. }
    ));
    assert_eq!(next_decision(&p), None);
}

#[test]
fn cancel_during_confirmation_sends_cancel_and_saves_nothing() {
    let mut rig = rig();
    let p = pending(&mut rig);
    rig.core.cancel();
    assert_eq!(next_decision(&p), Some(UserDecision::Cancel));
    assert_eq!(rig.core.view(p.t0), PairingView::Closed);
    assert_eq!(list_len(&rig), 0);
    assert!(p.ticket.cancel.is_cancelled());
}

#[test]
fn cancel_and_regenerate_while_saving_are_ignored_and_the_save_completes() {
    let mut rig = rig();
    let p = pending(&mut rig);
    rig.core.accept(p.t0);
    rig.core.cancel();
    rig.core.regenerate(Some(&net(1, 40000)), p.t0);
    assert!(matches!(rig.core.view(p.t0), PairingView::Saving { .. }));
    rig.core.confirmed(p.id, Ok(device(9, NAME)), p.t0);
    assert!(matches!(rig.core.view(p.t0), PairingView::Done { .. }));
    assert_eq!(list_len(&rig), 1);
}

#[test]
fn network_change_during_confirmation_cancels_and_saves_nothing() {
    let mut rig = rig();
    let p = pending(&mut rig);
    rig.core
        .tick(Some(&net(2, 40000)), p.t0 + Duration::from_secs(1));
    assert_eq!(next_decision(&p), Some(UserDecision::Cancel));
    assert_eq!(ended(rig.core.view(p.t0)), EndReason::NetworkChanged);
    assert_eq!(list_len(&rig), 0);
}

#[test]
fn regenerate_with_a_pending_connection_of_the_old_session_cancels_it() {
    let mut rig = rig();
    let p = pending(&mut rig);
    rig.core.regenerate(Some(&net(1, 40000)), p.t0);
    assert_eq!(next_decision(&p), Some(UserDecision::Cancel));
    assert!(matches!(rig.core.view(p.t0), PairingView::Waiting { .. }));
    assert_ne!(rig.core.session_id(), Some(p.id));
}

#[test]
fn late_confirmed_of_an_old_session_never_saves_or_changes_the_new_session() {
    let mut rig = rig();
    let p = pending(&mut rig);
    rig.core.regenerate(Some(&net(1, 40000)), p.t0);
    let before = rig.core.view(p.t0);
    rig.core.confirmed(p.id, Ok(device(9, NAME)), p.t0);
    assert_eq!(rig.core.view(p.t0), before);
    assert_eq!(list_len(&rig), 0);
}

#[test]
fn late_offer_pending_of_an_old_session_returns_false() {
    let mut rig = rig();
    let t0 = Instant::now();
    rig.core.open(Some(&net(1, 40000)), t0);
    let old = rig.core.begin_connection(1, t0).expect("ticket");
    rig.core.regenerate(Some(&net(1, 40000)), t0);
    let before = rig.core.view(t0);
    let (tx, _rx) = channel();
    assert!(!rig.core.offer_pending(
        old.id,
        NAME,
        FINGERPRINT.to_owned(),
        t0 + CONFIRMATION,
        tx,
        t0
    ));
    assert_eq!(rig.core.view(t0), before);
}

#[test]
fn accept_arriving_after_regenerate_is_ignored_by_the_new_session() {
    let mut rig = rig();
    let p = pending(&mut rig);
    rig.core.regenerate(Some(&net(1, 40000)), p.t0);
    let before = rig.core.view(p.t0);
    rig.core.accept(p.t0);
    rig.core.reject();
    assert_eq!(rig.core.view(p.t0), before);
    assert_eq!(next_decision(&p), Some(UserDecision::Cancel));
    assert_eq!(
        next_decision(&p),
        None,
        "ni Accept ni Reject llegan al hilo viejo"
    );
}

#[test]
fn stale_ids_and_commands_without_a_session_are_noops() {
    let mut rig = rig();
    let p = pending(&mut rig);
    rig.core.cancel();
    rig.core.accept(p.t0);
    rig.core.reject();
    rig.core.confirmed(p.id, Ok(device(9, NAME)), p.t0);
    rig.core.connection_failed(p.id, PairingError::Closed);
    assert_eq!(rig.core.view(p.t0), PairingView::Closed);
    assert_eq!(list_len(&rig), 0);
}

#[test]
fn connection_failures_only_end_the_session_when_the_code_may_be_gone() {
    let keeps = [
        PairingError::Refused(Refusal::WrongCode),
        PairingError::Refused(Refusal::Malformed),
        PairingError::Refused(Refusal::AlreadyUsed),
        PairingError::Refused(Refusal::Cancelled),
        PairingError::Cancelled,
    ];
    for error in keeps {
        let mut rig = rig();
        let t0 = Instant::now();
        rig.core.open(Some(&net(1, 40000)), t0);
        let ticket = rig.core.begin_connection(1, t0).expect("ticket");
        rig.core.connection_failed(ticket.id, error);
        assert!(
            matches!(rig.core.view(t0), PairingView::Waiting { .. }),
            "{error:?} no debe terminar la sesión"
        );
    }
    let ends = [
        (
            PairingError::Refused(Refusal::TooManyAttempts),
            EndReason::TooManyAttempts,
        ),
        (PairingError::Refused(Refusal::Expired), EndReason::Expired),
        (PairingError::Closed, EndReason::PhoneDisconnected),
        (PairingError::Handshake, EndReason::PhoneDisconnected),
        (PairingError::Timeout, EndReason::PhoneDisconnected),
    ];
    for (error, reason) in ends {
        let mut rig = rig();
        let t0 = Instant::now();
        rig.core.open(Some(&net(1, 40000)), t0);
        let ticket = rig.core.begin_connection(1, t0).expect("ticket");
        rig.core.connection_failed(ticket.id, error);
        assert_eq!(ended(rig.core.view(t0)), reason, "{error:?}");
    }
}

#[test]
fn late_confirmed_of_an_old_session_does_not_touch_a_new_session_that_is_saving() {
    let mut rig = rig();
    let old = pending(&mut rig);
    rig.core.cancel();
    // Sesión nueva, ya aceptada por el usuario y guardando.
    let t0 = Instant::now();
    rig.core.open(Some(&net(1, 40000)), t0);
    let ticket = rig.core.begin_connection(1, t0).expect("ticket");
    let (tx, _rx) = channel();
    assert!(rig.core.offer_pending(
        ticket.id,
        "Otro celular",
        FINGERPRINT.to_owned(),
        t0 + CONFIRMATION,
        tx,
        t0
    ));
    rig.core.accept(t0);
    // La respuesta tardía de la sesión vieja no guarda nada ni cierra la nueva.
    rig.core.confirmed(old.id, Ok(device(9, NAME)), t0);
    assert!(matches!(rig.core.view(t0), PairingView::Saving { .. }));
    assert_eq!(list_len(&rig), 0);
    rig.core
        .confirmed(ticket.id, Ok(device(7, "Otro celular")), t0);
    assert_eq!(list_len(&rig), 1);
}
