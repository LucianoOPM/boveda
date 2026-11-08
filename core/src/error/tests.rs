//! Pruebas unitarias de `error.rs`.

use super::*;

const ALL_STORAGE_KINDS: [StorageErrorKind; 7] = [
    StorageErrorKind::Io,
    StorageErrorKind::Busy,
    StorageErrorKind::Corrupt,
    StorageErrorKind::ReadOnly,
    StorageErrorKind::HotJournal,
    StorageErrorKind::JournalMode,
    StorageErrorKind::Other,
];

#[test]
fn storage_error_messages_are_spanish_and_fixed() {
    assert_eq!(Error::NotAVault.to_string(), "el archivo no es una bóveda");
    assert_eq!(Error::EntryNotFound.to_string(), "entrada no encontrada");
    assert!(
        Error::Storage(StorageErrorKind::Corrupt)
            .to_string()
            .starts_with("error de almacenamiento: ")
    );
}

#[test]
fn every_storage_kind_has_a_distinct_message() {
    let messages: Vec<String> = ALL_STORAGE_KINDS.iter().map(|k| k.to_string()).collect();
    for (i, message) in messages.iter().enumerate() {
        assert!(!message.is_empty());
        assert!(
            !messages[i + 1..].contains(message),
            "mensaje repetido: {message}"
        );
    }
}

#[test]
fn already_exists_message_is_fixed() {
    assert_eq!(Error::AlreadyExists.to_string(), "el archivo ya existe");
}

#[test]
fn new_vault_error_messages_are_fixed() {
    assert_eq!(Error::Locked.to_string(), "la bóveda está bloqueada");
    assert_eq!(
        Error::AlreadyUnlocked.to_string(),
        "la bóveda ya está desbloqueada"
    );
    assert_eq!(Error::Clock.to_string(), "la hora del sistema no es válida");
}
