//! Pruebas del portapapeles REAL de Windows (#98).
//!
//! Todas son `#[ignore]`: **sobrescriben y vacían el portapapeles del usuario**
//! y comparten un recurso global. Usan contraseñas ficticias. Se ejecutan a
//! mano, avisando antes:
//!
//! ```text
//! cargo test -p boveda-desktop --test clipboard_windows -- --ignored --test-threads=1 --nocapture
//! ```
#![cfg(windows)]

use std::cell::Cell;
use std::time::{Duration, Instant};

use boveda_desktop_lib::clipboard::{
    ClearOutcome, ClipboardBackend, ClipboardManager, Config, WindowsBackend,
};
use clipboard_win::{Clipboard, formats, raw};
use secrecy::SecretString;

/// Deja el portapapeles vacío al terminar la prueba, pase o falle.
struct EmptyOnDrop;

impl Drop for EmptyOnDrop {
    fn drop(&mut self) {
        if let Ok(_open) = Clipboard::new_attempts(10) {
            let _ = raw::empty();
        }
    }
}

/// El texto actual del portapapeles, o `None` si no hay texto.
fn read_text() -> Option<Vec<u8>> {
    let _open = Clipboard::new_attempts(10).expect("no se pudo abrir el portapapeles");
    if !raw::is_format_avail(formats::CF_UNICODETEXT) {
        return None;
    }
    let mut out = Vec::new();
    raw::get_string(&mut out).expect("no se pudo leer el texto");
    Some(out)
}

/// ¿Puede otro hilo abrir el portapapeles ahora mismo (sin reintentos)?
fn another_thread_can_open() -> bool {
    std::thread::spawn(|| Clipboard::new().is_ok())
        .join()
        .expect("el hilo no debe caer")
}

#[test]
#[ignore = "usa el portapapeles real: sobrescribe y vacía el del usuario"]
fn write_sensitive_puts_the_text_and_the_exclusion_formats() {
    let _cleanup = EmptyOnDrop;
    let mut backend = WindowsBackend::new();

    backend
        .write_sensitive("prueba-no-real-escritura")
        .expect("la escritura debe funcionar");

    assert_eq!(
        read_text().as_deref(),
        Some(&b"prueba-no-real-escritura"[..])
    );
    let _open = Clipboard::new_attempts(10).expect("no se pudo abrir el portapapeles");
    for name in ["CanIncludeInClipboardHistory", "CanUploadToCloudClipboard"] {
        let format = raw::register_format(name).expect("el formato debe registrarse");
        assert!(
            raw::is_format_avail(format.get()),
            "falta el formato de exclusión {name}"
        );
    }
}

#[test]
#[ignore = "usa el portapapeles real: sobrescribe y vacía el del usuario"]
fn clear_if_matches_clears_matching_text_even_with_exclusion_formats_present() {
    let _cleanup = EmptyOnDrop;
    let mut backend = WindowsBackend::new();
    backend.write_sensitive("prueba-no-real-borrar").unwrap();

    let outcome = backend.clear_if_matches(&|bytes| bytes == b"prueba-no-real-borrar");

    assert_eq!(outcome, ClearOutcome::Cleared);
    assert_eq!(read_text(), None);
    // La guarda se liberó: otro hilo puede abrir el portapapeles.
    assert!(another_thread_can_open());
}

#[test]
#[ignore = "usa el portapapeles real: sobrescribe y vacía el del usuario"]
fn clear_if_matches_leaves_different_text_untouched() {
    let _cleanup = EmptyOnDrop;
    let mut backend = WindowsBackend::new();
    backend.write_sensitive("copiado-por-el-usuario").unwrap();

    let outcome = backend.clear_if_matches(&|bytes| bytes == b"prueba-no-real-otra");

    assert_eq!(outcome, ClearOutcome::NotOurs);
    assert_eq!(read_text().as_deref(), Some(&b"copiado-por-el-usuario"[..]));
    // También se libera en el camino que no borra.
    assert!(another_thread_can_open());
}

#[test]
#[ignore = "usa el portapapeles real: sobrescribe y vacía el del usuario"]
fn clear_if_matches_leaves_a_non_text_clipboard_untouched() {
    let _cleanup = EmptyOnDrop;
    let format = raw::register_format("BovedaPruebaFormatoSinTexto").expect("registrar formato");
    {
        let _open = Clipboard::new_attempts(10).unwrap();
        raw::set(format.get(), &[1, 2, 3]).expect("poner un formato sin texto");
    }
    let mut backend = WindowsBackend::new();

    let outcome = backend.clear_if_matches(&|_| true);

    assert_eq!(outcome, ClearOutcome::NotOurs);
    let _open = Clipboard::new_attempts(10).unwrap();
    assert!(raw::is_format_avail(format.get()));
}

#[test]
#[ignore = "usa el portapapeles real: sobrescribe y vacía el del usuario"]
fn non_bmp_and_accented_text_roundtrips_and_is_cleared() {
    let _cleanup = EmptyOnDrop;
    let password = "Bóveda-🔐-prueba-no-real";
    let manager = ClipboardManager::with_backend(
        WindowsBackend::new(),
        Config {
            ttl: Duration::from_millis(400),
            ..Config::default()
        },
    );

    manager
        .deliver(&SecretString::from(password.to_owned()))
        .expect("la entrega debe funcionar");

    // Tras el viaje UTF-8 → UTF-16 → UTF-8 de Windows, el texto es el mismo.
    assert_eq!(read_text().as_deref(), Some(password.as_bytes()));
    // Y como la huella coincide, se borra a su plazo.
    let limit = Instant::now() + Duration::from_secs(5);
    while read_text().is_some() && Instant::now() < limit {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(read_text(), None);
}

#[test]
#[ignore = "usa el portapapeles real: sobrescribe y vacía el del usuario"]
fn clipboard_stays_open_while_matching() {
    let _cleanup = EmptyOnDrop;
    let mut backend = WindowsBackend::new();
    backend.write_sensitive("prueba-no-real-abierto").unwrap();
    let was_held = Cell::new(false);

    let outcome = backend.clear_if_matches(&|bytes| {
        // Esto corre mientras `clear_if_matches` tiene el portapapeles abierto:
        // ningún otro hilo puede abrirlo, es decir, nada puede cambiarlo entre
        // la lectura, la comparación y el vaciado.
        was_held.set(!another_thread_can_open());
        bytes == b"prueba-no-real-abierto"
    });

    assert_eq!(outcome, ClearOutcome::Cleared);
    assert!(
        was_held.get(),
        "el portapapeles no estaba abierto durante la comparación"
    );
    assert!(another_thread_can_open(), "la guarda no se liberó");
}

#[test]
#[ignore = "usa el portapapeles real: sobrescribe y vacía el del usuario"]
fn occupied_clipboard_is_unavailable_and_nothing_is_cleared() {
    let _cleanup = EmptyOnDrop;
    let mut backend = WindowsBackend::new();
    backend.write_sensitive("prueba-no-real-ocupado").unwrap();

    // Esta prueba mantiene el portapapeles abierto; otro hilo intenta borrar.
    let outcome = {
        let _held = Clipboard::new_attempts(10).unwrap();
        std::thread::spawn(|| {
            WindowsBackend::new().clear_if_matches(&|bytes| bytes == b"prueba-no-real-ocupado")
        })
        .join()
        .unwrap()
    };

    assert_eq!(outcome, ClearOutcome::Unavailable);
    // Soltado el portapapeles, el texto seguía ahí: no se borró a ciegas.
    assert_eq!(read_text().as_deref(), Some(&b"prueba-no-real-ocupado"[..]));
    // Y ahora sí se puede borrar normalmente.
    assert_eq!(
        backend.clear_if_matches(&|bytes| bytes == b"prueba-no-real-ocupado"),
        ClearOutcome::Cleared
    );
}

/// Demostración para las pruebas manuales (Win+V, nube y borrado a los 30 s).
///
/// Escribe una contraseña ficticia con el gestor real, espera 35 s y comprueba
/// que el portapapeles quedó vacío. Durante la espera, abrir Win+V: la
/// contraseña NO debe aparecer en el historial; con la sincronización del
/// portapapeles activa, tampoco en otro equipo.
#[test]
#[ignore = "usa el portapapeles real: sobrescribe y vacía el del usuario"]
fn manual_demo_writes_waits_and_clears() {
    let _cleanup = EmptyOnDrop;
    let manager = ClipboardManager::new();

    manager
        .deliver(&SecretString::from(
            "prueba-no-real-demostracion-win-v".to_owned(),
        ))
        .expect("la entrega debe funcionar");
    println!("Contraseña ficticia en el portapapeles. Abre Win+V y mira el historial.");
    println!("Se borrará sola a los 30 s; la prueba espera 35 s.");
    std::thread::sleep(Duration::from_secs(35));

    assert_eq!(read_text(), None, "el portapapeles debía quedar vacío");
}
