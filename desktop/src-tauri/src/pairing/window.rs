//! Ventana de emparejamiento, órdenes de Tauri y notificador real (#101).
//!
//! Hay una sola ventana (`pairing`), fija y centrada. Rust decide todo: la
//! ventana solo dibuja la [`PairingView`] que recibe y envía órdenes sin datos
//! (`pairing_cancel`, `pairing_regenerate`, `pairing_accept`, `pairing_reject`).
//! Todas las operaciones de ventana se hacen en el hilo principal
//! (`run_on_main_thread`) y se encolan, así que el controlador nunca espera a la
//! interfaz ni se re-entra a sí mismo.
//!
//! `content_protected(true)` pide a Windows excluir la ventana de las capturas
//! de pantalla (`SetWindowDisplayAffinity` con `WDA_EXCLUDEFROMCAPTURE`, según el
//! código de `tao` 0.37.3; el resultado de la llamada se ignora). **No es una
//! garantía**: no impide una cámara ni una captura por hardware, y no se
//! comprobó en versiones de Windows anteriores a la 10 2004.

use std::sync::Arc;

use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, State, UserAttentionType, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder, WindowEvent,
};

use super::{Notifier, PairingController, PairingView};

/// Etiqueta de la ventana.
pub const LABEL: &str = "pairing";
/// Evento que lleva la vista a la ventana.
const EVENT: &str = "pairing://state";
/// Ventana del QR (`DESIGN.md` §8).
const QR_SIZE: (f64, f64) = (360.0, 480.0);
/// Ventana de la confirmación (`DESIGN.md` §8).
const CONFIRM_SIZE: (f64, f64) = (360.0, 200.0);

/// Tamaño de la ventana para una vista; `None` si no debe haber ventana.
pub(crate) fn size_for(view: &PairingView) -> Option<(f64, f64)> {
    match view {
        PairingView::Closed => None,
        PairingView::Waiting { .. } | PairingView::NoNetwork | PairingView::Ended { .. } => {
            Some(QR_SIZE)
        }
        PairingView::Confirming { .. } | PairingView::Saving { .. } | PairingView::Done { .. } => {
            Some(CONFIRM_SIZE)
        }
    }
}

/// ¿Debe llamar la atención del usuario? Solo la confirmación: hay un plazo.
pub(crate) fn wants_attention(view: &PairingView) -> bool {
    matches!(view, PairingView::Confirming { .. })
}

/// Avisa a la ventana de cada cambio de vista.
pub struct TauriNotifier {
    app: AppHandle,
}

impl TauriNotifier {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl Notifier for TauriNotifier {
    fn changed(&self, view: &PairingView) {
        let app = self.app.clone();
        let view = view.clone();
        if let Err(error) = self.app.run_on_main_thread(move || apply(&app, &view)) {
            eprintln!("emparejamiento: no se pudo actualizar la ventana: {error}");
        }
    }
}

/// Crea la ventana si hace falta y le entrega la vista. Corre en el hilo principal.
fn apply(app: &AppHandle, view: &PairingView) {
    let Some(size) = size_for(view) else {
        // `destroy` no emite `CloseRequested`: no vuelve a llamar al controlador.
        if let Some(window) = app.get_webview_window(LABEL) {
            let _ = window.destroy();
        }
        return;
    };
    let window = match ensure_window(app, size) {
        Ok(window) => window,
        Err(error) => {
            eprintln!("emparejamiento: no se pudo abrir la ventana: {error}");
            return;
        }
    };
    let _ = window.set_size(LogicalSize::new(size.0, size.1));
    // El evento se pierde si el webview aún no escucha: al cargar pide
    // `pairing_snapshot`, que devuelve el estado vigente.
    let _ = app.emit_to(LABEL, EVENT, view);
    if wants_attention(view) {
        let _ = window.set_focus();
        let _ = window.request_user_attention(Some(UserAttentionType::Informational));
    }
}

fn ensure_window(app: &AppHandle, size: (f64, f64)) -> tauri::Result<WebviewWindow> {
    if let Some(window) = app.get_webview_window(LABEL) {
        return Ok(window);
    }
    let window = WebviewWindowBuilder::new(
        app,
        LABEL,
        WebviewUrl::App("index.html?view=pairing".into()),
    )
    .title("Emparejar celular")
    .inner_size(size.0, size.1)
    .resizable(false)
    .maximizable(false)
    .center()
    .content_protected(true)
    .build()?;
    let handle = app.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { api, .. } = event
            && let Some(controller) = handle.try_state::<Arc<PairingController>>()
        {
            if matches!(controller.snapshot(), PairingView::Saving { .. }) {
                // El usuario ya aceptó: el cierre del protocolo es corto.
                api.prevent_close();
            } else {
                controller.cancel();
            }
        }
    });
    Ok(window)
}

/// «Emparejar celular»: crea la sesión y la ventana, o enfoca la que ya existe.
pub fn open_window(app: &AppHandle) {
    let Some(controller) = app.try_state::<Arc<PairingController>>() else {
        eprintln!("emparejamiento: no disponible (identidad o lista de emparejados ilegibles)");
        return;
    };
    controller.open();
    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.set_focus();
    }
}

/// El estado vigente: el webview lo pide al cargar y al recargarse.
#[tauri::command]
pub fn pairing_snapshot(controller: State<'_, Arc<PairingController>>) -> PairingView {
    controller.snapshot()
}

/// Cancelar, Esc: invalida la sesión y cierra.
#[tauri::command]
pub fn pairing_cancel(controller: State<'_, Arc<PairingController>>) {
    controller.cancel();
}

/// «Generar otro código».
#[tauri::command]
pub fn pairing_regenerate(controller: State<'_, Arc<PairingController>>) {
    controller.regenerate();
}

/// «Emparejar»: Rust revisa que haya una confirmación vigente y su plazo.
#[tauri::command]
pub fn pairing_accept(controller: State<'_, Arc<PairingController>>) {
    controller.accept();
}

/// «Rechazar».
#[tauri::command]
pub fn pairing_reject(controller: State<'_, Arc<PairingController>>) {
    controller.reject();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pairing::{EndReason, PairingView};

    fn ended() -> PairingView {
        PairingView::Ended {
            reason: EndReason::Expired,
            placeholder: String::new(),
        }
    }

    #[test]
    fn qr_phases_use_the_tall_window() {
        let waiting = PairingView::Waiting {
            qr: String::new(),
            remaining_ms: 1,
        };
        for view in [waiting, PairingView::NoNetwork, ended()] {
            assert_eq!(size_for(&view), Some((360.0, 480.0)), "{view:?}");
        }
    }

    #[test]
    fn confirmation_phases_use_the_short_window() {
        let confirming = PairingView::Confirming {
            name: String::new(),
            fingerprint: String::new(),
            remaining_ms: 1,
        };
        let saving = PairingView::Saving {
            name: String::new(),
        };
        let done = PairingView::Done {
            name: String::new(),
        };
        for view in [confirming, saving, done] {
            assert_eq!(size_for(&view), Some((360.0, 200.0)), "{view:?}");
        }
    }

    #[test]
    fn closed_has_no_window() {
        assert_eq!(size_for(&PairingView::Closed), None);
    }

    #[test]
    fn only_confirming_asks_for_attention() {
        assert!(wants_attention(&PairingView::Confirming {
            name: String::new(),
            fingerprint: String::new(),
            remaining_ms: 1,
        }));
        assert!(!wants_attention(&ended()));
        assert!(!wants_attention(&PairingView::Closed));
    }
}
