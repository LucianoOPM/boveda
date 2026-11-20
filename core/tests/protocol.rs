//! Pruebas de punta a punta de la API pública `vault_core::protocol` (#94):
//! solo tipos y codificación, sin red.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use proptest::prelude::*;
use secrecy::{ExposeSecret, SecretString};
use vault_core::protocol::{
    Ack, AckOutcome, KeyPair, MAX_MESSAGE_LEN, MAX_PAIRING_OFFER_LEN, PairingCode, PairingOffer,
    ProtocolError, PublicKey, SendMessage,
};

const ADDRESS: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 20)), 50000);

fn offer(address: SocketAddr) -> PairingOffer {
    let pair = KeyPair::generate().unwrap();
    PairingOffer::new(pair.public(), address, PairingCode::generate().unwrap()).unwrap()
}

fn send(text: &str) -> SendMessage {
    SendMessage::new(SecretString::from(text)).unwrap()
}

/// Decodifica `bytes` como los tres tipos; ninguno debe entrar en pánico.
fn decode_all(bytes: &[u8]) -> [bool; 3] {
    [
        PairingOffer::decode(bytes).is_ok(),
        SendMessage::decode(bytes).is_ok(),
        Ack::decode(bytes).is_ok(),
    ]
}

#[test]
fn offer_roundtrip_with_generated_keys() {
    for address in [
        ADDRESS,
        SocketAddr::new(
            IpAddr::V6(Ipv6Addr::new(0xfd00, 0, 0, 0, 0, 0, 0, 1)),
            65535,
        ),
    ] {
        let original = offer(address);
        let bytes = original.encode().unwrap();
        assert!(bytes.len() <= MAX_PAIRING_OFFER_LEN);
        let decoded = PairingOffer::decode(&bytes).unwrap();
        assert_eq!(decoded.public_key(), original.public_key());
        assert_eq!(decoded.address(), address);
        assert_eq!(
            decoded.code().expose_bytes(),
            original.code().expose_bytes()
        );
    }
}

#[test]
fn send_and_ack_roundtrip() {
    let decoded = SendMessage::decode(&send("contraseña-ñ-🔐").encode().unwrap()).unwrap();
    assert_eq!(decoded.password().expose_secret(), "contraseña-ñ-🔐");
    assert_eq!(decoded.ttl_secs(), 30);
    for outcome in [AckOutcome::Copied, AckOutcome::Failed] {
        let ack = Ack::decode(&Ack::new(outcome).encode().unwrap()).unwrap();
        assert_eq!(ack.outcome(), outcome);
    }
}

#[test]
fn messages_are_not_interchangeable() {
    let offer_bytes = offer(ADDRESS).encode().unwrap();
    let send_bytes = send("clave").encode().unwrap();
    let ack_bytes = Ack::new(AckOutcome::Copied).encode().unwrap();
    assert_eq!(decode_all(&offer_bytes), [true, false, false]);
    assert_eq!(decode_all(&send_bytes), [false, true, false]);
    assert_eq!(decode_all(&ack_bytes), [false, false, true]);
}

#[test]
fn public_key_matches_the_private_key() {
    let pair = KeyPair::generate().unwrap();
    assert_eq!(PublicKey::from_private(pair.private()), pair.public());
    let fingerprint = pair.public().fingerprint().to_string();
    assert_eq!(fingerprint.len(), 19);
    assert_eq!(fingerprint.matches('-').count(), 3);
    assert_eq!(fingerprint, fingerprint.to_uppercase());
}

#[test]
fn secrets_do_not_leak_through_debug() {
    let pair = KeyPair::generate().unwrap();
    let message = send("contraseña-reconocible");
    let code = PairingCode::generate().unwrap();
    let debug = format!("{pair:?} {message:?} {code:?}");
    assert!(!debug.contains("reconocible"));
    assert!(!debug.contains(&format!("{:?}", pair.private().expose_secret())));
    assert!(!debug.contains(&format!("{:?}", code.expose_bytes())));
}

#[test]
fn error_is_a_standard_error() {
    fn assert_error<E: std::error::Error + Send + Sync + 'static>() {}
    assert_error::<ProtocolError>();
}

proptest! {
    #[test]
    fn any_valid_password_roundtrips(chars in prop::collection::vec(any::<char>(), 1..200)) {
        let text: String = chars.into_iter().collect();
        let decoded = SendMessage::decode(&send(&text).encode().unwrap()).unwrap();
        prop_assert_eq!(decoded.password().expose_secret().as_bytes(), text.as_bytes());
    }

    #[test]
    fn any_valid_offer_roundtrips(
        ip in any::<[u8; 4]>(),
        v6 in any::<bool>(),
        ip6 in any::<[u8; 16]>(),
        port in 1u16..=u16::MAX,
    ) {
        let ip = if v6 { IpAddr::from(ip6) } else { IpAddr::from(ip) };
        prop_assume!(!ip.is_unspecified() && !ip.is_multicast());
        let address = SocketAddr::new(ip, port);
        let original = offer(address);
        let bytes = original.encode().unwrap();
        prop_assert!(bytes.len() <= MAX_PAIRING_OFFER_LEN);
        let decoded = PairingOffer::decode(&bytes).unwrap();
        prop_assert_eq!(decoded.address(), address);
        prop_assert_eq!(decoded.public_key(), original.public_key());
        prop_assert_eq!(decoded.code().expose_bytes(), original.code().expose_bytes());
    }

    #[test]
    fn random_bytes_never_panic(bytes in prop::collection::vec(any::<u8>(), 0..5000)) {
        let _ = decode_all(&bytes);
    }

    #[test]
    fn a_changed_byte_never_panics(index in any::<prop::sample::Index>(), value in any::<u8>()) {
        let valid = [
            offer(ADDRESS).encode().unwrap().to_vec(),
            send("una contraseña de prueba").encode().unwrap().to_vec(),
            Ack::new(AckOutcome::Failed).encode().unwrap().to_vec(),
        ];
        for mut bytes in valid {
            let i = index.index(bytes.len());
            bytes[i] = value;
            let _ = decode_all(&bytes);
        }
    }

    #[test]
    fn any_suffix_is_always_rejected(suffix in prop::collection::vec(any::<u8>(), 1..64)) {
        let valid = [
            offer(ADDRESS).encode().unwrap().to_vec(),
            send("una contraseña de prueba").encode().unwrap().to_vec(),
            Ack::new(AckOutcome::Copied).encode().unwrap().to_vec(),
        ];
        for mut bytes in valid {
            bytes.extend_from_slice(&suffix);
            prop_assert!(bytes.len() <= MAX_MESSAGE_LEN);
            prop_assert_eq!(decode_all(&bytes), [false, false, false]);
        }
    }
}
