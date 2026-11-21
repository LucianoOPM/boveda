# Ventana del QR y confirmación al emparejar (#101) — Plan de implementación

> **Para quien ejecute:** SUB-SKILL REQUERIDA: `superpowers:executing-plans` (en esta sesión, como decidió el usuario). Los pasos usan casillas `- [ ]`. Cada tarea termina con pruebas y lint en verde y un commit pequeño.

**Objetivo:** «Emparejar celular» abre una ventana fija con el QR de la PC; cuando un celular completa el apretón de manos, la PC pide confirmación y solo entonces guarda al celular en la lista (#99).

**Arquitectura:** un `PairingCore` sin hilos ni reloj propio (la lógica de sesión, plazos, carreras y persistencia, probable con `now`) y un `PairingController` que lo corre con un hilo de revisión. El servidor de #100 enruta cada conexión por sus 2 primeros bytes (`peek`: 48 → emparejamiento, 96 → envío) y entrega las de emparejamiento a un `PairingHandler`. Rust dibuja el QR en SVG y el webview solo recibe la imagen y datos de estado.

**Tecnologías:** Rust (Tauri 2.12, `vault-core`, `windows-sys` 0.61, `qrcode` 0.14 con `default-features = false` y solo `std` y `svg`), React 19 + TypeScript con `bun` (sin paquetes nuevos).

**Diseño:** `docs/superpowers/specs/2026-11-20-ventana-qr-design.md` (léelo antes de empezar; este plan solo fija lo que el diseño deja a quien construye).

## Restricciones globales

- Código y nombres en inglés; comentarios, documentación y mensajes de commit en español. Commits Conventional Commits (`feat:`, `test:`, `docs:`, `chore:`), pequeños.
- Antes de cada commit: `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` y `cargo fmt --all`, **cada uno por separado y comprobando su código de salida** (nunca encadenar con `|` ni `;`).
- Nada de secretos en logs, errores, panics ni `Debug`: ni el código de un solo uso, ni la imagen del QR, ni llaves. Los tipos que los contienen no derivan `Debug` ni `Display`.
- Dependencias: solo `qrcode` 0.14 (`default-features = false`, features `std` y `svg`). Ninguna otra de Rust ni de `bun`; si algo parece necesitarla, se pregunta. `cargo audit` y `cargo deny check` deben pasar.
- Toda la criptografía vive en `vault-core`: aquí solo se llama a `serve_pairing`, `PendingConnection::{confirm, reject}`, `PairingSession`, `PairingCode::generate` y `PairingOffer`.
- Valores fijos: código de 2 minutos (`PairingSession`, 120 s con `Instant`); confirmación de 60 s (`PairingTimeouts::confirmation`); ventana del QR 360 × 480 y confirmación 360 × 200, fija; QR de 240 × 240 con zona de silencio de 4 módulos, módulos `#0F1720` sobre blanco; puerto preferido 47615; máximo 4 conexiones simultáneas; primera trama de 48 bytes (emparejamiento) o 96 (envío), con encabezado de largo u16 big-endian.
- Estilos solo con variables de `desktop/src/styles/tokens.css`. Español neutro con tuteo; el botón dice la acción.
- No se lanzan ejecutables de la app ni pruebas `#[ignore]` sobre el equipo del usuario sin que lo autorice; lo manual se declara pendiente del usuario.
- Alcance: no se amplía salvo lo necesario para enrutar con #100.

## Review Focus

Condiciones que el diseño implica y que ninguna tarea cubriría por sí sola; cada una tiene su prueba en la tarea indicada:

1. Una conexión con el código correcto llega justo al caducar: gana el plazo de Rust, no hay diálogo ni guardado (Tarea 6).
2. Una segunda conexión con el código correcto mientras hay una pendiente: no sustituye ni autoriza (Tarea 6).
3. Un salto grande de tiempo (suspensión de la PC) entre dos revisiones: la sesión caduca de una vez y no se acepta nada (Tarea 5).
4. La ventana se cierra y se vuelve a abrir, o el webview se recarga, durante la espera: `snapshot` devuelve el estado vigente y no crea otra sesión (Tareas 5 y 8).
5. La lista de celulares llena (64) o el candado envenenado al guardar: `Ended` con el motivo y nada se guarda (Tarea 6).

## Mapa de archivos

Rust (`desktop/src-tauri/src/`):

| Archivo | Responsabilidad |
| --- | --- |
| `qr.rs` (+ `qr/tests.rs`) | texto del QR a partir de la `PairingOffer`, SVG, `data:` URI, marcador fijo |
| `network.rs` (+ `network/nlm.rs`) | `Candidate`, `NetworkProbe::private_candidates`, elegibilidad (hardware, activo, Ethernet o Wi-Fi) |
| `pairing/address.rs` | `choose_address`: función pura |
| `server/route.rs` | clasificar la primera trama con `peek` |
| `server.rs`, `server/generation.rs`, `server/supervisor.rs` | `PairingHandler`, id de generación, `NetworkSnapshot` y `NetworkView` |
| `pairing.rs`, `pairing/view.rs`, `pairing/core.rs`, `pairing/controller.rs` | `PairingView`, `PairingCore` y `PairingController` (+ `PairingHandler`) |
| `pairing/window.rs` | ventana, órdenes de Tauri y notificador real |
| `tray.rs`, `lib.rs` | habilitar «Emparejar celular»; cableado y cierre |

Frontend (`desktop/src/`): `pairing/PairingWindow.tsx`, `pairing/types.ts`, `pairing/format.ts` (+ `format.test.ts`, que corre con `bun test`), `pairing/pairing.css`, `assets/fonts/` (copiadas de `mobile/assets/fonts/` con sus licencias OFL); `App.tsx` elige la vista por `?view=pairing`.

Documentación: `docs/especificacion.md`, `ARCHITECTURE.md`, `docs/diferencias-diseno.md`, `desktop/CLAUDE.md`, `desktop/README.md`.

---

### Task 1: QR — dependencia, medición y contrato del formato

**Files:**
- Modify: `desktop/src-tauri/Cargo.toml` (`qrcode = { version = "0.14", default-features = false, features = ["std", "svg"] }`, con comentario en español)
- Create: `desktop/src-tauri/src/qr.rs`, `desktop/src-tauri/src/qr/tests.rs`; `pub mod qr;` en `lib.rs`
- Modify: `docs/especificacion.md` (contrato del QR, tras medir)

**Interfaces:**
- Produces: `pub enum QrError { Encode }` (sin datos); `pub fn offer_text(offer: &PairingOffer) -> Result<Zeroizing<String>, QrError>` (texto del QR, ya en la codificación elegida); `pub fn render_svg(text: &str) -> Result<Zeroizing<String>, QrError>` (240 × 240, zona de silencio 4, módulos `#0F1720` sobre `#FFFFFF`, nivel de corrección elegido); `pub fn data_uri(svg: &str) -> String` (`data:image/svg+xml;…`, escapado propio sin dependencias); `pub const PLACEHOLDER_SVG: &str` (cuadrícula fija sin datos, para la caducidad).

- [ ] **Step 1: Agregar `qrcode` y comprobar el árbol**

Agrega la dependencia. Ejecuta `cargo tree -p boveda-desktop -i qrcode` (debe aparecer sin `image`) y `cargo deny check` y `cargo audit`. Si alguno falla por la crate, detente y pregunta.

- [ ] **Step 2: Prueba de medición (primero rota)**

En `qr/tests.rs`, `#[test] #[ignore] fn measure_offer_encodings()`: construye una `PairingOffer` real con IPv4 (`192.168.100.200:47615`) y con IPv6 y, para cada variante de texto (bytes crudos, base64url sin relleno, base32 en mayúsculas) y cada nivel de corrección (L, M, Q), imprime versión, módulos, px por módulo = 240 / (módulos + 8) y bytes del SVG. Corre `cargo test -p boveda-desktop qr::tests::measure -- --ignored --nocapture` (es una medición pura, sin red ni portapapeles) y pega la tabla en el diseño.

- [ ] **Step 3: Decidir con los criterios del diseño**

Aplica los criterios fijados en el diseño (nivel ≥ M; px por módulo lo más altos posible y nunca < 4; texto que todo lector entregue sin pérdida; base64url por defecto). Escribe la decisión y la tabla en `docs/especificacion.md` («Formato del QR», contrato con #103: codificación exacta, nivel, ejemplo de cadena con una oferta de vectores fijos).

- [ ] **Step 4: Pruebas del contrato (fallan primero)**

`offer_text_is_the_documented_encoding` (vector fijo: oferta con llave `[7; 32]`, `10.0.0.5:47615`, código `[9; 32]` → cadena exacta documentada), `offer_text_round_trips_through_decode` (decodifica el texto con el decodificador escrito aquí en la prueba y compara con `PairingOffer::decode`), `svg_uses_the_design_colors_and_size` (contiene `#0F1720`, `#FFFFFF`, `width="240"`, `height="240"` y ningún otro color), `qr_meets_the_scan_criteria` (con IPv4 y con IPv6: px por módulo ≥ 4 y nivel ≥ M), `data_uri_has_no_raw_markup_characters` (sin `<`, `>`, `#` ni `"` sin escapar), `placeholder_contains_no_offer_data` (no depende de entrada alguna y es una constante).

- [ ] **Step 5: Implementar** `offer_text`, `render_svg` (`qrcode::QrCode::with_error_correction_level` + `render::svg::Color`, `quiet_zone(true)` y tamaño mínimo de 240 px; si la crate no fija la zona en 4 módulos, documentarlo), `data_uri` y la codificación elegida (si es base64url, escribe el codificador a mano con los vectores de RFC 4648 en sus pruebas; no se agrega `base64`).

- [ ] **Step 6: Verificar y commit**

`cargo test -p boveda-desktop`, clippy y fmt en verde. Commit: `feat: generar el QR de la oferta de emparejamiento como SVG (#101)`.

---

### Task 2: Candidatos de red y elección de la dirección

**Files:**
- Modify: `desktop/src-tauri/src/network.rs`, `desktop/src-tauri/src/network/nlm.rs`
- Create: `desktop/src-tauri/src/pairing.rs` (solo `mod address;` por ahora), `desktop/src-tauri/src/pairing/address.rs` y su `tests` en el mismo archivo `#[cfg(test)]`
- Test: `desktop/src-tauri/tests/network_windows.rs` (una prueba `#[ignore]` de solo lectura)

**Interfaces:**
- Produces: `pub struct Candidate { pub ip: Ipv4Addr, pub eligible: bool, pub has_gateway: bool, pub metric: u32 }` (`Clone`, `Copy`, `PartialEq`, `Eq`, `Debug`: no es secreto); `NetworkProbe::private_candidates(&mut self) -> Result<Vec<Candidate>, ProbeError>` (requerido) y `private_addresses` pasa a ser un método con implementación por defecto: `normalize` de las `ip` de los candidatos; `FakeProbe::set_candidates(Result<Vec<Candidate>, ProbeError>)` y `FakeProbe::set` (el actual) convierte cada IP en un candidato elegible con puerta de enlace y métrica 0; `pub fn choose_address(candidates: &[Candidate], listening: &BTreeSet<Ipv4Addr>) -> Option<Ipv4Addr>`.

- [ ] **Step 1: Pruebas de `choose_address` (fallan primero)**

Nombres y aserciones: `prefers_gateway_over_lower_metric` (A sin puerta de enlace y métrica 1, B con puerta de enlace y métrica 50 → B); `lowest_metric_among_gateways`; `ties_break_on_lowest_ip`; `without_gateway_uses_lowest_metric_among_eligible`; `ineligible_is_never_chosen_even_with_lowest_metric` (virtual con métrica 1 → se elige la física con 100); `only_ineligible_gives_none`; `address_not_listening_is_ignored` (el candidato no está en `listening` → no cuenta); `empty_gives_none`.

- [ ] **Step 2: Implementar** `choose_address` como función pura (filtra por `eligible` y `listening`; orden: con puerta de enlace, menor métrica, menor IP).

- [ ] **Step 3: Pruebas de `FakeProbe` y de `private_addresses` por defecto (fallan primero)**, luego cambia el trait y adapta `FakeProbe`. Las pruebas existentes del supervisor deben seguir en verde sin tocarlas.

- [ ] **Step 4: `NlmProbe::private_candidates`**

Amplía `read_adapters` para devolver por adaptador: `IfType` (solo `IF_TYPE_ETHERNET_CSMACD` y `IF_TYPE_IEEE80211`), `OperStatus == IfOperStatusUp`, `FirstGatewayAddress` no nula, `Ipv4Metric` y, para el hardware, `GetIfEntry2` con el `InterfaceLuid` del adaptador y el bit `HardwareInterface` de `InterfaceAndOperStatusFlags`. **Antes de escribirlo,** lee la definición de `MIB_IF_ROW2` en el código fuente de `windows-sys` 0.61 (carpeta del registro de Cargo) y confirma el nombre del campo y el bit; si `HardwareInterface` no distingue Hyper-V/WSL en este equipo, documenta qué señal se usa en su lugar y avisa al usuario. Una `GetIfEntry2` que falla deja el adaptador no elegible (falla cerrando). Mantén el `unsafe` aquí y con comentario `SAFETY`.

- [ ] **Step 5: Prueba real de solo lectura** (`#[ignore]`, en `tests/network_windows.rs`): imprime los candidatos del equipo y comprueba que ninguno elegible es loopback ni link-local. Se ejecuta solo con la autorización del usuario; si no, queda declarada como pendiente.

- [ ] **Step 6: Verificar y commit**: `feat: elegir la dirección del QR entre interfaces físicas activas (#101)`.

---

### Task 3: Instantánea del servidor

**Files:**
- Modify: `desktop/src-tauri/src/server.rs`, `server/supervisor.rs`, `server/generation.rs`, sus `tests`

**Interfaces:**
- Consumes: `Candidate` (Tarea 2).
- Produces: `pub struct NetworkSnapshot { pub generation: u64, pub port: u16, pub candidates: Vec<Candidate> }` (`Clone`, `PartialEq`, `Debug`); `pub trait NetworkView: Send + Sync + 'static { fn snapshot(&self) -> Option<NetworkSnapshot>; }`; `ServerSupervisor: NetworkView`; `Generation::start(config, ips, deps, id: u64)` y `Generation::id()`.

- [ ] **Step 1: Pruebas (fallan primero)** en `supervisor/tests.rs`: `snapshot_is_none_without_private_network`; `snapshot_reports_generation_port_and_candidates_restricted_to_listening_ips`; `generation_id_changes_when_the_set_changes`; `generation_id_is_unchanged_across_ticks_with_the_same_set`; `rebuilt_listener_gets_a_new_id` (el listener cae y se reconstruye con otro id).
- [ ] **Step 2: Implementar.** El id es `started` (ya cuenta las generaciones). `Supervisor::snapshot()` toma los candidatos de la última consulta del detector (guárdalos en `tick`) filtrados a `current`; `ServerSupervisor` lo publica en un `Arc<Mutex<Option<NetworkSnapshot>>>` igual que ya hace con el puerto.
- [ ] **Step 3: Verificar y commit**: `feat: publicar la instantánea de red del servidor (#101)`.

---

### Task 4: El servidor enruta emparejamiento y envío por la primera trama

Toca código de #100: hasta ahora `run_connection` llamaba siempre a `receive_send`, que rechaza todo lo que no empiece con la trama de 96 bytes.

**Files:**
- Create: `desktop/src-tauri/src/server/route.rs` (con sus pruebas en `route/tests.rs`)
- Modify: `server.rs` (`PairingHandler` y `Deps.pairing`), `server/generation.rs` (`run_connection`), `server/test_support.rs`, `desktop/src-tauri/src/lib.rs` (un manejador provisional que cierra todo, hasta la Tarea 7)

**Interfaces:**
- Produces: `pub trait PairingHandler: Send + Sync + 'static { fn serve(&self, stream: TcpStream, generation: u64); }`; `Deps.pairing: Arc<dyn PairingHandler>`; `pub(crate) enum Route { Pairing, Send, Reject }`; `pub(crate) fn classify(stream: &TcpStream, cancel: &CancelToken, step: Duration) -> Route`. En `test_support.rs`: `RecordingPairing` (guarda las conexiones recibidas y su generación) y `Fixture::deps()` lo incluye.

- [ ] **Step 1: Pruebas de `classify` con sockets reales en `127.0.0.1` (fallan primero):** `length_48_routes_to_pairing`, `length_96_routes_to_send`, `other_length_is_rejected` (0, 1, 47, 49, 95, 97, 512, 0xFFFF), `peek_does_not_consume_the_header` (tras `classify`, `read_frame_header` del núcleo lee el mismo largo), `one_byte_then_silence_times_out_as_reject`, `one_byte_then_close_is_reject`, `immediate_close_is_reject`, `cancel_during_wait_is_reject`.
- [ ] **Step 2: Implementar `classify`** con `TcpStream::peek` en lecturas de 250 ms hasta tener 2 bytes o vencer `step` (usa `set_read_timeout`; `peek` puede devolver 1 byte: vuelve a intentar sin consumir). `Reject` cierra sin leer más ni escribir.
- [ ] **Step 3: Pruebas de la generación (fallan primero)** en `generation/tests.rs`: `send_still_works_after_routing` (las pruebas existentes de envío siguen en verde sin cambios), `pairing_frame_goes_to_the_handler_with_the_generation_id`, `garbage_frame_is_closed_without_touching_either_path`, `pairing_connection_uses_one_of_the_four_slots`, `no_pairing_handler_call_for_wrong_length_first_frame`.
- [ ] **Step 4: Implementar** el enrutado en `run_connection` (`classify` primero; después `receive_send` o `handler.serve(stream, generation_id)`); `Shared` lleva el id. El manejador provisional de `lib.rs` descarta la conexión.
- [ ] **Step 5: Verificar y commit**: `feat: enrutar emparejamiento y envío por la primera trama (#101)`.

---

### Task 5: `PairingCore` — sesiones, plazos y estados

**Files:**
- Create: `desktop/src-tauri/src/pairing/view.rs`, `pairing/core.rs`, `pairing/tests.rs`; completa `pairing.rs`

**Interfaces:**
- Consumes: `NetworkSnapshot`, `choose_address` (Tareas 2–3), `qr::{offer_text, render_svg, data_uri, PLACEHOLDER_SVG}` (Tarea 1), `vault_core::protocol::{KeyPair, PairingCode, PairingOffer, PairingSession, CancelToken}`.
- Produces (`pairing.rs`): `pub struct SessionId(u64)` (`Copy`, `Eq`, interno); `pub trait IdentitySource: Send + Sync + 'static { fn load(&self) -> Result<KeyPair, IdentityUnavailable>; }` con implementación para `IdentityStore` (`Ok(None)` también es error) y `pub struct IdentityUnavailable;`; `pub enum EndReason { Expired, Rejected, ConfirmationTimeout, NetworkChanged, PhoneDisconnected, SaveFailed, IdentityUnavailable }`; `pub enum PairingView` (serde, `tag = "phase"`, camelCase): `Closed`, `NoNetwork`, `Waiting { qr: String, remaining_ms: u64 }`, `Confirming { name: String, fingerprint: String, remaining_ms: u64 }`, `Saving { name: String }`, `Done { name: String }`, `Ended { reason: EndReason, placeholder: String }`; `impl Debug for PairingView` que **solo** imprime la fase.
- Produces (`core.rs`): `pub(crate) struct PairingCore` con `new(config: PairingConfig, identity: Arc<dyn IdentitySource>, paired: Arc<Mutex<PairedDeviceStore>>)`, `open(&mut self, net: Option<&NetworkSnapshot>, now: Instant) -> bool`, `regenerate(&mut self, net: Option<&NetworkSnapshot>, now: Instant)`, `cancel(&mut self)`, `tick(&mut self, net: Option<&NetworkSnapshot>, now: Instant)`, `view(&self, now: Instant) -> PairingView`, `changed(&mut self) -> bool` (¿cambió la vista desde la última lectura?); `PairingConfig { session_ttl: Duration, confirmation: Duration }` con `Default` = 120 s y 60 s.

- [ ] **Step 1: Pruebas (fallan primero), con un `IdentitySource` y un `NetworkSnapshot` de prueba:** `open_without_network_shows_no_network_and_no_qr`; `open_with_network_creates_one_session_with_a_qr`; `open_twice_keeps_the_same_session` (mismo `qr`, mismo `SessionId`); `qr_encodes_the_chosen_address_and_port` (decodifica el texto con la prueba de la Tarea 1 y compara dirección, puerto y llave pública de la PC); `remaining_counts_from_creation_with_the_given_now`; `expiry_ends_the_session_and_drops_the_image` (tras `tick` pasada la caducidad: `Ended(Expired)` con `placeholder`, y ninguna cadena de la vista ni del `Debug` contiene `data:image`, y el código ya no es utilizable: `PairingSession` con `Expired`/`Cancelled`); `big_time_jump_expires_at_once` (un `tick` 10 minutos después caduca sin pasar por otro estado); `regenerate_creates_a_new_session_and_never_reuses_the_old_one` (nuevo `SessionId`, nuevo `qr`, y el `CancelToken` anterior queda cancelado); `cancel_closes_and_drops_the_image`; `identity_unreadable_ends_with_identity_unavailable_and_no_session`; `network_generation_change_ends_with_network_changed`; `network_port_or_address_change_ends_with_network_changed`; `network_gone_ends_with_network_changed_or_no_network_on_reopen`; `view_debug_never_prints_the_image_or_names` (para cada fase).
- [ ] **Step 2: Implementar** `PairingCore`: una sola sesión viva a la vez (`Option<Session>` con `id`, `Arc<Mutex<PairingSession>>`, `CancelToken`, `offer_generation`, `address`, `expires_at`, `svg: Zeroizing<String>`); `SessionId` creciente; el `KeyPair` se lee con `identity.load()` por sesión. Al terminar una sesión (caducar, cancelar, terminar), llama `PairingSession::cancel`, cancela el `CancelToken` y vacía el SVG.
- [ ] **Step 3: Verificar y commit**: `feat: lógica de sesión del emparejamiento con plazos en Rust (#101)`.

---

### Task 6: `PairingCore` — conexión, confirmación, guardado y carreras

**Files:**
- Modify: `pairing/core.rs`, `pairing/tests.rs`

**Interfaces:**
- Produces: `pub(crate) struct Ticket { pub id: SessionId, pub session: Arc<Mutex<PairingSession>>, pub cancel: CancelToken }`; `pub enum Decision { Accept, Reject, Cancel }`; `core.begin_connection(&mut self, generation: u64, now: Instant) -> Option<Ticket>`; `core.offer_pending(&mut self, id: SessionId, name: &DeviceName, fingerprint: String, deadline: Instant, decide: Sender<Decision>, now: Instant) -> bool`; `core.accept(&mut self, now: Instant)`, `core.reject(&mut self)`; `core.connection_failed(&mut self, id: SessionId, error: PairingError)`; `core.confirmed(&mut self, id: SessionId, result: Result<PairedDevice, PairingError>, now: Instant)`. `accept`/`reject`/`cancel` actúan solo sobre la sesión vigente.

- [ ] **Step 1: Pruebas (fallan primero):**
  - Conexión y confirmación: `connection_for_the_current_generation_gets_a_ticket`; `connection_from_another_generation_gets_none`; `no_ticket_without_a_session_or_after_expiry`; `pending_shows_name_and_fingerprint_with_the_remaining_time`; `accept_sends_the_decision_and_moves_to_saving`; `accept_after_the_deadline_is_ignored_even_if_the_view_still_says_confirming` (Rust decide con `now`); `reject_sends_reject_and_ends_with_rejected`; `confirmation_timeout_ends_the_session_and_sends_cancel`.
  - Guardado: `saves_only_after_confirmed_ok` (la lista no cambia en `Confirming`, `Saving` ni en ningún error); `confirmed_ok_saves_the_device_with_paired_at_in_ms` (nombre y llave del celular); `already_paired_is_success_and_keeps_the_existing_name_without_duplicates`; `save_failure_ends_with_save_failed_and_leaves_the_list_unchanged` (lista llena de 64; candado envenenado); `confirm_error_saves_nothing_and_ends_phone_disconnected_or_timeout`.
  - Carreras (todas con una conexión simulada con canal): `connection_arrives_at_the_expiry_instant_no_dialog_no_save` (`offer_pending` con `now >= expires_at` → `false`, sin diálogo); `second_connection_with_the_right_code_while_one_is_pending_does_not_replace_it` (la `PairingSession` ya está `Consumed`: `accept_first` da `AlreadyUsed` y la vista no cambia); `cancel_during_confirmation_sends_cancel_and_saves_nothing`; `cancel_while_saving_is_ignored_and_the_save_completes` (el usuario ya aceptó); `network_change_during_waiting_and_during_confirmation_cancels_and_saves_nothing`; `regenerate_with_a_pending_connection_of_the_old_session_rejects_it` (la decisión `Cancel` llega al hilo viejo); `late_confirmed_of_an_old_session_never_saves_or_changes_the_new_session` (`confirmed(old_id, Ok(..))` tras regenerar: lista intacta, vista de la nueva sin cambios); `late_offer_pending_of_an_old_session_returns_false`; `accept_command_arriving_after_regenerate_acts_on_the_new_session_and_is_ignored_there` (la nueva sesión está en `Waiting`, no pasa nada); `commands_without_a_session_are_noops`.
- [ ] **Step 2: Implementar** con los `SessionId` como única llave de correlación: cualquier llamada con un id que no sea el vigente se descarta (y el llamador cierra o rechaza su conexión). `accept` solo cuenta si la fase es `Confirming` y `now < deadline`. Cancelar con la fase `Saving` no hace nada (diseño: el usuario ya aceptó, el cierre del protocolo es corto y acotado). `AlreadyPaired` es éxito. El `paired_at` sale de `SystemTime` en ms Unix en un solo punto, inyectable para pruebas.
- [ ] **Step 3: Mutaciones** (confirma que cada una hace fallar su prueba y vuelve a dejar el código): quitar la comparación de `now < deadline` en `accept`; guardar en `offer_pending` en vez de en `confirmed`; ignorar el `SessionId` en `confirmed`.
- [ ] **Step 4: Verificar y commit**: `feat: confirmación, guardado y carreras del emparejamiento (#101)`.

---

### Task 7: `PairingController` y el manejador de conexiones, de punta a punta

**Files:**
- Create: `pairing/controller.rs`, `desktop/src-tauri/tests/pairing_flow.rs` (integración por `127.0.0.1`, compilada solo en Windows)
- Modify: `pairing.rs`, `lib.rs` (usa el controlador en lugar del manejador provisional)

**Interfaces:**
- Consumes: `PairingCore`, `NetworkView`, `PairingHandler` (Tareas 3–6).
- Produces: `pub trait Notifier: Send + Sync + 'static { fn changed(&self, view: &PairingView); }`; `pub struct PairingController` con `start(core: PairingCore, net: Arc<dyn NetworkView>, notifier: Arc<dyn Notifier>, timeouts: PairingTimeouts) -> Self`, `open(&self)`, `regenerate(&self)`, `cancel(&self)`, `accept(&self)`, `reject(&self)`, `snapshot(&self) -> PairingView`, `shutdown(&self)`; `impl PairingHandler for PairingController`. Un hilo revisa cada 250 ms (`tick`) y notifica cuando `changed()`; también notifica tras cada orden.

- [ ] **Step 1: Pruebas de punta a punta (fallan primero)**, con el servidor real (`Generation`), el controlador, un `Notifier` de prueba y `pair_with_pc` como celular:
  - `right_code_and_accept_pairs_and_saves` (el celular obtiene `PairedPc`; la lista tiene al celular con su nombre; la vista pasa por `Confirming`, `Saving`, `Done`);
  - `phone_without_the_code_cannot_pair_and_nothing_is_saved` (código de otra oferta: el celular ve `Closed`; la lista vacía);
  - `reject_does_not_save_and_phone_sees_rejected`;
  - `confirmation_timeout_does_not_save` (plazos cortos);
  - `closing_the_session_during_confirmation_does_not_save`;
  - `reusing_the_code_after_a_pairing_is_refused`;
  - `regenerate_invalidates_the_previous_code_for_a_phone_that_arrives_late`;
  - `send_still_works_on_the_same_port_while_a_session_is_open`;
  - `expiry_while_waiting_closes_connections_without_data`.
- [ ] **Step 2: Implementar `serve`** (corre en el hilo del servidor): `begin_connection` → `serve_pairing` con el `CancelToken` de la sesión y `timeouts` → `offer_pending` (con un canal de decisión); según la decisión, `confirm()` / `reject()`; para `Cancel`, el plazo o un canal cerrado, `reject()` si se puede; `confirmed`/`connection_failed` con el `SessionId`. El candado del núcleo nunca se mantiene durante entrada o salida de red. Todo error se registra con `eprintln!` usando solo el `Display` de `PairingError` (sin datos).
- [ ] **Step 3: Verificar** (`cargo test -p boveda-desktop --test pairing_flow`) y commit: `feat: controlador del emparejamiento y manejador de conexiones (#101)`.

---

### Task 8: Ventana, órdenes de Tauri y bandeja

**Files:**
- Create: `pairing/window.rs`
- Modify: `tray.rs`, `lib.rs`, `desktop/CLAUDE.md`

**Interfaces:**
- Consumes: `PairingController`, `PairingView`.
- Produces: `pub fn open_window(app: &AppHandle)` (la crea o la enfoca), `TauriNotifier` (emite `pairing://state` solo a la ventana `pairing`, la redimensiona a 360 × 480 o 360 × 200, la enfoca en `Confirming`, la cierra en `Closed`), órdenes `pairing_snapshot`, `pairing_cancel`, `pairing_regenerate`, `pairing_accept`, `pairing_reject` (sin parámetros). `CloseRequested` de la ventana llama a `controller.cancel()`.

- [ ] **Step 1: Verificar con Context7 y el código de Tauri 2.12** que existen `WebviewWindowBuilder::{inner_size, resizable, center, content_protected}`, `Emitter::emit_to` y el evento de cierre; si `content_protected` existe, actívalo y escribe en la especificación **solo lo comprobado**; investiga si WebView2 guarda la imagen en su perfil (`%LOCALAPPDATA%\com.jesus.boveda\EBWebView`) y documenta el resultado como límite si no se puede confirmar. No se promete ninguna protección no comprobada.
- [ ] **Step 2: Pruebas (fallan primero):** en `tray.rs`, actualizar `window_items_are_disabled_until_their_windows_exist` para que «Emparejar celular» esté habilitada y «Dispositivos emparejados» no; en `pairing/window.rs`, `size_for_phase` (QR 360 × 480; `Confirming`, `Saving` y `Done` 360 × 200; `Ended` y `NoNetwork` 360 × 480) como función pura.
- [ ] **Step 3: Implementar** la ventana (`WebviewWindowBuilder` con etiqueta `pairing`, `index.html?view=pairing`, sin redimensionar ni maximizar, centrada), las órdenes y el cableado en `lib.rs`: un solo `PairingController` como estado, creado donde ya se abren `identity` y la lista; si faltan, el controlador existe y la ventana explica el error (`IdentityUnavailable` o lista ilegible). `ID_PAIR` abre la ventana. En `RunEvent::Exit`, `controller.shutdown()` antes de parar el servidor.
- [ ] **Step 4: Verificar** que `cargo build -p boveda-desktop` y las pruebas pasan; commit: `feat: ventana de emparejamiento y opción de la bandeja (#101)`.

---

### Task 9: Interfaz de la ventana

**Files:**
- Create: `desktop/src/pairing/{PairingWindow.tsx,types.ts,format.ts,format.test.ts,pairing.css}`, `desktop/src/assets/fonts/*` (copiar de `mobile/assets/fonts/`: `PlusJakartaSans-{Regular,Bold,ExtraBold}.ttf`, `JetBrainsMono-{Medium,Bold,ExtraBold}.ttf`, `OFL-PlusJakartaSans.txt`, `OFL-JetBrainsMono.txt`; no se copia `lucide.ttf`)
- Modify: `desktop/src/App.tsx`, `desktop/src/styles/tokens.css` (solo `@font-face` y, si falta, tokens nuevos del diseño), `desktop/tsconfig.json` (excluir `**/*.test.ts` de `tsc`)

**Interfaces:**
- Consumes: eventos `pairing://state` y órdenes de la Tarea 8. `types.ts` replica `PairingView` (unión discriminada por `phase`).
- Produces: `formatRemaining(ms: number): string` (`"CADUCA EN 1:45"` se arma con `m:ss`; 0 → `0:00`; negativo → `0:00`), `isWarning(ms: number): boolean` (últimos 20 s), `reasonText(reason: EndReason): string`.

- [ ] **Step 1: Pruebas** con `bun test` (sin paquetes nuevos): `format_remaining_shows_minutes_and_seconds` (105000 → `1:45`, 59999 → `1:00`, 0 y -5 → `0:00`), `warning_in_the_last_20_seconds` (20000 sí, 20001 no), `every_end_reason_has_a_text` (todas las variantes de `EndReason`).
- [ ] **Step 2: Implementar** `PairingWindow`: estado solo desde `pairing_snapshot` y el evento; cuenta regresiva local cada 1000 ms (`setInterval`, sin reloj de pared), corregida por cada evento y con `prefers-reduced-motion` respetado; tarjeta blanca de radio 16 con el QR a 240 × 240 en un `<img src={qr}>` (nunca `dangerouslySetInnerHTML`); `Waiting`: «Escanea este código con Bóveda en tu celular», «CADUCA EN m:ss» en `metric`, `warning` en los últimos 20 s, «Cancelar»; `Confirming`: «¿Emparejar con [nombre]?» (nombre truncado con elipsis y también en `title`), la huella en `secret`, «Emparejar» y «Rechazar» con el foco por defecto; `Ended`: motivo y «Generar otro código» sobre el marcador al 20 %; `NoNetwork`: «No se encontró una red privada. Conecta la PC a una red que Windows marque como privada.»; `Done`: «Emparejado con [nombre]» y «Listo». Esc llama a `pairing_cancel` (salvo en `Saving`); anillo de foco de 2 px en `accent`; objetivos ≥ 32 × 32; `forced-colors`. Al recibir `Closed`, o cualquier fase distinta de `Waiting`, el estado del `qr` se pone en `null`.
- [ ] **Step 3: Verificar:** `bun test` y `bun run build` (incluye `tsc`) en verde, desde `desktop/`. Lo visual y el teclado quedan para la prueba manual del usuario.
- [ ] **Step 4: Commit:** `feat: interfaz de la ventana del QR y la confirmación (#101)`.

---

### Task 10: Documentación y verificación final

**Files:**
- Modify: `docs/especificacion.md` (subsección «Ventana del QR y confirmación (#101)»; ajuste de «Servidor de la PC» y del paso 1 de «Emparejamiento»), `ARCHITECTURE.md`, `docs/diferencias-diseno.md` (quitar la fila de las fuentes; actualizar «Emparejar celular»; anotar el estado «Emparejado con [nombre]» y el marcador fijo en lugar del QR atenuado), `desktop/CLAUDE.md` (la interfaz solo muestra la imagen del QR), `desktop/README.md` (el emparejamiento usa el mismo puerto y las mismas reglas de Firewall)

- [ ] **Step 1:** Escribe cada cambio con las decisiones del diseño y lo que realmente se comprobó (QR, `content_protected`, WebView2). Lo no comprobado se escribe como límite, no como garantía.
- [ ] **Step 2: Verificación completa**, cada comando por separado y con su código de salida: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo audit`, `cargo deny check`, y desde `desktop/`: `bun test` y `bun run build`. `bun run tauri build` descarga NSIS si no está en caché: **pregunta antes** de ejecutarlo.
- [ ] **Step 3: Lista de pruebas manuales para el PR** (a cargo del usuario): ver el QR, la cuenta regresiva y el color de aviso; Esc, Cancelar y la X; que «Emparejar celular» con la ventana abierta solo la enfoca; la caducidad con el marcador y «Generar otro código»; la confirmación con «Rechazar» enfocado; red sin perfil privado; el escaneo con un celular real queda para #103 y #108.
- [ ] **Step 4: Commit:** `docs: documentar la ventana del QR y su contrato con el celular (#101)`.

---

## Autorrevisión

- **Cobertura del diseño:** enrutado (T4), dirección y elegibilidad (T2), estado en Rust, una sola sesión, plazos, cancelación, red cambiada y carreras (T5–T7), guardado tras `confirm` y `AlreadyPaired` (T6), imagen sensible y `Debug` (T5, T8, T9), caducidad con marcador (T5, T9), formato del QR con medición (T1), fuentes (T9), bandeja (T8), documentación (T10).
- **Alcance:** no se toca `core/`, ni #103, ni #107.
- **Riesgo conocido:** la señal de «adaptador virtual» (`HardwareInterface`) se verifica contra el código de `windows-sys` y contra este equipo en la Tarea 2; si no distingue, se avisa antes de seguir.
