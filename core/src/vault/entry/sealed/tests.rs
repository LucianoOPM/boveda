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

// --- Vectores de descifrado v1 ---

// Formato v1. No modificar: si esta prueba falla, cambió el formato en disco.
//
// Generados una sola vez con `SealedEntry::seal`: llave `00..1f`, id `a0..af`,
// versión de estructura 1; servicio "Correo", URL "https://correo.example",
// usuario "ana@example.com", etiquetas ["Trabajo", "Médico"]; contraseña
// "contraseña de prueba 1" y notas "Notas de prueba".
const V1_SEARCH_HEX: &str = "f38e6e53187979eff2cba63b3c212032f84a98183efcb6eae942cb1a0d19b9c0f95e00578dd612e1205e41892679adc2f38e902ff9ae8cc52ff0220ae6a187087023f3608918d8d9a810af0d131624bda14cbbe8b306218ebad53811cfb43709da99c996f3de34ddf4";
const V1_SECRET_HEX: &str = "96b6fe039149fe1a03760e6d8f6d7c98e9c0ebd6544f08cdb9b71d3af68dc21005dc2b511b1b74de3bcd4c851a545471cc474aeafd57565c1701af981de69f5922b4d612d123197e9e744bbce25b5ae69e";

/// Decodifica hexadecimal en minúsculas (solo para los vectores de prueba).
fn hex(s: &str) -> Vec<u8> {
    assert_eq!(s.len() % 2, 0);
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

#[test]
fn v1_vectors_decrypt_to_known_content() {
    let key = VaultKey::new(Box::new(core::array::from_fn(|i| i as u8)));
    let id = EntryId::from_bytes(core::array::from_fn(|i| 0xa0 + i as u8));
    let sealed = SealedEntry::from_parts(
        id,
        1,
        EncryptedPart::from_bytes(hex(V1_SEARCH_HEX)),
        EncryptedPart::from_bytes(hex(V1_SECRET_HEX)),
    );

    let search = sealed.open_search(&key).unwrap();
    assert_eq!(search.service(), "Correo");
    assert_eq!(search.url(), Some("https://correo.example"));
    assert_eq!(search.username(), Some("ana@example.com"));
    assert_eq!(search.tags(), ["Trabajo", "Médico"]);

    let secret = sealed.open_secret(&key).unwrap();
    assert_eq!(secret.password().expose_secret(), "contraseña de prueba 1");
    assert_eq!(
        secret.notes().map(|n| n.expose_secret()),
        Some("Notas de prueba")
    );
}

// --- Propiedades ---

mod props {
    use proptest::collection::vec;
    use proptest::option;
    use proptest::prelude::*;

    use super::*;

    /// Texto arbitrario o palabras con espacios y mayúsculas mezcladas.
    fn text() -> impl Strategy<Value = String> {
        prop_oneof![any::<String>(), "[ \t]{0,3}[a-zA-ZñÑ]{1,8}[ \t]{0,3}"]
    }

    proptest! {
        #[test]
        fn sealed_round_trip_prop(
            service in text(),
            url in option::of(text()),
            username in option::of(text()),
            tags in vec(text(), 0..8),
            password in any::<String>(),
            notes in option::of(any::<String>()),
            id in any::<[u8; 16]>(),
        ) {
            let tags: Vec<&str> = tags.iter().map(String::as_str).collect();
            let search = SearchData::new(&service, url.as_deref(), username.as_deref(), &tags);
            let secret = Secret::new(
                SecretString::from(password.as_str()),
                notes.as_deref().map(SecretString::from),
            );
            if let (Ok(search), Ok(secret)) = (search, secret) {
                let sealed =
                    SealedEntry::seal(&key(5), EntryId::from_bytes(id), &search, &secret).unwrap();
                prop_assert_eq!(sealed.open_search(&key(5)).unwrap(), search);
                let opened = sealed.open_secret(&key(5)).unwrap();
                prop_assert_eq!(
                    opened.password().expose_secret(),
                    secret.password().expose_secret()
                );
                prop_assert_eq!(
                    opened.notes().map(|n| n.expose_secret().to_owned()),
                    secret.notes().map(|n| n.expose_secret().to_owned())
                );
            }
        }
    }
}
