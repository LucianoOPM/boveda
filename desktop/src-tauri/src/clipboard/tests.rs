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

fn manager_with(fake: &FakeBackend, ttl_ms: u64, retry_ms: u64) -> ClipboardManager {
    ClipboardManager::with_backend(
        fake.clone(),
        Config {
            ttl: Duration::from_millis(ttl_ms),
            retry_interval: Duration::from_millis(retry_ms),
            ..fast_config()
        },
    )
}

#[test]
fn uncertain_write_that_left_the_text_is_cleared_immediately() {
    let fake = FakeBackend::default();
    fake.fail_next_write(WriteError::Uncertain, true);
    let manager = manager_with(&fake, 200, 20);

    assert_eq!(
        manager.deliver(&secret("prueba-no-real-1")),
        Err(ClipboardError::WriteFailed)
    );

    // Vacío en cuanto `deliver` contesta, sin esperar ningún plazo.
    assert_eq!(fake.content(), FakeContent::Empty);
    // Y no quedó ninguna entrada: pasa el plazo y no hay más llamadas.
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(
        fake.calls(),
        vec![FakeCall::Write, FakeCall::Clear(ClearOutcome::Cleared)]
    );
}

#[test]
fn uncertain_write_with_unavailable_clipboard_keeps_a_cleanup_entry() {
    let fake = FakeBackend::default();
    fake.fail_next_write(WriteError::Uncertain, true);
    fake.set_unavailable_for(1);
    let manager = manager_with(&fake, 5_000, 30);

    assert_eq!(
        manager.deliver(&secret("prueba-no-real-1")),
        Err(ClipboardError::WriteFailed)
    );

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

#[test]
fn uncertain_cleanup_does_not_remove_other_content() {
    let fake = FakeBackend::default();
    fake.fail_next_write(WriteError::Uncertain, true);
    fake.set_unavailable_for(1);
    let manager = manager_with(&fake, 5_000, 100);
    assert_eq!(
        manager.deliver(&secret("prueba-no-real-1")),
        Err(ClipboardError::WriteFailed)
    );
    // Otra aplicación copia algo antes del reintento.
    fake.set_content(text("copiado-por-otra-app"));

    assert!(wait_until(|| clear_calls(&fake) == 2));

    assert_eq!(fake.content(), text("copiado-por-otra-app"));
    assert_eq!(
        fake.calls().last(),
        Some(&FakeCall::Clear(ClearOutcome::NotOurs))
    );
}

#[test]
fn uncertain_write_keeps_the_previous_expiry() {
    let fake = FakeBackend::default();
    let manager = manager_with(&fake, 300, 20);
    let start = Instant::now();
    assert_eq!(manager.deliver(&secret("prueba-no-real-1")), Ok(()));
    // La escritura dudosa no dejó su texto: el portapapeles sigue con el primero.
    fake.fail_next_write(WriteError::Uncertain, false);
    assert_eq!(
        manager.deliver(&secret("prueba-no-real-2")),
        Err(ClipboardError::WriteFailed)
    );

    assert!(wait_until(|| fake.content() == FakeContent::Empty));

    assert!(start.elapsed() >= Duration::from_millis(300));
    assert_eq!(
        fake.calls(),
        vec![
            FakeCall::Write,
            FakeCall::Write,
            FakeCall::Clear(ClearOutcome::NotOurs),
            FakeCall::Clear(ClearOutcome::Cleared),
        ]
    );
}

#[test]
fn old_cleanup_never_removes_a_newer_password() {
    let fake = FakeBackend::default();
    fake.fail_next_write(WriteError::Uncertain, true);
    fake.set_unavailable_for(1);
    let manager = manager_with(&fake, 600, 100);
    assert_eq!(
        manager.deliver(&secret("prueba-no-real-1")),
        Err(ClipboardError::WriteFailed)
    );
    // Un envío correcto reemplaza la limpieza pendiente.
    assert_eq!(manager.deliver(&secret("prueba-no-real-2")), Ok(()));

    // Pasa el momento del reintento viejo (100 ms): la contraseña nueva sigue.
    std::thread::sleep(Duration::from_millis(250));
    assert_eq!(fake.content(), text("prueba-no-real-2"));
    assert_eq!(
        fake.calls(),
        vec![
            FakeCall::Write,
            FakeCall::Clear(ClearOutcome::Unavailable),
            FakeCall::Write,
        ]
    );
    // Y se borra a su propio vencimiento.
    assert!(wait_until(|| fake.content() == FakeContent::Empty));
}

#[test]
fn cleanup_entries_are_bounded() {
    let fake = FakeBackend::default();
    fake.set_unavailable_for(usize::MAX);
    let manager = manager_with(&fake, 5_000, 100);
    for attempt in 1..=3 {
        fake.fail_next_write(WriteError::Uncertain, true);
        assert_eq!(
            manager.deliver(&secret(&format!("prueba-no-real-{attempt}"))),
            Err(ClipboardError::WriteFailed)
        );
    }

    // Tres intentos inmediatos y, de la última entrada (las anteriores fueron
    // reemplazadas), tres reintentos más: seis llamadas y ninguna después.
    assert!(wait_until(|| clear_calls(&fake) == 6));
    std::thread::sleep(Duration::from_millis(400));
    assert_eq!(clear_calls(&fake), 6);
}

#[test]
fn shutdown_clears_the_pending_password() {
    let fake = FakeBackend::default();
    let manager = manager_with(&fake, 60_000, 20);
    assert_eq!(manager.deliver(&secret("prueba-no-real-1")), Ok(()));

    manager.shutdown(Duration::from_secs(2));

    assert_eq!(fake.content(), FakeContent::Empty);
}

#[test]
fn shutdown_does_not_clear_other_content() {
    let fake = FakeBackend::default();
    let manager = manager_with(&fake, 60_000, 20);
    assert_eq!(manager.deliver(&secret("prueba-no-real-1")), Ok(()));
    fake.set_content(text("copiado-por-el-usuario"));

    manager.shutdown(Duration::from_secs(2));

    assert_eq!(fake.content(), text("copiado-por-el-usuario"));
}

#[test]
fn shutdown_with_unavailable_clipboard_returns_within_the_timeout_and_does_not_clear() {
    let fake = FakeBackend::default();
    fake.set_unavailable_for(usize::MAX);
    let manager = manager_with(&fake, 60_000, 20);
    assert_eq!(manager.deliver(&secret("prueba-no-real-1")), Ok(()));

    let start = Instant::now();
    manager.shutdown(Duration::from_millis(300));

    assert!(start.elapsed() < Duration::from_millis(500));
    assert_eq!(fake.content(), text("prueba-no-real-1"));
    // Tres intentos y ninguno borró nada.
    assert_eq!(clear_calls(&fake), 3);
}

#[test]
fn shutdown_respects_a_deadline_shorter_than_its_attempts() {
    let fake = FakeBackend::default();
    fake.set_unavailable_for(usize::MAX);
    let manager = ClipboardManager::with_backend(
        fake.clone(),
        Config {
            ttl: Duration::from_secs(60),
            shutdown_interval: Duration::from_millis(200),
            shutdown_attempts: 50,
            ..fast_config()
        },
    );
    assert_eq!(manager.deliver(&secret("prueba-no-real-1")), Ok(()));

    let start = Instant::now();
    manager.shutdown(Duration::from_millis(300));

    // 50 intentos de 200 ms serían 10 s: el plazo corta mucho antes.
    assert!(start.elapsed() < Duration::from_millis(700));
    assert!(clear_calls(&fake) < 50);
    assert_eq!(fake.content(), text("prueba-no-real-1"));
}

#[test]
fn shutdown_twice_and_deliver_after_shutdown_are_safe() {
    let fake = FakeBackend::default();
    let manager = manager(&fake);
    manager.shutdown(Duration::from_secs(2));

    manager.shutdown(Duration::from_secs(2));

    assert_eq!(
        manager.deliver(&secret("prueba-no-real-1")),
        Err(ClipboardError::Stopped)
    );
    assert!(fake.calls().is_empty());
}

#[test]
fn shutdown_without_pending_entries_returns_quickly() {
    let fake = FakeBackend::default();
    let manager = manager(&fake);

    let start = Instant::now();
    manager.shutdown(Duration::from_secs(2));

    assert!(start.elapsed() < Duration::from_millis(500));
    assert!(fake.calls().is_empty());
}
