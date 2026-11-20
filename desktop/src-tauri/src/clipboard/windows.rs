//! Portapapeles real de Windows: escribe con `arboard` (con las exclusiones del
//! historial y de la nube) y borra con `clipboard-win` bajo una sola apertura.
//!
//! `arboard` abre y cierra el portapapeles en cada operación y no deja
//! mantenerlo abierto, así que con él solo, «leer y luego vaciar» serían dos
//! aperturas y otra aplicación podría copiar entre las dos. Por eso el borrado
//! condicional usa directamente la guarda de `clipboard-win`, **abierta con una
//! ventana propietaria** (`MessageWindow`): con un propietario `NULL` Windows no
//! bloquea el portapapeles y la guarda no excluiría a nadie.

use arboard::SetExtWindows;
use clipboard_win::{Clipboard as OpenClipboard, formats, raw};
use zeroize::Zeroizing;

use super::backend::{ClearOutcome, ClipboardBackend, WriteError};
use super::message_window::MessageWindow;

/// Reintentos al abrir el portapapeles (5 ms entre uno y otro), como Chromium.
const OPEN_ATTEMPTS: usize = 5;

#[derive(Default)]
pub struct WindowsBackend;

impl WindowsBackend {
    pub fn new() -> Self {
        Self
    }
}

impl ClipboardBackend for WindowsBackend {
    fn write_sensitive(&mut self, text: &str) -> Result<(), WriteError> {
        let mut clipboard = arboard::Clipboard::new().map_err(|_| WriteError::NotWritten)?;
        clipboard
            .set()
            .exclude_from_history()
            .exclude_from_cloud()
            .text(text)
            .map_err(|error| match error {
                // No se pudo abrir: no se tocó nada.
                arboard::Error::ClipboardOccupied => WriteError::NotWritten,
                // El texto pudo quedar puesto aunque falló después (por ejemplo
                // al agregar las exclusiones, con el texto ya en el portapapeles).
                _ => WriteError::Uncertain,
            })
    }

    fn clear_if_matches(&mut self, matches: &dyn Fn(&[u8]) -> bool) -> ClearOutcome {
        // La ventana se declara ANTES que la guarda: las variables se sueltan en
        // orden inverso, así que primero se cierra el portapapeles y después se
        // destruye la ventana (también en las salidas anticipadas y en los
        // errores). Sin ventana no se puede bloquear: no se borra nada.
        let Some(window) = MessageWindow::new() else {
            return ClearOutcome::Unavailable;
        };
        // Una sola guarda para leer, comparar y vaciar: mientras viva, ninguna
        // otra aplicación puede abrir ni cambiar el portapapeles.
        let Ok(_open) = OpenClipboard::new_attempts_for(window.handle(), OPEN_ATTEMPTS) else {
            return ClearOutcome::Unavailable;
        };
        if !raw::is_format_avail(formats::CF_UNICODETEXT) {
            return ClearOutcome::NotOurs;
        }
        // `get_string` reserva el tamaño justo de una vez (sin reasignaciones
        // que dejen copias) y el búfer se borra al soltarse.
        let mut text = Zeroizing::new(Vec::new());
        if raw::get_string(&mut text).is_err() {
            return ClearOutcome::Unavailable;
        }
        if !matches(&text) {
            return ClearOutcome::NotOurs;
        }
        match raw::empty() {
            Ok(()) => ClearOutcome::Cleared,
            Err(_) => ClearOutcome::Unavailable,
        }
    }
}
