use std::time::Duration;

use tauri::{Manager, RunEvent};

use clipboard::ClipboardManager;

#[cfg(windows)]
pub mod atomic_file;
mod autostart;
pub mod clipboard;
#[cfg(windows)]
pub mod dpapi;
#[cfg(windows)]
pub mod identity;
#[cfg(windows)]
pub mod protected_file;
mod tray;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        // Debe registrarse primero. Si ya hay una instancia, la segunda avisa y
        // termina sola; la primera no tiene ventanas que mostrar, así que el
        // aviso no necesita hacer nada.
        .plugin(tauri_plugin_single_instance::init(|_app, _argv, _cwd| {}))
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .app_name("Bóveda")
                .build(),
        )
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            tray::setup(app)?;
            // El gestor del portapapeles vive toda la sesión; el servidor de
            // red (#100) lo usará al recibir una contraseña.
            app.manage(ClipboardManager::start());
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error al construir la aplicación");

    app.run(|app, event| {
        // La app vive en la bandeja y casi nunca tiene ventanas: cerrar la
        // última no debe terminar el proceso. Solo "Salir" (`app.exit`), que
        // trae un código de salida, lo termina.
        match event {
            RunEvent::ExitRequested {
                code: None, api, ..
            } => api.prevent_exit(),
            // Al salir de verdad (`app.exit` emite primero `ExitRequested` y
            // después `Exit`), un último intento acotado de borrar la
            // contraseña pendiente: si Windows no deja comprobarlo, no se borra.
            RunEvent::Exit => {
                if let Some(clipboard) = app.try_state::<ClipboardManager>() {
                    clipboard.shutdown(Duration::from_secs(2));
                }
            }
            _ => {}
        }
    });
}
