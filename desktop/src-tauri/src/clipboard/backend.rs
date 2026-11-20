//! Lo que el gestor necesita del portapapeles del sistema. Es un trait para
//! probar la lógica con un doble y para que lo de Windows quede en un solo
//! archivo.

/// Resultado de [`ClipboardBackend::clear_if_matches`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClearOutcome {
    /// El portapapeles tenía el texto buscado y se vació.
    Cleared,
    /// El portapapeles está vacío, tiene otro texto o no tiene texto (una
    /// imagen, por ejemplo): no se tocó nada.
    NotOurs,
    /// No se pudo abrir o leer: no se sabe qué contiene, así que no se tocó
    /// nada. Nunca se borra a ciegas.
    Unavailable,
}

/// Fallo al escribir.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteError {
    /// El texto no llegó al portapapeles (por ejemplo, no se pudo abrir).
    NotWritten,
    /// El texto pudo quedar en el portapapeles aunque la operación falló
    /// (por ejemplo, al agregar las exclusiones ya con el texto puesto).
    Uncertain,
}

pub trait ClipboardBackend: Send + 'static {
    /// Escribe `text` excluido del historial y de la nube.
    fn write_sensitive(&mut self, text: &str) -> Result<(), WriteError>;

    /// Vacía el portapapeles **solo si** su texto cumple `matches`.
    ///
    /// Contrato: leer, comparar y vaciar son una sola operación, bajo una sola
    /// apertura del portapapeles; ninguna otra aplicación puede cambiarlo entre
    /// la comprobación y el borrado. `matches` recibe los bytes UTF-8 del texto
    /// y no debe conservarlos.
    fn clear_if_matches(&mut self, matches: &dyn Fn(&[u8]) -> bool) -> ClearOutcome;
}
