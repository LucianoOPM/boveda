//! Trabajador único del portapapeles: el único que usa el backend, así que no
//! hay carreras entre nuestras propias operaciones.

use std::sync::mpsc::{Receiver, SyncSender};
use std::time::Instant;

use secrecy::{ExposeSecret, SecretString};

use super::backend::ClipboardBackend;
use super::fingerprint::{Fingerprint, FingerprintKey};
use super::{ClipboardError, Config};

pub(super) enum Command {
    Deliver {
        password: SecretString,
        reply: SyncSender<Result<(), ClipboardError>>,
    },
}

/// Una contraseña que hay que borrar si sigue en el portapapeles. Solo guarda
/// su huella.
struct Pending {
    fingerprint: Fingerprint,
    due: Instant,
    retries_left: u8,
}

pub(super) fn run<B: ClipboardBackend>(
    mut backend: B,
    config: Config,
    key: FingerprintKey,
    rx: Receiver<Command>,
) {
    let mut pending: Vec<Pending> = Vec::new();

    // Termina cuando el gestor se suelta y el canal se cierra.
    while let Ok(command) = rx.recv() {
        match command {
            Command::Deliver { password, reply } => {
                let result = deliver(&mut backend, &config, &key, &mut pending, &password);
                // Se suelta (y se borra) antes de contestar.
                drop(password);
                let _ = reply.send(result);
            }
        }
    }
}

fn deliver<B: ClipboardBackend>(
    backend: &mut B,
    config: &Config,
    key: &FingerprintKey,
    pending: &mut Vec<Pending>,
    password: &SecretString,
) -> Result<(), ClipboardError> {
    let text = password.expose_secret();
    let fingerprint = key.fingerprint(text.as_bytes());
    match backend.write_sensitive(text) {
        Ok(()) => {
            // La escritura vació el portapapeles y puso la contraseña nueva:
            // la entrada anterior ya no tiene nada que vigilar.
            pending.clear();
            pending.push(Pending {
                fingerprint,
                due: Instant::now() + config.ttl,
                retries_left: config.max_retries,
            });
            Ok(())
        }
        Err(_) => Err(ClipboardError::WriteFailed),
    }
}
