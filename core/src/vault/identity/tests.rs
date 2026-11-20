//! Pruebas unitarias de `vault/identity.rs`.

use secrecy::ExposeSecret;

use super::*;
use crate::crypto::keys::VaultKey;

fn key(byte: u8) -> VaultKey {
    VaultKey::new(Box::new([byte; 32]))
}

fn identity_of(byte: u8) -> Identity {
    Identity::from_bytes([byte; IDENTITY_KEY_LEN])
}

#[test]
fn associated_data_has_the_documented_layout() {
    let ad = associated_data(1);
    assert_eq!(ad.len(), 17);
    assert_eq!(&ad[..15], b"boveda-identity");
    assert_eq!(&ad[15..], &[0x01, 0x00]);
    assert_eq!(associated_data(0x0102)[15..], [0x02, 0x01]);
}

#[test]
fn sealed_identity_is_72_bytes_and_round_trips() {
    let sealed = seal(&key(1), &identity_of(9)).unwrap();
    assert_eq!(sealed.as_bytes().len(), 72);
    let opened = open(&key(1), IDENTITY_SCHEMA_VERSION, &sealed).unwrap();
    assert_eq!(opened.private_key().expose_secret(), &[9u8; 32]);
}

#[test]
fn sealed_identity_does_not_contain_the_private_key() {
    let sealed = seal(&key(1), &identity_of(0xAB)).unwrap();
    assert!(!sealed.as_bytes().windows(8).any(|w| w == [0xAB; 8]));
}

#[test]
fn open_rejects_another_key_and_another_version() {
    let sealed = seal(&key(1), &identity_of(9)).unwrap();
    assert_eq!(
        open(&key(2), IDENTITY_SCHEMA_VERSION, &sealed).err(),
        Some(Error::CorruptIdentity)
    );
    assert_eq!(
        open(&key(1), IDENTITY_SCHEMA_VERSION + 1, &sealed).err(),
        Some(Error::CorruptIdentity)
    );
}

#[test]
fn open_rejects_every_flipped_byte() {
    let sealed = seal(&key(1), &identity_of(9)).unwrap();
    for i in 0..sealed.as_bytes().len() {
        let mut bytes = sealed.as_bytes().to_vec();
        bytes[i] ^= 0x01;
        let altered = EncryptedPart::from_bytes(bytes);
        assert_eq!(
            open(&key(1), IDENTITY_SCHEMA_VERSION, &altered).err(),
            Some(Error::CorruptIdentity),
            "byte {i}"
        );
    }
}

#[test]
fn open_rejects_a_plaintext_that_is_not_32_bytes() {
    // Bien autenticado pero con otra longitud: es una identidad dañada.
    let sealed = aead::seal_with_ad(
        &key(1),
        &associated_data(IDENTITY_SCHEMA_VERSION),
        &[5u8; 31],
    )
    .unwrap();
    assert_eq!(
        open(&key(1), IDENTITY_SCHEMA_VERSION, &sealed).err(),
        Some(Error::CorruptIdentity)
    );
}

#[test]
fn open_does_not_validate_or_transform_the_key_bytes() {
    // Cualquier secuencia de 32 bytes sirve como llave privada X25519 (se
    // recorta al usarla): todo ceros y todo unos también se leen tal cual.
    for byte in [0x00u8, 0xFF] {
        let sealed = seal(&key(1), &identity_of(byte)).unwrap();
        let opened = open(&key(1), IDENTITY_SCHEMA_VERSION, &sealed).unwrap();
        assert_eq!(opened.private_key().expose_secret(), &[byte; 32]);
    }
}

#[test]
fn debug_does_not_show_the_key() {
    let shown = format!("{:?}", identity_of(7));
    assert_eq!(shown, "Identity { .. }");
}

#[test]
fn generated_identities_differ() {
    let a = generate().unwrap();
    let b = generate().unwrap();
    assert_ne!(
        a.private_key().expose_secret(),
        b.private_key().expose_secret()
    );
    assert_ne!(a.private_key().expose_secret(), &[0u8; 32]);
}
