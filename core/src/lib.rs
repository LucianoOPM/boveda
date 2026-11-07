//! Núcleo de Bóveda: criptografía, formato de la bóveda, búsqueda y protocolo.
//!
//! Lo que usan las apps se expone desde el módulo `vault` (y, más adelante,
//! `protocol`); el resto es privado al crate.

mod crypto;
mod error;
mod search;
pub mod vault;
