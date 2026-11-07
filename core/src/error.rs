//! Errores del núcleo.
//!
//! Ningún mensaje ni campo de estos errores contiene secretos: solo describen
//! qué falló, nunca con qué datos.

/// Error de las operaciones de `vault-core`.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Error {
    /// La llave derivada no autenticó la llave de bóveda cifrada.
    ///
    /// El cifrado autenticado no distingue una contraseña incorrecta de una
    /// cabecera alterada: ambos casos llegan aquí.
    #[error("contraseña incorrecta o cabecera dañada")]
    WrongPassword,

    /// Parámetros de Argon2id fuera de los límites permitidos.
    #[error("parámetros de derivación de llave no válidos")]
    InvalidKdfParams,

    /// La cabecera declara una versión de formato que esta versión no conoce.
    #[error("versión de formato de bóveda no soportada: {0}")]
    UnsupportedFormatVersion(u16),

    /// La cabecera declara un algoritmo de derivación desconocido.
    #[error("algoritmo de derivación de llave no soportado: {0}")]
    UnsupportedAlgorithm(u8),

    /// La llave de bóveda cifrada no tiene la longitud esperada.
    #[error("llave de bóveda cifrada con longitud no válida")]
    MalformedEncryptedKey,

    /// El generador de números aleatorios del sistema falló.
    #[error("el generador aleatorio del sistema falló")]
    Random,

    /// La entrada no se pudo leer: versión de estructura desconocida, bytes
    /// mal formados o contenido que viola las reglas de su versión.
    #[error("entrada dañada")]
    CorruptEntry,

    /// Los datos para crear o editar una entrada no cumplen las reglas.
    #[error("entrada no válida: {0}")]
    InvalidEntry(InvalidEntryField),

    /// El archivo no es una bóveda: no es SQLite o su `application_id` no es
    /// el de Bóveda.
    #[error("el archivo no es una bóveda")]
    NotAVault,

    /// No existe una entrada viva con ese identificador (nunca existió o es
    /// una lápida).
    #[error("entrada no encontrada")]
    EntryNotFound,

    /// Falló el almacenamiento. Nunca incluye el error original de SQLite,
    /// que puede traer SQL, rutas o valores de columnas.
    #[error("error de almacenamiento: {0}")]
    Storage(StorageErrorKind),

    /// `create` sobre una ruta que ya existe. No incluye la ruta.
    #[error("el archivo ya existe")]
    AlreadyExists,
}

/// Tipo de fallo del almacenamiento, sin datos.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum StorageErrorKind {
    /// Disco lleno, permisos o error de lectura o escritura.
    #[error("error de lectura o escritura del archivo")]
    Io,
    /// Otro proceso tiene bloqueado el archivo.
    #[error("el archivo está bloqueado por otro proceso")]
    Busy,
    /// SQLite detectó que el archivo entero está dañado.
    #[error("el archivo de la bóveda está dañado")]
    Corrupt,
    /// Escritura sobre una conexión o un archivo de solo lectura.
    #[error("el archivo se abrió en solo lectura")]
    ReadOnly,
    /// El archivo trae un `-journal` caliente y se abrió en solo lectura.
    #[error("el archivo tiene una transacción sin terminar")]
    HotJournal,
    /// El modo del diario no quedó en `delete`.
    #[error("no se pudo fijar el modo del diario")]
    JournalMode,
    /// Un código de SQLite no previsto.
    #[error("error interno de SQLite")]
    Other,
}

/// Campo de una entrada que no cumple las reglas, y por qué.
///
/// Nunca incluye el valor del campo.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum InvalidEntryField {
    #[error("el servicio está vacío")]
    ServiceEmpty,
    #[error("el servicio es demasiado largo")]
    ServiceTooLong,
    #[error("la URL es demasiado larga")]
    UrlTooLong,
    #[error("el usuario es demasiado largo")]
    UsernameTooLong,
    #[error("la contraseña está vacía")]
    PasswordEmpty,
    #[error("la contraseña es demasiado larga")]
    PasswordTooLong,
    #[error("las notas son demasiado largas")]
    NotesTooLong,
    #[error("hay demasiadas etiquetas")]
    TooManyTags,
    #[error("una etiqueta es demasiado larga")]
    TagTooLong,
}

/// Resultado con el [`Error`] del núcleo.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests;
