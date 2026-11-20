//! Pruebas unitarias de `vault/paired_pc.rs`.

use std::net::{Ipv4Addr, Ipv6Addr};

use super::*;
use crate::crypto::keys::VaultKey;

fn key(byte: u8) -> VaultKey {
    VaultKey::new(Box::new([byte; 32]))
}

fn id(byte: u8) -> PairedPcId {
    PairedPcId::from_bytes([byte; PAIRED_PC_ID_LEN])
}

fn pc(last_ip: Option<IpAddr>, last_seen_at: Option<i64>) -> PairedPc {
    PairedPc::new(
        id(3),
        [9; PUBLIC_KEY_LEN],
        "Escritorio".to_owned(),
        last_ip,
        1_800_000_000_000,
        last_seen_at,
    )
}

#[test]
fn associated_data_has_the_documented_layout() {
    let ad = associated_data(&[0xA5; PAIRED_PC_ID_LEN], 1);
    assert_eq!(ad.len(), 27);
    assert_eq!(&ad[..9], b"boveda-pc");
    assert_eq!(&ad[9..25], &[0xA5; 16]);
    assert_eq!(&ad[25..], &[0x01, 0x00]);
    assert_eq!(associated_data(&[0; 16], 0x0102)[25..], [0x02, 0x01]);
}

#[test]
fn seal_and_open_round_trip_with_ipv4_ipv6_and_no_ip() {
    let cases = [
        pc(None, None),
        pc(
            Some(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 77))),
            Some(1_800_000_000_500),
        ),
        pc(Some(IpAddr::V6(Ipv6Addr::LOCALHOST)), Some(5)),
    ];
    for original in cases {
        let sealed = seal(&key(1), &original).unwrap();
        let opened = open(&key(1), original.id(), PC_SCHEMA_VERSION, &sealed).unwrap();
        assert_eq!(opened.id(), original.id());
        assert_eq!(opened.public_key(), original.public_key());
        assert_eq!(opened.name(), original.name());
        assert_eq!(opened.last_ip(), original.last_ip());
        assert_eq!(opened.paired_at(), original.paired_at());
        assert_eq!(opened.last_seen_at(), original.last_seen_at());
    }
}

#[test]
fn name_and_ip_are_not_in_the_ciphertext() {
    let mut original = pc(Some(IpAddr::V4(Ipv4Addr::new(10, 20, 30, 40))), None);
    original.name = "equipo-canario-xyz".to_owned();
    let sealed = seal(&key(1), &original).unwrap();
    let bytes = sealed.as_bytes();
    assert!(!bytes.windows(10).any(|w| w == b"equipo-can"));
}

#[test]
fn open_rejects_another_id_key_and_version() {
    let original = pc(None, None);
    let sealed = seal(&key(1), &original).unwrap();
    // Movida a otro id.
    assert_eq!(
        open(&key(1), id(4), PC_SCHEMA_VERSION, &sealed).err(),
        Some(Error::CorruptPairedPc)
    );
    assert_eq!(
        open(&key(2), original.id(), PC_SCHEMA_VERSION, &sealed).err(),
        Some(Error::CorruptPairedPc)
    );
    assert_eq!(
        open(&key(1), original.id(), PC_SCHEMA_VERSION + 1, &sealed).err(),
        Some(Error::CorruptPairedPc)
    );
}

#[test]
fn open_rejects_every_flipped_byte() {
    let original = pc(Some(IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4))), Some(7));
    let sealed = seal(&key(1), &original).unwrap();
    for i in 0..sealed.as_bytes().len() {
        let mut bytes = sealed.as_bytes().to_vec();
        bytes[i] ^= 0x01;
        let altered = EncryptedPart::from_bytes(bytes);
        assert_eq!(
            open(&key(1), original.id(), PC_SCHEMA_VERSION, &altered).err(),
            Some(Error::CorruptPairedPc),
            "byte {i}"
        );
    }
}

/// Cifra `plain` tal cual, con el dato autenticado de la PC, para probar
/// contenidos bien autenticados pero inválidos.
fn sealed_plain(plain: &[u8]) -> EncryptedPart {
    aead::seal_with_ad(
        &key(1),
        &associated_data(id(3).as_bytes(), PC_SCHEMA_VERSION),
        plain,
    )
    .unwrap()
}

fn raw_bytes(raw: &RawPcV1) -> Vec<u8> {
    postcard::to_allocvec(raw).unwrap()
}

fn valid_raw() -> RawPcV1 {
    RawPcV1 {
        public_key: [9; PUBLIC_KEY_LEN],
        name: "Escritorio".to_owned(),
        last_ip: None,
        paired_at: 1,
        last_seen_at: None,
    }
}

#[test]
fn open_rejects_an_ip_that_is_not_4_or_16_bytes() {
    for len in [0usize, 1, 5, 15, 17] {
        let mut raw = valid_raw();
        raw.last_ip = Some(vec![1; len]);
        let sealed = sealed_plain(&raw_bytes(&raw));
        assert_eq!(
            open(&key(1), id(3), PC_SCHEMA_VERSION, &sealed).err(),
            Some(Error::CorruptPairedPc),
            "longitud {len}"
        );
    }
}

#[test]
fn open_rejects_an_invalid_name_and_trailing_bytes() {
    let mut raw = valid_raw();
    raw.name = "   ".to_owned();
    assert_eq!(
        open(
            &key(1),
            id(3),
            PC_SCHEMA_VERSION,
            &sealed_plain(&raw_bytes(&raw))
        )
        .err(),
        Some(Error::CorruptPairedPc)
    );
    raw.name = "x".repeat(PAIRED_PC_NAME_MAX_BYTES + 1);
    assert_eq!(
        open(
            &key(1),
            id(3),
            PC_SCHEMA_VERSION,
            &sealed_plain(&raw_bytes(&raw))
        )
        .err(),
        Some(Error::CorruptPairedPc)
    );
    let mut bytes = raw_bytes(&valid_raw());
    bytes.push(0);
    assert_eq!(
        open(&key(1), id(3), PC_SCHEMA_VERSION, &sealed_plain(&bytes)).err(),
        Some(Error::CorruptPairedPc)
    );
    // Y la válida sí abre.
    assert!(
        open(
            &key(1),
            id(3),
            PC_SCHEMA_VERSION,
            &sealed_plain(&raw_bytes(&valid_raw()))
        )
        .is_ok()
    );
}

#[test]
fn validate_name_trims_and_enforces_the_limits_in_bytes() {
    assert_eq!(validate_name("  Casa  ").unwrap(), "Casa");
    assert_eq!(
        validate_name("").err(),
        Some(Error::InvalidPairedPc(InvalidPairedPcField::NameEmpty))
    );
    assert_eq!(
        validate_name(" \t ").err(),
        Some(Error::InvalidPairedPc(InvalidPairedPcField::NameEmpty))
    );
    assert!(validate_name(&"a".repeat(128)).is_ok());
    assert_eq!(
        validate_name(&"a".repeat(129)).err(),
        Some(Error::InvalidPairedPc(InvalidPairedPcField::NameTooLong))
    );
    // 64 × "é" son 128 bytes; 65 son 130.
    assert!(validate_name(&"é".repeat(64)).is_ok());
    assert_eq!(
        validate_name(&"é".repeat(65)).err(),
        Some(Error::InvalidPairedPc(InvalidPairedPcField::NameTooLong))
    );
}

#[test]
fn debug_shows_only_the_id() {
    let shown = format!(
        "{:?}",
        pc(Some(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 9))), None)
    );
    assert!(shown.contains("PairedPc"));
    assert!(!shown.contains("Escritorio"));
    assert!(!shown.contains("10.0.0.9"));
}

#[test]
fn generated_ids_differ() {
    assert_ne!(
        PairedPcId::generate().unwrap(),
        PairedPcId::generate().unwrap()
    );
}
