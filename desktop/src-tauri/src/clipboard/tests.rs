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

fn manager_with_ttl(fake: &FakeBackend, ttl: Duration) -> ClipboardManager {
    ClipboardManager::with_backend(
        fake.clone(),
        Config {
            ttl,
            ..fast_config()
        },
    )
}

fn clear_calls(fake: &FakeBackend) -> usize {
    fake.calls()
        .iter()
        .filter(|call| matches!(call, FakeCall::Clear(_)))
        .count()
}

#[test]
fn clears_the_clipboard_when_it_still_holds_the_password() {
    let fake = FakeBackend::default();
    let manager = manager(&fake);
    assert_eq!(manager.deliver(&secret("prueba-no-real-1")), Ok(()));

    assert!(wait_until(|| fake.content() == FakeContent::Empty));

    // Un intento de borrado, una sola llamada al backend: sin lecturas sueltas.
    assert_eq!(
        fake.calls(),
        vec![FakeCall::Write, FakeCall::Clear(ClearOutcome::Cleared)]
    );
}

#[test]
fn does_not_clear_different_text() {
    let fake = FakeBackend::default();
    let manager = manager(&fake);
    assert_eq!(manager.deliver(&secret("prueba-no-real-1")), Ok(()));
    fake.set_content(text("copiado-por-el-usuario"));

    assert!(wait_until(|| clear_calls(&fake) == 1));

    assert_eq!(fake.content(), text("copiado-por-el-usuario"));
    assert_eq!(
        fake.calls(),
        vec![FakeCall::Write, FakeCall::Clear(ClearOutcome::NotOurs)]
    );
}

#[test]
fn does_not_clear_when_the_clipboard_is_empty() {
    let fake = FakeBackend::default();
    let manager = manager(&fake);
    assert_eq!(manager.deliver(&secret("prueba-no-real-1")), Ok(()));
    fake.set_content(FakeContent::Empty);

    assert!(wait_until(|| clear_calls(&fake) == 1));

    assert_eq!(
        fake.calls(),
        vec![FakeCall::Write, FakeCall::Clear(ClearOutcome::NotOurs)]
    );
}

#[test]
fn does_not_clear_an_image() {
    let fake = FakeBackend::default();
    let manager = manager(&fake);
    assert_eq!(manager.deliver(&secret("prueba-no-real-1")), Ok(()));
    fake.set_content(FakeContent::Image);

    assert!(wait_until(|| clear_calls(&fake) == 1));

    assert_eq!(fake.content(), FakeContent::Image);
}

#[test]
fn content_changed_by_another_app_before_expiry_is_kept() {
    let fake = FakeBackend::default();
    let manager = manager_with_ttl(&fake, Duration::from_millis(300));
    assert_eq!(manager.deliver(&secret("prueba-no-real-1")), Ok(()));
    std::thread::sleep(Duration::from_millis(100));
    fake.set_content(text("copiado-por-otra-app"));

    assert!(wait_until(|| clear_calls(&fake) == 1));

    assert_eq!(fake.content(), text("copiado-por-otra-app"));
}

#[test]
fn same_password_copied_by_hand_is_cleared_at_the_original_expiry() {
    // Comportamiento aprobado: por contenido no se distingue de la entrega, así
    // que se borra cuando vence el plazo del envío original, ni antes ni después.
    let fake = FakeBackend::default();
    let manager = manager_with_ttl(&fake, Duration::from_millis(300));
    let start = Instant::now();
    assert_eq!(manager.deliver(&secret("prueba-no-real-1")), Ok(()));
    std::thread::sleep(Duration::from_millis(100));
    fake.set_content(text("prueba-no-real-1"));

    assert!(wait_until(|| fake.content() == FakeContent::Empty));

    let elapsed = start.elapsed();
    assert!(elapsed >= Duration::from_millis(300), "{elapsed:?}");
    assert!(elapsed < Duration::from_millis(900), "{elapsed:?}");
}

#[test]
fn second_delivery_replaces_the_timer() {
    let fake = FakeBackend::default();
    let manager = manager_with_ttl(&fake, Duration::from_millis(400));
    let start = Instant::now();
    assert_eq!(manager.deliver(&secret("prueba-no-real-1")), Ok(()));
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(manager.deliver(&secret("prueba-no-real-2")), Ok(()));

    // El vencimiento del primero (400 ms) ya pasó y no debe haber borrado nada.
    std::thread::sleep(start + Duration::from_millis(480) - Instant::now());
    assert_eq!(fake.content(), text("prueba-no-real-2"));
    assert_eq!(clear_calls(&fake), 0);

    // El segundo vence a los 150 + 400 ms.
    assert!(wait_until(|| fake.content() == FakeContent::Empty));
    assert!(start.elapsed() >= Duration::from_millis(550));
}

#[test]
fn failed_write_keeps_the_original_expiry() {
    let fake = FakeBackend::default();
    let manager = manager(&fake);
    let start = Instant::now();
    assert_eq!(manager.deliver(&secret("prueba-no-real-1")), Ok(()));
    fake.fail_next_write(WriteError::NotWritten, false);
    assert_eq!(
        manager.deliver(&secret("prueba-no-real-2")),
        Err(ClipboardError::WriteFailed)
    );

    assert!(wait_until(|| fake.content() == FakeContent::Empty));

    let elapsed = start.elapsed();
    assert!(elapsed >= Duration::from_millis(200), "{elapsed:?}");
    assert!(elapsed < Duration::from_millis(600), "{elapsed:?}");
}

#[test]
fn rejected_password_keeps_the_previous_timer() {
    let fake = FakeBackend::default();
    let manager = manager(&fake);
    assert_eq!(manager.deliver(&secret("prueba-no-real-1")), Ok(()));
    assert_eq!(
        manager.deliver(&secret("prueba\0no-real")),
        Err(ClipboardError::InvalidPassword)
    );

    assert!(wait_until(|| fake.content() == FakeContent::Empty));
}

#[test]
fn unavailable_retries_at_most_three_times_then_gives_up_without_clearing() {
    let fake = FakeBackend::default();
    fake.set_unavailable_for(usize::MAX);
    let manager = manager(&fake);
    assert_eq!(manager.deliver(&secret("prueba-no-real-1")), Ok(()));

    // El primer intento y tres reintentos: cuatro llamadas.
    assert!(wait_until(|| clear_calls(&fake) == 4));
    // Pasan varios intervalos más: no hay un quinto intento.
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(clear_calls(&fake), 4);
    assert_eq!(fake.content(), text("prueba-no-real-1"));
}

#[test]
fn unavailable_then_available_clears() {
    let fake = FakeBackend::default();
    fake.set_unavailable_for(1);
    let manager = manager(&fake);
    assert_eq!(manager.deliver(&secret("prueba-no-real-1")), Ok(()));

    assert!(wait_until(|| fake.content() == FakeContent::Empty));

    assert_eq!(
        fake.calls(),
        vec![
            FakeCall::Write,
            FakeCall::Clear(ClearOutcome::Unavailable),
            FakeCall::Clear(ClearOutcome::Cleared),
        ]
    );
}
