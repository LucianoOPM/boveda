//! Trabajador único del portapapeles: el único que usa el backend, así que no
//! hay carreras entre nuestras propias operaciones.

use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender};
use std::time::Instant;

use secrecy::{ExposeSecret, SecretString};

use super::backend::{ClearOutcome, ClipboardBackend};
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

    loop {
        // Se espera al siguiente vencimiento (o a un comando, lo que llegue
        // antes); `Instant` es monótono, así que cambiar la hora del sistema
        // no alarga ni acorta el plazo.
        let next_due = pending.iter().map(|entry| entry.due).min();
        let received = match next_due {
            Some(due) => rx.recv_timeout(due.saturating_duration_since(Instant::now())),
            None => rx.recv().map_err(|_| RecvTimeoutError::Disconnected),
        };
        match received {
            Ok(Command::Deliver { password, reply }) => {
                let result = deliver(&mut backend, &config, &key, &mut pending, &password);
                // Se suelta (y se borra) antes de contestar.
                drop(password);
                let _ = reply.send(result);
            }
            Err(RecvTimeoutError::Timeout) => {}
            // El gestor se soltó: no queda nadie a quien atender.
            Err(RecvTimeoutError::Disconnected) => break,
        }
        process_due(&mut backend, &config, &key, &mut pending, Instant::now());
    }
}

/// Intenta borrar cada entrada vencida. `Cleared` y `NotOurs` la terminan;
/// `Unavailable` la reprograma hasta `max_retries` veces y después se rinde:
/// nunca se borra a ciegas ni se vigila sin límite.
fn process_due<B: ClipboardBackend>(
    backend: &mut B,
    config: &Config,
    key: &FingerprintKey,
    pending: &mut Vec<Pending>,
    now: Instant,
) {
    pending.retain_mut(|entry| {
        if entry.due > now {
            return true;
        }
        let fingerprint = entry.fingerprint;
        match backend.clear_if_matches(&|bytes| key.fingerprint(bytes) == fingerprint) {
            ClearOutcome::Cleared | ClearOutcome::NotOurs => false,
            ClearOutcome::Unavailable if entry.retries_left == 0 => {
                eprintln!("No se pudo comprobar el portapapeles: no se borró nada.");
                false
            }
            ClearOutcome::Unavailable => {
                entry.retries_left -= 1;
                entry.due = now + config.retry_interval;
                true
            }
        }
    });
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
