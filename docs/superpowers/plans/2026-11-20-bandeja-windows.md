# Bandeja de Windows, menú e inicio con Windows (#97) — Plan de implementación

> **Para quien ejecute:** SUB-SKILL requerida: `superpowers:subagent-driven-development` (recomendada) o `superpowers:executing-plans`. Los pasos usan casillas (`- [ ]`).

**Meta:** que la app de `desktop/` viva solo en la bandeja, con el menú de cuatro opciones, instancia única e «Iniciar con Windows» recordado entre sesiones.

**Arquitectura:** `lib.rs` arma el `Builder` (single-instance primero, autostart después) y evita que el proceso termine al no haber ventanas. `tray.rs` separa una descripción pura del menú (`menu_spec`, probable sin ventana) de su construcción con `TrayIconBuilder`, y una lógica de la casilla sobre un trait mínimo que usa `is_enabled()` como única fuente de verdad. El frontend queda como esqueleto React vacío con `tokens.css`.

**Tecnologías:** Tauri 2.12+, `tauri-plugin-single-instance` 2.5.5, `tauri-plugin-autostart` 2.7.0, React 19, Vite, bun.

**Diseño:** `docs/superpowers/specs/2026-11-20-bandeja-windows-design.md`

## Restricciones globales

- Bun como gestor (`bun install`, `bun run tauri build`); no npm, yarn ni pnpm ni sus lockfiles.
- Solo las cuatro opciones de menú; ninguna ventana nueva; sin IPC ni comandos de Tauri; ningún secreto cruza al webview.
- «Emparejar celular» y «Dispositivos emparejados» deshabilitadas; «Iniciar con Windows» y «Salir» funcionan.
- La casilla se inicializa y reconcilia con `autolaunch().is_enabled()`; sin archivo de ajustes.
- Ningún color ni tamaño literal fuera de `src/styles/tokens.css`; Cyber Lime fijo; la CSP no permite orígenes externos.
- Identificadores en inglés; comentarios, documentación y commits en español, en Conventional Commits.
- No agregar más dependencias que los dos plugins aprobados; quitar `tauri-plugin-opener`.
- La versión de Rust no se toca (`rust-toolchain.toml`).
- Antes de terminar: `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all`, `cargo audit`, `cargo deny check` y `bun run tauri build` pasan.

## Enfoque de revisión

Entradas o condiciones que el diseño implica y que un usuario puede encontrar; cada una tiene su prueba en la tarea indicada:

1. `enable()` falla (p. ej., ruta inexistente): la casilla muestra el estado real, no el pedido (Tarea 3).
2. `is_enabled()` falla al leer: no se inventa estado; se conserva el último conocido (Tarea 3).
3. Estado inicial «ya activado» al arrancar: la marca del menú nace activada (Tareas 2 y 3).
4. El usuario hace clic izquierdo en el ícono: se ve el menú y no se abre ninguna ventana (Tarea 5, a mano).
5. Segunda instancia con el proceso aún arrancando: un solo ícono (Tarea 5, a mano).

## Estructura de archivos

- Modificar `desktop/src-tauri/Cargo.toml`: dependencias; quitar `opener`; feature `tray-icon`.
- Modificar `desktop/src-tauri/src/lib.rs`: `Builder`, plugins, `prevent_exit`, sin `greet`.
- Crear `desktop/src-tauri/src/tray.rs`: `menu_spec`, `setup` y eventos del menú.
- Crear `desktop/src-tauri/src/autostart.rs`: trait `AutostartControl` y `toggle_autostart` (lógica pura con pruebas).
- Modificar `desktop/src-tauri/tauri.conf.json`; eliminar `desktop/src-tauri/capabilities/default.json`.
- Modificar `desktop/package.json`, `desktop/bun.lock`, `desktop/index.html`, `desktop/src/main.tsx`, `desktop/src/App.tsx`; crear `desktop/src/styles/tokens.css`; eliminar los archivos de la plantilla.
- Modificar `docs/diferencias-diseno.md`, `ARCHITECTURE.md`, `docs/roadmap.md`, `desktop/README.md` si es de la plantilla.

---

### Task 1: Dependencias y esqueleto sin ventana

**Archivos:**
- Modificar: `desktop/src-tauri/Cargo.toml`, `desktop/src-tauri/src/lib.rs`, `desktop/src-tauri/tauri.conf.json`
- Eliminar: `desktop/src-tauri/capabilities/default.json`

**Interfaces:**
- Produce: `pub fn run()` en `boveda_desktop_lib` que arranca la app sin ventanas y con los dos plugins; el módulo `tray` se declara en la Tarea 2.

- [ ] **Paso 1: Agregar dependencias.** En `desktop/src-tauri/Cargo.toml`: `tauri = { version = "2", features = ["tray-icon"] }`, `tauri-plugin-single-instance = "2.5.5"`, `tauri-plugin-autostart = "2.7.0"`; quitar `tauri-plugin-opener`. Conservar `serde` y `serde_json` solo si `lib.rs` los sigue usando; si no, quitarlos.
- [ ] **Paso 2: Subir Tauri.** Ejecuta `cargo update -p tauri` (los plugins piden `tauri ^2.12`; el lock tiene 2.11.6). Esperado: `Cargo.lock` con `tauri` 2.12.x y los dos plugins nuevos; sin cambios en `core` ni `mobile/rust`.
- [ ] **Paso 3: Revisar la cadena de dependencias.** Ejecuta `cargo deny check` y `cargo audit` desde la raíz. Esperado: sin errores. Si falla por una licencia o un aviso nuevos, detente y repórtalo antes de seguir.
- [ ] **Paso 4: Reescribir `lib.rs`.** `run()` con `tauri::Builder::default()`: `.plugin(tauri_plugin_single_instance::init(|_app, _argv, _cwd| {}))` **primero**, luego `.plugin(tauri_plugin_autostart::Builder::new().app_name("Bóveda").build())`; construir con `.build(tauri::generate_context!())` y `.run(|_app, event| ...)` que, en `RunEvent::ExitRequested { code: None, api, .. }`, llama a `api.prevent_exit()`. Eliminar `greet`, `invoke_handler` y `opener`. Comentarios en español que expliquen por qué el callback queda vacío y por qué se previene la salida.
- [ ] **Paso 5: Quitar la ventana y la capability.** En `tauri.conf.json`: `"windows": []` y conservar `security.csp` y el resto. Eliminar `capabilities/default.json`.
- [ ] **Paso 6: Verificar que compila.** Ejecuta `cargo clippy -p boveda-desktop --all-targets -- -D warnings`. Esperado: sin avisos. (`tauri::generate_context!` necesita el frontend compilado: si pide `../dist`, ejecuta antes `bun install && bun run build` desde `desktop/`.)
- [ ] **Paso 7: Commit.** `git add desktop Cargo.lock && git commit -m "feat: base de la app de bandeja sin ventana ni plantilla de Rust (#97)"`

---

### Task 2: Descripción pura del menú y bandeja

**Archivos:**
- Crear: `desktop/src-tauri/src/tray.rs`
- Modificar: `desktop/src-tauri/src/lib.rs` (`mod tray;` y llamada a `tray::setup` en `.setup`)

**Interfaces:**
- Produce (para la Tarea 3):
  - `pub(crate) const ID_PAIR: &str = "pair"; ID_DEVICES = "devices"; ID_AUTOSTART = "autostart"; ID_QUIT = "quit";`
  - `pub(crate) struct MenuItemSpec { pub id: &'static str, pub label: &'static str, pub enabled: bool, pub checked: Option<bool> }`
  - `pub(crate) enum MenuEntry { Item(MenuItemSpec), Separator }`
  - `pub(crate) fn menu_spec(autostart_enabled: bool) -> Vec<MenuEntry>`
  - `pub fn setup(app: &tauri::App) -> tauri::Result<()>`

- [ ] **Paso 1: Pruebas que fallan** (`#[cfg(test)] mod tests` en `tray.rs`):
  - `menu_has_exactly_four_items_and_one_separator`: `menu_spec(false)` tiene 5 entradas en este orden: `ID_PAIR`, `ID_DEVICES`, `ID_AUTOSTART`, `Separator`, `ID_QUIT`; las cuatro etiquetas son «Emparejar celular», «Dispositivos emparejados», «Iniciar con Windows» y «Salir».
  - `window_items_are_disabled_until_their_windows_exist`: `ID_PAIR` y `ID_DEVICES` con `enabled == false`; `ID_AUTOSTART` y `ID_QUIT` con `enabled == true`.
  - `autostart_check_follows_given_state`: `checked == Some(true)` con `menu_spec(true)` y `Some(false)` con `menu_spec(false)`; las otras tres opciones tienen `checked == None`.
- [ ] **Paso 2: Verificar que fallan.** `cargo test -p boveda-desktop tray` → no compila o falla por `menu_spec` inexistente.
- [ ] **Paso 3: Implementar `menu_spec`** con los tipos de Interfaces, sin dependencias de Tauri.
- [ ] **Paso 4: Verificar que pasan.** `cargo test -p boveda-desktop tray` → 3 pruebas OK.
- [ ] **Paso 5: Implementar `setup`.** Construye el `Menu` de Tauri recorriendo `menu_spec(estado)` (`CheckMenuItem::with_id` si `checked` es `Some`, `MenuItem::with_id` si no, `PredefinedMenuItem::separator`), con `estado = app.autolaunch().is_enabled().unwrap_or(false)` (`use tauri_plugin_autostart::ManagerExt`). `TrayIconBuilder::with_id("main")` con `.icon(app.default_window_icon().cloned().expect(...))`, `.tooltip("Bóveda")`, `.menu(&menu)` y `.show_menu_on_left_click(true)`. Comentario en español: el clic izquierdo que muestra el menú es **temporal**; el comportamiento de `DESIGN.md` §8 lo implementa #107. Guarda el `CheckMenuItem` de la casilla en el estado de la app (`app.manage(...)`) para poder reconciliarlo en la Tarea 3. Registrar `on_menu_event` solo para `ID_QUIT` → `app.exit(0)`; las otras dos opciones no hacen nada.
- [ ] **Paso 6: Lint y commit.** `cargo clippy -p boveda-desktop --all-targets -- -D warnings`; `git commit -m "feat: ícono de bandeja y menú de cuatro opciones (#97)"`

---

### Task 3: «Iniciar con Windows» con el estado real

**Archivos:**
- Crear: `desktop/src-tauri/src/autostart.rs`
- Modificar: `desktop/src-tauri/src/tray.rs`, `desktop/src-tauri/src/lib.rs` (`mod autostart;`)

**Interfaces:**
- Consume: `ID_AUTOSTART` y el `CheckMenuItem` guardado en la Tarea 2.
- Produce:
  - `pub(crate) trait AutostartControl { fn enable(&self) -> Result<(), String>; fn disable(&self) -> Result<(), String>; fn is_enabled(&self) -> Result<bool, String>; }`
  - `pub(crate) fn toggle_autostart(control: &dyn AutostartControl, shown: bool) -> bool` — devuelve el estado que la casilla debe mostrar.
  - `impl AutostartControl for tauri_plugin_autostart::AutoLaunchManager` (delegando; el error se pasa a `String` con `to_string()`).

- [ ] **Paso 1: Pruebas que fallan** (con un doble `FakeControl` en el módulo de pruebas, con campos `enabled: Cell<bool>`, `fail_enable`, `fail_disable`, `fail_read`):
  - `enables_when_disabled`: estado real `false` → `toggle_autostart(&fake, false)` devuelve `true` y el doble quedó activado.
  - `disables_when_enabled`: estado real `true` → devuelve `false` y quedó desactivado.
  - `shows_real_state_when_enable_fails`: `fail_enable`, estado real `false` → devuelve `false`.
  - `shows_real_state_when_disable_fails`: `fail_disable`, estado real `true` → devuelve `true`.
  - `keeps_shown_state_when_reading_fails`: `fail_read`, `shown = true` → devuelve `true` sin intentar cambiar nada (el doble cuenta las llamadas a `enable`/`disable` y deben ser 0).
  - `trusts_real_state_over_shown_one`: estado real `true`, `shown = false` (casilla desfasada) → intenta `disable()` y devuelve `false`.
- [ ] **Paso 2: Verificar que fallan.** `cargo test -p boveda-desktop autostart` → fallan o no compilan.
- [ ] **Paso 3: Implementar `toggle_autostart`.** Lee `is_enabled()`; si falla, devuelve `shown` y registra con `eprintln!` un texto fijo más el error del plugin (nada sensible). Si lee `real`, intenta `disable()` si `real` es `true` y `enable()` si es `false`, registrando el error si falla; al final vuelve a leer `is_enabled()` y devuelve ese valor, o `real` si la lectura falla.
- [ ] **Paso 4: Verificar que pasan.** `cargo test -p boveda-desktop autostart` → 6 pruebas OK.
- [ ] **Paso 5: Conectarlo al menú.** En el `on_menu_event` de `tray.rs`, para `ID_AUTOSTART`: llama a `toggle_autostart(&*app.autolaunch(), item.is_checked()?)` (el menú de Windows ya cambió la marca al hacer clic; por eso se **reconcilia siempre**) y fija el resultado con `item.set_checked(resultado)`.
- [ ] **Paso 6: Lint y commit.** `cargo clippy -p boveda-desktop --all-targets -- -D warnings`; `git commit -m "feat: casilla de iniciar con Windows con el estado real del sistema (#97)"`

---

### Task 4: Frontend sin plantilla y `tokens.css`

**Archivos:**
- Modificar: `desktop/package.json`, `desktop/bun.lock`, `desktop/index.html`, `desktop/src/main.tsx`, `desktop/src/App.tsx`
- Crear: `desktop/src/styles/tokens.css`
- Eliminar: `desktop/src/App.css`, `desktop/src/assets/react.svg`, `desktop/public/tauri.svg`, `desktop/public/vite.svg`, `desktop/src/vite-env.d.ts` solo si deja de hacer falta

**Interfaces:**
- Produce: variables CSS en `:root` para las futuras ventanas (#101, #107).

- [ ] **Paso 1: Quitar `@tauri-apps/plugin-opener`.** `bun remove @tauri-apps/plugin-opener` desde `desktop/`; verifica que `bun.lock` es el único lockfile.
- [ ] **Paso 2: Vaciar la plantilla.** `App.tsx` exporta un componente que devuelve `null`; `main.tsx` monta `App` e importa `./styles/tokens.css`; `index.html` con `lang="es"`, `<title>Bóveda</title>` y sin el `<link rel="icon">` de Vite; borrar los archivos listados.
- [ ] **Paso 3: Escribir `tokens.css`** según `docs/DESIGN.md`, con los mismos nombres de la tabla en kebab-case y un comentario de cabecera que cite la sección: superficies y bordes §2.1 (`--bg-canvas`, `--bg-base`, `--surface-1`, `--surface-2`, `--surface-elevated`, `--icon-box`, `--border-subtle`, `--border-card`, `--border-interactive`, `--border-pressed`, `--outline`, `--lock-scrim`, `--capsule-bg`, `--capsule-border`); texto §2.2; acento Cyber Lime fijo §2.3 (`--accent: #B5FF00`, `--accent-dim`, `--accent-border`, `--on-accent`, `--accent-glow` al 28 %); estados y sintaxis §2.4; familias §3 (`--font-ui`: Plus Jakarta Sans y `--font-mono`: JetBrains Mono, con alternativas del sistema, sin `@font-face` ni CDN); espaciado §4.1 (`--space-4` … `--space-48`) y radios §4.2. `body` usa `--bg-base`, `--text-primary` y `--font-ui`; respeta `prefers-reduced-motion` y `forced-colors` solo si hay algo que adaptar (si no, no se agregan).
- [ ] **Paso 4: Verificar.** `bun run build` desde `desktop/` → sin errores de `tsc` ni de Vite; `grep -rn "#[0-9A-Fa-f]\{6\}" desktop/src --include="*.tsx" --include="*.css"` solo encuentra `tokens.css`.
- [ ] **Paso 5: Commit.** `git commit -m "refactor: quitar la plantilla del frontend y preparar tokens.css (#97)"`

---

### Task 5: Documentación y verificación final

**Archivos:**
- Modificar: `docs/diferencias-diseno.md`, `ARCHITECTURE.md`, `docs/roadmap.md`; `desktop/README.md` si es de la plantilla (reemplazarlo por dos líneas o borrarlo)

- [ ] **Paso 1: `docs/diferencias-diseno.md`.** Lee el formato del archivo y agrega, en una sección de la app de Windows: (a) el clic izquierdo muestra el menú; es temporal y #107 implementa lo de §8; (b) el ícono de bandeja es el de la app, sin versión monocroma de 16 px ni el punto de `accent` al haber una contraseña (llega con #98); (c) «Emparejar celular» y «Dispositivos emparejados» están deshabilitadas hasta #101 y #107; (d) las fuentes aún no se empaquetan.
- [ ] **Paso 2: `ARCHITECTURE.md`.** En el mapa de `desktop/` y en su sección: `lib.rs` más `tray.rs` y `autostart.rs`; `server.rs`, `clipboard.rs` e `identity.rs` siguen siendo futuros; `src/` queda como esqueleto con `styles/tokens.css`; el estado de «Iniciar con Windows» se lee del registro (`Run`) vía `tauri-plugin-autostart`, sin archivo de ajustes; agregar los dos plugins y su versión.
- [ ] **Paso 3: `docs/roadmap.md`.** Marca como hecho solo lo de la bandeja y el inicio con Windows de la línea «App Tauri en la bandeja…»; la ventana del QR sigue pendiente (aclararlo en la línea).
- [ ] **Paso 4: Verificación automática.** Desde la raíz: `cargo fmt --all`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo audit`, `cargo deny check`; desde `desktop/`: `bun run tauri build`. Esperado: todo pasa. Anota la salida.
- [ ] **Paso 5: Verificación a mano** con el `.exe` de `desktop/src-tauri/target/release/` (o `target/release/` de la raíz, según el workspace), no con `tauri dev` (el plugin registra la ruta de `current_exe()`). Avisa al usuario antes de probar. Comprobar: (1) arranca sin ventana y con un ícono; (2) abrir el `.exe` por segunda vez no crea otro ícono; (3) el menú tiene las cuatro opciones, las dos primeras deshabilitadas; (4) clic izquierdo y derecho muestran el menú; (5) activar «Iniciar con Windows», cerrar sesión y volver: sigue marcada y la app arrancó; (6) desactivarla: la marca se quita y `reg query HKCU\Software\Microsoft\Windows\CurrentVersion\Run` ya no lista «Bóveda»; (7) «Salir» cierra el proceso (`tasklist`). Deja la casilla desactivada al terminar.
- [ ] **Paso 6: Commit.** `git commit -m "docs: bandeja de Windows en la arquitectura, el roadmap y las diferencias de diseño (#97)"`

---

## Autorrevisión

- **Cobertura del diseño:** menú y bandeja (T2), sin ventana, instancia única y plugins (T1), casilla con estado real (T3), plantilla y tokens (T4), documentación y verificaciones (T5), subida de Tauri a 2.12 (T1).
- **Consistencia de tipos:** `ID_AUTOSTART`, `MenuEntry`, `menu_spec`, `AutostartControl` y `toggle_autostart` se definen una vez y las tareas posteriores usan los mismos nombres.
- **Pendientes del constructor:** nombres exactos de los tokens de espaciado y radios; si `vite-env.d.ts` sigue haciendo falta; dónde queda el `.exe` según el workspace.
