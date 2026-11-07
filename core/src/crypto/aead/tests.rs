//! Pruebas unitarias de `crypto/aead.rs`.

use super::*;

#[test]
fn associated_data_matches_known_vector() {
    let id: [u8; ENTRY_ID_LEN] = core::array::from_fn(|i| i as u8); // 00..0f
    #[rustfmt::skip]
    let expected: [u8; AAD_LEN] = [
        b'b', b'o', b'v', b'e', b'd', b'a', b'-', b'v', b'1',
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07,
        0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
        0x01, 0x00, // versión 1, little-endian
        0x02,       // secreto
    ];
    assert_eq!(associated_data(&id, 1, Part::Secret), expected);
}

#[test]
fn associated_data_encodes_version_little_endian() {
    let ad = associated_data(&[0; ENTRY_ID_LEN], 0x0102, Part::Search);
    assert_eq!(&ad[25..27], &[0x02, 0x01]);
    assert_eq!(ad[27], 1);
}

#[test]
fn part_values_are_fixed() {
    assert_eq!(Part::Search as u8, 1);
    assert_eq!(Part::Secret as u8, 2);
}

// --- Cifrar y descifrar una parte ---

const ID: [u8; ENTRY_ID_LEN] = [7; ENTRY_ID_LEN];
const V: u16 = 1;

/// Llave de bóveda fija: estas pruebas no necesitan generarla al azar.
fn key(byte: u8) -> VaultKey {
    VaultKey::new(Box::new([byte; 32]))
}

fn seal(plaintext: &[u8]) -> EncryptedPart {
    seal_part(&key(1), &ID, V, Part::Secret, plaintext).unwrap()
}

#[test]
fn open_returns_the_same_plaintext() {
    let sealed = seal(b"hola");
    let plain = open_part(&key(1), &ID, V, Part::Secret, &sealed).unwrap();
    assert_eq!(plain.as_slice(), b"hola");
}

#[test]
fn empty_plaintext_round_trips_and_is_40_bytes() {
    let sealed = seal(b"");
    assert_eq!(sealed.as_bytes().len(), 40);
    assert_eq!(MIN_SEALED_LEN, 40);
    let plain = open_part(&key(1), &ID, V, Part::Secret, &sealed).unwrap();
    assert!(plain.is_empty());
}

#[test]
fn sealed_length_is_exact() {
    let plaintext = vec![0x5a; 16448];
    let sealed = seal(&plaintext);
    assert_eq!(sealed.as_bytes().len(), 24 + 16448 + 16);
}

#[test]
fn ciphertext_does_not_contain_plaintext() {
    let sealed = seal(&[b'A'; 64]);
    assert!(
        !sealed.as_bytes()[NONCE_LEN..]
            .windows(16)
            .any(|w| w == [b'A'; 16])
    );
}

#[test]
fn two_seals_differ() {
    let first = seal(b"mismo contenido");
    let second = seal(b"mismo contenido");
    assert_ne!(
        &first.as_bytes()[..NONCE_LEN],
        &second.as_bytes()[..NONCE_LEN]
    );
    assert_ne!(first, second);
}

#[test]
fn open_rejects_other_id() {
    let sealed = seal(b"hola");
    let other = [8; ENTRY_ID_LEN];
    assert_eq!(
        open_part(&key(1), &other, V, Part::Secret, &sealed).unwrap_err(),
        Error::CorruptEntry
    );
}

#[test]
fn open_rejects_other_version() {
    let sealed = seal(b"hola");
    assert_eq!(
        open_part(&key(1), &ID, 2, Part::Secret, &sealed).unwrap_err(),
        Error::CorruptEntry
    );
}

#[test]
fn open_rejects_other_part() {
    let sealed = seal(b"hola");
    assert_eq!(
        open_part(&key(1), &ID, V, Part::Search, &sealed).unwrap_err(),
        Error::CorruptEntry
    );
}

#[test]
fn open_rejects_other_key() {
    let sealed = seal(b"hola");
    assert_eq!(
        open_part(&key(2), &ID, V, Part::Secret, &sealed).unwrap_err(),
        Error::CorruptEntry
    );
}

#[test]
fn open_rejects_short_parts() {
    for len in [0, 1, 39, 40] {
        let part = EncryptedPart::from_bytes(vec![0; len]);
        assert_eq!(
            open_part(&key(1), &ID, V, Part::Secret, &part).unwrap_err(),
            Error::CorruptEntry,
            "longitud {len}"
        );
    }
}

#[test]
fn debug_hides_bytes() {
    let part = EncryptedPart::from_bytes(vec![0xab; 50]);
    let debug = format!("{part:?}");
    assert!(!debug.contains("171"), "{debug}"); // 0xab en decimal
    assert!(!debug.to_lowercase().contains("ab, "), "{debug}");
    assert!(debug.contains("50"), "{debug}");
}
