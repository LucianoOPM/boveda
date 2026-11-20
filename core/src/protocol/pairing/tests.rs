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
