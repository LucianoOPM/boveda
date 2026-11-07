//! Núcleo de Bóveda: criptografía, formato de la bóveda, búsqueda y protocolo.
//!
//! Lo que usan las apps se expondrá desde los módulos `vault` y `protocol`;
//! el resto es privado al crate.

mod error;

pub use error::{Error, Result};
