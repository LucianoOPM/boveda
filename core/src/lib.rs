//! Núcleo de Bóveda: criptografía, formato de la bóveda, búsqueda y protocolo.
//!
//! Lo que usan las apps se expondrá desde los módulos `vault` y `protocol`;
//! el resto es privado al crate.

// Mientras no exista la API pública `Vault`, el código de llaves y cabecera
// solo lo usan sus pruebas. Quitar esta línea cuando `Vault` lo consuma.
#![cfg_attr(not(test), allow(dead_code))]

mod crypto;
mod error;
mod vault;

pub use error::{Error, Result};
