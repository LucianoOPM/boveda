//! Pruebas unitarias de `vault/entry/sealed.rs`.

use secrecy::{ExposeSecret, SecretString};

use super::*;

const V: u16 = ENTRY_SCHEMA_VERSION;

/// Llave de bóveda fija: estas pruebas no necesitan generarla al azar.
fn key(byte: u8) -> VaultKey {
    VaultKey::new(Box::new([byte; 32]))
}

fn sample_search() -> SearchData {
    SearchData::new(
        "Correo",
        Some("https://correo.example"),
        Some("ana@example.com"),
        &["Trabajo", "Personal"],
    )
    .unwrap()
}

fn sample_secret() -> Secret {
    Secret::new(
        SecretString::from("contraseña secreta"),
        Some(SecretString::from("unas notas")),
    )
    .unwrap()
}

fn sample(id_byte: u8) -> SealedEntry {
    SealedEntry::seal(
        &key(1),
        EntryId::from_bytes([id_byte; 16]),
        &sample_search(),
        &sample_secret(),
    )
    .unwrap()
}

#[test]
fn entry_id_len_matches_aead() {
    assert_eq!(aead::ENTRY_ID_LEN, super::super::ENTRY_ID_LEN);
}

#[test]
fn seal_then_open_returns_same_entry() {
    let sealed = sample(1);
    assert_eq!(sealed.entry_schema_version(), ENTRY_SCHEMA_VERSION);
    assert_eq!(sealed.id(), EntryId::from_bytes([1; 16]));
    assert_eq!(sealed.open_search(&key(1)).unwrap(), sample_search());
    let secret = sealed.open_secret(&key(1)).unwrap();
    assert_eq!(secret.password().expose_secret(), "contraseña secreta");
    assert_eq!(
        secret.notes().map(|n| n.expose_secret()),
        Some("unas notas")
    );
}

#[test]
fn secret_moved_to_other_entry_is_rejected() {
    let a = sample(1);
    let b = sample(2);
    let moved = SealedEntry::from_parts(
        b.id(),
        V,
        b.search_bytes().clone(),
        a.secret_bytes().clone(),
    );
    assert_eq!(moved.open_secret(&key(1)).unwrap_err(), Error::CorruptEntry);
    assert!(moved.open_search(&key(1)).is_ok());
}

#[test]
fn swapped_parts_are_rejected() {
    let s = sample(1);
    let swapped = SealedEntry::from_parts(
        s.id(),
        V,
        s.secret_bytes().clone(),
        s.search_bytes().clone(),
    );
    assert_eq!(
        swapped.open_search(&key(1)).unwrap_err(),
        Error::CorruptEntry
    );
    assert_eq!(
        swapped.open_secret(&key(1)).unwrap_err(),
        Error::CorruptEntry
    );
}

#[test]
fn other_version_is_rejected() {
    let s = sample(1);
    for version in [2, 0, u16::MAX] {
        let other = SealedEntry::from_parts(
            s.id(),
            version,
            s.search_bytes().clone(),
            s.secret_bytes().clone(),
        );
        assert_eq!(
            other.open_search(&key(1)).unwrap_err(),
            Error::CorruptEntry,
            "versión {version}"
        );
        assert_eq!(
            other.open_secret(&key(1)).unwrap_err(),
            Error::CorruptEntry,
            "versión {version}"
        );
    }
}

#[test]
fn other_vault_key_is_rejected() {
    let s = sample(1);
    assert_eq!(s.open_search(&key(2)).unwrap_err(), Error::CorruptEntry);
    assert_eq!(s.open_secret(&key(2)).unwrap_err(), Error::CorruptEntry);
}

#[test]
fn debug_hides_ciphertext() {
    let s = sample(1);
    let debug = format!("{s:?}");
    assert!(!debug.contains("Correo"), "{debug}");
    assert!(!debug.contains("contraseña"), "{debug}");
    let first = s.search_bytes().as_bytes()[0];
    let second = s.search_bytes().as_bytes()[1];
    assert!(!debug.contains(&format!("{first}, {second}")), "{debug}");
    assert!(debug.contains("entry_schema_version: 1"), "{debug}");
}
