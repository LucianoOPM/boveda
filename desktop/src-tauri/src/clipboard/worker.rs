//! Trabajador único del portapapeles: el único que usa el backend, así que no
//! hay carreras entre nuestras propias operaciones.

use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender};
use std::time::Instant;

use secrecy::{ExposeSecret, SecretString};

use super::backend::{ClearOutcome, ClipboardBackend, WriteError};
use super::fingerprint::{Fingerprint, FingerprintKey};
use super::{ClipboardError, Config};

pub(super) enum Command {
    Deliver {
        password: SecretString,
        reply: SyncSender<Result<(), ClipboardError>>,
    },
    /// Último intento de limpieza antes de salir; el trabajador termina después.
    Shutdown {
        deadline: Instant,
        reply: SyncSender<()>,
    },
}

/// Una contraseña que hay que borrar si sigue en el portapapeles. Solo guarda
/// su huella.
struct Pending {
    fingerprint: Fingerprint,
    due: Instant,
    retries_left: u8,
    /// `true` en la limpieza de una escritura de resultado dudoso. Hay como
    /// máximo una de limpieza y una vigente (la de la última escritura correcta).
    cleanup: bool,
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
            Ok(Command::Shutdown { deadline, reply }) => {
                shutdown(&mut backend, &config, &key, &pending, deadline);
                let _ = reply.send(());
                break;
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

/// Limpieza al salir: hasta `shutdown_attempts` intentos por entrada, sin pasar
/// de `deadline`. Si Windows no deja comprobar, no se borra y el cierre sigue.
fn shutdown<B: ClipboardBackend>(
    backend: &mut B,
    config: &Config,
    key: &FingerprintKey,
    pending: &[Pending],
    deadline: Instant,
) {
    for entry in pending {
        // Un intento lento puede haber agotado el plazo: no se empieza otra entrada.
        if Instant::now() >= deadline {
            break;
        }
        let fingerprint = entry.fingerprint;
        for attempt in 1..=config.shutdown_attempts {
            let outcome = backend.clear_if_matches(&|bytes| key.fingerprint(bytes) == fingerprint);
            let last = attempt == config.shutdown_attempts;
            if outcome != ClearOutcome::Unavailable
                || last
                || Instant::now() + config.shutdown_interval >= deadline
            {
                if outcome == ClearOutcome::Unavailable {
                    eprintln!("No se pudo comprobar el portapapeles al salir: no se borró nada.");
                }
                break;
            }
            std::thread::sleep(config.shutdown_interval);
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
                cleanup: false,
            });
            Ok(())
        }
        Err(WriteError::NotWritten) => Err(ClipboardError::WriteFailed),
        // El texto pudo quedar puesto (p. ej. falló al agregar las exclusiones):
        // se intenta limpiar de inmediato, con la huella de ESTA contraseña. Si
        // no se puede comprobar, queda una entrada de limpieza con reintentos
        // acotados; la entrada vigente anterior conserva su vencimiento.
        Err(WriteError::Uncertain) => {
            let outcome = backend.clear_if_matches(&|bytes| key.fingerprint(bytes) == fingerprint);
            if outcome == ClearOutcome::Unavailable && config.max_retries > 0 {
                pending.retain(|entry| !entry.cleanup);
                pending.push(Pending {
                    fingerprint,
                    due: Instant::now() + config.retry_interval,
                    // El intento inmediato fue el primero.
                    retries_left: config.max_retries - 1,
                    cleanup: true,
                });
            }
            Err(ClipboardError::WriteFailed)
        }
    }
}
