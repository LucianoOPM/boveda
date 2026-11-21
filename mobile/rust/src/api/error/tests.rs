use super::*;
use std::io::ErrorKind;
use vault_core::protocol::{PairingError, ProtocolError, Refusal, SendError};
use vault_core::vault::{
    Error as E, InvalidEntryField as F, InvalidPairedPcField as P, StorageErrorKind as S,
};

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
        (E::PasswordTooShort, VaultError::PasswordTooShort),
        (E::CorruptIdentity, VaultError::CorruptIdentity),
        (E::CorruptPairedPc, VaultError::CorruptPairedPc),
        (E::PairedPcNotFound, VaultError::PairedPcNotFound),
        (E::PairedPcExists, VaultError::PairedPcExists),
        (
            E::InvalidPairedPc(P::NameEmpty),
            VaultError::InvalidPairedPcNameEmpty,
        ),
        (
            E::InvalidPairedPc(P::NameTooLong),
            VaultError::InvalidPairedPcNameTooLong,
        ),
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

#[test]
fn maps_every_pairing_error() {
    let cases = [
        (
            PairingError::Refused(Refusal::WrongCode),
            VaultError::ProtocolFailure,
        ),
        (PairingError::PeerNotAuthentic, VaultError::PeerNotAuthentic),
        (PairingError::Rejected, VaultError::PairingRejected),
        (
            PairingError::ConfirmationTimeout,
            VaultError::PairingConfirmationTimeout,
        ),
        (PairingError::Timeout, VaultError::NetworkTimeout),
        (PairingError::Cancelled, VaultError::Cancelled),
        (PairingError::Closed, VaultError::ConnectionClosed),
        (PairingError::Io(ErrorKind::TimedOut), VaultError::NetworkIo),
        (PairingError::FrameTooLarge, VaultError::ProtocolFailure),
        (PairingError::Handshake, VaultError::ProtocolFailure),
        (
            PairingError::Protocol(ProtocolError::Malformed),
            VaultError::ProtocolFailure,
        ),
    ];
    for (core, expected) in cases {
        assert_eq!(VaultError::from(core), expected);
    }
}

#[test]
fn maps_every_send_error() {
    let cases = [
        (SendError::NotPaired, VaultError::ProtocolFailure),
        (SendError::Expired, VaultError::SendExpired),
        (SendError::PeerNotAuthentic, VaultError::PeerNotAuthentic),
        (SendError::Timeout, VaultError::NetworkTimeout),
        (SendError::Cancelled, VaultError::Cancelled),
        (SendError::Closed, VaultError::ConnectionClosed),
        (SendError::Io(ErrorKind::TimedOut), VaultError::NetworkIo),
        (SendError::FrameTooLarge, VaultError::ProtocolFailure),
        (SendError::Handshake, VaultError::ProtocolFailure),
        (
            SendError::Protocol(ProtocolError::Malformed),
            VaultError::ProtocolFailure,
        ),
    ];
    for (core, expected) in cases {
        assert_eq!(VaultError::from(core), expected);
    }
}
