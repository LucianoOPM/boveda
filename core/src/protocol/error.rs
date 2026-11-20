//! Errores del protocolo.
//!
//! Ningún mensaje ni campo de estos errores contiene secretos ni bytes del
//! mensaje: solo describen qué falló. El error de `postcard` nunca se
//! reenvía tal cual.

/// Error de los tipos y la codificación del protocolo.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ProtocolError {
    /// El mensaje (o lo que se iba a emitir) pasa del máximo permitido.
    #[error("el mensaje es demasiado grande")]
    TooLarge,

    /// El mensaje termina antes de lo esperado.
    #[error("el mensaje está truncado")]
    Truncated,

    /// La versión del protocolo es desconocida.
    #[error("versión del protocolo no soportada: {0}")]
    UnsupportedVersion(u8),

    /// El tipo de mensaje no es el esperado o es desconocido.
    #[error("tipo de mensaje inesperado: {0}")]
    UnexpectedKind(u8),

    /// El cuerpo del mensaje no está bien formado.
    #[error("el mensaje está mal formado")]
    Malformed,

    /// Sobran bytes después del cuerpo del mensaje.
    #[error("el mensaje trae bytes de más")]
    TrailingBytes,

    /// Un campo está fuera de rango.
    #[error("campo no válido: {0}")]
    InvalidField(InvalidField),

    /// El generador de números aleatorios del sistema falló.
    #[error("el generador aleatorio del sistema falló")]
    Random,

    /// No se pudo codificar el mensaje.
    #[error("no se pudo codificar el mensaje")]
    Encode,
}

/// Campo de un mensaje que no cumple las reglas, y por qué.
///
/// Nunca incluye el valor del campo.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum InvalidField {
    #[error("el tiempo de vida no es el del protocolo")]
    Ttl,
    #[error("la contraseña está vacía")]
    PasswordEmpty,
    #[error("la contraseña es demasiado larga")]
    PasswordTooLong,
    #[error("el puerto no es válido")]
    Port,
    #[error("la dirección no es válida")]
    Address,
}

/// Resultado con el [`ProtocolError`].
pub type Result<T> = std::result::Result<T, ProtocolError>;

#[cfg(test)]
mod tests;
