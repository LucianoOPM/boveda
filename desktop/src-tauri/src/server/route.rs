//! Enrutado de una conexión nueva por su primera trama (#101).
//!
//! El emparejamiento y el envío comparten puerto. Se distinguen por el largo
//! que anuncia el encabezado de la primera trama (u16 big-endian): 48 bytes es
//! el emparejamiento y 96 el envío. Se mira con `peek`, sin consumirlo, para que
//! `serve_pairing` y `receive_send` lo lean como siempre.

use std::io::ErrorKind;
use std::net::TcpStream;
use std::thread;
use std::time::{Duration, Instant};

use vault_core::protocol::{CANCEL_POLL, CancelToken, FIRST_FRAME_LEN, SEND_FIRST_FRAME_LEN};

#[cfg(test)]
mod tests;

/// A dónde va una conexión.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Route {
    /// Primera trama de 48 bytes: emparejamiento.
    Pairing,
    /// Primera trama de 96 bytes: envío de una contraseña.
    Send,
    /// Cualquier otra cosa: se cierra sin leer más ni escribir.
    Reject,
}

/// Espera como máximo `step` a tener los 2 bytes del encabezado y decide la ruta.
/// Un cierre, un plazo vencido, una cancelación, un error o un largo que no es
/// 48 ni 96 dan `Reject`. No consume ningún byte.
pub(crate) fn classify(stream: &TcpStream, cancel: &CancelToken, step: Duration) -> Route {
    let deadline = Instant::now() + step;
    let mut header = [0u8; 2];
    loop {
        if cancel.is_cancelled() {
            return Route::Reject;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Route::Reject;
        }
        if stream
            .set_read_timeout(Some(remaining.min(CANCEL_POLL)))
            .is_err()
        {
            return Route::Reject;
        }
        match stream.peek(&mut header) {
            Ok(0) => return Route::Reject,
            Ok(read) if read >= header.len() => {
                return match usize::from(u16::from_be_bytes(header)) {
                    FIRST_FRAME_LEN => Route::Pairing,
                    SEND_FIRST_FRAME_LEN => Route::Send,
                    _ => Route::Reject,
                };
            }
            // Llegó un solo byte: `peek` volvería al instante, así que se espera un poco.
            Ok(_) => thread::sleep(Duration::from_millis(5)),
            Err(error)
                if matches!(
                    error.kind(),
                    ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::Interrupted
                ) => {}
            Err(_) => return Route::Reject,
        }
    }
}
