use std::time::{Duration, Instant};

use secrecy::SecretString;

use super::backend::{ClearOutcome, ClipboardBackend, WriteError};
use super::fake::{FakeBackend, FakeCall, FakeContent};
use super::fingerprint::FingerprintKey;
use super::{ClipboardError, ClipboardManager, Config};

/// Tiempos cortos para que las pruebas no esperen 30 s.
fn fast_config() -> Config {
    Config {
        ttl: Duration::from_millis(200),
        retry_interval: Duration::from_millis(20),
        max_retries: 3,
        shutdown_interval: Duration::from_millis(10),
        shutdown_attempts: 3,
    }
}

fn secret(text: &str) -> SecretString {
    SecretString::from(text.to_owned())
}

fn manager(fake: &FakeBackend) -> ClipboardManager {
    ClipboardManager::with_backend(fake.clone(), fast_config())
}

/// Espera (hasta 3 s) a que se cumpla la condición; devuelve si se cumplió.
fn wait_until(condition: impl Fn() -> bool) -> bool {
    let limit = Instant::now() + Duration::from_secs(3);
    while Instant::now() < limit {
        if condition() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    condition()
}

fn text(value: &str) -> FakeContent {
    FakeContent::Text(value.to_owned())
}

/// Un backend que hace caer al trabajador: sirve para probar `Stopped`.
struct PanickingBackend;

impl ClipboardBackend for PanickingBackend {
    fn write_sensitive(&mut self, _text: &str) -> Result<(), WriteError> {
        panic!("el trabajador cae a propósito en esta prueba");
    }

    fn clear_if_matches(&mut self, _matches: &dyn Fn(&[u8]) -> bool) -> ClearOutcome {
        ClearOutcome::NotOurs
    }
}

#[test]
fn same_bytes_same_fingerprint_within_a_key() {
    let key = FingerprintKey::new();
    assert!(key.fingerprint(b"prueba-no-real") == key.fingerprint(b"prueba-no-real"));
}

#[test]
fn different_bytes_different_fingerprint() {
    let key = FingerprintKey::new();
    assert!(key.fingerprint(b"prueba-no-real-1") != key.fingerprint(b"prueba-no-real-2"));
}

#[test]
fn different_keys_give_different_fingerprints() {
    // La clave de `RandomState` es aleatoria: dos gestores no comparten
    // huellas (la probabilidad de coincidir por azar es de 2^-64).
    let first = FingerprintKey::new();
    let second = FingerprintKey::new();
    assert!(first.fingerprint(b"prueba-no-real") != second.fingerprint(b"prueba-no-real"));
}

#[test]
fn deliver_writes_the_password_to_the_backend() {
    let fake = FakeBackend::default();
    let manager = manager(&fake);

    assert_eq!(manager.deliver(&secret("prueba-no-real-1")), Ok(()));

    assert_eq!(fake.content(), text("prueba-no-real-1"));
    assert_eq!(fake.calls(), vec![FakeCall::Write]);
}

#[test]
fn password_with_nul_is_rejected_before_touching_the_clipboard() {
    let fake = FakeBackend::default();
    fake.set_content(text("copiado-por-el-usuario"));
    let manager = manager(&fake);

    assert_eq!(
        manager.deliver(&secret("prueba\0no-real")),
        Err(ClipboardError::InvalidPassword)
    );

    assert_eq!(fake.content(), text("copiado-por-el-usuario"));
    assert!(fake.calls().is_empty());
}

#[test]
fn rejected_password_does_not_replace_a_previous_delivery() {
    let fake = FakeBackend::default();
    let manager = manager(&fake);
    assert_eq!(manager.deliver(&secret("prueba-no-real-1")), Ok(()));

    assert_eq!(
        manager.deliver(&secret("prueba\0no-real")),
        Err(ClipboardError::InvalidPassword)
    );

    assert_eq!(fake.content(), text("prueba-no-real-1"));
    assert_eq!(fake.calls(), vec![FakeCall::Write]);
}

#[test]
fn empty_password_is_rejected() {
    let fake = FakeBackend::default();
    let manager = manager(&fake);

    assert_eq!(
        manager.deliver(&secret("")),
        Err(ClipboardError::InvalidPassword)
    );

    assert!(fake.calls().is_empty());
}

#[test]
fn failed_write_reports_write_failed_and_leaves_the_clipboard_alone() {
    let fake = FakeBackend::default();
    fake.set_content(text("copiado-por-el-usuario"));
    fake.fail_next_write(WriteError::NotWritten, false);
    let manager = manager(&fake);

    assert_eq!(
        manager.deliver(&secret("prueba-no-real-1")),
        Err(ClipboardError::WriteFailed)
    );

    assert_eq!(fake.content(), text("copiado-por-el-usuario"));
}

#[test]
fn deliver_after_the_worker_stopped_returns_stopped() {
    let manager = ClipboardManager::with_backend(PanickingBackend, fast_config());

    // La primera entrega hace caer al trabajador mientras la atiende...
    assert_eq!(
        manager.deliver(&secret("prueba-no-real-1")),
        Err(ClipboardError::Stopped)
    );
    // ...y las siguientes encuentran el canal cerrado, sin colgarse.
    assert_eq!(
        manager.deliver(&secret("prueba-no-real-2")),
        Err(ClipboardError::Stopped)
    );
}
