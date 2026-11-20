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

// --- Código de emparejamiento y contraseña ---

use secrecy::{ExposeSecret, ExposeSecretMut, SecretBox, SecretString};
use zeroize::ZeroizeOnDrop;

fn assert_zeroize_on_drop<T: ZeroizeOnDrop>() {}

fn code_from(bytes: [u8; PAIRING_CODE_LEN]) -> PairingCode {
    let mut secret = SecretBox::<[u8; PAIRING_CODE_LEN]>::default();
    secret.expose_secret_mut().copy_from_slice(&bytes);
    PairingCode::from_bytes(secret)
}

#[derive(Serialize, Deserialize)]
struct WithSecret {
    ttl: u8,
    #[serde(with = "secret_str")]
    password: SecretString,
}

fn with_secret(text: &str) -> WithSecret {
    WithSecret {
        ttl: 30,
        password: SecretString::from(text),
    }
}

#[test]
fn pairing_code_generate_is_random_and_32_bytes() {
    let a = PairingCode::generate().unwrap();
    let b = PairingCode::generate().unwrap();
    assert_eq!(a.expose_bytes().len(), 32);
    assert_ne!(a.expose_bytes(), b.expose_bytes());
    assert_ne!(a.expose_bytes(), &[0u8; 32]);
}

#[test]
fn pairing_code_debug_hides_the_value() {
    let debug = format!("{:?}", code_from([0xAB; 32]));
    assert!(!debug.to_lowercase().contains("ab"), "{debug}");
    assert!(!debug.contains("171"), "{debug}");
    assert!(!debug.contains('['), "{debug}");
}

#[test]
fn pairing_code_is_zeroize_on_drop() {
    assert_zeroize_on_drop::<PairingCode>();
}

#[test]
fn pairing_code_serializes_as_its_32_bytes() {
    let bytes: [u8; 32] = core::array::from_fn(|i| i as u8 + 1);
    let encoded = encode_body(Kind::PairingOffer, &code_from(bytes)).unwrap();
    assert_eq!(&encoded[..2], &[PROTOCOL_VERSION, 1]);
    assert_eq!(&encoded[2..], &bytes);
    let decoded: PairingCode = decode_body(Kind::PairingOffer, &encoded).unwrap();
    assert_eq!(decoded.expose_bytes(), &bytes);
}

#[test]
fn secret_str_roundtrip_keeps_bytes() {
    let long = "a".repeat(1024);
    let texts = [
        "",
        "a",
        "  x  ",
        "\u{e9}",
        "e\u{301}",
        "🔐",
        "a\0b",
        long.as_str(),
    ];
    for text in texts {
        let bytes = encode_body(Kind::Send, &with_secret(text)).unwrap();
        let decoded: WithSecret = decode_body(Kind::Send, &bytes).unwrap();
        assert_eq!(decoded.password.expose_secret().as_bytes(), text.as_bytes());
        assert_eq!(decoded.ttl, 30);
    }
    // NFC y NFD son distintas: no se normaliza.
    assert_ne!("\u{e9}".as_bytes(), "e\u{301}".as_bytes());
}

#[test]
fn secret_str_serialization_does_not_reallocate() {
    let bytes = encode_body(Kind::Send, &with_secret(&"z".repeat(1000))).unwrap();
    assert_eq!(bytes.capacity(), bytes.len());
}

#[test]
fn secret_str_with_trailing_bytes_still_errors() {
    let mut bytes = encode_body(Kind::Send, &with_secret("clave"))
        .unwrap()
        .to_vec();
    bytes.push(0);
    assert_eq!(
        decode_body::<WithSecret>(Kind::Send, &bytes).err(),
        Some(ProtocolError::TrailingBytes)
    );
}

// --- PairingOffer ---

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use crate::protocol::identity::PublicKey;

const V4: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 20)), 50000);

fn v6(port: u16) -> SocketAddr {
    SocketAddr::new(IpAddr::V6(Ipv6Addr::new(0xfd00, 0, 0, 0, 0, 0, 0, 1)), port)
}

fn offer(address: SocketAddr) -> Result<PairingOffer> {
    PairingOffer::new(
        PublicKey::from_bytes([0x11; 32]),
        address,
        code_from([0x22; 32]),
    )
}

/// `versión | tipo | llave | dirección | código` armado a mano.
fn raw_offer(address: &[u8]) -> Vec<u8> {
    let mut bytes = vec![PROTOCOL_VERSION, 1];
    bytes.extend_from_slice(&[0x11; 32]);
    bytes.extend_from_slice(address);
    bytes.extend_from_slice(&[0x22; 32]);
    bytes
}

#[test]
fn pairing_offer_roundtrip_ipv4_and_ipv6() {
    for address in [V4, v6(50000), v6(1)] {
        let original = offer(address).unwrap();
        let bytes = original.encode().unwrap();
        let decoded = PairingOffer::decode(&bytes).unwrap();
        assert_eq!(decoded.public_key(), PublicKey::from_bytes([0x11; 32]));
        assert_eq!(decoded.address(), address);
        assert_eq!(decoded.code().expose_bytes(), &[0x22; 32]);
    }
}

#[test]
fn pairing_offer_wire_bytes_ipv4() {
    // Variante 0 (IPv4), 192.168.1.20 y el puerto 50000 como varint (D0 86 03).
    let expected = raw_offer(&[0, 192, 168, 1, 20, 0xD0, 0x86, 0x03]);
    assert_eq!(expected.len(), 74);
    assert_eq!(offer(V4).unwrap().encode().unwrap().as_slice(), expected);
}

#[test]
fn pairing_offer_wire_bytes_ipv6() {
    let mut address = vec![1, 0xfd];
    address.extend_from_slice(&[0; 14]);
    address.push(1);
    address.extend_from_slice(&[0xD0, 0x86, 0x03]);
    let expected = raw_offer(&address);
    assert_eq!(expected.len(), 86);
    assert_eq!(
        offer(v6(50000)).unwrap().encode().unwrap().as_slice(),
        expected
    );
}

#[test]
fn pairing_offer_max_len_is_86() {
    assert_eq!(MAX_PAIRING_OFFER_LEN, 86);
    assert_eq!(offer(v6(65535)).unwrap().encode().unwrap().len(), 86);
    assert_eq!(
        offer(SocketAddr::new(V4.ip(), 65535))
            .unwrap()
            .encode()
            .unwrap()
            .len(),
        74
    );
    // Ningún puerto da un contenido mayor.
    for port in [1u16, 127, 128, 16383, 16384, 65535] {
        assert!(offer(v6(port)).unwrap().encode().unwrap().len() <= MAX_PAIRING_OFFER_LEN);
    }
}

#[test]
fn port_zero_is_invalid() {
    assert_eq!(
        offer(SocketAddr::new(V4.ip(), 0)).err(),
        Some(ProtocolError::InvalidField(InvalidField::Port))
    );
    let bytes = raw_offer(&[0, 192, 168, 1, 20, 0]);
    assert_eq!(
        PairingOffer::decode(&bytes).err(),
        Some(ProtocolError::InvalidField(InvalidField::Port))
    );
}

#[test]
fn unspecified_and_multicast_addresses_are_invalid() {
    let invalid = [
        SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 50000),
        SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 50000),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(224, 0, 0, 251)), 50000),
        SocketAddr::new(
            IpAddr::V6(Ipv6Addr::new(0xff02, 0, 0, 0, 0, 0, 0, 0xfb)),
            50000,
        ),
    ];
    for address in invalid {
        assert_eq!(
            offer(address).err(),
            Some(ProtocolError::InvalidField(InvalidField::Address)),
            "{address}"
        );
    }
    // Lo mismo al decodificar bytes armados a mano.
    for ip in [[0u8, 0, 0, 0], [224, 0, 0, 251]] {
        let mut address = vec![0];
        address.extend_from_slice(&ip);
        address.extend_from_slice(&[0xD0, 0x86, 0x03]);
        assert_eq!(
            PairingOffer::decode(&raw_offer(&address)).err(),
            Some(ProtocolError::InvalidField(InvalidField::Address))
        );
    }
    let mut address = vec![1];
    address.extend_from_slice(&[0; 16]);
    address.extend_from_slice(&[0xD0, 0x86, 0x03]);
    assert_eq!(
        PairingOffer::decode(&raw_offer(&address)).err(),
        Some(ProtocolError::InvalidField(InvalidField::Address))
    );
}

#[test]
fn loopback_and_private_addresses_are_accepted() {
    for address in [
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 50000),
        SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 50000),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5)), 50000),
    ] {
        assert!(offer(address).is_ok(), "{address}");
    }
}

#[test]
fn pairing_offer_truncated_prefixes_fail() {
    let bytes = offer(V4).unwrap().encode().unwrap();
    for len in 0..bytes.len() {
        let error = PairingOffer::decode(&bytes[..len]).err().unwrap();
        assert!(
            matches!(error, ProtocolError::Truncated | ProtocolError::Malformed),
            "prefijo de {len} bytes: {error:?}"
        );
    }
}

#[test]
fn pairing_offer_trailing_bytes_are_rejected() {
    let mut bytes = offer(V4).unwrap().encode().unwrap().to_vec();
    bytes.push(0);
    assert_eq!(
        PairingOffer::decode(&bytes).err(),
        Some(ProtocolError::TrailingBytes)
    );
}

#[test]
fn pairing_offer_rejects_other_kinds_and_versions() {
    let mut bytes = offer(V4).unwrap().encode().unwrap().to_vec();
    bytes[1] = 3;
    assert_eq!(
        PairingOffer::decode(&bytes).err(),
        Some(ProtocolError::UnexpectedKind(3))
    );
    bytes[1] = 1;
    bytes[0] = 2;
    assert_eq!(
        PairingOffer::decode(&bytes).err(),
        Some(ProtocolError::UnsupportedVersion(2))
    );
}

#[test]
fn pairing_offer_debug_hides_the_code_and_the_address() {
    let debug = format!("{:?}", offer(V4).unwrap());
    assert!(!debug.to_lowercase().contains("2222"), "{debug}");
    assert!(!debug.contains("34, 34"), "{debug}");
    assert!(!debug.contains("192.168"), "{debug}");
    assert!(debug.contains("PublicKey(1111-1111-1111-1111)"), "{debug}");
}

#[test]
fn pairing_offer_encode_buffer_does_not_reallocate() {
    for address in [V4, v6(50000)] {
        let bytes = offer(address).unwrap().encode().unwrap();
        assert_eq!(bytes.capacity(), bytes.len());
    }
}
