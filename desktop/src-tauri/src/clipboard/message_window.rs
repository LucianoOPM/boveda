//! Ventana de solo mensajes que hace de propietaria del portapapeles (#98).
//!
//! Con un propietario `NULL`, `OpenClipboard` **no bloquea** el portapapeles:
//! medido con dos procesos independientes en Windows 11 (build 26300), otro
//! proceso pudo abrirlo, escribir y hasta borrar mientras el primero lo
//! «retenía». Con una ventana propietaria real sí excluye a otros procesos y a
//! otros hilos. Por eso el borrado condicional abre con una ventana propia.
//!
//! Es de solo mensajes (`HWND_MESSAGE`): invisible, sin barra de tareas, y se
//! crea con la clase predefinida `STATIC`, sin registrar ninguna. Vive lo que
//! dura una operación y se destruye al soltarse: así no queda como propietaria
//! del portapapeles (de lo contrario, cada copia de otra aplicación le enviaría
//! `WM_DESTROYCLIPBOARD` a un hilo que no procesa mensajes).
//!
//! Todo el `unsafe` de las llamadas a Win32 está en este archivo.

use std::ptr;

use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::WindowsAndMessaging::{CreateWindowExW, DestroyWindow, HWND_MESSAGE};

/// Una ventana de solo mensajes. No es `Send` (guarda un puntero crudo): una
/// ventana pertenece al hilo que la creó y debe destruirse en él.
pub(super) struct MessageWindow(HWND);

impl MessageWindow {
    /// Crea la ventana en el hilo actual, o `None` si Windows no la crea.
    pub(super) fn new() -> Option<Self> {
        let class: Vec<u16> = "STATIC".encode_utf16().chain(Some(0)).collect();
        // SAFETY: `class` es una cadena UTF-16 terminada en cero que vive hasta
        // el final de la llamada; el resto de los punteros son nulos, que es lo
        // que `CreateWindowExW` admite; el padre `HWND_MESSAGE` es una constante
        // válida para una ventana de solo mensajes.
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                ptr::null(),
                0,
                0,
                0,
                0,
                0,
                HWND_MESSAGE,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null(),
            )
        };
        if hwnd.is_null() {
            None
        } else {
            Some(Self(hwnd))
        }
    }

    /// El manejador, para abrir el portapapeles con esta ventana como propietaria.
    pub(super) fn handle(&self) -> HWND {
        self.0
    }
}

impl Drop for MessageWindow {
    fn drop(&mut self) {
        // SAFETY: `self.0` lo creó `CreateWindowExW` en este mismo hilo (la
        // estructura no es `Send`) y solo se destruye aquí, una vez.
        if unsafe { DestroyWindow(self.0) } == 0 {
            eprintln!("No se pudo destruir la ventana del portapapeles.");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::IsWindow;

    #[test]
    fn creates_a_valid_window_and_destroys_it_on_drop() {
        let window = MessageWindow::new().expect("no se pudo crear la ventana");
        let hwnd = window.handle();
        assert!(!hwnd.is_null());
        // SAFETY: `IsWindow` acepta cualquier valor de `HWND`, válido o no.
        assert_ne!(unsafe { IsWindow(hwnd) }, 0);

        drop(window);

        // SAFETY: igual que arriba; el manejador ya destruido devuelve 0.
        assert_eq!(unsafe { IsWindow(hwnd) }, 0);
    }

    #[test]
    fn each_window_is_distinct() {
        let first = MessageWindow::new().expect("primera ventana");
        let second = MessageWindow::new().expect("segunda ventana");
        assert_ne!(first.handle(), second.handle());
    }
}
