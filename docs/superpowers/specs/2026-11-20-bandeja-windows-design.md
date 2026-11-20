# Bandeja de Windows, menú de cuatro opciones e inicio con Windows (#97)

Parte de la épica #92. Tamaño: cambio pequeño; este diseño es breve y recoge lo acordado en la conversación.

## Intención

La app de `desktop/` deja de ser la plantilla de Tauri (ventana de 800 × 600 con un `greet`) y pasa a vivir solo en la bandeja: sin ventana al arrancar, una sola instancia, menú de cuatro opciones e "Iniciar con Windows" como casilla que se recuerda entre sesiones. Es la base sobre la que #98 (portapapeles), #100 (servidor), #101 (ventana del QR) y #107 (dispositivos emparejados) montan lo suyo.

## Metas

- Ícono en la bandeja con el menú de `docs/DESIGN.md` §8: **Emparejar celular**, **Dispositivos emparejados**, **Iniciar con Windows** (con casilla) y, tras un separador, **Salir**. Nada más.
- «Emparejar celular» y «Dispositivos emparejados» aparecen **deshabilitadas** hasta que #101 y #107 implementen sus ventanas.
- «Iniciar con Windows» y «Salir» funcionan. «Salir» termina el proceso.
- Sin ventana al arrancar. Abrir la app dos veces no crea dos íconos.
- Quitar todo lo de la plantilla en `desktop/` (`greet`, `opener`, `App.tsx` de ejemplo, logos, estilos y la ventana de `tauri.conf.json`).
- Conservar React y dejar `src/styles/tokens.css` con los tokens de `docs/DESIGN.md`.

## Fuera de alcance y antimetas

- Contenido de las ventanas (#101, #107), servidor (#100), portapapeles (#98), identidad con DPAPI.
- No se agregan ventanas, ni siquiera vacías, ni un almacén de ajustes.
- Las pantallas no se maquetan; `tokens.css` solo se prepara.
- Antimeta: que la casilla muestre un estado que no es el real del sistema.

## Restricciones

- `desktop/CLAUDE.md`: bun como gestor; la app vive en la bandeja con cuatro opciones y sin otras ventanas; nada de secretos por el IPC.
- `docs/DESIGN.md` §2 a §4, §5.2 y §8. La PC usa Cyber Lime fijo. Ningún color ni tamaño literal fuera de `tokens.css`; la CSP no permite orígenes externos.
- Verificación de cierre: `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all`, `cargo audit`, `cargo deny check` y `bun run tauri build`.

## Aproximaciones consideradas

1. **Plugins oficiales `tauri-plugin-single-instance` y `tauri-plugin-autostart` + `TrayIconBuilder` de Tauri (elegida).** Poco código propio, mantenido por el equipo de Tauri, ya aprobado por el usuario.
2. Implementar a mano el mutex de instancia única y la clave `Run` del registro con `windows-sys`. Más control, pero más `unsafe` y más superficie propia para algo que ya existe.
3. Guardar la casilla en un archivo de ajustes. Dos fuentes de verdad que pueden discrepar; descartada por el usuario.

## Diseño

### Dependencias (revisadas)

| Crate | Versión | Qué hace en Windows | Notas de la revisión |
| --- | --- | --- | --- |
| `tauri-plugin-single-instance` | 2.5.5 (estable más reciente) | `CreateMutexW` con nombre derivado del `identifier`; si ya existe, avisa a la primera instancia con `WM_COPYDATA` y la segunda termina con `exit(0)` | Sin red (el `tokio` con `net` que declara lo usa la rama de Linux). Debe registrarse **primero** en el `Builder`. Licencia MIT/Apache-2.0 |
| `tauri-plugin-autostart` | 2.7.0 (estable más reciente) | Usa `auto-launch` 0.6: escribe y borra el valor de la clave `Run` del usuario con la ruta de `current_exe()`; `disable` trata "no existe" como éxito | Licencia MIT/Apache-2.0 |

- Ambas piden `tauri ^2.12`; el `Cargo.lock` tiene 2.11.6. Subir Tauri a 2.12.x es parte de este cambio (`cargo update -p tauri`, dentro de la serie 2). Si `cargo deny` o `cargo audit` objetan algo nuevo, se resuelve antes de seguir.
- Solo se usa la API de Rust (`ManagerExt::autolaunch()`); no se agrega `@tauri-apps/plugin-autostart` al frontend ni permisos de capability.
- Se quita `tauri-plugin-opener` (Rust y JS), porque era de la plantilla y la app no abre enlaces.

### Rust (`desktop/src-tauri/src/`)

- `lib.rs`: arma el `Builder` (single-instance primero; después autostart con `app_name("Bóveda")`), llama a `tray::setup` en `setup` y, en `RunEvent::ExitRequested` con `code == None`, hace `api.prevent_exit()` para que la app sin ventanas no se cierre sola. «Salir» llama a `app.exit(0)`, que sí termina. El callback de single-instance no hace nada: la segunda instancia solo se cierra.
- `tray.rs` (como en `ARCHITECTURE.md`): 
  - Una descripción pura del menú, `menu_spec(autostart: bool) -> Vec<MenuItemSpec>`, con id, texto, habilitada y marca. Es lo que se prueba sin ventana.
  - `setup` construye `Menu` y `TrayIconBuilder` a partir de esa descripción (`CheckMenuItem` para la casilla). Ícono: el de la app (`default_window_icon`), como solución provisional.
  - `show_menu_on_left_click(true)`. **Temporal:** `DESIGN.md` §8 pide que el clic izquierdo abra «Dispositivos emparejados»; eso lo implementa #107.
  - Al abrir el menú no hace falta refrescar nada; la casilla se **inicializa** con `is_enabled()` y se **reconcilia** tras cada cambio.
- «Iniciar con Windows»: el evento intenta `enable()` o `disable()` según lo contrario del estado real, y después **siempre** lee `is_enabled()` y fija con `set_checked` ese valor. Si el cambio falla, la casilla queda reflejando el estado real (no el que el clic parecía pedir) y el error se registra sin datos sensibles. La lógica vive en una función que recibe un trait mínimo (`enable`, `disable`, `is_enabled`) para probarla con un doble que falla.
- Ningún secreto toca esta parte: no hay IPC ni comandos (`greet` se elimina; `invoke_handler` desaparece).

### Configuración de Tauri

- `tauri.conf.json`: `app.windows` queda vacío (sin ventana al arrancar); se conservan `identifier` y el resto. `build.beforeBuildCommand` y `frontendDist` siguen, porque `tauri build` necesita el frontend aunque no haya ventana.
- `capabilities/default.json`: referencia la ventana `main`, que ya no existe; se elimina (o se deja sin permisos) para que el compilado no apunte a una ventana inexistente. Las ventanas de #101 y #107 traerán sus capabilities.

### Frontend (`desktop/src/`)

- Se borran `App.css`, `assets/react.svg`, `public/tauri.svg`, `public/vite.svg` y el contenido de `App.tsx`.
- Quedan `main.tsx`, un `App.tsx` mínimo que no pinta nada y `styles/tokens.css`; `index.html` con `lang="es"`, título «Bóveda» y sin el ícono de Vite.
- `tokens.css` (§5.2: mismos nombres, en kebab-case): superficies y bordes (§2.1), texto (§2.2), Cyber Lime fijo (§2.3), estados (§2.4), familias de fuente (§3), espaciado (§4.1) y radios (§4.2). Los nombres salen de la tabla de colores (`bgBase` → `--bg-base`).
- Las fuentes **no se empaquetan todavía**: `tokens.css` declara las familias con alternativas del sistema y el empaquetado (`src/assets/fonts/`) llega con la primera ventana (#101). Es una decisión mía, anotada aquí para que no se pierda.

### Documentación

- `docs/diferencias-diseno.md`: se anotan las diferencias con `DESIGN.md` §8 que quedan abiertas: clic izquierdo que muestra el menú (temporal, #107 lo define), ícono de bandeja sin versión monocroma de 16 px y sin el punto de `accent`, y opciones deshabilitadas hasta #101 y #107.
- `ARCHITECTURE.md`: describe `desktop/` con lo generado (un `lib.rs` más `tray.rs`; `server.rs`, `clipboard.rs` e `identity.rs` siguen siendo futuros) y el ajuste de la casilla en el registro.
- `docs/roadmap.md`: se marca la tarea de la bandeja cuando esté hecha.

## Pruebas

- Unitarias en `tray.rs` (`cargo test --workspace`): el menú tiene exactamente cuatro opciones en el orden de §8 y un separador antes de «Salir»; las dos de ventanas están deshabilitadas; la marca de «Iniciar con Windows» sigue al estado recibido.
- Unitarias de la lógica de la casilla con un doble: activar cambia el estado; si `enable()` falla, la casilla queda en el valor real leído; si `is_enabled()` falla, no se inventa un estado (se conserva el anterior y se registra).
- A mano, con el compilado de `bun run tauri build` (el estado de la bandeja, la instancia única y el reinicio no se pueden probar sin sesión de Windows): arrancar sin ventana; abrir el `.exe` dos veces deja un solo ícono; activar la casilla, reiniciar sesión y comprobar que sigue marcada y que la app arrancó; desactivarla; «Salir» cierra el proceso. Se prueba el `.exe` compilado y no `tauri dev`, porque `autostart` registra la ruta de `current_exe()`.

## Qué queda al constructor

- La estructura exacta de `menu_spec` y del trait de la casilla.
- Si `capabilities/default.json` se borra o queda vacío, según lo que acepte el compilado.
- Los nombres definitivos de los tokens que no salgan literalmente de las tablas (espaciado y radios).
