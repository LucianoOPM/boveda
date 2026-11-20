//! Lógica de la casilla «Iniciar con Windows».

/// Lo mínimo que la casilla necesita del sistema. Es un trait para poder
/// probar la lógica con un doble que falla a voluntad.
pub(crate) trait AutostartControl {
    fn enable(&self) -> Result<(), String>;
    fn disable(&self) -> Result<(), String>;
    fn is_enabled(&self) -> Result<bool, String>;
}

impl AutostartControl for tauri_plugin_autostart::AutoLaunchManager {
    // Los métodos propios del gestor tienen prioridad sobre los del trait, así
    // que estas llamadas van al plugin y no a sí mismas.
    fn enable(&self) -> Result<(), String> {
        self.enable().map_err(|error| error.to_string())
    }

    fn disable(&self) -> Result<(), String> {
        self.disable().map_err(|error| error.to_string())
    }

    fn is_enabled(&self) -> Result<bool, String> {
        self.is_enabled().map_err(|error| error.to_string())
    }
}

/// Cambia «Iniciar con Windows» al estado contrario del real y devuelve el
/// estado que la casilla debe mostrar. `checked_after_click` es la marca que
/// la casilla ya tiene tras el clic (Windows la invierte antes de avisar);
/// solo sirve para restaurar la anterior si el sistema no deja leer el estado
/// real. Si el cambio falla, se devuelve igualmente lo que el sistema dice, no
/// lo que se pedía.
pub(crate) fn toggle_autostart(control: &dyn AutostartControl, checked_after_click: bool) -> bool {
    let real = match control.is_enabled() {
        Ok(real) => real,
        Err(error) => {
            eprintln!("No se pudo leer «Iniciar con Windows»: {error}");
            return !checked_after_click;
        }
    };
    let changed = if real {
        control.disable()
    } else {
        control.enable()
    };
    if let Err(error) = changed {
        eprintln!("No se pudo cambiar «Iniciar con Windows»: {error}");
    }
    control.is_enabled().unwrap_or(real)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[derive(Default)]
    struct FakeControl {
        enabled: Cell<bool>,
        fail_enable: bool,
        fail_disable: bool,
        fail_read: bool,
        changes: Cell<u32>,
    }

    impl AutostartControl for FakeControl {
        fn enable(&self) -> Result<(), String> {
            self.changes.set(self.changes.get() + 1);
            if self.fail_enable {
                return Err("no se pudo activar".into());
            }
            self.enabled.set(true);
            Ok(())
        }

        fn disable(&self) -> Result<(), String> {
            self.changes.set(self.changes.get() + 1);
            if self.fail_disable {
                return Err("no se pudo desactivar".into());
            }
            self.enabled.set(false);
            Ok(())
        }

        fn is_enabled(&self) -> Result<bool, String> {
            if self.fail_read {
                return Err("no se pudo leer".into());
            }
            Ok(self.enabled.get())
        }
    }

    fn fake(enabled: bool) -> FakeControl {
        FakeControl {
            enabled: Cell::new(enabled),
            ..Default::default()
        }
    }

    #[test]
    fn enables_when_disabled() {
        let control = fake(false);

        assert!(toggle_autostart(&control, false));
        assert!(control.enabled.get());
    }

    #[test]
    fn disables_when_enabled() {
        let control = fake(true);

        assert!(!toggle_autostart(&control, true));
        assert!(!control.enabled.get());
    }

    #[test]
    fn shows_real_state_when_enable_fails() {
        let control = FakeControl {
            fail_enable: true,
            ..fake(false)
        };

        assert!(!toggle_autostart(&control, true));
    }

    #[test]
    fn shows_real_state_when_disable_fails() {
        let control = FakeControl {
            fail_disable: true,
            ..fake(true)
        };

        assert!(toggle_autostart(&control, false));
    }

    #[test]
    fn restores_previous_state_when_reading_fails() {
        // Windows ya invirtió la marca al hacer clic: la casilla muestra
        // `true` (tras el clic) y antes mostraba `false`.
        let control = FakeControl {
            fail_read: true,
            ..fake(false)
        };

        assert!(!toggle_autostart(&control, true));
        assert_eq!(control.changes.get(), 0);
    }

    #[test]
    fn restores_previous_state_when_reading_fails_and_it_was_enabled() {
        let control = FakeControl {
            fail_read: true,
            ..fake(true)
        };

        assert!(toggle_autostart(&control, false));
        assert_eq!(control.changes.get(), 0);
    }

    #[test]
    fn trusts_real_state_over_shown_one() {
        let control = fake(true);

        assert!(!toggle_autostart(&control, false));
        assert!(!control.enabled.get());
    }
}
