//! Proceso hijo de las pruebas reales del portapapeles (#98).
//!
//! No es parte de la app: lo lanzan las pruebas `#[ignore]` de
//! `tests/clipboard_windows.rs` para actuar como «otra aplicación» (otro
//! proceso) mientras la prueba mantiene el portapapeles abierto. Hace una sola
//! cosa y termina; no deja nada en ejecución. Solo recibe datos ficticios.
//!
//! Uso: `clipboard_probe open`, `clipboard_probe write <texto>` o
//! `clipboard_probe clear <texto esperado>`. Imprime una línea (`OPEN=…`,
//! `WRITE=…` o `CLEAR=…`).

#[cfg(windows)]
fn main() {
    use boveda_desktop_lib::clipboard::{ClipboardBackend, WindowsBackend};
    use clipboard_win::{Clipboard, raw};

    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        // Un solo intento, sin reintentos: devuelve de inmediato si está ocupado.
        Some("open") => match Clipboard::new() {
            Ok(_guard) => println!("OPEN=ok"),
            Err(_) => println!("OPEN=busy"),
        },
        // Intenta modificar el portapapeles como lo haría otra aplicación.
        Some("write") => match (Clipboard::new(), args.get(1)) {
            (Ok(_guard), Some(text)) => match raw::set_string(text) {
                Ok(()) => println!("WRITE=ok"),
                Err(_) => println!("WRITE=failed"),
            },
            (Err(_), _) => println!("WRITE=busy"),
            (Ok(_), None) => std::process::exit(2),
        },
        // Usa el mismo borrado condicional que la app.
        Some("clear") => {
            let Some(expected) = args.get(1) else {
                std::process::exit(2);
            };
            let outcome =
                WindowsBackend::new().clear_if_matches(&|bytes| bytes == expected.as_bytes());
            println!("CLEAR={outcome:?}");
        }
        _ => std::process::exit(2),
    }
}

#[cfg(not(windows))]
fn main() {}
