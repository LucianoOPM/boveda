# Portapapeles de Windows: escritura excluida, aviso y borrado condicional (#98) — Plan de implementación

> **Para quien ejecute:** SUB-SKILL requerida: `superpowers:subagent-driven-development` (recomendada) o `superpowers:executing-plans`. Los pasos usan casillas (`- [ ]`).

**Meta:** un módulo `clipboard` en `desktop/` que escribe una contraseña en el portapapeles de Windows excluida del historial y de la nube, avisa con una notificación y la borra a los 30 s solo si el portapapeles todavía la contiene, con la lectura, la comparación y el vaciado bajo una sola apertura.

**Arquitectura:** un trait `ClipboardBackend` (escribir sensible; `clear_if_matches` atómico) separa la lógica de Windows. Un `ClipboardManager` lanza **un solo hilo trabajador** con un canal y `recv_timeout` sobre `Instant`; es el único que toca el backend. `WindowsBackend` escribe con `arboard` y borra con `clipboard-win` bajo una sola guarda. Las pruebas usan un `FakeBackend` y tiempos cortos.

**Tecnologías:** Rust 1.100 (edición 2024), Tauri 2.12, `arboard` 3.7.0 (`default-features = false`), `clipboard-win` 5.4.x (solo Windows), `tauri-plugin-notification` 2.8.1, `secrecy` y `zeroize` (las del lock).

**Diseño:** `docs/superpowers/specs/2026-11-20-portapapeles-windows-design.md`

## Restricciones globales

- Alcance: solo portapapeles, notificación y borrado condicional. Nada de #100 (red) ni #104 (flujo completo); nadie llama a `deliver` salvo las pruebas.
- Las contraseñas nunca cruzan al webview: sin IPC, comandos ni capabilities nuevos. Sin secretos en logs, errores, panics ni `Debug`.
- Hash: `std::collections::hash_map::RandomState` creada una vez por gestor; `DefaultHasher::new()` no sirve (sin clave aleatoria). Solo se guarda `u64`. Sin BLAKE2 ni cambios en `core/`.
- Plazos del diseño: 30 s de vida; vencimiento con `Unavailable` reintenta 3 veces cada 250 ms; salida con 3 intentos cada 100 ms y espera total de 2 s; como máximo 2 entradas pendientes (una vigente y una de limpieza de escritura dudosa). Nunca borrar a ciegas.
- Contraseña vacía o con `\0`: `ClipboardError::InvalidPassword` antes de tocar el portapapeles.
- `clipboard-win` solo con `[target.'cfg(windows)'.dependencies]`; sin más dependencias que las del diseño. `BSL-1.0` en `deny.toml`.
- `clipboard` y su API son `pub` para evitar código muerto; **no** se usa `#[allow(dead_code)]` para silenciar advertencias.
- Identificadores en inglés; comentarios, documentación y commits en español, en Conventional Commits.
- Correr cada comando por separado y mirar su código de salida; nunca encadenar `fmt`/`clippy`/`test` con `;`, `&&` o `| tail` antes de un commit.
- Las pruebas `#[ignore]` **sobrescriben y vacían el portapapeles real**: avisar al usuario antes de correrlas y usar `--test-threads=1`.
- Antes de cerrar: `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all`, `cargo audit` y `cargo deny check` pasan.

## Enfoque de revisión

1. Contraseña con `\0` o vacía: se rechaza y no cambia el portapapeles ni el temporizador anterior (Task 3).
2. Texto fuera del plano básico (p. ej. `🔐`, acentos) tras el viaje UTF-8 → UTF-16 → UTF-8 de Windows: la huella sigue coincidiendo y se borra (Task 7).
3. `deliver` después de `shutdown` o con el trabajador caído: error controlado, sin cuelgue ni pánico (Task 3 y 6).
4. Un envío nuevo mientras hay reintentos de borrado pendientes: se atiende de inmediato y la limpieza vieja no borra la contraseña nueva (Task 5).
5. El portapapeles ocupado en el cierre: el cierre no pasa de 2 s y no borra (Task 6).

## Estructura de archivos

- Modificar `Cargo.lock`, `desktop/src-tauri/Cargo.toml`, `deny.toml`: dependencias y licencia.
- Crear `desktop/src-tauri/src/clipboard.rs`: API pública (`ClipboardManager`, `ClipboardError`, `Config`, `deliver_and_notify`, `notify_ready`) y declaración de submódulos.
- Crear `desktop/src-tauri/src/clipboard/backend.rs`: trait `ClipboardBackend`, `ClearOutcome`, `WriteError`.
- Crear `desktop/src-tauri/src/clipboard/fingerprint.rs`: `Fingerprint` y `FingerprintKey`.
- Crear `desktop/src-tauri/src/clipboard/worker.rs`: `Pending`, `Command` y el bucle del trabajador.
- Crear `desktop/src-tauri/src/clipboard/windows.rs`: `WindowsBackend` (`cfg(windows)`).
- Crear `desktop/src-tauri/src/clipboard/fake.rs` (`cfg(test)`): `FakeBackend`.
- Crear `desktop/src-tauri/src/clipboard/tests.rs` (`cfg(test)`): pruebas del gestor.
- Crear `desktop/src-tauri/tests/clipboard_windows.rs`: pruebas reales `#[ignore]`.
- Modificar `desktop/src-tauri/src/lib.rs`: plugin de notificaciones, gestor como estado y cierre.
- Modificar `ARCHITECTURE.md`, `docs/especificacion.md`, `docs/roadmap.md`, `docs/diferencias-diseno.md`.

### Task 1: Dependencias y licencia

**Files:**
- Modify: `desktop/src-tauri/Cargo.toml`, `deny.toml`, `Cargo.lock`

**Interfaces:**
- Produces: las crates `arboard`, `clipboard_win`, `secrecy`, `zeroize` y `tauri_plugin_notification` disponibles para `desktop/`.

- [ ] **Step 1: Agregar al `Cargo.toml` de `desktop/src-tauri`:** `arboard = { version = "3.7.0", default-features = false }`, `secrecy = "0.10"`, `zeroize = "1.9"`, `tauri-plugin-notification = "2.8.1"`, y bajo `[target.'cfg(windows)'.dependencies]` `clipboard-win = "5.4"`. Los mismos requisitos de versión que usa `core/Cargo.toml` para `secrecy` y `zeroize`.
- [ ] **Step 2: Agregar `"BSL-1.0"` a `licenses.allow` de `deny.toml`** con un comentario que diga que es la licencia de `clipboard-win` (Boost, permisiva), aprobada en la #98.
- [ ] **Step 3: Resolver.** Guardar antes la lista de nombres de `Cargo.lock` (`grep '^name = ' Cargo.lock | sort > <scratchpad>/antes.txt`), correr `cargo build -p boveda-desktop` y guardar la lista de después. Esperado: compila; `arboard` y `clipboard-win` aparecen como nuevos (hoy no están). Confirmar con `cargo tree -p boveda-desktop -i clipboard-win` que hay **una sola versión** de `clipboard-win` y que `image` **no** aparece.
- [ ] **Step 4: Revisión de seguridad y licencias.** Correr por separado `cargo deny check` y `cargo audit`. Esperado: sin errores; si objetan una licencia o un aviso nuevo, detenerse y consultar al usuario (no ampliar `allow` por cuenta propia). Anotar la diferencia de crates (`diff antes.txt despues.txt`) para la PR.
- [ ] **Step 5: Commit.** `chore: dependencias del portapapeles de Windows y licencia BSL-1.0 (#98)` con `Cargo.toml`, `Cargo.lock` y `deny.toml`.

### Task 2: Trait del backend, huella y doble de pruebas

**Files:**
- Create: `desktop/src-tauri/src/clipboard.rs` (solo `mod` y reexportaciones), `clipboard/backend.rs`, `clipboard/fingerprint.rs`, `clipboard/fake.rs`, `clipboard/tests.rs`
- Modify: `desktop/src-tauri/src/lib.rs` (`pub mod clipboard;`)

**Interfaces:**
- Produces, en `backend.rs`:
  - `pub trait ClipboardBackend: Send + 'static { fn write_sensitive(&mut self, text: &str) -> Result<(), WriteError>; fn clear_if_matches(&mut self, matches: &dyn Fn(&[u8]) -> bool) -> ClearOutcome; }`. El contrato de `clear_if_matches` es que leer, comparar y vaciar son **una sola operación** (una apertura).
  - `pub enum ClearOutcome { Cleared, NotOurs, Unavailable }`
  - `pub enum WriteError { NotWritten, Uncertain }` (`Uncertain`: el texto pudo quedar en el portapapeles).
- Produces, en `fingerprint.rs`: `pub(super) struct FingerprintKey(RandomState)` con `new() -> Self` y `fingerprint(&self, bytes: &[u8]) -> Fingerprint`; `#[derive(Clone, Copy, PartialEq, Eq)] pub(super) struct Fingerprint(u64)` (sin `Debug`).
- Produces, en `fake.rs`: `#[derive(Clone, Default)] pub(super) struct FakeBackend` (manejador sobre `Arc<Mutex<…>>`) con `content()`, `set_content(FakeContent)`, `fail_next_write(WriteError, leaves_text: bool)`, `set_unavailable_for(n_calls: usize)`, `calls() -> Vec<FakeCall>`; `enum FakeContent { Empty, Text(String), Image }`; `enum FakeCall { Write, Clear(ClearOutcome) }`.

- [ ] **Step 1: Pruebas de la huella** en `tests.rs`: `same_bytes_same_fingerprint_within_a_key`, `different_bytes_different_fingerprint`, `different_keys_give_different_fingerprints` (dos `FingerprintKey::new()` con los mismos bytes; la probabilidad de coincidir es 2⁻⁶⁴).
- [ ] **Step 2: Correr** `cargo test -p boveda-desktop clipboard::` → falla por tipos inexistentes.
- [ ] **Step 3: Implementar** el trait, los enums, `FingerprintKey` (`RandomState` + `build_hasher()` + `write(bytes)` + `finish()`) y el `FakeBackend`. El doble de `clear_if_matches` evalúa `matches` solo si el contenido es `Text`, vacía solo si coincide y registra una única `FakeCall::Clear`; `Unavailable` mientras queden llamadas ocupadas. El de `write_sensitive` registra `FakeCall::Write`, reemplaza el contenido por `Text` y falla según `fail_next_write` (con o sin dejar el texto puesto).
- [ ] **Step 4: Correr** las pruebas → pasan.
- [ ] **Step 5: Commit.** `feat: trait del portapapeles, huella con clave aleatoria y doble de pruebas (#98)`.

### Task 3: Gestor y escritura (`deliver`)

**Files:**
- Modify: `clipboard.rs`; Create: `clipboard/worker.rs`; Test: `clipboard/tests.rs`

**Interfaces:**
- Consumes: Task 2.
- Produces, en `clipboard.rs`:
  - `pub struct Config { pub ttl: Duration, pub retry_interval: Duration, pub max_retries: u8, pub shutdown_interval: Duration, pub shutdown_attempts: u8 }` con `Default` = 30 s, 250 ms, 3, 100 ms, 3.
  - `#[derive(Debug, PartialEq, Eq)] pub enum ClipboardError { InvalidPassword, WriteFailed, Stopped }` con `Display` y `std::error::Error` a mano (sin datos del secreto).
  - `pub struct ClipboardManager` con `pub fn with_backend<B: ClipboardBackend>(backend: B, config: Config) -> Self` y `pub fn deliver(&self, password: &SecretString) -> Result<(), ClipboardError>`.
- Produces, en `worker.rs`: `pub(super) enum Command { Deliver { password: SecretString, reply: SyncSender<Result<(), ClipboardError>> }, Shutdown { deadline: Instant, reply: SyncSender<()> } }`, `struct Pending { fingerprint: Fingerprint, due: Instant, retries_left: u8 }` y `pub(super) fn run<B: ClipboardBackend>(backend: B, config: Config, key: FingerprintKey, rx: Receiver<Command>)`.

- [ ] **Step 1: Pruebas** (con `FakeBackend` y `Config` de tiempos cortos):
  - `deliver_writes_the_password_to_the_backend`: contenido `Text` igual a la contraseña y una `FakeCall::Write`.
  - `password_with_nul_is_rejected_before_touching_the_clipboard`: `Err(InvalidPassword)`, sin llamadas al backend y el contenido previo intacto; y la misma prueba con un temporizador ya armado, que sigue vigente.
  - `empty_password_is_rejected`.
  - `failed_write_reports_write_failed_and_keeps_the_previous_expiry` (se completa en Task 4 con el vencimiento).
  - `deliver_after_the_worker_stopped_returns_stopped`: trabajador caído (se suelta el receptor) → `Err(Stopped)` sin colgarse.
- [ ] **Step 2: Correr** → fallan.
- [ ] **Step 3: Implementar** `deliver`: valida (vacía o con `\0` → `InvalidPassword`) en el hilo que llama; copia a un `SecretString` propio (una asignación exacta), lo envía con un canal de respuesta de capacidad 1 y espera la respuesta; si el envío o la respuesta fallan, `Stopped`. En `worker::run`, `Deliver`: calcular la huella de los bytes, `write_sensitive`; `Ok` → la lista pasa a una sola entrada `Pending { due: now + ttl, retries_left: max_retries }` y responde `Ok`; `NotWritten` → responde `WriteFailed` sin tocar la lista. (`Uncertain` se completa en Task 5.) El bucle usa `recv_timeout` hasta el vencimiento más próximo (o un valor largo si no hay entradas) y termina si el canal se cierra.
- [ ] **Step 4: Correr** → pasan.
- [ ] **Step 5: Commit.** `feat: gestor del portapapeles con trabajador único y escritura sensible (#98)`.

### Task 4: Vencimiento y borrado condicional

**Files:**
- Modify: `clipboard/worker.rs`; Test: `clipboard/tests.rs`

**Interfaces:**
- Consumes: Task 3 (`run`, `Pending`).

- [ ] **Step 1: Pruebas** (`ttl` ≈ 150 ms; esperar con un ayudante `wait_until(|| cond, Duration::from_secs(3))`):
  - `clears_the_clipboard_when_it_still_holds_the_password` → `Empty` tras el vencimiento.
  - `does_not_clear_different_text`, `does_not_clear_when_the_clipboard_is_empty`, `does_not_clear_an_image` → el contenido queda igual y la entrada termina.
  - `content_changed_by_another_app_before_expiry_is_kept`.
  - `same_password_copied_by_hand_is_cleared_at_the_original_expiry` (comportamiento aprobado: se borra al vencer el plazo del envío original, no más tarde).
  - `second_delivery_replaces_the_timer`: dos envíos seguidos; tras el vencimiento del primero, el portapapeles sigue con el segundo; se borra al vencer el segundo.
  - `failed_write_keeps_the_original_expiry`: el primer envío se borra a su hora aunque el segundo haya fallado con `NotWritten`.
  - `unavailable_retries_at_most_three_times_then_gives_up_without_clearing`: ocupado siempre → exactamente 3 reintentos tras el primer intento, contenido intacto, ningún `Cleared`.
  - `unavailable_then_available_clears`: ocupado 1 vez → se borra en el reintento.
  - `reads_compares_and_clears_in_a_single_call`: las `calls()` del fake solo contienen `Write` y `Clear(_)`; nunca una lectura y un vaciado separados.
- [ ] **Step 2: Correr** → fallan.
- [ ] **Step 3: Implementar** el procesamiento de entradas vencidas en el bucle: `clear_if_matches(&|bytes| fingerprint == key.fingerprint(bytes))` por cada entrada vencida; `Cleared` y `NotOurs` la quitan; `Unavailable` con `retries_left > 0` la reprograma (`due = now + retry_interval`, un reintento menos) y con 0 la descarta. Registrar con `eprintln!` solo el hecho (sin contenido ni huella).
- [ ] **Step 4: Correr** → pasan.
- [ ] **Step 5: Commit.** `feat: borrado condicional a los 30 s con reintentos acotados (#98)`.

### Task 5: Escritura de resultado dudoso

**Files:**
- Modify: `clipboard/worker.rs`; Test: `clipboard/tests.rs`

- [ ] **Step 1: Pruebas:**
  - `uncertain_write_that_left_the_text_is_cleared_immediately`: `fail_next_write(Uncertain, leaves_text = true)` → el contenido queda vacío al responder, `deliver` devuelve `Err(WriteFailed)` y no queda ninguna entrada nueva.
  - `uncertain_write_with_unavailable_clipboard_keeps_a_cleanup_entry`: ocupado → `Err(WriteFailed)`, y cuando el portapapeles vuelve y aún tiene el texto, se borra en un reintento; con el contenido cambiado por otra app, no se borra.
  - `uncertain_write_keeps_the_previous_expiry`: la contraseña anterior conserva su vencimiento original y se borra si sigue ahí.
  - `old_cleanup_never_removes_a_newer_password`: tras un envío dudoso pendiente, un envío correcto nuevo reemplaza la lista y la limpieza vieja ya no existe, así que la contraseña nueva sobrevive hasta su propio vencimiento.
  - `cleanup_entries_are_bounded`: varios dudosos seguidos con el portapapeles ocupado nunca dejan más de 2 entradas (comprobable porque el fake no recibe más de 2 llamadas por ronda de vencimiento) ni reintentan para siempre.
- [ ] **Step 2: Correr** → fallan.
- [ ] **Step 3: Implementar** la rama `Uncertain` de `Deliver`: un intento inmediato de `clear_if_matches` con la huella nueva; `Cleared` o `NotOurs` → nada más; `Unavailable` → agrega `Pending { due: now + retry_interval, retries_left: max_retries - 1 }` como **entrada de limpieza**, que reemplaza a la entrada de limpieza anterior si la hay (como máximo una de limpieza y una vigente). Responde siempre `Err(WriteFailed)`.
- [ ] **Step 4: Correr** → pasan.
- [ ] **Step 5: Commit.** `feat: limpieza inmediata y acotada tras una escritura de resultado dudoso (#98)`.

### Task 6: Cierre (`shutdown`)

**Files:**
- Modify: `clipboard.rs`, `clipboard/worker.rs`; Test: `clipboard/tests.rs`

**Interfaces:**
- Produces: `pub fn shutdown(&self, timeout: Duration)` en `ClipboardManager`. Tras él, `deliver` devuelve `Err(Stopped)`; llamarlo dos veces no cuelga ni entra en pánico.

- [ ] **Step 1: Pruebas:**
  - `shutdown_clears_the_pending_password`: con una entrada vigente y contenido coincidente, `shutdown(2 s)` deja el portapapeles vacío.
  - `shutdown_does_not_clear_other_content`.
  - `shutdown_with_unavailable_clipboard_returns_within_the_timeout_and_does_not_clear`: ocupado siempre, `timeout` de 300 ms, `shutdown_attempts` 3 → vuelve en menos de `timeout + holgura` y sin borrar.
  - `shutdown_twice_and_deliver_after_shutdown_are_safe`.
  - `shutdown_without_pending_entries_returns_quickly`.
- [ ] **Step 2: Correr** → fallan.
- [ ] **Step 3: Implementar** el comando `Shutdown { deadline, reply }`: por cada entrada hasta `shutdown_attempts` intentos separados `shutdown_interval` mientras el resultado sea `Unavailable` y no pase `deadline`; responde por `reply` y termina el bucle. `shutdown` envía el comando, espera con `recv_timeout(timeout)` y, tanto si responde como si vence, no se queda esperando al hilo.
- [ ] **Step 4: Correr** → pasan.
- [ ] **Step 5: Commit.** `feat: limpieza acotada del portapapeles al salir (#98)`.

### Task 7: Backend real de Windows

**Files:**
- Create: `clipboard/windows.rs`, `desktop/src-tauri/tests/clipboard_windows.rs`
- Modify: `clipboard.rs` (`#[cfg(windows)] pub use windows::WindowsBackend;` y `ClipboardManager::new()` con `WindowsBackend` y `Config::default()`)

**Interfaces:**
- Produces: `pub struct WindowsBackend` con `pub fn new() -> Self` que implementa `ClipboardBackend`.
- Reglas: `write_sensitive` usa `arboard::Clipboard::new()?.set().exclude_from_history().exclude_from_cloud().text(text)` (con `arboard::SetExtWindows`); error `ClipboardOccupied` → `WriteError::NotWritten`, cualquier otro (incluido `Unknown`, que también cubre un fallo al agregar las exclusiones con el texto ya puesto) → `Uncertain`. `clear_if_matches` abre **una sola** guarda `clipboard_win::Clipboard::new_attempts(5)` (error → `Unavailable`), comprueba `raw::is_format_avail(formats::CF_UNICODETEXT)` (si no → `NotOurs`), lee con `raw::get_string` a un `Zeroizing<Vec<u8>>` con `Vec::new()` (un solo reservado exacto; error de lectura → `Unavailable`), llama a `matches(&bytes)` y solo si es verdadero `raw::empty()` (error → `Unavailable`; éxito → `Cleared`); si no, `NotOurs`. La guarda es una única variable local, así que se libera en `Drop` en todos los caminos.

- [ ] **Step 1: Pruebas reales** en `tests/clipboard_windows.rs`, todas `#[ignore = "usa el portapapeles real"]` y con contraseñas ficticias («prueba-no-real-…»). **Avisar al usuario antes de correrlas.**
  - `write_sensitive_puts_the_text_and_the_exclusion_formats`: tras escribir, abrir con `clipboard_win::Clipboard::new_attempts` y comprobar el texto y que `register_format("CanIncludeInClipboardHistory")` y `register_format("CanUploadToCloudClipboard")` están disponibles (`is_format_avail`).
  - `clear_if_matches_clears_matching_text_even_with_exclusion_formats_present`.
  - `clear_if_matches_leaves_different_text_untouched`.
  - `clear_if_matches_leaves_a_non_text_clipboard_untouched` (poner un formato propio registrado con `clipboard_win::raw::set`, sin texto).
  - `non_bmp_and_accented_text_roundtrips_and_is_cleared` (`"Bóveda-🔐-prueba-no-real"` escrita y borrada con una `FingerprintKey` real; usar `ClipboardManager::with_backend(WindowsBackend::new(), Config { ttl: 300 ms, .. })`).
  - `clipboard_stays_open_while_matching`: dentro del cierre `matches`, un hilo intenta `clipboard_win::Clipboard::new()` (sin reintentos) y debe fallar; después de que `clear_if_matches` vuelve, otra apertura sí funciona. Evidencia de que la lectura y la comparación ocurren bajo la misma apertura y de que se libera.
  - `occupied_clipboard_is_unavailable_and_nothing_is_cleared`: la prueba mantiene el portapapeles abierto; `clear_if_matches` devuelve `Unavailable`; al soltarlo, una apertura posterior funciona (la guarda no quedó colgada).
  - `manual_demo_writes_waits_and_clears` (documenta en su comentario los pasos manuales de Win+V): escribe una contraseña ficticia por el gestor, imprime cuándo mirar Win+V, espera 35 s y comprueba que el portapapeles quedó vacío.
- [ ] **Step 2: Compilar y ver fallar** (`WindowsBackend` no existe): `cargo test -p boveda-desktop --test clipboard_windows --no-run`.
- [ ] **Step 3: Implementar** `WindowsBackend` según las reglas de arriba y `ClipboardManager::new()`.
- [ ] **Step 4: Correr** `cargo test -p boveda-desktop --test clipboard_windows -- --ignored --test-threads=1 --nocapture` (con permiso del usuario). Esperado: todo pasa. Confirmar con una mutación: reemplazar `Clipboard::new_attempts(5)` por dos aperturas separadas (una para leer y otra para vaciar) y comprobar que `clipboard_stays_open_while_matching` o la prueba de ocupado fallan; restaurar con `git restore` solo si el archivo no tiene otros cambios sin confirmar.
- [ ] **Step 5: Verificar que las pruebas normales no las ejecutan** (`cargo test -p boveda-desktop` las muestra como `ignored`). Commit: `feat: backend de Windows con escritura excluida y borrado bajo una sola apertura (#98)`.

### Task 8: Notificación y cableado en la app

**Files:**
- Modify: `clipboard.rs`, `desktop/src-tauri/src/lib.rs`; Test: `clipboard/tests.rs`

**Interfaces:**
- Produces: `pub fn deliver_and_notify<E: std::fmt::Display>(&self, password: &SecretString, notify: impl FnOnce() -> Result<(), E>) -> Result<(), ClipboardError>` en `ClipboardManager` y `pub fn notify_ready<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> tauri_plugin_notification::Result<()>` (título «Bóveda», cuerpo «Contraseña lista para pegar», sin datos de la contraseña).

- [ ] **Step 1: Pruebas:**
  - `notification_runs_after_a_successful_write`.
  - `notification_does_not_run_when_the_write_fails_or_the_password_is_invalid`.
  - `failed_notification_keeps_the_timer_and_the_clear`: `notify` devuelve `Err`; `deliver_and_notify` devuelve `Ok(())` y la contraseña se borra a su hora.
- [ ] **Step 2: Correr** → fallan.
- [ ] **Step 3: Implementar** `deliver_and_notify` (`deliver` y, solo si fue `Ok`, `notify`; un error del aviso se registra con `eprintln!` sin datos sensibles) y `notify_ready` con `NotificationExt`. En `lib.rs`: `.plugin(tauri_plugin_notification::init())`; en `setup`, `app.manage(ClipboardManager::new())`; y en el cierre de la app (`RunEvent::Exit`, comprobando en la documentación que se emite una sola vez al terminar; si no, `ExitRequested { code: Some(_) }`) `app.state::<ClipboardManager>().shutdown(Duration::from_secs(2))`.
- [ ] **Step 4: Correr** las pruebas y `cargo clippy --workspace --all-targets -- -D warnings`. Esperado: sin advertencias de código muerto y **sin** `#[allow(dead_code)]`; si aparecen, corregir con el código o consultar al usuario.
- [ ] **Step 5: Commit.** `feat: notificación al copiar y limpieza al salir de la app (#98)`.

### Task 9: Documentación y cierre

**Files:**
- Modify: `ARCHITECTURE.md`, `docs/especificacion.md`, `docs/roadmap.md`, `docs/diferencias-diseno.md`

- [ ] **Step 1: `ARCHITECTURE.md`:** reemplazar `clipboard.rs` del árbol por la carpeta `clipboard/` con sus archivos; actualizar «desktop/ (Windows, Tauri 2)» (gestor, trabajador, backend, notificación, borrado bajo una apertura) y agregar, en la fila de borrado de memoria o como fila nueva de «Decisiones clave», **solo lo verificado**: `arboard` 3.7.0 convierte a UTF-16 dentro del bloque global que entrega a Windows (esa copia no se puede borrar y queda fuera de la garantía); en el fallo de `SetClipboardData` el bloque se libera sin borrarse; la lectura va a un `Zeroizing<Vec<u8>>`; `deliver` copia la contraseña una vez a un `SecretString` de vida corta. Fila nueva: «Borrado del portapapeles bajo una sola apertura».
- [ ] **Step 2: `docs/especificacion.md`:** párrafo «Decidido el 2026-11-20 (#98)» con las reglas del diseño (huella con `RandomState`, rechazo de contraseñas con NUL o vacías, misma contraseña copiada a mano se borra al vencer el envío, reintentos y cierre acotados, nunca se borra a ciegas).
- [ ] **Step 3: `docs/roadmap.md`:** marcar el portapapeles como hecho **como módulo** (#98) y dejar claro que el flujo completo sigue en #100 y #104; actualizar la fecha.
- [ ] **Step 4: `docs/diferencias-diseno.md`:** nota sobre el nombre del toast en desarrollo (PowerShell) frente a la app instalada.
- [ ] **Step 5: Verificación final**, cada una por separado: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo audit`, `cargo deny check`. Listar los crates nuevos de `Cargo.lock` (Task 1) para la PR.
- [ ] **Step 6: Commit.** `docs: portapapeles de Windows en la arquitectura, la especificación y el roadmap (#98)`.
- [ ] **Step 7: Pruebas manuales (las corre el usuario con el compilado de `bun run tauri build --no-bundle`, preguntar antes de compilar, o con `manual_demo_writes_waits_and_clears`):** (1) Win+V no muestra la contraseña; (2) con la sincronización del portapapeles activa, no aparece en otro equipo; (3) a los 30 s el portapapeles queda vacío; (4) si antes se copia otra cosa, no se toca; (5) el toast «Contraseña lista para pegar» se ve. Documentar en la PR cuáles se corrieron y cuáles no.

## Addenda (2026-11-20, tras la sonda entre procesos)

- **Cambio:** el borrado condicional abre con una ventana propietaria (`clipboard/message_window.rs`, `windows-sys`) en vez de `NULL`; ver «Hallazgo» en el diseño. La Task 7 del plan describía `new_attempts(5)`: queda sustituido por `new_attempts_for(hwnd, 5)`.
- **Pruebas reales:** el usuario cambió la validación a manual; las `#[ignore]` no se ejecutan más (Task 7, paso 4, y la mutación, quedan sin hacer).
- **Disparador de depuración** `dev_demo.rs`: se añadió para el recorrido manual y se retiró antes de cerrar la PR (revertido).
