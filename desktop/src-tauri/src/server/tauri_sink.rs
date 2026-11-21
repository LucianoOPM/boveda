//! Destino real de las contraseñas: el gestor del portapapeles y su aviso (#100).

use std::fmt;

use secrecy::SecretString;
use tauri::{AppHandle, Manager};

use super::{DeliverFailed, PasswordSink};
use crate::clipboard::{ClipboardManager, notify_ready};

/// Entrega al `ClipboardManager` que la app tiene como estado de Tauri y, si
/// la copia salió bien, muestra el aviso «Contraseña lista para pegar».
pub struct TauriSink {
    app: AppHandle,
}

impl TauriSink {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl PasswordSink for TauriSink {
    fn deliver(&self, password: &SecretString) -> Result<(), DeliverFailed> {
        let manager = self.app.state::<ClipboardManager>();
        deliver_with(&manager, password, || notify_ready(&self.app))
    }
}

/// Escribe la contraseña y avisa solo si quedó escrita. Un aviso que falla se
/// registra (lo hace el gestor) y no cambia el resultado. El error del
/// portapapeles no lleva datos del secreto, así que su tipo puede registrarse.
pub(crate) fn deliver_with<E: fmt::Display>(
    manager: &ClipboardManager,
    password: &SecretString,
    notify: impl FnOnce() -> Result<(), E>,
) -> Result<(), DeliverFailed> {
    manager
        .deliver_and_notify(password, notify)
        .map_err(|error| {
            eprintln!("servidor: el portapapeles falló ({error:?})");
            DeliverFailed
        })
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use secrecy::SecretString;

    use super::deliver_with;
    use crate::clipboard::{ClearOutcome, ClipboardBackend, ClipboardManager, Config, WriteError};
    use crate::server::DeliverFailed;

    /// Portapapeles de pruebas: anota lo escrito y puede fallar.
    struct Recording {
        written: Arc<Mutex<Vec<String>>>,
        fail: bool,
    }

    impl ClipboardBackend for Recording {
        fn write_sensitive(&mut self, text: &str) -> Result<(), WriteError> {
            if self.fail {
                return Err(WriteError::NotWritten);
            }
            self.written.lock().expect("candado").push(text.to_owned());
            Ok(())
        }

        fn clear_if_matches(&mut self, _matches: &dyn Fn(&[u8]) -> bool) -> ClearOutcome {
            ClearOutcome::Unavailable
        }
    }

    fn manager(fail: bool) -> (ClipboardManager, Arc<Mutex<Vec<String>>>) {
        let written = Arc::new(Mutex::new(Vec::new()));
        let backend = Recording {
            written: Arc::clone(&written),
            fail,
        };
        let config = Config {
            retry_interval: Duration::from_millis(5),
            max_retries: 0,
            shutdown_interval: Duration::from_millis(5),
            shutdown_attempts: 1,
            ..Config::default()
        };
        (ClipboardManager::with_backend(backend, config), written)
    }

    #[test]
    fn ok_delivery_writes_to_the_clipboard_and_notifies() {
        let (manager, written) = manager(false);
        let notified = Arc::new(Mutex::new(0));
        let counter = Arc::clone(&notified);
        let result = deliver_with(&manager, &SecretString::from("clave"), move || {
            *counter.lock().expect("candado") += 1;
            Ok::<(), String>(())
        });
        assert_eq!(result, Ok(()));
        assert_eq!(*written.lock().expect("candado"), ["clave"]);
        assert_eq!(*notified.lock().expect("candado"), 1);
        manager.shutdown(Duration::from_millis(200));
    }

    #[test]
    fn clipboard_error_maps_to_deliver_failed_without_notifying() {
        let (manager, written) = manager(true);
        let notified = Arc::new(Mutex::new(0));
        let counter = Arc::clone(&notified);
        let result = deliver_with(&manager, &SecretString::from("clave"), move || {
            *counter.lock().expect("candado") += 1;
            Ok::<(), String>(())
        });
        assert_eq!(result, Err(DeliverFailed));
        assert!(written.lock().expect("candado").is_empty());
        assert_eq!(*notified.lock().expect("candado"), 0);
        manager.shutdown(Duration::from_millis(200));
    }

    #[test]
    fn a_failing_notification_does_not_fail_the_delivery() {
        let (manager, _written) = manager(false);
        let result = deliver_with(&manager, &SecretString::from("clave"), || {
            Err::<(), _>("sin permiso".to_owned())
        });
        assert_eq!(result, Ok(()));
        manager.shutdown(Duration::from_millis(200));
    }
}
