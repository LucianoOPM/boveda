//! Pruebas del portapapeles REAL de Windows (#98).
//!
//! Todas son `#[ignore]`: **sobrescriben y vacían el portapapeles del usuario**
//! y comparten un recurso global. Usan contraseñas ficticias. Se ejecutan a
//! mano, avisando antes:
//!
//! ```text
//! cargo build -p boveda-desktop --example clipboard_probe
//! cargo test -p boveda-desktop --test clipboard_windows -- --ignored --test-threads=1 --nocapture
//! ```
//!
//! Para comprobar la exclusión usan **otro proceso** (`examples/clipboard_probe.rs`):
//! con `OpenClipboard(NULL)`, Windows 11 (build 26300) deja que otro hilo del
//! MISMO proceso abra el portapapeles aunque ya esté abierto, así que una
//! comprobación con hilos no demuestra nada. Lo que importa es otra aplicación.
#![cfg(windows)]

use std::cell::Cell;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
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

/// El ejecutable de la sonda, en `examples/` del directorio de salida de Cargo
/// (el ejecutable de la prueba vive en el directorio de compilación, que no es
/// ese).
fn probe_exe() -> PathBuf {
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target"));
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let path = target
        .join(profile)
        .join("examples")
        .join("clipboard_probe.exe");
    assert!(
        path.exists(),
        "falta {path:?}: compílalo con `cargo build -p boveda-desktop --example clipboard_probe`"
    );
    path
}

/// Lanza la sonda como un proceso independiente, espera a que termine (como
/// mucho 10 s; si no, la mata) y devuelve su línea de salida.
fn run_probe(args: &[&str]) -> String {
    let mut child = Command::new(probe_exe())
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("no se pudo lanzar la sonda");
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(10)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("la sonda no terminó a tiempo");
            }
        }
    }
    let mut out = String::new();
    child
        .stdout
        .take()
        .expect("salida de la sonda")
        .read_to_string(&mut out)
        .expect("leer la salida de la sonda");
    out.trim().to_owned()
}

/// ¿Puede otra aplicación (otro proceso) abrir el portapapeles ahora mismo?
fn another_process_can_open() -> bool {
    run_probe(&["open"]) == "OPEN=ok"
}

#[test]
#[ignore = "usa el portapapeles real: sobrescribe y vacía el del usuario"]
fn another_process_cannot_open_while_the_guard_is_held_and_can_after() {
    let _cleanup = EmptyOnDrop;
    assert!(
        another_process_can_open(),
        "libre: la sonda debe poder abrir"
    );

    {
        let _held = Clipboard::new_attempts(10).expect("no se pudo abrir el portapapeles");
        assert_eq!(run_probe(&["open"]), "OPEN=busy");
    }

    assert!(
        another_process_can_open(),
        "liberado: la sonda debe poder abrir"
    );
}

#[test]
#[ignore = "usa el portapapeles real: sobrescribe y vacía el del usuario"]
fn another_process_cannot_modify_the_clipboard_while_the_guard_is_held() {
    let _cleanup = EmptyOnDrop;
    let mut backend = WindowsBackend::new();
    backend.write_sensitive("prueba-no-real-original").unwrap();

    {
        let _held = Clipboard::new_attempts(10).expect("no se pudo abrir el portapapeles");
        assert_eq!(run_probe(&["write", "prueba-no-real-ajeno"]), "WRITE=busy");
    }

    assert_eq!(
        read_text().as_deref(),
        Some(&b"prueba-no-real-original"[..])
    );
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
    // La guarda se liberó: otra aplicación puede abrir el portapapeles.
    assert!(another_process_can_open());
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
    assert!(another_process_can_open());
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
    let observed = Cell::new((String::new(), String::new()));

    let outcome = backend.clear_if_matches(&|bytes| {
        // Esto corre mientras `clear_if_matches` tiene el portapapeles abierto
        // (entre la lectura y el vaciado): otra aplicación no puede abrirlo ni
        // cambiarlo. Con la mutación de «dos aperturas» esto falla.
        observed.set((
            run_probe(&["open"]),
            run_probe(&["write", "prueba-no-real-ajeno"]),
        ));
        bytes == b"prueba-no-real-abierto"
    });

    let (open, write) = observed.take();
    assert_eq!(
        open, "OPEN=busy",
        "otra aplicación pudo abrir durante la comparación"
    );
    assert_eq!(
        write, "WRITE=busy",
        "otra aplicación pudo escribir durante la comparación"
    );
    assert_eq!(outcome, ClearOutcome::Cleared);
    assert!(another_process_can_open(), "la guarda no se liberó");
}

#[test]
#[ignore = "usa el portapapeles real: sobrescribe y vacía el del usuario"]
fn occupied_clipboard_is_unavailable_and_nothing_is_cleared() {
    let _cleanup = EmptyOnDrop;
    let mut backend = WindowsBackend::new();
    backend.write_sensitive("prueba-no-real-ocupado").unwrap();

    // Esta prueba mantiene el portapapeles abierto; otra aplicación (otro
    // proceso, con el mismo borrado condicional) intenta borrar.
    let outcome = {
        let _held = Clipboard::new_attempts(10).unwrap();
        run_probe(&["clear", "prueba-no-real-ocupado"])
    };

    assert_eq!(outcome, "CLEAR=Unavailable");
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
    let manager = ClipboardManager::start();

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
