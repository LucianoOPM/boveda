//! Error de la fachada: un enum sin campos, para que Dart lo reciba como un
//! `enum` (sin freezed) y para que ningún dato, y menos un secreto, cruce con
//! él.

use vault_core::protocol::{PairingError, SendError};
use vault_core::vault::{Error, InvalidEntryField, InvalidPairedPcField, StorageErrorKind};

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
    /// Ya hay una operación de red en curso (emparejar o enviar).
    Busy,
    /// La operación de red se canceló: la bóveda se bloqueó (o se importó un
    /// respaldo) mientras corría.
    Cancelled,
    /// El texto del QR no es una oferta de emparejamiento válida.
    InvalidPairingOffer,
    /// El nombre del celular no es válido (vacío, largo o con caracteres de
    /// control).
    InvalidDeviceName,
    /// La dirección de la PC no es `ip:puerto`.
    InvalidAddress,
    /// El id de la PC no tiene exactamente 32 caracteres hexadecimales en
    /// minúsculas.
    InvalidPairedPcId,
    /// La identidad del celular existe pero no se puede leer.
    CorruptIdentity,
    /// La fila de la PC emparejada no se puede leer.
    CorruptPairedPc,
    PairedPcNotFound,
    /// Esa llave pública ya está en la lista de PCs.
    PairedPcExists,
    InvalidPairedPcNameEmpty,
    InvalidPairedPcNameTooLong,
    /// Campos que el núcleo agregue en el futuro (`InvalidPairedPcField` es
    /// `#[non_exhaustive]`).
    InvalidPairedPcOther,
    /// No se pudo abrir la conexión con la PC (rechazada, inalcanzable o sin
    /// respuesta en el plazo).
    ConnectFailed,
    /// Se agotó el tiempo de un paso del protocolo.
    NetworkTimeout,
    /// La PC cerró la conexión. No distingue un corte de red de «la PC no
    /// reconoce al celular»: por diseño se ven igual.
    ConnectionClosed,
    /// Error de entrada o salida de la red.
    NetworkIo,
    /// La respuesta de la PC no se autentica: no es la PC esperada.
    PeerNotAuthentic,
    /// El usuario rechazó el emparejamiento en la PC.
    PairingRejected,
    /// El usuario no decidió a tiempo en la PC.
    PairingConfirmationTimeout,
    /// La PC consideró caducado el envío.
    SendExpired,
    /// El mensaje no cumple el protocolo, falló el apretón de manos o el
    /// núcleo agregó un error que la fachada aún no distingue.
    ProtocolFailure,
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
            Error::CorruptIdentity => Self::CorruptIdentity,
            Error::CorruptPairedPc => Self::CorruptPairedPc,
            Error::PairedPcNotFound => Self::PairedPcNotFound,
            Error::PairedPcExists => Self::PairedPcExists,
            Error::InvalidPairedPc(field) => Self::from(field),
        }
    }
}

impl From<InvalidPairedPcField> for VaultError {
    fn from(field: InvalidPairedPcField) -> Self {
        match field {
            InvalidPairedPcField::NameEmpty => Self::InvalidPairedPcNameEmpty,
            InvalidPairedPcField::NameTooLong => Self::InvalidPairedPcNameTooLong,
            _ => Self::InvalidPairedPcOther,
        }
    }
}

impl From<PairingError> for VaultError {
    fn from(error: PairingError) -> Self {
        // `PairingError` es `#[non_exhaustive]`: lo que el núcleo agregue cae
        // en `ProtocolFailure`. `Refused` solo lo ve la app de la PC.
        match error {
            PairingError::PeerNotAuthentic => Self::PeerNotAuthentic,
            PairingError::Rejected => Self::PairingRejected,
            PairingError::ConfirmationTimeout => Self::PairingConfirmationTimeout,
            PairingError::Timeout => Self::NetworkTimeout,
            PairingError::Cancelled => Self::Cancelled,
            PairingError::Closed => Self::ConnectionClosed,
            PairingError::Io(_) => Self::NetworkIo,
            _ => Self::ProtocolFailure,
        }
    }
}

impl From<SendError> for VaultError {
    fn from(error: SendError) -> Self {
        // `SendError` es `#[non_exhaustive]`. `NotPaired` solo lo ve la app
        // de la PC.
        match error {
            SendError::Expired => Self::SendExpired,
            SendError::PeerNotAuthentic => Self::PeerNotAuthentic,
            SendError::Timeout => Self::NetworkTimeout,
            SendError::Cancelled => Self::Cancelled,
            SendError::Closed => Self::ConnectionClosed,
            SendError::Io(_) => Self::NetworkIo,
            _ => Self::ProtocolFailure,
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
