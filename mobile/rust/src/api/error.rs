//! Error de la fachada: un enum sin campos, para que Dart lo reciba como un
//! `enum` (sin freezed) y para que ningún dato, y menos un secreto, cruce con
//! él.

use vault_core::vault::{Error, InvalidEntryField, StorageErrorKind};

/// Error que reciben las funciones de Dart.
///
/// Las variantes del núcleo se corresponden una a una con
/// `vault_core::vault::Error`; las de almacenamiento y de entrada no válida
/// se aplanan en una variante por caso. Las últimas cuatro son propias de la
/// fachada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaultError {
    WrongPassword,
    InvalidKdfParams,
    /// Pierde el número de versión: la app solo muestra un mensaje genérico.
    UnsupportedFormatVersion,
    /// Pierde el número de algoritmo, como `UnsupportedFormatVersion`.
    UnsupportedAlgorithm,
    MalformedEncryptedKey,
    Random,
    CorruptEntry,
    NotAVault,
    EntryNotFound,
    Locked,
    AlreadyUnlocked,
    Clock,
    WrongVaultKey,
    AlreadyExists,
    /// La contraseña maestra nueva no llega al mínimo de caracteres.
    PasswordTooShort,
    StorageIo,
    StorageBusy,
    StorageCorrupt,
    StorageReadOnly,
    StorageHotJournal,
    StorageJournalMode,
    /// También recibe los tipos de almacenamiento que el núcleo agregue en
    /// el futuro (`StorageErrorKind` es `#[non_exhaustive]`).
    StorageOther,
    InvalidEntryServiceEmpty,
    InvalidEntryServiceTooLong,
    InvalidEntryUrlTooLong,
    InvalidEntryUsernameTooLong,
    InvalidEntryPasswordEmpty,
    InvalidEntryPasswordTooLong,
    InvalidEntryNotesTooLong,
    InvalidEntryTooManyTags,
    InvalidEntryTagTooLong,
    /// Campos que el núcleo agregue en el futuro (`InvalidEntryField` es
    /// `#[non_exhaustive]`).
    InvalidEntryOther,
    /// La sesión no tiene bóveda: no se creó, no se abrió o una importación
    /// fallida no pudo reabrir la anterior.
    NoVault,
    /// `create` u `open` sobre una sesión que ya tiene una bóveda.
    VaultAlreadyOpen,
    /// El id no tiene exactamente 32 caracteres hexadecimales en minúsculas.
    InvalidEntryId,
    /// Una operación anterior hizo panic con el candado de la sesión tomado.
    SessionPoisoned,
}

/// Resultado de la fachada.
pub type FacadeResult<T> = Result<T, VaultError>;

impl From<Error> for VaultError {
    fn from(error: Error) -> Self {
        // Sin `_`: si el núcleo agrega una variante, la fachada debe dejar
        // de compilar hasta mapearla.
        match error {
            Error::WrongPassword => Self::WrongPassword,
            Error::InvalidKdfParams => Self::InvalidKdfParams,
            Error::UnsupportedFormatVersion(_) => Self::UnsupportedFormatVersion,
            Error::UnsupportedAlgorithm(_) => Self::UnsupportedAlgorithm,
            Error::MalformedEncryptedKey => Self::MalformedEncryptedKey,
            Error::Random => Self::Random,
            Error::CorruptEntry => Self::CorruptEntry,
            Error::InvalidEntry(field) => Self::from(field),
            Error::NotAVault => Self::NotAVault,
            Error::EntryNotFound => Self::EntryNotFound,
            Error::Storage(kind) => Self::from(kind),
            Error::Locked => Self::Locked,
            Error::AlreadyUnlocked => Self::AlreadyUnlocked,
            Error::Clock => Self::Clock,
            Error::WrongVaultKey => Self::WrongVaultKey,
            Error::AlreadyExists => Self::AlreadyExists,
            Error::PasswordTooShort => Self::PasswordTooShort,
        }
    }
}

impl From<StorageErrorKind> for VaultError {
    fn from(kind: StorageErrorKind) -> Self {
        match kind {
            StorageErrorKind::Io => Self::StorageIo,
            StorageErrorKind::Busy => Self::StorageBusy,
            StorageErrorKind::Corrupt => Self::StorageCorrupt,
            StorageErrorKind::ReadOnly => Self::StorageReadOnly,
            StorageErrorKind::HotJournal => Self::StorageHotJournal,
            StorageErrorKind::JournalMode => Self::StorageJournalMode,
            _ => Self::StorageOther,
        }
    }
}

impl From<InvalidEntryField> for VaultError {
    fn from(field: InvalidEntryField) -> Self {
        match field {
            InvalidEntryField::ServiceEmpty => Self::InvalidEntryServiceEmpty,
            InvalidEntryField::ServiceTooLong => Self::InvalidEntryServiceTooLong,
            InvalidEntryField::UrlTooLong => Self::InvalidEntryUrlTooLong,
            InvalidEntryField::UsernameTooLong => Self::InvalidEntryUsernameTooLong,
            InvalidEntryField::PasswordEmpty => Self::InvalidEntryPasswordEmpty,
            InvalidEntryField::PasswordTooLong => Self::InvalidEntryPasswordTooLong,
            InvalidEntryField::NotesTooLong => Self::InvalidEntryNotesTooLong,
            InvalidEntryField::TooManyTags => Self::InvalidEntryTooManyTags,
            InvalidEntryField::TagTooLong => Self::InvalidEntryTagTooLong,
            _ => Self::InvalidEntryOther,
        }
    }
}

#[cfg(test)]
mod tests;
