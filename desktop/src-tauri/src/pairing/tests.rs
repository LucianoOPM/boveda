use std::net::{Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use secrecy::SecretBox;
use vault_core::protocol::{KeyPair, PairingOffer, Refusal};

use super::core::PairingCore;
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
    let core = PairingCore::new(identity.clone(), paired);
    Rig {
        core,
        identity,
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
