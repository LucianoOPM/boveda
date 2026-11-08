//! Fortaleza de la contraseña maestra, para el indicador del primer uso.
//!
//! Es la única función de `api/`, junto con `VaultSession::new`, que lleva
//! `#[frb(sync)]`: no toca la `Vault` ni el `Mutex` de la sesión y es
//! instantánea, así que no puede congelar la interfaz esperando a Argon2id.

use flutter_rust_bridge::frb;
use vault_core::vault;

use super::session::secret_from;

/// Fortaleza según la longitud; la regla vive en el núcleo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasswordStrength {
    /// Menos de 12 caracteres: no se puede crear la bóveda con ella.
    TooShort,
    /// De 12 a 15 caracteres.
    Acceptable,
    /// 16 caracteres o más.
    Strong,
}

impl From<vault::PasswordStrength> for PasswordStrength {
    fn from(strength: vault::PasswordStrength) -> Self {
        match strength {
            vault::PasswordStrength::TooShort => Self::TooShort,
            vault::PasswordStrength::Acceptable => Self::Acceptable,
            vault::PasswordStrength::Strong => Self::Strong,
        }
    }
}

/// Fortaleza de `password`. El `String` recibido se borra con `zeroize`.
#[frb(sync)]
pub fn password_strength(password: String) -> PasswordStrength {
    vault::password_strength(&secret_from(password)).into()
}

#[cfg(test)]
mod tests;
