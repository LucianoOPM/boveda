//! Modelo de entrada: identificador, datos de búsqueda y secreto.
//!
//! Cada entrada se divide en dos partes que se cifran por separado: los datos
//! de búsqueda (se descifran al desbloquear) y el secreto (solo al copiar,
//! mostrar o enviar). Aquí se definen los tipos validados y su serialización
//! a texto plano; el cifrado vive en otro módulo.

use crate::error::{Error, Result};

/// Longitud del identificador de entrada.
pub(crate) const ENTRY_ID_LEN: usize = 16;

/// Identificador de una entrada: 16 bytes aleatorios del generador del sistema.
///
/// No es secreto, así que deriva `Debug`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct EntryId([u8; ENTRY_ID_LEN]);

impl EntryId {
    /// Genera un identificador nuevo con el generador del sistema.
    pub(crate) fn generate() -> Result<Self> {
        let mut bytes = [0u8; ENTRY_ID_LEN];
        getrandom::fill(&mut bytes).map_err(|_| Error::Random)?;
        Ok(Self(bytes))
    }

    /// Reconstruye un identificador leído del almacenamiento.
    pub(crate) fn from_bytes(bytes: [u8; ENTRY_ID_LEN]) -> Self {
        Self(bytes)
    }

    pub(crate) fn as_bytes(&self) -> &[u8; ENTRY_ID_LEN] {
        &self.0
    }
}

#[cfg(test)]
mod tests;
