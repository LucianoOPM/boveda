//! Tramas, plazos y cancelación del emparejamiento (#95).
//!
//! Lo mínimo que necesita el conductor síncrono: [`TimedStream`] (un
//! `Read + Write` con plazos), [`CancelToken`], [`PairingTimeouts`] y la
//! lectura y escritura de tramas `longitud (u16, big-endian) | bytes`. La
//! #96 lo reutiliza y lo amplía.
//!
//! # Cancelación efectiva
//!
//! Una lectura bloqueada no mira el [`CancelToken`]: por eso, antes de cada
//! `read` se fija un plazo corto (como máximo [`CANCEL_POLL`]) y, cuando
//! vence, se revisan el token y los plazos antes de volver a leer. Así la
//! cancelación y los tiempos de espera surten efecto en como máximo
//! `CANCEL_POLL` aunque el otro extremo no mande nada. No se usa `read_exact`:
//! ante un error de tiempo descartaría los bytes ya leídos; aquí se lleva la
//! posición dentro de la trama.

use std::io::{self, ErrorKind, Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use super::error::ProtocolError;
use super::pairing::{CONFIRMATION_TIMEOUT, Frame, MAX_FRAME_LEN, PairingError};

/// Cada cuánto como máximo se revisan la cancelación y los plazos durante una
/// lectura o una escritura bloqueadas.
pub const CANCEL_POLL: Duration = Duration::from_millis(250);

/// Un `Read + Write` con plazos de entrada y salida.
///
/// **Contrato:** con un plazo configurado, `read` y `write` deben volver con
/// `WouldBlock` o `TimedOut` una vez cumplido ese tiempo, en vez de bloquear
/// sin fin. `TcpStream` lo cumple. Una implementación que lo ignore pierde la
/// garantía de cancelación y de tiempos de espera.
pub trait TimedStream: Read + Write {
    /// Fija el plazo de cada lectura (`None` es sin plazo).
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> io::Result<()>;

    /// Fija el plazo de cada escritura (`None` es sin plazo).
    fn set_write_timeout(&mut self, timeout: Option<Duration>) -> io::Result<()>;
}

impl TimedStream for TcpStream {
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> io::Result<()> {
        TcpStream::set_read_timeout(self, timeout)
    }

    fn set_write_timeout(&mut self, timeout: Option<Duration>) -> io::Result<()> {
        TcpStream::set_write_timeout(self, timeout)
    }
}

/// Señal para interrumpir una operación en curso desde otro hilo. Los clones
/// comparten el estado.
#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    /// Un token sin cancelar.
    pub fn new() -> Self {
        Self::default()
    }

    /// Cancela las operaciones que usan este token (o un clon).
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    /// Si ya se canceló.
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// Plazos del emparejamiento.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PairingTimeouts {
    /// Máximo por trama leída o escrita.
    pub step: Duration,
    /// Máximo desde que la PC acepta la conexión hasta tener la trama 3.
    pub handshake: Duration,
    /// Tiempo del usuario para decidir en la PC.
    pub confirmation: Duration,
    /// Tiempo que espera el celular la respuesta, con margen sobre el de la PC.
    pub reply_wait: Duration,
}

impl Default for PairingTimeouts {
    fn default() -> Self {
        Self {
            step: Duration::from_secs(10),
            handshake: Duration::from_secs(20),
            confirmation: CONFIRMATION_TIMEOUT,
            reply_wait: Duration::from_secs(65),
        }
    }
}

fn io_error(error: &io::Error) -> PairingError {
    match error.kind() {
        ErrorKind::ConnectionReset
        | ErrorKind::ConnectionAborted
        | ErrorKind::BrokenPipe
        | ErrorKind::UnexpectedEof => PairingError::Closed,
        kind => PairingError::Io(kind),
    }
}

/// Tiempo que se puede esperar ahora: el menor entre `CANCEL_POLL` y lo que
/// falta del plazo. Revisa antes la cancelación y que el plazo no haya vencido.
fn next_slice(limit: Instant, cancel: &CancelToken) -> Result<Duration, PairingError> {
    if cancel.is_cancelled() {
        return Err(PairingError::Cancelled);
    }
    let remaining = limit.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err(PairingError::Timeout);
    }
    Ok(remaining.min(CANCEL_POLL))
}

/// El menor entre el plazo total y el de este paso, contado desde ahora.
fn step_limit(deadline: Instant, step: Duration) -> Instant {
    deadline.min(Instant::now() + step)
}

/// Llena `buffer` completo, juntando lecturas parciales.
fn read_full<S: TimedStream>(
    stream: &mut S,
    buffer: &mut [u8],
    limit: Instant,
    cancel: &CancelToken,
) -> Result<(), PairingError> {
    let mut position = 0;
    while position < buffer.len() {
        let slice = next_slice(limit, cancel)?;
        stream
            .set_read_timeout(Some(slice))
            .map_err(|error| io_error(&error))?;
        match stream.read(&mut buffer[position..]) {
            Ok(0) => return Err(PairingError::Closed),
            Ok(n) => position += n,
            Err(error) => match error.kind() {
                ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::Interrupted => {}
                _ => return Err(io_error(&error)),
            },
        }
    }
    Ok(())
}

/// Lee el encabezado de una trama y devuelve su largo **tal cual llegó**, sin
/// validarlo: en la primera trama de la PC cualquier largo distinto de 48
/// cuenta como intento fallido, y para eso hay que verlo. Lo valida
/// [`read_frame_body`], antes de reservar memoria.
pub(crate) fn read_frame_header<S: TimedStream>(
    stream: &mut S,
    deadline: Instant,
    step: Duration,
    cancel: &CancelToken,
) -> Result<usize, PairingError> {
    let mut header = [0u8; 2];
    read_full(stream, &mut header, step_limit(deadline, step), cancel)?;
    Ok(usize::from(u16::from_be_bytes(header)))
}

/// Lee el cuerpo de una trama de `len` bytes. `len` debe estar entre 1 y
/// `MAX_FRAME_LEN`: se comprueba **antes** de reservar y de leer nada.
pub(crate) fn read_frame_body<S: TimedStream>(
    stream: &mut S,
    len: usize,
    deadline: Instant,
    step: Duration,
    cancel: &CancelToken,
) -> Result<Frame, PairingError> {
    check_len(len)?;
    let mut body = vec![0u8; len];
    read_full(stream, &mut body, step_limit(deadline, step), cancel)?;
    Ok(body)
}

/// Escribe una trama con su prefijo de largo.
pub(crate) fn write_frame<S: TimedStream>(
    stream: &mut S,
    frame: &[u8],
    deadline: Instant,
    step: Duration,
    cancel: &CancelToken,
) -> Result<(), PairingError> {
    check_len(frame.len())?;
    let mut bytes = Vec::with_capacity(frame.len() + 2);
    bytes.extend_from_slice(&(frame.len() as u16).to_be_bytes());
    bytes.extend_from_slice(frame);
    let limit = step_limit(deadline, step);
    let mut position = 0;
    while position < bytes.len() {
        let slice = next_slice(limit, cancel)?;
        stream
            .set_write_timeout(Some(slice))
            .map_err(|error| io_error(&error))?;
        match stream.write(&bytes[position..]) {
            Ok(0) => return Err(PairingError::Closed),
            Ok(n) => position += n,
            Err(error) => match error.kind() {
                ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::Interrupted => {}
                _ => return Err(io_error(&error)),
            },
        }
    }
    Ok(())
}

/// Comprueba que el otro extremo sigue ahí antes de escribir la aceptación.
///
/// Con TCP, la primera escritura tras el cierre del otro lado sale bien y solo
/// la siguiente falla, así que escribir no basta para saber que el celular
/// sigue conectado. Una lectura de sondeo con un plazo mínimo lo detecta: fin
/// de la conexión es `Closed`, y cualquier byte es un error de protocolo,
/// porque el celular no debe mandar nada mientras espera la decisión. Queda la
/// carrera de una desconexión entre el sondeo y la escritura, que es la misma
/// ya aceptada de "se escribió pero no se recibió".
pub(crate) fn ensure_peer_open<S: TimedStream>(stream: &mut S) -> Result<(), PairingError> {
    stream
        .set_read_timeout(Some(Duration::from_millis(1)))
        .map_err(|error| io_error(&error))?;
    let mut byte = [0u8; 1];
    match stream.read(&mut byte) {
        Ok(0) => Err(PairingError::Closed),
        Ok(_) => Err(PairingError::Protocol(ProtocolError::Malformed)),
        Err(error) => match error.kind() {
            ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::Interrupted => Ok(()),
            _ => Err(io_error(&error)),
        },
    }
}

/// Una trama mide de 1 a `MAX_FRAME_LEN` bytes.
fn check_len(len: usize) -> Result<(), PairingError> {
    match len {
        0 => Err(PairingError::Protocol(ProtocolError::Malformed)),
        n if n > MAX_FRAME_LEN => Err(PairingError::FrameTooLarge),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests;
