use std::io::ErrorKind;
use std::time::Duration;

use super::*;
use crate::protocol::SEND_TTL_SECS;

#[test]
fn constants_match_the_spec() {
    assert_eq!(SEND_PATTERN, "Noise_IK_25519_ChaChaPoly_BLAKE2s");
    assert_eq!(SEND_PROLOGUE, b"boveda/envio/v1");
    assert_eq!(SEND_TTL, Duration::from_secs(u64::from(SEND_TTL_SECS)));
    assert_eq!(SEND_TTL, Duration::from_secs(30));
    assert_eq!(SEND_FIRST_FRAME_LEN, 96);
    assert_eq!(SEND_SECOND_FRAME_LEN, 48);
}

#[test]
fn send_timeouts_default_match_the_spec() {
    let timeouts = SendTimeouts::default();
    assert_eq!(timeouts.step, Duration::from_secs(10));
    assert_eq!(timeouts.total, SEND_TTL);
    assert_eq!(timeouts.ack_wait, Duration::from_secs(10));
}

#[test]
fn send_error_from_pairing_error_maps_each_variant() {
    use crate::protocol::{PairingError, ProtocolError, Refusal};
    let cases = [
        (PairingError::Timeout, SendError::Timeout),
        (PairingError::Cancelled, SendError::Cancelled),
        (PairingError::Closed, SendError::Closed),
        (
            PairingError::Io(ErrorKind::BrokenPipe),
            SendError::Io(ErrorKind::BrokenPipe),
        ),
        (PairingError::FrameTooLarge, SendError::FrameTooLarge),
        (PairingError::PeerNotAuthentic, SendError::PeerNotAuthentic),
        (
            PairingError::Protocol(ProtocolError::Malformed),
            SendError::Protocol(ProtocolError::Malformed),
        ),
        (PairingError::Handshake, SendError::Handshake),
        (PairingError::Rejected, SendError::Handshake),
        (PairingError::ConfirmationTimeout, SendError::Handshake),
        (
            PairingError::Refused(Refusal::WrongCode),
            SendError::Handshake,
        ),
    ];
    for (from, expected) in cases {
        assert_eq!(SendError::from(from), expected);
    }
}

#[test]
fn send_error_from_protocol_error_wraps_it() {
    use crate::protocol::ProtocolError;
    assert_eq!(
        SendError::from(ProtocolError::TooLarge),
        SendError::Protocol(ProtocolError::TooLarge)
    );
}

#[test]
fn send_error_messages_are_distinct_and_carry_no_data() {
    use crate::protocol::ProtocolError;
    let all = [
        SendError::NotPaired,
        SendError::Expired,
        SendError::PeerNotAuthentic,
        SendError::Timeout,
        SendError::Cancelled,
        SendError::Closed,
        SendError::Io(ErrorKind::Other),
        SendError::FrameTooLarge,
        SendError::Handshake,
        SendError::Protocol(ProtocolError::Malformed),
    ];
    let messages: Vec<String> = all.iter().map(ToString::to_string).collect();
    for (i, a) in messages.iter().enumerate() {
        assert!(!a.is_empty());
        for b in &messages[i + 1..] {
            assert_ne!(a, b);
        }
    }
    // Solo describen qué falló: el `Debug` de una variante con datos solo
    // muestra su clase.
    assert_eq!(
        format!("{:?}", SendError::Io(ErrorKind::Other)),
        "Io(Other)"
    );
}

// --- Apretón de manos IK y autorización (Task 3) ---

use std::cell::{Cell, RefCell};
use std::time::Instant;

use secrecy::{ExposeSecret, SecretBox};
use snow::Builder;

use crate::protocol::{KeyPair, PublicKey};

fn keys(n: u8) -> KeyPair {
    KeyPair::from_private(SecretBox::new(Box::new([n; 32])))
}

/// La PC tiene la llave 1 y el celular, la 2.
fn pc_key() -> PublicKey {
    keys(1).public()
}

fn first_frame(phone: &KeyPair, pc: &PublicKey) -> Vec<u8> {
    let (_, frame) = Sender::new(phone, pc).unwrap().first_message().unwrap();
    frame
}

#[test]
fn noise_params_parse() {
    assert!(noise_params().is_ok());
}

#[test]
fn first_frame_is_96_bytes_and_second_is_48() {
    let frame = first_frame(&keys(2), &pc_key());
    assert_eq!(frame.len(), SEND_FIRST_FRAME_LEN);
    let (_, second) = Responder::new(&keys(1))
        .read_first(Instant::now(), &frame, |_| true)
        .unwrap();
    assert_eq!(second.len(), SEND_SECOND_FRAME_LEN);
}

#[test]
fn paired_phone_gets_a_second_frame_and_is_asked_with_its_recovered_key() {
    let phone = keys(2);
    let asked = RefCell::new(Vec::new());
    let now = Instant::now();
    let (awaiting, _) = Responder::new(&keys(1))
        .read_first(now, &first_frame(&phone, &pc_key()), |key| {
            asked.borrow_mut().push(*key);
            true
        })
        .unwrap();
    assert_eq!(*asked.borrow(), vec![phone.public()]);
    assert_eq!(awaiting.peer(), phone.public());
    assert_eq!(awaiting.deadline, now + SEND_TTL);
}

#[test]
fn unpaired_phone_gets_no_second_frame() {
    let phone = keys(2);
    let calls = Cell::new(0);
    let asked = RefCell::new(None);
    let result = Responder::new(&keys(1)).read_first(
        Instant::now(),
        &first_frame(&phone, &pc_key()),
        |key| {
            calls.set(calls.get() + 1);
            *asked.borrow_mut() = Some(*key);
            false
        },
    );
    assert_eq!(result.err(), Some(SendError::NotPaired));
    assert_eq!(calls.get(), 1);
    assert_eq!(*asked.borrow(), Some(phone.public()));
}

#[test]
fn first_frame_for_another_pc_key_is_a_handshake_error_and_nobody_is_asked() {
    let called = Cell::new(false);
    let frame = first_frame(&keys(2), &keys(3).public());
    let result = Responder::new(&keys(1)).read_first(Instant::now(), &frame, |_| {
        called.set(true);
        true
    });
    assert_eq!(result.err(), Some(SendError::Handshake));
    assert!(!called.get());
}

#[test]
fn first_frame_with_wrong_length_or_garbage_is_a_handshake_error() {
    let good = first_frame(&keys(2), &pc_key());
    let mut long = good.clone();
    long.push(0);
    let cases = [good[..95].to_vec(), long, vec![0u8; 96], Vec::new()];
    for frame in cases {
        let called = Cell::new(false);
        let result = Responder::new(&keys(1)).read_first(Instant::now(), &frame, |_| {
            called.set(true);
            true
        });
        assert_eq!(result.err(), Some(SendError::Handshake));
        assert!(!called.get());
    }
}

#[test]
fn first_frame_with_application_payload_is_a_handshake_error() {
    let phone = keys(2);
    let pc = pc_key();
    let mut handshake = Builder::new(noise_params().unwrap())
        .local_private_key(phone.private().expose_secret())
        .unwrap()
        .remote_public_key(pc.as_bytes())
        .unwrap()
        .prologue(SEND_PROLOGUE)
        .unwrap()
        .build_initiator()
        .unwrap();
    let mut buffer = [0u8; 512];
    let len = handshake.write_message(&[1], &mut buffer).unwrap();
    assert_eq!(len, SEND_FIRST_FRAME_LEN + 1);
    // Con la carga de 1 byte la trama ya no mide 96: se rechaza por largo.
    let called = Cell::new(false);
    let result = Responder::new(&keys(1)).read_first(Instant::now(), &buffer[..len], |_| {
        called.set(true);
        true
    });
    assert_eq!(result.err(), Some(SendError::Handshake));
    assert!(!called.get());
}

#[test]
fn first_frame_for_another_protocol_prologue_is_rejected() {
    let phone = keys(2);
    let pc = pc_key();
    let mut handshake = Builder::new(noise_params().unwrap())
        .local_private_key(phone.private().expose_secret())
        .unwrap()
        .remote_public_key(pc.as_bytes())
        .unwrap()
        .prologue(b"boveda/emparejamiento/v1")
        .unwrap()
        .build_initiator()
        .unwrap();
    let mut buffer = [0u8; 512];
    let len = handshake.write_message(&[], &mut buffer).unwrap();
    assert_eq!(len, SEND_FIRST_FRAME_LEN);
    let result = Responder::new(&keys(1)).read_first(Instant::now(), &buffer[..len], |_| true);
    assert_eq!(result.err(), Some(SendError::Handshake));
}
