//! Portapapeles de Windows: escritura excluida del historial y de la nube,
//! aviso y borrado condicional a los 30 s (#98).
//!
//! Diseño: `docs/superpowers/specs/2026-11-20-portapapeles-windows-design.md`.

mod backend;
mod fingerprint;

#[cfg(test)]
mod fake;
#[cfg(test)]
mod tests;

pub use backend::{ClearOutcome, ClipboardBackend, WriteError};
