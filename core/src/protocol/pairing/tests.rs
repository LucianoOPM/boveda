use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use super::*;
use crate::vault::PAIRED_PC_NAME_MAX_BYTES;

fn key(first: u8) -> PublicKey {
    let mut bytes = [0u8; 32];
    bytes[..8].copy_from_slice(&[first, 0xB2, 0xC3, 0xD4, 0xE5, 0xF6, 0x07, 0x18]);
    PublicKey::from_bytes(bytes)
}

fn address() -> SocketAddr {
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 20)), 50000)
}

#[test]
fn provisional_name_uses_fingerprint() {
    let pc = PairedPc::new(key(0xA1), address());
    assert_eq!(pc.provisional_name(), "PC A1B2-C3D4-E5F6-0718");
}

#[test]
fn provisional_name_is_valid_for_the_vault() {
    let name = PairedPc::new(key(0xA1), address()).provisional_name();
    assert!(!name.is_empty() && name.len() <= PAIRED_PC_NAME_MAX_BYTES);
    assert_eq!(name.trim(), name);
}

#[test]
fn into_new_paired_pc_uses_given_name_or_provisional() {
    let given = PairedPc::new(key(0xA1), address()).into_new_paired_pc(Some("Casa".into()));
    assert_eq!(given.name, "Casa");
    assert_eq!(given.public_key, *key(0xA1).as_bytes());
    let provisional = PairedPc::new(key(0xA1), address()).into_new_paired_pc(None);
    assert_eq!(provisional.name, "PC A1B2-C3D4-E5F6-0718");
}

#[test]
fn into_new_paired_pc_sets_last_ip_canonical() {
    let mapped = SocketAddr::new(
        IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0xc0a8, 0x0114)),
        50000,
    );
    let new = PairedPc::new(key(1), mapped).into_new_paired_pc(None);
    assert_eq!(
        new.last_ip,
        Some(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 20)))
    );
}

#[test]
fn paired_device_debug_shows_only_fingerprint() {
    let name = DeviceName::new("Pixel secreto").unwrap();
    let device = PairedDevice::new(key(0xA1), name);
    let text = format!("{device:?}");
    assert!(text.contains("A1B2-C3D4-E5F6-0718"), "{text}");
    assert!(!text.contains("secreto"), "{text}");
    assert_eq!(device.name().as_str(), "Pixel secreto");
    assert_eq!(device.public_key(), key(0xA1));
}

#[test]
fn pairing_error_messages_carry_no_data() {
    let errors = [
        PairingError::Refused(Refusal::WrongCode),
        PairingError::PeerNotAuthentic,
        PairingError::Rejected,
        PairingError::ConfirmationTimeout,
        PairingError::Timeout,
        PairingError::Cancelled,
        PairingError::Closed,
        PairingError::Io(std::io::ErrorKind::BrokenPipe),
        PairingError::FrameTooLarge,
        PairingError::Handshake,
        PairingError::Protocol(ProtocolError::Malformed),
    ];
    for error in errors {
        let text = error.to_string();
        assert!(!text.is_empty(), "{error:?}");
        assert!(!text.contains("BrokenPipe"), "{text}");
    }
}

#[test]
fn constants_match_the_spec() {
    assert_eq!(PAIRING_PATTERN, "Noise_XKpsk0_25519_ChaChaPoly_BLAKE2s");
    assert_eq!(PAIRING_PROLOGUE, b"boveda/emparejamiento/v1");
    assert_eq!(PAIRING_PROLOGUE.len(), 24);
    assert_eq!(PAIRING_CODE_TTL, std::time::Duration::from_secs(120));
    assert_eq!(MAX_FAILED_ATTEMPTS, 3);
    assert_eq!((FIRST_FRAME_LEN, MAX_FRAME_LEN), (48, 512));
}

#[test]
fn noise_params_parse_the_agreed_pattern() {
    let params = noise_params().unwrap();
    assert_eq!(params.name, PAIRING_PATTERN);
    // Con la PSK en la posición 0: un código incorrecto falla en la trama 1.
    assert!(params.handshake.is_psk());
}

// ---- Sesión del código y primeros mensajes (Task 4) ----

use std::time::Instant;

use secrecy::SecretBox;

use crate::protocol::{KeyPair, PairingCode, PairingOffer};

fn keypair(n: u8) -> KeyPair {
    KeyPair::from_private(SecretBox::new(Box::new([n; 32])))
}

fn code(n: u8) -> PairingCode {
    PairingCode::from_bytes(SecretBox::new(Box::new([n; 32])))
}

fn device_name() -> DeviceName {
    DeviceName::new("Pixel de Ana").unwrap()
}

/// Sesión de la PC (llave 1, código 7) y un celular (llave 2) que escanea su QR.
fn setup(t0: Instant) -> (PairingSession, Initiator) {
    let session = PairingSession::new(keypair(1), code(7), t0);
    (session, phone(keypair(1).public(), 7))
}

fn phone(pc_key: PublicKey, code_n: u8) -> Initiator {
    let offer = PairingOffer::new(pc_key, address(), code(code_n)).unwrap();
    Initiator::new(&keypair(2), &offer, device_name()).unwrap()
}

fn first_frame(code_n: u8) -> Frame {
    let (_, frame) = phone(keypair(1).public(), code_n).first_message().unwrap();
    frame
}

#[test]
fn first_frame_is_48_bytes() {
    assert_eq!(first_frame(7).len(), FIRST_FRAME_LEN);
}

#[test]
fn correct_code_reaches_awaiting_third() {
    let t0 = Instant::now();
    let (mut session, initiator) = setup(t0);
    let (_, frame) = initiator.first_message().unwrap();
    let third = session.accept_first(t0, &frame).unwrap();
    assert_eq!(third.second_message().len(), 48);
}

#[test]
fn wrong_code_is_refused_and_counts() {
    let t0 = Instant::now();
    let (mut session, _) = setup(t0);
    assert_eq!(
        session.accept_first(t0, &first_frame(8)).err(),
        Some(Refusal::WrongCode)
    );
    // El código correcto aún sirve: un intento fallido no lo invalida.
    assert!(session.accept_first(t0, &first_frame(7)).is_ok());
}

#[test]
fn third_failed_attempt_exhausts() {
    let t0 = Instant::now();
    let (mut session, _) = setup(t0);
    for _ in 0..MAX_FAILED_ATTEMPTS {
        assert_eq!(
            session.accept_first(t0, &first_frame(8)).err(),
            Some(Refusal::WrongCode)
        );
    }
    assert_eq!(
        session.accept_first(t0, &first_frame(7)).err(),
        Some(Refusal::TooManyAttempts)
    );
}

#[test]
fn malformed_length_counts_as_attempt() {
    let t0 = Instant::now();
    let (mut session, _) = setup(t0);
    assert_eq!(
        session.accept_first(t0, &[0u8; 47]).err(),
        Some(Refusal::Malformed)
    );
    assert_eq!(
        session.accept_first(t0, &[0u8; 49]).err(),
        Some(Refusal::Malformed)
    );
    assert_eq!(session.note_malformed(t0), Refusal::Malformed);
    assert_eq!(
        session.accept_first(t0, &first_frame(7)).err(),
        Some(Refusal::TooManyAttempts)
    );
}

#[test]
fn consumed_after_first_success() {
    let t0 = Instant::now();
    let (mut session, _) = setup(t0);
    assert!(session.accept_first(t0, &first_frame(7)).is_ok());
    assert_eq!(
        session.accept_first(t0, &first_frame(7)).err(),
        Some(Refusal::AlreadyUsed)
    );
    assert_eq!(session.note_malformed(t0), Refusal::AlreadyUsed);
}

#[test]
fn expired_at_exactly_120s() {
    let t0 = Instant::now();
    let (mut session, _) = setup(t0);
    let just_before = t0 + PAIRING_CODE_TTL - Duration::from_millis(1);
    assert!(session.accept_first(just_before, &first_frame(7)).is_ok());
    let (mut session, _) = setup(t0);
    assert_eq!(
        session
            .accept_first(t0 + PAIRING_CODE_TTL, &first_frame(7))
            .err(),
        Some(Refusal::Expired)
    );
}

#[test]
fn expired_does_not_use_the_code_or_count() {
    let t0 = Instant::now();
    let (mut session, _) = setup(t0);
    let late = t0 + PAIRING_CODE_TTL + Duration::from_secs(1);
    // Con un código incorrecto sigue siendo `Expired` (no `WrongCode`): no se
    // intentó descifrar con el código y nada cuenta.
    for _ in 0..5 {
        assert_eq!(
            session.accept_first(late, &first_frame(8)).err(),
            Some(Refusal::Expired)
        );
    }
    assert_eq!(session.note_malformed(late), Refusal::Expired);
}

#[test]
fn cancelled_session_refuses() {
    let t0 = Instant::now();
    let (mut session, _) = setup(t0);
    session.cancel();
    assert_eq!(
        session.accept_first(t0, &first_frame(7)).err(),
        Some(Refusal::Cancelled)
    );
    assert_eq!(session.note_malformed(t0), Refusal::Cancelled);
}

#[test]
fn refusals_do_not_change_after_final_state() {
    let t0 = Instant::now();
    let (mut session, _) = setup(t0);
    for _ in 0..MAX_FAILED_ATTEMPTS {
        let _ = session.accept_first(t0, &first_frame(8));
    }
    for _ in 0..3 {
        assert_eq!(
            session.accept_first(t0, &first_frame(7)).err(),
            Some(Refusal::TooManyAttempts)
        );
    }
}

#[test]
fn pc_with_other_private_key_cannot_answer() {
    // El QR trae la llave 1, pero quien responde tiene la llave 3: no puede
    // descifrar la trama 1 y no responde nada.
    let t0 = Instant::now();
    let mut impostor = PairingSession::new(keypair(3), code(7), t0);
    assert_eq!(
        impostor.accept_first(t0, &first_frame(7)).err(),
        Some(Refusal::WrongCode)
    );
}

/// Un celular que mandó su trama 1 y la trama 2 genuina que la PC le devuelve.
fn exchange_to_second(t0: Instant) -> (AwaitingSecond, Frame) {
    let (mut session, initiator) = setup(t0);
    let (awaiting, frame) = initiator.first_message().unwrap();
    let third = session.accept_first(t0, &frame).unwrap();
    (awaiting, third.second_message().to_vec())
}

#[test]
fn genuine_second_message_is_accepted() {
    let (awaiting, genuine) = exchange_to_second(Instant::now());
    let (_, frame3) = awaiting.read_second(&genuine).unwrap();
    assert!(frame3.len() <= MAX_FRAME_LEN);
}

#[test]
fn altered_second_message_is_peer_not_authentic() {
    let (awaiting, mut frame) = exchange_to_second(Instant::now());
    frame[40] ^= 0x01;
    assert_eq!(
        awaiting.read_second(&frame).err(),
        Some(PairingError::PeerNotAuthentic)
    );
}

#[test]
fn forged_second_message_is_peer_not_authentic() {
    let (awaiting, _) = exchange_to_second(Instant::now());
    assert_eq!(
        awaiting.read_second(&[0x42; 48]).err(),
        Some(PairingError::PeerNotAuthentic)
    );
}
