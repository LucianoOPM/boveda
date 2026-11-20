use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::{Duration, Instant};

use super::*;

/// Qué entrega el siguiente `read` del flujo guionizado.
enum Step {
    /// Bytes disponibles (se parten si el búfer es más chico).
    Data(Vec<u8>),
    /// Espera ese tiempo y luego entrega los bytes (goteo lento).
    After(Duration, Vec<u8>),
    /// Fin de la conexión.
    Eof,
    /// Un error de entrada o salida.
    Fail(io::ErrorKind),
}

/// Flujo en memoria que respeta el plazo de lectura configurado: sin datos
/// disponibles, duerme el plazo y devuelve `TimedOut`, como un `TcpStream`.
struct Script {
    steps: VecDeque<Step>,
    read_timeout: Option<Duration>,
    write_timeout: Option<Duration>,
    consumed: usize,
    written: Vec<u8>,
    /// Si es `true`, `write` nunca avanza (`WouldBlock` tras el plazo).
    write_blocks: bool,
}

impl Script {
    fn new(steps: Vec<Step>) -> Self {
        Self {
            steps: steps.into(),
            read_timeout: None,
            write_timeout: None,
            consumed: 0,
            written: Vec::new(),
            write_blocks: false,
        }
    }

    /// Un flujo mudo: nunca entrega nada.
    fn silent() -> Self {
        Self::new(Vec::new())
    }
}

impl Read for Script {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let timeout = self
            .read_timeout
            .expect("se leyó sin un plazo configurado: podría bloquear sin fin");
        match self.steps.pop_front() {
            None => {
                thread::sleep(timeout);
                Err(io::ErrorKind::TimedOut.into())
            }
            Some(Step::Eof) => Ok(0),
            Some(Step::Fail(kind)) => Err(kind.into()),
            Some(Step::After(delay, bytes)) => {
                thread::sleep(delay);
                self.steps.push_front(Step::Data(bytes));
                self.read(buf)
            }
            Some(Step::Data(mut bytes)) => {
                let n = bytes.len().min(buf.len());
                buf[..n].copy_from_slice(&bytes[..n]);
                self.consumed += n;
                if n < bytes.len() {
                    self.steps.push_front(Step::Data(bytes.split_off(n)));
                }
                Ok(n)
            }
        }
    }
}

impl Write for Script {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.write_blocks {
            thread::sleep(self.write_timeout.expect("sin plazo de escritura"));
            return Err(io::ErrorKind::WouldBlock.into());
        }
        self.written.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl TimedStream for Script {
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> io::Result<()> {
        self.read_timeout = timeout;
        Ok(())
    }

    fn set_write_timeout(&mut self, timeout: Option<Duration>) -> io::Result<()> {
        self.write_timeout = timeout;
        Ok(())
    }
}

const STEP: Duration = Duration::from_secs(10);

fn far() -> Instant {
    Instant::now() + Duration::from_secs(60)
}

fn read_all(stream: &mut Script) -> Result<Frame, PairingError> {
    let cancel = CancelToken::new();
    let len = read_frame_header(stream, far(), STEP, &cancel)?;
    read_frame_body(stream, len, far(), STEP, &cancel)
}

#[test]
fn reads_frame_delivered_one_byte_at_a_time() {
    let steps = [0u8, 3, 7, 8, 9]
        .iter()
        .map(|b| Step::Data(vec![*b]))
        .collect();
    let mut stream = Script::new(steps);
    assert_eq!(read_all(&mut stream).unwrap(), vec![7, 8, 9]);
}

#[test]
fn reads_frame_split_across_reads() {
    let mut stream = Script::new(vec![
        Step::Data(vec![0]),
        Step::Data(vec![4, 1, 2]),
        Step::Data(vec![3, 4, 99]),
    ]);
    assert_eq!(read_all(&mut stream).unwrap(), vec![1, 2, 3, 4]);
    // El byte que sobra pertenece a la trama siguiente: no se consume de más.
    assert_eq!(stream.consumed, 6);
}

#[test]
fn rejects_zero_length() {
    let mut stream = Script::new(vec![Step::Data(vec![0, 0])]);
    assert_eq!(
        read_all(&mut stream).err(),
        Some(PairingError::Protocol(ProtocolError::Malformed))
    );
}

#[test]
fn rejects_over_512_without_reading_the_body() {
    let mut data = vec![0xFF, 0xFF];
    data.extend(std::iter::repeat_n(0u8, 4096));
    let mut stream = Script::new(vec![Step::Data(data)]);
    assert_eq!(
        read_all(&mut stream).err(),
        Some(PairingError::FrameTooLarge)
    );
    assert_eq!(stream.consumed, 2, "solo se leyó el encabezado");
    // 512 sí se acepta, y 513 no.
    let cancel = CancelToken::new();
    let mut ok = Script::new(vec![Step::Data(vec![0; 512])]);
    assert_eq!(
        read_frame_body(&mut ok, 512, far(), STEP, &cancel)
            .unwrap()
            .len(),
        512
    );
    let mut over = Script::new(vec![Step::Data(vec![0; 513])]);
    assert_eq!(
        read_frame_body(&mut over, 513, far(), STEP, &cancel).err(),
        Some(PairingError::FrameTooLarge)
    );
    assert_eq!(over.consumed, 0);
}

#[test]
fn header_returns_the_raw_length() {
    let mut stream = Script::new(vec![Step::Data(vec![0xFF, 0xFF, 1])]);
    let cancel = CancelToken::new();
    assert_eq!(
        read_frame_header(&mut stream, far(), STEP, &cancel).unwrap(),
        65535
    );
}

#[test]
fn eof_before_header_is_closed() {
    let mut stream = Script::new(vec![Step::Data(vec![0]), Step::Eof]);
    assert_eq!(read_all(&mut stream).err(), Some(PairingError::Closed));
    let mut empty = Script::new(vec![Step::Eof]);
    assert_eq!(read_all(&mut empty).err(), Some(PairingError::Closed));
}

#[test]
fn eof_mid_frame_is_closed() {
    let mut stream = Script::new(vec![Step::Data(vec![0, 5, 1, 2]), Step::Eof]);
    assert_eq!(read_all(&mut stream).err(), Some(PairingError::Closed));
}

#[test]
fn connection_reset_is_closed() {
    for kind in [
        io::ErrorKind::ConnectionReset,
        io::ErrorKind::ConnectionAborted,
        io::ErrorKind::BrokenPipe,
        io::ErrorKind::UnexpectedEof,
    ] {
        let mut stream = Script::new(vec![Step::Fail(kind)]);
        assert_eq!(
            read_all(&mut stream).err(),
            Some(PairingError::Closed),
            "{kind:?}"
        );
    }
    let mut other = Script::new(vec![Step::Fail(io::ErrorKind::PermissionDenied)]);
    assert_eq!(
        read_all(&mut other).err(),
        Some(PairingError::Io(io::ErrorKind::PermissionDenied))
    );
}

#[test]
fn interrupted_is_retried() {
    let mut stream = Script::new(vec![
        Step::Fail(io::ErrorKind::Interrupted),
        Step::Data(vec![0, 1, 42]),
    ]);
    assert_eq!(read_all(&mut stream).unwrap(), vec![42]);
}

#[test]
fn silent_peer_times_out_at_step() {
    let mut stream = Script::silent();
    let cancel = CancelToken::new();
    let started = Instant::now();
    let result = read_frame_header(&mut stream, far(), Duration::from_millis(300), &cancel);
    assert_eq!(result.err(), Some(PairingError::Timeout));
    let elapsed = started.elapsed();
    assert!(elapsed >= Duration::from_millis(300), "{elapsed:?}");
    assert!(elapsed < Duration::from_secs(2), "{elapsed:?}");
}

#[test]
fn slow_drip_hits_total_deadline() {
    // Un byte cada 100 ms: nunca cumple el paso (10 s), pero sí vence el
    // plazo total de 500 ms.
    let steps = (0..200)
        .map(|_| Step::After(Duration::from_millis(100), vec![0x01]))
        .collect();
    let mut stream = Script::new(steps);
    let cancel = CancelToken::new();
    let started = Instant::now();
    let deadline = started + Duration::from_millis(500);
    let len = read_frame_header(&mut stream, deadline, STEP, &cancel);
    // El encabezado `01 01` da 257 bytes; el cuerpo no llega a completarse.
    assert_eq!(len.unwrap(), 0x0101);
    let body = read_frame_body(&mut stream, 0x0101, deadline, STEP, &cancel);
    assert_eq!(body.err(), Some(PairingError::Timeout));
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
fn cancel_interrupts_a_blocked_read_within_poll() {
    let mut stream = Script::silent();
    let cancel = CancelToken::new();
    let other = cancel.clone();
    let canceller = thread::spawn(move || {
        thread::sleep(Duration::from_millis(100));
        other.cancel();
    });
    let started = Instant::now();
    let result = read_frame_header(&mut stream, far(), STEP, &cancel);
    canceller.join().unwrap();
    assert_eq!(result.err(), Some(PairingError::Cancelled));
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "{:?}",
        started.elapsed()
    );
}

#[test]
fn already_cancelled_token_does_not_read() {
    let mut stream = Script::new(vec![Step::Data(vec![0, 1, 9])]);
    let cancel = CancelToken::new();
    cancel.cancel();
    assert!(cancel.is_cancelled());
    assert_eq!(
        read_frame_header(&mut stream, far(), STEP, &cancel).err(),
        Some(PairingError::Cancelled)
    );
    assert_eq!(stream.consumed, 0);
}

#[test]
fn write_frame_prefixes_the_length() {
    let mut stream = Script::silent();
    write_frame(&mut stream, &[1, 2, 3], far(), STEP, &CancelToken::new()).unwrap();
    assert_eq!(stream.written, vec![0, 3, 1, 2, 3]);
}

#[test]
fn write_frame_rejects_empty_and_oversized() {
    let mut stream = Script::silent();
    let cancel = CancelToken::new();
    assert_eq!(
        write_frame(&mut stream, &[], far(), STEP, &cancel).err(),
        Some(PairingError::Protocol(ProtocolError::Malformed))
    );
    assert_eq!(
        write_frame(&mut stream, &[0; 513], far(), STEP, &cancel).err(),
        Some(PairingError::FrameTooLarge)
    );
    assert!(stream.written.is_empty());
}

#[test]
fn write_times_out() {
    let mut stream = Script::silent();
    stream.write_blocks = true;
    let started = Instant::now();
    let result = write_frame(
        &mut stream,
        &[1],
        far(),
        Duration::from_millis(300),
        &CancelToken::new(),
    );
    assert_eq!(result.err(), Some(PairingError::Timeout));
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
fn timeouts_default_match_the_spec() {
    let t = PairingTimeouts::default();
    assert_eq!(t.step, Duration::from_secs(10));
    assert_eq!(t.handshake, Duration::from_secs(20));
    assert_eq!(t.confirmation, Duration::from_secs(60));
    assert_eq!(t.reply_wait, Duration::from_secs(65));
    assert_eq!(CANCEL_POLL, Duration::from_millis(250));
}

#[test]
fn tcp_stream_timeouts_apply_to_a_silent_peer() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let (_server, _) = listener.accept().unwrap();
    let mut client = client;
    let started = Instant::now();
    let result = read_frame_header(
        &mut client,
        far(),
        Duration::from_millis(300),
        &CancelToken::new(),
    );
    assert_eq!(result.err(), Some(PairingError::Timeout));
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
fn tcp_stream_cancel_interrupts_a_blocked_read() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let mut client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let (_server, _) = listener.accept().unwrap();
    let cancel = CancelToken::new();
    let other = cancel.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(100));
        other.cancel();
    });
    let started = Instant::now();
    let result = read_frame_header(&mut client, far(), STEP, &cancel);
    assert_eq!(result.err(), Some(PairingError::Cancelled));
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "{:?}",
        started.elapsed()
    );
}
