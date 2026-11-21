//! Ícono de la bandeja y su menú de cuatro opciones (`docs/DESIGN.md` §8).

use tauri::{
    App, Manager, Wry,
    menu::{CheckMenuItem, IsMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
};
use tauri_plugin_autostart::ManagerExt;

use crate::autostart::toggle_autostart;

pub(crate) const ID_PAIR: &str = "pair";
pub(crate) const ID_DEVICES: &str = "devices";
pub(crate) const ID_AUTOSTART: &str = "autostart";
pub(crate) const ID_QUIT: &str = "quit";

/// Una opción del menú, descrita sin tocar Tauri para poder probarla.
pub(crate) struct MenuItemSpec {
    pub id: &'static str,
    pub label: &'static str,
    pub enabled: bool,
    /// `Some` solo en las opciones con casilla.
    pub checked: Option<bool>,
}

pub(crate) enum MenuEntry {
    Item(MenuItemSpec),
    Separator,
}

/// El menú completo. «Emparejar celular» abre la ventana del QR (#101);
/// «Dispositivos emparejados» sigue deshabilitada hasta que #107 implemente la suya.
pub(crate) fn menu_spec(autostart_enabled: bool) -> Vec<MenuEntry> {
    let item = |id, label, enabled, checked| {
        MenuEntry::Item(MenuItemSpec {
            id,
            label,
            enabled,
            checked,
        })
    };
    vec![
        item(ID_PAIR, "Emparejar celular", true, None),
        item(ID_DEVICES, "Dispositivos emparejados", false, None),
        item(
            ID_AUTOSTART,
            "Iniciar con Windows",
            true,
            Some(autostart_enabled),
        ),
        MenuEntry::Separator,
        item(ID_QUIT, "Salir", true, None),
    ]
}

/// La casilla de «Iniciar con Windows», guardada como estado de la app para
/// poder fijar su marca tras cada cambio.
pub(crate) struct AutostartItem(pub CheckMenuItem<Wry>);

pub fn setup(app: &App) -> tauri::Result<()> {
    // Si no se puede leer el registro, se muestra desactivada: es lo que el
    // sistema hará de todos modos si no hay una entrada válida.
    let autostart_enabled = app.autolaunch().is_enabled().unwrap_or(false);

    let mut built: Vec<Box<dyn IsMenuItem<Wry>>> = Vec::new();
    for entry in menu_spec(autostart_enabled) {
        match entry {
            MenuEntry::Separator => built.push(Box::new(PredefinedMenuItem::separator(app)?)),
            MenuEntry::Item(spec) => match spec.checked {
                Some(checked) => {
                    let item = CheckMenuItem::with_id(
                        app,
                        spec.id,
                        spec.label,
                        spec.enabled,
                        checked,
                        None::<&str>,
                    )?;
                    app.manage(AutostartItem(item.clone()));
                    built.push(Box::new(item));
                }
                None => built.push(Box::new(MenuItem::with_id(
                    app,
                    spec.id,
                    spec.label,
                    spec.enabled,
                    None::<&str>,
                )?)),
            },
        }
    }
    let refs: Vec<&dyn IsMenuItem<Wry>> = built.iter().map(|item| item.as_ref()).collect();
    let menu = Menu::with_items(app, &refs)?;

    let mut tray = TrayIconBuilder::with_id("main")
        .tooltip("Bóveda")
        .menu(&menu)
        // Temporal: `DESIGN.md` §8 pide que el clic izquierdo abra «Dispositivos
        // emparejados»; ese comportamiento lo implementa #107.
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| {
            match event.id().as_ref() {
                ID_QUIT => app.exit(0),
                #[cfg(windows)]
                ID_PAIR => crate::pairing::window::open_window(app),
                ID_AUTOSTART => {
                    // Windows ya cambió la marca al hacer clic; se reconcilia
                    // siempre con el estado real del sistema.
                    let item = app.state::<AutostartItem>();
                    let checked_after_click = item.0.is_checked().unwrap_or(true);
                    let real = toggle_autostart(&*app.autolaunch(), checked_after_click);
                    if let Err(error) = item.0.set_checked(real) {
                        eprintln!("No se pudo actualizar la casilla: {error}");
                    }
                }
                _ => {}
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items(entries: &[MenuEntry]) -> Vec<&MenuItemSpec> {
        entries
            .iter()
            .filter_map(|entry| match entry {
                MenuEntry::Item(item) => Some(item),
                MenuEntry::Separator => None,
            })
            .collect()
    }

    fn item<'a>(entries: &'a [MenuEntry], id: &str) -> &'a MenuItemSpec {
        items(entries)
            .into_iter()
            .find(|item| item.id == id)
            .expect("la opción debe existir")
    }

    #[test]
    fn menu_has_exactly_four_items_and_one_separator() {
        let entries = menu_spec(false);

        assert_eq!(entries.len(), 5);
        assert!(matches!(entries[3], MenuEntry::Separator));
        let ids: Vec<&str> = items(&entries).iter().map(|item| item.id).collect();
        assert_eq!(ids, [ID_PAIR, ID_DEVICES, ID_AUTOSTART, ID_QUIT]);
        let labels: Vec<&str> = items(&entries).iter().map(|item| item.label).collect();
        assert_eq!(
            labels,
            [
                "Emparejar celular",
                "Dispositivos emparejados",
                "Iniciar con Windows",
                "Salir"
            ]
        );
    }

    #[test]
    fn pair_is_enabled_and_devices_wait_for_107() {
        let entries = menu_spec(false);

        assert!(item(&entries, ID_PAIR).enabled);
        assert!(!item(&entries, ID_DEVICES).enabled);
        assert!(item(&entries, ID_AUTOSTART).enabled);
        assert!(item(&entries, ID_QUIT).enabled);
    }

    #[test]
    fn autostart_check_follows_given_state() {
        assert_eq!(item(&menu_spec(true), ID_AUTOSTART).checked, Some(true));
        assert_eq!(item(&menu_spec(false), ID_AUTOSTART).checked, Some(false));
        for id in [ID_PAIR, ID_DEVICES, ID_QUIT] {
            assert_eq!(item(&menu_spec(true), id).checked, None);
        }
    }
}
