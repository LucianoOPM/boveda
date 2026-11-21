use std::time::Duration;

use tauri::{Manager, RunEvent};

use clipboard::ClipboardManager;

#[cfg(windows)]
pub mod announce;
#[cfg(windows)]
pub mod atomic_file;
mod autostart;
#[cfg(windows)]
mod backoff;
pub mod clipboard;
#[cfg(windows)]
pub mod dpapi;
#[cfg(windows)]
pub mod identity;
#[cfg(windows)]
pub mod network;
#[cfg(windows)]
pub mod paired_devices;
#[cfg(windows)]
pub mod pairing;
#[cfg(windows)]
pub mod protected_file;
pub mod qr;
#[cfg(windows)]
pub mod server;
mod tray;

/// Garantiza que la identidad de la PC exista tras el primer arranque (#99).
///
/// El par de llaves se descarta: el emparejamiento (#101) y el servidor (#100)
/// abrirán los almacenes por su cuenta con la misma carpeta. Si falla (archivo
/// dañado, de otra cuenta), solo se registra el tipo de error y la app sigue en
/// la bandeja; cuando exista una ventana de emparejamiento, ahí se mostrará.
/// Nunca se regenera una identidad que no se pudo leer.
#[cfg(windows)]
fn ensure_identity(app: &tauri::App) {
    let Ok(dir) = app.path().app_data_dir() else {
        eprintln!("identidad de la PC: no se encontró la carpeta de datos de la app");
        return;
    };
    let store = identity::IdentityStore::new(dir.join("identity.bin"));
    if let Err(err) = store.load_or_create() {
        eprintln!("identidad de la PC: {err}");
    }
}

/// Arranca el emparejamiento (#101) y el servidor de red local (#100).
///
/// La lista de emparejados es única (un solo dueño con escritura): la comparten
/// el servidor, que la consulta para autorizar, y el controlador del
/// emparejamiento, que agrega celulares. Sin lista legible no se arranca ninguno
/// de los dos. Sin identidad legible el servidor no arranca (nunca se regenera,
/// ver `ensure_identity`), pero el controlador sí existe y la ventana explica el
/// problema. El servidor solo escucha en redes privadas; ver `server::supervisor`.
#[cfg(windows)]
fn start_pairing_and_server(app: &tauri::App) {
    use std::sync::{Arc, Mutex};

    use announce::MdnsAnnouncer;
    use network::NlmProbe;
    use pairing::{LateNetworkView, PairingController, PairingCore, window::TauriNotifier};
    use server::{Config, Deps, NetworkView, ServerSupervisor, TauriSink};
    use vault_core::protocol::PairingTimeouts;

    let Ok(dir) = app.path().app_data_dir() else {
        eprintln!("servidor: no se encontró la carpeta de datos de la app");
        return;
    };
    let paired = match paired_devices::PairedDeviceStore::open(dir.join("paired-devices.bin")) {
        Ok(store) => Arc::new(Mutex::new(store)),
        Err(err) => {
            eprintln!("servidor: lista de emparejados no disponible: {err}; no se arranca");
            return;
        }
    };
    let identity_store = identity::IdentityStore::new(dir.join("identity.bin"));
    let identity = match identity_store.load() {
        Ok(Some(identity)) => Some(identity),
        Ok(None) => {
            eprintln!("servidor: no hay identidad de la PC; no se arranca");
            None
        }
        Err(err) => {
            eprintln!("servidor: identidad no disponible: {err}; no se arranca");
            None
        }
    };

    // El controlador necesita saber qué escucha el servidor y el servidor
    // necesita al controlador: la vista de la red se completa después.
    let network = Arc::new(LateNetworkView::default());
    let controller = Arc::new(PairingController::start(
        PairingCore::new(Arc::new(identity_store), Arc::clone(&paired)),
        Arc::clone(&network) as Arc<dyn NetworkView>,
        Arc::new(TauriNotifier::new(app.handle().clone())),
        PairingTimeouts::default(),
        Duration::from_millis(250),
    ));
    app.manage(Arc::clone(&controller));

    let Some(identity) = identity else {
        return;
    };
    let deps = Deps {
        identity: Arc::new(identity),
        paired,
        sink: Arc::new(TauriSink::new(app.handle().clone())),
        pairing: controller,
        announcer: Arc::new(|| Box::new(MdnsAnnouncer::new())),
    };
    let supervisor = Arc::new(ServerSupervisor::start(
        NlmProbe::new(),
        deps,
        Config::default(),
    ));
    network.set(Arc::clone(&supervisor) as Arc<dyn NetworkView>);
    app.manage(supervisor);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
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
            // red (#100) lo usa al recibir una contraseña.
            app.manage(ClipboardManager::start());
            ensure_identity(app);
            #[cfg(windows)]
            start_pairing_and_server(app);
            Ok(())
        });
    // Órdenes de la ventana de emparejamiento: sin datos, Rust decide (#101).
    #[cfg(windows)]
    let builder = builder.invoke_handler(tauri::generate_handler![
        pairing::window::pairing_snapshot,
        pairing::window::pairing_cancel,
        pairing::window::pairing_regenerate,
        pairing::window::pairing_accept,
        pairing::window::pairing_reject,
    ]);
    let app = builder
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
                // Primero el emparejamiento (cancela la sesión y su código) y
                // el servidor, para que ya no llegue ninguna contraseña nueva
                // mientras se vacía el portapapeles.
                #[cfg(windows)]
                {
                    if let Some(controller) =
                        app.try_state::<std::sync::Arc<pairing::PairingController>>()
                    {
                        controller.shutdown();
                    }
                    if let Some(server) =
                        app.try_state::<std::sync::Arc<server::ServerSupervisor>>()
                    {
                        server.stop();
                    }
                }
                if let Some(clipboard) = app.try_state::<ClipboardManager>() {
                    clipboard.shutdown(Duration::from_secs(2));
                }
            }
            _ => {}
        }
    });
}
