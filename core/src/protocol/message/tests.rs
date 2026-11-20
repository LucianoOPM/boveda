use serde::{Deserialize, Serialize};

use super::*;

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Probe {
    a: u8,
    b: String,
}

fn probe(len: usize) -> Probe {
    Probe {
        a: 7,
        b: "x".repeat(len),
    }
}

fn frame(len: usize) -> Zeroizing<Vec<u8>> {
    encode_body(Kind::Ack, &probe(len)).unwrap()
}

#[test]
fn roundtrip() {
    let value = Probe {
        a: 9,
        b: "hola".into(),
    };
    let bytes = encode_body(Kind::Ack, &value).unwrap();
    assert_eq!(decode_body::<Probe>(Kind::Ack, &bytes).unwrap(), value);
}

#[test]
fn frame_starts_with_version_and_kind() {
    let bytes = frame(3);
    assert_eq!(&bytes[..2], &[PROTOCOL_VERSION, 3]);
    assert_eq!(PROTOCOL_VERSION, 1);
}

#[test]
fn empty_and_one_byte_inputs_are_truncated() {
    for bytes in [&[][..], &[1u8][..]] {
        assert_eq!(
            decode_body::<Probe>(Kind::Ack, bytes).unwrap_err(),
            ProtocolError::Truncated
        );
    }
}

#[test]
fn unknown_version_wins_over_everything() {
    let valid_body = frame(3)[2..].to_vec();
    let bodies: [Vec<u8>; 3] = [vec![], vec![0xff; 20], valid_body];
    for version in [0u8, 2, 255] {
        for body in &bodies {
            let mut bytes = vec![version, 3];
            bytes.extend_from_slice(body);
            assert_eq!(
                decode_body::<Probe>(Kind::Ack, &bytes).unwrap_err(),
                ProtocolError::UnsupportedVersion(version)
            );
        }
    }
    // Aun con un solo byte: la versión se mira antes que la longitud mínima.
    assert_eq!(
        decode_body::<Probe>(Kind::Ack, &[2]).unwrap_err(),
        ProtocolError::UnsupportedVersion(2)
    );
}

#[test]
fn wrong_kind_is_unexpected_kind() {
    let mut bytes = frame(3).to_vec();
    bytes[1] = 9;
    assert_eq!(
        decode_body::<Probe>(Kind::Ack, &bytes).unwrap_err(),
        ProtocolError::UnexpectedKind(9)
    );
    // Un mensaje de otro tipo bien formado tampoco se acepta.
    let ack = frame(3);
    assert_eq!(
        decode_body::<Probe>(Kind::Send, &ack).unwrap_err(),
        ProtocolError::UnexpectedKind(3)
    );
}

#[test]
fn trailing_bytes_are_rejected() {
    for extra in [1usize, 100] {
        let mut bytes = frame(3).to_vec();
        bytes.extend(std::iter::repeat_n(0u8, extra));
        assert_eq!(
            decode_body::<Probe>(Kind::Ack, &bytes).unwrap_err(),
            ProtocolError::TrailingBytes
        );
    }
}

#[test]
fn every_proper_prefix_fails() {
    let bytes = frame(20);
    for len in 0..bytes.len() {
        let error = decode_body::<Probe>(Kind::Ack, &bytes[..len]).unwrap_err();
        assert!(
            matches!(error, ProtocolError::Truncated | ProtocolError::Malformed),
            "prefijo de {len} bytes: {error:?}"
        );
    }
}

#[test]
fn exactly_max_len_is_decoded_and_one_more_is_too_large() {
    // versión, tipo, `a`, longitud en 2 bytes (varint) y el texto.
    let exact = frame(MAX_MESSAGE_LEN - 5);
    assert_eq!(exact.len(), MAX_MESSAGE_LEN);
    assert_eq!(
        decode_body::<Probe>(Kind::Ack, &exact).unwrap(),
        probe(MAX_MESSAGE_LEN - 5)
    );

    // Pasado el máximo no se mira nada más: ni siquiera la versión.
    let mut over = vec![0u8; MAX_MESSAGE_LEN + 1];
    over[0] = 9;
    assert_eq!(
        decode_body::<Probe>(Kind::Ack, &over).unwrap_err(),
        ProtocolError::TooLarge
    );
}

#[test]
fn encode_over_max_len_is_too_large() {
    assert_eq!(
        encode_body(Kind::Ack, &probe(MAX_MESSAGE_LEN - 4)).unwrap_err(),
        ProtocolError::TooLarge
    );
}

#[test]
fn encode_buffer_does_not_reallocate() {
    for len in [0, 5, 127, 128, 1000] {
        let bytes = frame(len);
        assert_eq!(bytes.capacity(), bytes.len(), "texto de {len} bytes");
    }
}

#[test]
fn bad_utf8_is_malformed() {
    let bytes = [PROTOCOL_VERSION, 3, 1, 2, 0xff, 0xfe];
    assert_eq!(
        decode_body::<Probe>(Kind::Ack, &bytes).unwrap_err(),
        ProtocolError::Malformed
    );
}
