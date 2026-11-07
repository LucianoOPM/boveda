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
