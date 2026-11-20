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

// --- Envío y confirmación por el canal de transporte (Task 4) ---

use secrecy::SecretString;

use crate::protocol::{AckOutcome, SendMessage};

fn message(text: &str) -> SendMessage {
    SendMessage::new(SecretString::from(text.to_string())).unwrap()
}

/// Un apretón de manos completo: los dos extremos listos para el mensaje.
struct Session {
    pc: AwaitingSend,
    phone: AwaitingResponse,
    second: Vec<u8>,
    now: Instant,
}

fn session() -> Session {
    let now = Instant::now();
    let (phone, first) = Sender::new(&keys(2), &pc_key())
        .unwrap()
        .first_message()
        .unwrap();
    let (pc, second) = Responder::new(&keys(1))
        .read_first(now, &first, |_| true)
        .unwrap();
    Session {
        pc,
        phone,
        second,
        now,
    }
}

/// Un celular nuevo cuya trama 2 no corresponde a ninguna sesión.
fn lone_phone() -> AwaitingResponse {
    Sender::new(&keys(2), &pc_key())
        .unwrap()
        .first_message()
        .unwrap()
        .0
}

/// Una sesión nueva con la trama 3 ya armada: el estado de la PC, el del
/// celular que espera el `Ack`, la trama 3 y el instante del apretón de manos.
fn sent(text: &str) -> (AwaitingSend, AwaitingAck, Vec<u8>, Instant) {
    let s = session();
    let (ack, third) = s.phone.read_second(&s.second, &message(text)).unwrap();
    (s.pc, ack, third, s.now)
}

#[test]
fn full_exchange_in_memory_delivers_the_password_and_copied() {
    let (pc, awaiting_ack, third, now) = sent("hunter2 ñ");
    let received = pc.read_send(now, &third).unwrap();
    assert_eq!(received.message().password().expose_secret(), "hunter2 ñ");
    assert_eq!(received.message().ttl_secs(), 30);
    assert_eq!(received.peer(), keys(2).public());
    let ack = received.ack(AckOutcome::Copied).unwrap();
    assert_eq!(awaiting_ack.read_ack(&ack).unwrap(), AckOutcome::Copied);
}

#[test]
fn ack_failed_roundtrips() {
    let (pc, awaiting_ack, third, now) = sent("x");
    let ack = pc
        .read_send(now, &third)
        .unwrap()
        .ack(AckOutcome::Failed)
        .unwrap();
    assert_eq!(awaiting_ack.read_ack(&ack).unwrap(), AckOutcome::Failed);
}

#[test]
fn send_frame_with_a_1024_byte_password_is_1045_bytes_and_fits_2048() {
    let (pc, awaiting_ack, third, now) = sent(&"a".repeat(1024));
    assert_eq!(third.len(), 1045);
    assert!(third.len() <= crate::protocol::MAX_SEND_FRAME_LEN);
    let received = pc.read_send(now, &third).unwrap();
    assert_eq!(received.message().password().expose_secret().len(), 1024);
    let ack = received.ack(AckOutcome::Copied).unwrap();
    assert_eq!(ack.len(), 19);
    assert_eq!(awaiting_ack.read_ack(&ack).unwrap(), AckOutcome::Copied);
}

#[test]
fn second_frame_from_another_session_or_altered_is_peer_not_authentic() {
    let other = session().second;
    assert_eq!(
        lone_phone().read_second(&other, &message("x")).err(),
        Some(SendError::PeerNotAuthentic)
    );

    let genuine = session();
    for i in [0, 20, 47] {
        let mut altered = genuine.second.clone();
        altered[i] ^= 1;
        assert_eq!(
            lone_phone().read_second(&altered, &message("x")).err(),
            Some(SendError::PeerNotAuthentic)
        );
    }
    // De otro largo, vacía y de ceros: tampoco se autentican.
    for frame in [vec![0u8; 47], vec![0u8; 48], Vec::new(), vec![0u8; 49]] {
        assert_eq!(
            lone_phone().read_second(&frame, &message("x")).err(),
            Some(SendError::PeerNotAuthentic)
        );
    }
}

#[test]
fn send_just_before_the_deadline_is_accepted_and_at_the_deadline_expired() {
    let (pc, _, third, now) = sent("x");
    let before = now + SEND_TTL - Duration::from_nanos(1);
    assert!(pc.read_send(before, &third).is_ok());

    let (pc, _, third, now) = sent("x");
    assert_eq!(
        pc.read_send(now + SEND_TTL, &third).err(),
        Some(SendError::Expired)
    );
    let (pc, _, third, now) = sent("x");
    let late = now + SEND_TTL + Duration::from_secs(60);
    assert_eq!(pc.read_send(late, &third).err(), Some(SendError::Expired));
}

#[test]
fn expired_send_does_not_deliver_a_message_even_if_it_decrypts() {
    let (pc, _, third, now) = sent("secreto");
    let result = pc.read_send(now + SEND_TTL, &third);
    assert!(matches!(result, Err(SendError::Expired)));
}

#[test]
fn replayed_first_frame_gets_a_second_frame_but_no_valid_send() {
    let now = Instant::now();
    let (phone, first) = Sender::new(&keys(2), &pc_key())
        .unwrap()
        .first_message()
        .unwrap();
    // La sesión real.
    let (pc1, second1) = Responder::new(&keys(1))
        .read_first(now, &first, |_| true)
        .unwrap();
    let (_, third1) = phone.read_second(&second1, &message("real")).unwrap();
    assert!(pc1.read_send(now, &third1).is_ok());
    // Un atacante reenvía la misma primera trama: la PC contesta con una
    // llave efímera nueva...
    let (pc2, second2) = Responder::new(&keys(1))
        .read_first(now, &first, |_| true)
        .unwrap();
    assert_ne!(second1, second2);
    // ...pero la trama 3 capturada de la primera sesión no sirve en la segunda.
    assert_eq!(
        pc2.read_send(now, &third1).err(),
        Some(SendError::Handshake)
    );
}

#[test]
fn send_frame_from_another_session_is_rejected() {
    let (_, _, third_a, _) = sent("a");
    let (pc_b, _, _, now_b) = sent("b");
    assert_eq!(
        pc_b.read_send(now_b, &third_a).err(),
        Some(SendError::Handshake)
    );
}

#[test]
fn altered_send_frame_is_rejected() {
    let len = sent("hola").2.len();
    for i in [0, len / 2, len - 1] {
        let (pc, _, mut third, now) = sent("hola");
        third[i] ^= 0x01;
        assert_eq!(pc.read_send(now, &third).err(), Some(SendError::Handshake));
    }
}

#[test]
fn unencrypted_empty_short_or_oversized_send_frame_is_rejected() {
    let plain = message("hola").encode().unwrap().to_vec();
    let cases = [
        (plain, SendError::Handshake),
        (Vec::new(), SendError::Handshake),
        (vec![0u8; 15], SendError::Handshake),
        (vec![0u8; 16], SendError::Handshake),
        (
            vec![0u8; crate::protocol::MAX_SEND_FRAME_LEN + 1],
            SendError::FrameTooLarge,
        ),
    ];
    for (frame, expected) in cases {
        let s = session();
        assert_eq!(s.pc.read_send(s.now, &frame).err(), Some(expected));
    }
}

#[test]
fn altered_or_foreign_ack_is_rejected() {
    // `read_ack` consume el estado: una sesión por caso.
    let (pc, _, third, now) = sent("x");
    let foreign = pc
        .read_send(now, &third)
        .unwrap()
        .ack(AckOutcome::Copied)
        .unwrap();
    let (_, awaiting_ack, _, _) = sent("x");
    assert_eq!(
        awaiting_ack.read_ack(&foreign).err(),
        Some(SendError::Handshake)
    );

    let (pc, awaiting_ack, third, now) = sent("x");
    let mut altered = pc
        .read_send(now, &third)
        .unwrap()
        .ack(AckOutcome::Copied)
        .unwrap();
    altered[0] ^= 1;
    assert_eq!(
        awaiting_ack.read_ack(&altered).err(),
        Some(SendError::Handshake)
    );
}

#[test]
fn authentic_frame_that_is_not_an_ack_is_a_protocol_error() {
    // Un cifrado válido de la sesión cuyo contenido no es un `Ack`.
    let (pc, awaiting_ack, third, now) = sent("x");
    let mut received = pc.read_send(now, &third).unwrap();
    let mut buffer = [0u8; 64];
    let len = received
        .transport
        .write_message(&[1, 3], &mut buffer)
        .unwrap();
    assert!(matches!(
        awaiting_ack.read_ack(&buffer[..len]).err(),
        Some(SendError::Protocol(_))
    ));
}

#[test]
fn password_never_shows_in_debug_or_errors() {
    let secret = "CENTINELA-contraseña-0451";
    let (pc, _, third, now) = sent(secret);
    let received = pc.read_send(now, &third).unwrap();
    let shown = format!("{received:?} {:?}", received.message());
    assert!(!shown.contains(secret) && !shown.contains("0451"));
    // Un error de una trama alterada tampoco la lleva.
    let (pc, _, mut third, now) = sent(secret);
    third[3] ^= 1;
    let error = pc.read_send(now, &third).err().unwrap();
    assert!(!format!("{error} {error:?}").contains(secret));
}
