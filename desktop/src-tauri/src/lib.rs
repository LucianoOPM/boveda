use tauri::RunEvent;

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
        .build(tauri::generate_context!())
        .expect("error al construir la aplicación");

    app.run(|_app, event| {
        // La app vive en la bandeja y casi nunca tiene ventanas: cerrar la
        // última no debe terminar el proceso. Solo "Salir" (`app.exit`), que
        // trae un código de salida, lo termina.
        if let RunEvent::ExitRequested {
            code: None, api, ..
        } = event
        {
            api.prevent_exit();
        }
    });
}
