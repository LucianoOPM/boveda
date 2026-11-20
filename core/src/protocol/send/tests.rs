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
