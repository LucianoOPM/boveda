//! Regla de la contraseña maestra.
//!
//! Se basa solo en la longitud, como recomienda NIST SP 800-63B, y no en
//! tipos de caracteres (una regla de tipos acepta `P@ssword1`). Cuenta
//! caracteres Unicode (`char`), no bytes, y no recorta: los espacios cuentan.
//!
//! Se aplica solo al crear la bóveda y al cambiar la contraseña maestra,
//! nunca al desbloquear ni al importar: las bóvedas y los respaldos que ya
//! existen con una contraseña más corta siguen abriendo.

use secrecy::{ExposeSecret, SecretString};

use crate::error::{Error, Result};

/// Longitud mínima de la contraseña maestra, en caracteres.
pub const MIN_MASTER_PASSWORD_CHARS: usize = 12;

/// Desde esta longitud, en caracteres, la contraseña cuenta como fuerte.
pub const STRONG_MASTER_PASSWORD_CHARS: usize = 16;

/// Fortaleza de una contraseña maestra según su longitud.
///
/// Puede derivar `Debug`: no contiene la contraseña.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasswordStrength {
    /// Menos de [`MIN_MASTER_PASSWORD_CHARS`]: no se permite.
    TooShort,
    /// Desde el mínimo y hasta antes de [`STRONG_MASTER_PASSWORD_CHARS`].
    Acceptable,
    /// [`STRONG_MASTER_PASSWORD_CHARS`] o más.
    Strong,
}

/// Fortaleza de `password`. Cuenta sobre el secreto, sin copiarlo.
pub fn password_strength(password: &SecretString) -> PasswordStrength {
    let chars = password.expose_secret().chars().count();
    if chars < MIN_MASTER_PASSWORD_CHARS {
        PasswordStrength::TooShort
    } else if chars < STRONG_MASTER_PASSWORD_CHARS {
        PasswordStrength::Acceptable
    } else {
        PasswordStrength::Strong
    }
}

/// [`Error::PasswordTooShort`] si `password` no llega al mínimo.
pub(crate) fn require_master_password(password: &SecretString) -> Result<()> {
    match password_strength(password) {
        PasswordStrength::TooShort => Err(Error::PasswordTooShort),
        PasswordStrength::Acceptable | PasswordStrength::Strong => Ok(()),
    }
}

#[cfg(test)]
mod tests;
