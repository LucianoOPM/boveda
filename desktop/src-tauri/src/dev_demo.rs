//! DISPARADOR TEMPORAL PARA LAS PRUEBAS MANUALES (#98). Solo existe en
//! compilaciones de depuración (`debug_assertions`): `tauri build` no lo incluye.
//! Se quita cuando #100 y #104 conecten el envío real.
//!
//! Sin el servidor de red (#100) nada llama todavía al gestor del portapapeles,
//! así que no habría forma de ver el aviso, el borrado ni la limpieza al salir
//! en la app real. Con la variable de entorno `BOVEDA_DEMO_PASSWORDS` (contraseñas
//! FICTICIAS separadas por comas), la app entrega la primera a los 10 s de
//! arrancar y cada una de las siguientes 5 s después de la anterior, usando el
//! mismo `ClipboardManager` y el mismo aviso que usará el envío real. Sin la
//! variable no hace nada. No escribe las contraseñas en ninguna salida.

use std::time::Duration;

use secrecy::SecretString;
use tauri::{AppHandle, Manager};

use crate::clipboard::{ClipboardManager, notify_ready};

const VARIABLE: &str = "BOVEDA_DEMO_PASSWORDS";
const FIRST_DELAY: Duration = Duration::from_secs(10);
const BETWEEN: Duration = Duration::from_secs(5);

/// Separa por comas, recorta los espacios y descarta los elementos vacíos.
fn parse_passwords(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Si la variable tiene contraseñas, lanza un hilo que las entrega con su aviso.
pub fn start(app: AppHandle) {
    let Ok(raw) = std::env::var(VARIABLE) else {
        return;
    };
    let passwords = parse_passwords(&raw);
    if passwords.is_empty() {
        return;
    }
    eprintln!(
        "[demo] {} contraseña(s) ficticia(s): la primera en {} s.",
        passwords.len(),
        FIRST_DELAY.as_secs()
    );
    std::thread::spawn(move || {
        std::thread::sleep(FIRST_DELAY);
        let total = passwords.len();
        for (index, password) in passwords.into_iter().enumerate() {
            if index > 0 {
                std::thread::sleep(BETWEEN);
            }
            let Some(manager) = app.try_state::<ClipboardManager>() else {
                return;
            };
            let secret = SecretString::from(password);
            let result = manager.deliver_and_notify(&secret, || notify_ready(&app));
            match result {
                Ok(()) => eprintln!("[demo] contraseña {} de {total} copiada.", index + 1),
                Err(error) => eprintln!("[demo] contraseña {} de {total}: {error}.", index + 1),
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_passwords_splits_on_commas_and_trims() {
        assert_eq!(
            parse_passwords("prueba-no-real-1, prueba-no-real-2 ,prueba-no-real-3"),
            vec![
                "prueba-no-real-1".to_owned(),
                "prueba-no-real-2".to_owned(),
                "prueba-no-real-3".to_owned()
            ]
        );
    }

    #[test]
    fn parse_passwords_ignores_empty_items() {
        assert_eq!(
            parse_passwords("a,,  ,b,"),
            vec!["a".to_owned(), "b".to_owned()]
        );
        assert!(parse_passwords("").is_empty());
        assert!(parse_passwords("   ").is_empty());
    }
}
