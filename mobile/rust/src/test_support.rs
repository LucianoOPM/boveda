//! Ayudantes de las pruebas de la fachada. Solo existe con `cfg(test)`.

use std::path::PathBuf;

/// Contraseña de `core/tests/fixtures/vault-v2.db`.
pub(crate) const FIXTURE_PASSWORD: &str = "prueba-bóveda-v2";

/// Contraseña de `core/tests/fixtures/vault-v3.db`.
pub(crate) const V3_PASSWORD: &str = "prueba-bóveda-v3";

/// Copia `vault-v2.db` a un directorio temporal (debe vivir toda la prueba)
/// y devuelve la ruta absoluta de la copia. Nunca se abre el original.
pub(crate) fn fixture_copy() -> (tempfile::TempDir, PathBuf) {
    copy_fixture("vault-v2.db")
}

/// Como [`fixture_copy`], con `vault-v3.db`: trae la identidad (llave
/// `[7; 32]`) y dos PCs emparejadas, `Escritorio` y `Portátil`.
pub(crate) fn fixture_v3_copy() -> (tempfile::TempDir, PathBuf) {
    copy_fixture("vault-v3.db")
}

fn copy_fixture(name: &str) -> (tempfile::TempDir, PathBuf) {
    let source = format!(
        "{}/../../core/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    let (dir, path) = temp_path();
    std::fs::copy(source, &path).unwrap();
    (dir, path)
}

/// Directorio temporal y una ruta absoluta `boveda.db` que no existe.
pub(crate) fn temp_path() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("boveda.db");
    (dir, path)
}

pub(crate) fn path_string(path: &std::path::Path) -> String {
    path.to_str().unwrap().to_owned()
}

use std::io::{ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use secrecy::{ExposeSecret, SecretBox};
use vault_core::protocol::{
    receive_send, serve_pairing, CancelToken, KeyPair, PairedDevice, PairingCode, PairingError,
    PairingOffer, PairingSession, PairingTimeouts, PublicKey, SendError, SendTimeouts,
};

use crate::api::session::VaultSession;

/// Qué hace la PC de las pruebas con el emparejamiento que le llega.
#[derive(Clone, Copy)]
pub(crate) enum PcDecision {
    Accept,
    Reject,
    /// Suelta la conexión sin decidir.
    Drop,
}

/// Qué hace la PC de las pruebas con el envío que le llega.
#[derive(Clone, Copy)]
pub(crate) enum PcSend {
    /// Contesta `Copied`.
    Copied,
    /// Contesta `Failed`.
    Failed,
    /// Recibe el mensaje y cierra sin contestar.
    Silent,
    /// No tiene emparejado al celular: cierra sin escribir nada.
    Forget,
}

/// Plazos cortos del envío para las pruebas.
pub(crate) fn quick_send() -> SendTimeouts {
    SendTimeouts {
        step: Duration::from_millis(500),
        total: Duration::from_secs(4),
        ack_wait: Duration::from_secs(2),
    }
}

/// Plazos cortos para las pruebas de red.
pub(crate) fn quick_pairing() -> PairingTimeouts {
    PairingTimeouts {
        step: Duration::from_millis(500),
        handshake: Duration::from_secs(4),
        confirmation: Duration::from_secs(4),
        reply_wait: Duration::from_secs(4),
    }
}

/// La PC de las pruebas: un oyente en `127.0.0.1` con puerto del sistema.
/// Tiene la llave `[1; 32]` y, para el emparejamiento, el código `[7; 32]`.
pub(crate) struct FakePc {
    listener: TcpListener,
    addr: SocketAddr,
    session: Arc<Mutex<PairingSession>>,
}

fn pc_keys() -> KeyPair {
    KeyPair::from_private(SecretBox::new(Box::new([1; 32])))
}

impl FakePc {
    pub(crate) fn new() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let code = PairingCode::from_bytes(SecretBox::new(Box::new([7; 32])));
        let session = PairingSession::new(pc_keys(), code, Instant::now());
        Self {
            listener,
            addr,
            session: Arc::new(Mutex::new(session)),
        }
    }

    pub(crate) fn public_key() -> PublicKey {
        pc_keys().public()
    }

    pub(crate) fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Texto del QR de esta PC, como lo escribe la PC real
    /// (`desktop/src-tauri/src/qr.rs`).
    pub(crate) fn offer_text(&self) -> String {
        let code = PairingCode::from_bytes(SecretBox::new(Box::new([7; 32])));
        let offer = PairingOffer::new(Self::public_key(), self.addr, code).unwrap();
        qr_text(&offer)
    }

    /// Atiende **una** conexión de emparejamiento en otro hilo y devuelve el
    /// dispositivo emparejado si `decision` es `Accept`.
    pub(crate) fn serve_pairing(
        &self,
        timeouts: PairingTimeouts,
        decision: PcDecision,
    ) -> JoinHandle<Result<Option<PairedDevice>, PairingError>> {
        let listener = self.listener.try_clone().unwrap();
        let session = self.session.clone();
        thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let pending = serve_pairing(stream, &session, &CancelToken::new(), &timeouts)?;
            match decision {
                PcDecision::Accept => pending.confirm().map(Some),
                PcDecision::Reject => pending.reject().map(|()| None),
                PcDecision::Drop => Ok(None),
            }
        })
    }

    /// Atiende **un** envío en otro hilo. Devuelve la contraseña recibida y
    /// la llave pública del celular (cualquier celular está emparejado, salvo
    /// con `PcSend::Forget`).
    pub(crate) fn serve_send(
        &self,
        timeouts: SendTimeouts,
        behavior: PcSend,
    ) -> JoinHandle<Result<(String, PublicKey), SendError>> {
        let listener = self.listener.try_clone().unwrap();
        thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let paired = !matches!(behavior, PcSend::Forget);
            let incoming = receive_send(
                stream,
                &pc_keys(),
                |_| paired,
                &CancelToken::new(),
                &timeouts,
            )?;
            let got = (
                incoming.message().password().expose_secret().to_owned(),
                incoming.peer(),
            );
            match behavior {
                PcSend::Copied => incoming.copied()?,
                PcSend::Failed => incoming.failed()?,
                PcSend::Silent | PcSend::Forget => drop(incoming),
            }
            Ok(got)
        })
    }

    /// Acepta **una** conexión y cuenta los bytes que recibe hasta que se
    /// cierre (o pasen 2 s).
    pub(crate) fn serve_count_bytes(&self) -> JoinHandle<usize> {
        let listener = self.listener.try_clone().unwrap();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let (mut total, mut buffer) = (0, [0u8; 256]);
            while let Ok(n) = stream.read(&mut buffer) {
                if n == 0 {
                    break;
                }
                total += n;
            }
            total
        })
    }

    /// Acepta **una** conexión y la mantiene abierta sin escribir nada hasta
    /// que se envíe algo por el canal devuelto (o pasen 10 s).
    pub(crate) fn serve_mute(&self) -> (JoinHandle<()>, Sender<()>) {
        let listener = self.listener.try_clone().unwrap();
        let (release, wait) = mpsc::channel::<()>();
        let handle = thread::spawn(move || {
            let (_stream, _) = listener.accept().unwrap();
            let _ = wait.recv_timeout(Duration::from_secs(10));
        });
        (handle, release)
    }

    /// Acepta **una** conexión y contesta una trama con longitud 65535.
    pub(crate) fn serve_garbage(&self) -> JoinHandle<()> {
        let listener = self.listener.try_clone().unwrap();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let _ = stream.write_all(&[0xFF, 0xFF, 1, 2, 3]);
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut buffer = [0u8; 64];
            while matches!(stream.read(&mut buffer), Ok(n) if n > 0) {}
        })
    }

    /// `true` si nadie se conectó desde la última vez que se miró.
    pub(crate) fn nobody_connected(&self) -> bool {
        self.listener.set_nonblocking(true).unwrap();
        let none = matches!(self.listener.accept(), Err(e) if e.kind() == ErrorKind::WouldBlock);
        self.listener.set_nonblocking(false).unwrap();
        none
    }
}

/// Texto del QR: la oferta en base64url sin relleno (igual que la PC real).
pub(crate) fn qr_text(offer: &PairingOffer) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let bytes = offer.encode().unwrap();
    let mut text = String::new();
    for chunk in bytes.chunks(3) {
        let n =
            chunk.iter().fold(0u32, |acc, b| (acc << 8) | u32::from(*b)) << (8 * (3 - chunk.len()));
        for i in 0..=chunk.len() {
            text.push(char::from(ALPHABET[((n >> (18 - 6 * i)) & 63) as usize]));
        }
    }
    text
}

/// Un respaldo exportado de la bóveda de ejemplo v2 y una sesión
/// desbloqueada sobre otra copia, donde importarlo. Los directorios viven
/// hasta el final del proceso de pruebas.
pub(crate) fn backup_and_target() -> (PathBuf, VaultSession) {
    let (source_dir, source_path) = fixture_copy();
    let source = VaultSession::new(path_string(&source_path));
    source.open().unwrap();
    source.unlock(FIXTURE_PASSWORD.into()).unwrap();
    let backup = source_dir.path().join("respaldo.db");
    source.export_backup(path_string(&backup)).unwrap();
    let (target_dir, target_path) = fixture_copy();
    let target = VaultSession::new(path_string(&target_path));
    target.open().unwrap();
    target.unlock(FIXTURE_PASSWORD.into()).unwrap();
    std::mem::forget(source_dir);
    std::mem::forget(target_dir);
    (backup, target)
}
