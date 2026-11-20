//! Portapapeles de Windows: escritura excluida del historial y de la nube,
//! aviso y borrado condicional a los 30 s (#98).
//!
//! Diseño: `docs/superpowers/specs/2026-11-20-portapapeles-windows-design.md`.

mod backend;
mod fingerprint;
#[cfg(windows)]
mod windows;
mod worker;

#[cfg(test)]
mod fake;
#[cfg(test)]
mod tests;

use std::fmt;
use std::sync::mpsc::{Sender, sync_channel};
use std::thread;
use std::time::{Duration, Instant};

use secrecy::{ExposeSecret, SecretString};

pub use backend::{ClearOutcome, ClipboardBackend, WriteError};
#[cfg(windows)]
pub use windows::WindowsBackend;

use fingerprint::FingerprintKey;
use worker::Command;

/// Tiempos del gestor. En producción son los de [`Config::default`]; las
/// pruebas usan valores cortos.
#[derive(Clone, Debug)]
pub struct Config {
    /// Cuánto vive una contraseña en el portapapeles.
    pub ttl: Duration,
    /// Espera entre reintentos cuando el portapapeles está ocupado.
    pub retry_interval: Duration,
    /// Reintentos tras el primer intento de borrado.
    pub max_retries: u8,
    /// Espera entre intentos al salir de la app.
    pub shutdown_interval: Duration,
    /// Intentos por entrada al salir de la app.
    pub shutdown_attempts: u8,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ttl: Duration::from_secs(30),
            retry_interval: Duration::from_millis(250),
            max_retries: 3,
            shutdown_interval: Duration::from_millis(100),
            shutdown_attempts: 3,
        }
    }
}

/// Por qué no se pudo entregar una contraseña. Nunca contiene datos del secreto.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipboardError {
    /// Vacía o con un `\0`: Windows la pegaría truncada, así que no se escribe.
    InvalidPassword,
    /// No se pudo escribir en el portapapeles.
    WriteFailed,
    /// El gestor ya no está activo (cerrado o con el trabajador caído).
    Stopped,
}

impl fmt::Display for ClipboardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidPassword => "la contraseña no se puede copiar tal cual",
            Self::WriteFailed => "no se pudo escribir en el portapapeles",
            Self::Stopped => "el gestor del portapapeles no está activo",
        })
    }
}

impl std::error::Error for ClipboardError {}

/// Escribe contraseñas en el portapapeles y las borra al vencer su plazo. Un
/// solo hilo trabajador atiende todo; al soltar el gestor, el hilo termina.
pub struct ClipboardManager {
    tx: Sender<Command>,
}

impl ClipboardManager {
    /// Arranca el gestor real (lanza su hilo): portapapeles de Windows y los
    /// plazos de producción. Se llama `start` y no `new` porque tiene ese efecto.
    #[cfg(windows)]
    pub fn start() -> Self {
        Self::with_backend(WindowsBackend::new(), Config::default())
    }

    pub fn with_backend<B: ClipboardBackend>(backend: B, config: Config) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        let key = FingerprintKey::new();
        thread::Builder::new()
            .name("clipboard-worker".into())
            .spawn(move || worker::run(backend, config, key, rx))
            .expect("no se pudo iniciar el hilo del portapapeles");
        Self { tx }
    }

    /// Escribe la contraseña y arma su borrado. Vuelve cuando el portapapeles
    /// quedó escrito o falló; es lo que el envío necesita para contestar
    /// `Copied` o `Failed`.
    pub fn deliver(&self, password: &SecretString) -> Result<(), ClipboardError> {
        let text = password.expose_secret();
        if text.is_empty() || text.contains('\0') {
            return Err(ClipboardError::InvalidPassword);
        }
        // Una copia exacta y de vida corta que cruza al hilo; se borra al soltarse.
        let owned = SecretString::from(String::from(text));
        let (reply, wait) = sync_channel(1);
        self.tx
            .send(Command::Deliver {
                password: owned,
                reply,
            })
            .map_err(|_| ClipboardError::Stopped)?;
        wait.recv().map_err(|_| ClipboardError::Stopped)?
    }

    /// `deliver` y, solo si la copia salió bien, el aviso. Si el aviso falla se
    /// registra (sin datos sensibles) y no cambia el resultado: la contraseña ya
    /// está en el portapapeles y su borrado sigue armado.
    pub fn deliver_and_notify<E: fmt::Display>(
        &self,
        password: &SecretString,
        notify: impl FnOnce() -> Result<(), E>,
    ) -> Result<(), ClipboardError> {
        self.deliver(password)?;
        if let Err(error) = notify() {
            eprintln!("No se pudo mostrar el aviso del portapapeles: {error}");
        }
        Ok(())
    }

    /// Último intento de limpieza al salir de la app. Espera como mucho
    /// `timeout`: si Windows no deja comprobar el portapapeles no se borra
    /// nada y el cierre sigue. Después, `deliver` devuelve `Stopped`; llamarlo
    /// otra vez no hace nada.
    pub fn shutdown(&self, timeout: Duration) {
        let (reply, wait) = sync_channel(1);
        let deadline = Instant::now() + timeout;
        if self.tx.send(Command::Shutdown { deadline, reply }).is_err() {
            return;
        }
        let _ = wait.recv_timeout(timeout);
    }
}

/// El aviso «Contraseña lista para pegar». No lleva la contraseña ni el nombre
/// del servicio.
pub fn notify_ready<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> tauri_plugin_notification::Result<()> {
    use tauri_plugin_notification::NotificationExt;

    app.notification()
        .builder()
        .title("Bóveda")
        .body("Contraseña lista para pegar")
        .show()
}
