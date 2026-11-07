//! Pruebas unitarias de `vault/entry.rs`.

use super::*;

// --- Identificador ---

#[test]
fn generated_ids_are_16_bytes_and_distinct() {
    let a = EntryId::generate().unwrap();
    let b = EntryId::generate().unwrap();
    assert_eq!(a.as_bytes().len(), ENTRY_ID_LEN);
    assert_ne!(a, b);
}

#[test]
fn id_round_trips_through_bytes() {
    let bytes: [u8; 16] = core::array::from_fn(|i| i as u8);
    assert_eq!(EntryId::from_bytes(bytes).as_bytes(), &bytes);
}
