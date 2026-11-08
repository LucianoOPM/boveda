use super::*;
use vault_core::vault::{Error as E, InvalidEntryField as F, StorageErrorKind as S};

#[test]
fn maps_every_core_error() {
    let cases = [
        (E::WrongPassword, VaultError::WrongPassword),
        (E::InvalidKdfParams, VaultError::InvalidKdfParams),
        (
            E::UnsupportedFormatVersion(9),
            VaultError::UnsupportedFormatVersion,
        ),
        (E::UnsupportedAlgorithm(7), VaultError::UnsupportedAlgorithm),
        (E::MalformedEncryptedKey, VaultError::MalformedEncryptedKey),
        (E::Random, VaultError::Random),
        (E::CorruptEntry, VaultError::CorruptEntry),
        (E::NotAVault, VaultError::NotAVault),
        (E::EntryNotFound, VaultError::EntryNotFound),
        (E::Locked, VaultError::Locked),
        (E::AlreadyUnlocked, VaultError::AlreadyUnlocked),
        (E::Clock, VaultError::Clock),
        (E::WrongVaultKey, VaultError::WrongVaultKey),
        (E::AlreadyExists, VaultError::AlreadyExists),
        (E::Storage(S::Io), VaultError::StorageIo),
        (E::Storage(S::Busy), VaultError::StorageBusy),
        (E::Storage(S::Corrupt), VaultError::StorageCorrupt),
        (E::Storage(S::ReadOnly), VaultError::StorageReadOnly),
        (E::Storage(S::HotJournal), VaultError::StorageHotJournal),
        (E::Storage(S::JournalMode), VaultError::StorageJournalMode),
        (E::Storage(S::Other), VaultError::StorageOther),
        (
            E::InvalidEntry(F::ServiceEmpty),
            VaultError::InvalidEntryServiceEmpty,
        ),
        (
            E::InvalidEntry(F::ServiceTooLong),
            VaultError::InvalidEntryServiceTooLong,
        ),
        (
            E::InvalidEntry(F::UrlTooLong),
            VaultError::InvalidEntryUrlTooLong,
        ),
        (
            E::InvalidEntry(F::UsernameTooLong),
            VaultError::InvalidEntryUsernameTooLong,
        ),
        (
            E::InvalidEntry(F::PasswordEmpty),
            VaultError::InvalidEntryPasswordEmpty,
        ),
        (
            E::InvalidEntry(F::PasswordTooLong),
            VaultError::InvalidEntryPasswordTooLong,
        ),
        (
            E::InvalidEntry(F::NotesTooLong),
            VaultError::InvalidEntryNotesTooLong,
        ),
        (
            E::InvalidEntry(F::TooManyTags),
            VaultError::InvalidEntryTooManyTags,
        ),
        (
            E::InvalidEntry(F::TagTooLong),
            VaultError::InvalidEntryTagTooLong,
        ),
    ];
    for (core, expected) in cases {
        assert_eq!(VaultError::from(core), expected);
    }
}
