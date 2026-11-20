# Servidor de Windows (#100) — plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** La app de bandeja escucha y se anuncia por mDNS solo en las IPv4 de redes que Windows marca como `Private`, y rechaza a los celulares no emparejados en el intercambio Noise.

**Architecture:** Un supervisor revisa cada 5 s el conjunto de IPv4 privadas (detector inyectable). Cada conjunto no vacío tiene una *generación*: listeners con un mismo puerto, hilo de `accept`, trabajadores con `receive_send`, `CancelToken` propio y un anuncio mDNS con reintento independiente. Si el conjunto cambia o el detector falla, se baja la generación completa. El núcleo (`vault-core`) no cambia.

**Tech Stack:** Rust 2024, `vault-core` (`receive_send`, `SendTimeouts`, `CancelToken`), `mdns-sd` 0.22, `windows` 0.62 (solo `desktop/`), `windows-sys` 0.61, Tauri 2 (NSIS).

**Spec:** `docs/superpowers/specs/2026-11-20-servidor-windows-design.md`

## Global Constraints

- Puerto preferido `47615`; si no sirve, **un único puerto** para todos los listeners (nunca `bind(ip:0)` por IP). Revisión de red cada **5 s**; sondeo de `accept` de **100 ms**; máximo **4** conexiones simultáneas (la excedente se cierra sin leer ni escribir); plazos de `SendTimeouts::default()` (10 s por trama, 30 s total, 10 s de `Ack`).
- "Privada" = `NLM_NETWORK_CATEGORY_PRIVATE`. `Public` y `DomainAuthenticated` no cuentan. Se descartan loopback, link-local (`169.254.0.0/16`), no especificadas y multicast. Jamás `0.0.0.0`. Solo IPv4; IPv6 fuera.
- Cualquier error o duda del detector = conjunto vacío (fail closed).
- Servicio `_boveda._tcp.local.`; instancia y `host_name` `boveda-<8 hex aleatorios>` / `boveda-<8 hex>.local.`, nunca el nombre de la PC; TXT `v=1` e `id=<huella corta>`; direcciones explícitas, sin `enable_addr_auto`.
- Orden de arranque: todos los listeners enlazados → anuncio. Orden de parada: retirar anuncio → cancelar token → cerrar listeners → esperar trabajadores con plazo.
- Un fallo de mDNS nunca baja ni reinicia el listener. La autorización es solo `receive_send`.
- La contraseña va de `vault-core` al Rust nativo y al `ClipboardManager`; nunca al webview, a disco ni a logs. `copied()` solo tras escribir bien el portapapeles; si no, `failed()`.
- `windows` 0.62 solo en `desktop/` con el mínimo de features (justificar cada extra). Sin más crates de red que `mdns-sd`. Comparar `grep '^name = ' Cargo.lock` antes y después. Sin cambios en `core/`.
- Errores y logs solo con su tipo; nada de contenido, IPs de celulares, llaves ni contraseñas. Sin `#[allow(dead_code)]`.
- Comentarios y commits en español (Conventional Commits); identificadores en inglés. Sin Python. Un commit por tarea.

## Review Focus

- Cliente que conecta y no envía nada: se cierra por plazo y libera su ranura (Task 6).
- El trabajador falla o hace panic: la ranura se libera igual (Task 6).
- Mutex de la lista de emparejados envenenado: `is_paired` devuelve `false`, sin panic (Task 6).
- `stop()` con un trabajador colgado: no cuelga el cierre de la app; espera acotada (Task 6).
- Primer `tick` con detector en error o conjunto vacío (arranque sin red): no sube nada y no hace panic (Task 9).

## File Structure

Todo en `desktop/src-tauri/src/`, cada módulo `#[cfg(windows)]` como `identity.rs`:

- `backoff.rs` — espera creciente y acotada, con `Instant` inyectado.
- `network.rs` — trait `NetworkProbe`, filtro y normalización; `network/nlm.rs` — backend COM (todo el `unsafe`).
- `announce.rs` — trait `Announcer`, `Service`, `AnnounceSlot` (reintento independiente); `announce/mdns.rs` — `MdnsAnnouncer`.
- `server.rs` — raíz, `Config`, `Deps`, `PasswordSink`; `server/bind.rs` (mismo puerto); `server/generation.rs`; `server/supervisor.rs` (`Supervisor` lógico y `ServerSupervisor` con hilo).
- `windows/hooks.nsh` (en `src-tauri/`) y `tauri.conf.json` — reglas de Firewall.

---

### Task 1: Verificar que `mdns-sd` solo anuncia por las IPv4 privadas (compuerta)

**Files:**
- Modify: `desktop/src-tauri/Cargo.toml` (agregar `mdns-sd = "0.22"` en `[target.'cfg(windows)'.dependencies]`)
- Create: `desktop/src-tauri/tests/mdns_interfaces.rs` (`#[ignore]`, red real)
- Modify: `docs/superpowers/specs/2026-11-20-servidor-windows-design.md` (sección «Verificación de `mdns-sd`» con la evidencia)

**Interfaces:**
- Produces: la configuración exacta de `ServiceDaemon` (qué `disable_interface`/`enable_interface`/`ServiceInfo::set_interfaces` usar) que consumen Tasks 7–8, o el bloqueo documentado.

- [ ] **Step 1:** Agregar la dependencia y ejecutar `cargo fetch`. Registrar los crates nuevos que entran al lock (esperados: `fastrand`, `flume`, `if-addrs`, `mio`, `socket-pktinfo`; `socket2` ya está).
- [ ] **Step 2:** Leer en `~/.cargo/registry/src/*/mdns-sd-0.22.*/src/` dónde se crean los sockets IPv4/IPv6, cómo `disable_interface`/`enable_interface` y `set_interfaces` filtran qué interfaces abren, anuncian y responden consultas (incluido el loopback, habilitado por defecto) y si una interfaz deshabilitada deja de recibir y emitir.
- [ ] **Step 3: Escribir la prueba real** `only_announces_on_private_ipv4`: un `ServiceDaemon` anunciante configurado con la combinación candidata registra un servicio con una IPv4 local; un segundo daemon *consultante* restringido a IPv6 (`disable_interface(IfKind::IPv4)`) hace `browse("_boveda._tcp.local.")` durante 5 s y **no** debe recibir `ServiceResolved`; otro restringido a IPv4 sí debe recibirlo. Debe fallar con la configuración por defecto si esta anuncia por IPv6 y pasar con la candidata.
- [ ] **Step 4:** Ejecutar `cargo test -p boveda-desktop --test mdns_interfaces -- --ignored --nocapture`. Esperado: PASS con la candidata.
- [ ] **Step 5: Compuerta.** Si la combinación no demuestra que solo se anuncia y responde por las IPv4 privadas, **detener todo el plan y consultar al usuario** (no seguir en silencio). Si sí, escribir en el diseño la configuración final y la evidencia (versión leída, archivos revisados, resultado de la prueba).
- [ ] **Step 6: Commit** — `git add` los tres archivos y el lock; `git commit -m "chore: verificar que mdns-sd solo anuncia por IPv4 privadas (#100)"`.

### Task 2: Espera creciente y acotada (`backoff.rs`)

**Files:** Create `desktop/src-tauri/src/backoff.rs`; Modify `lib.rs` (`#[cfg(windows)] mod backoff;`).

**Interfaces:**
- Produces: `pub(crate) struct Backoff { .. }` con `Backoff::new(base: Duration, max: Duration) -> Self`, `ready(&self, now: Instant) -> bool`, `fail(&mut self, now: Instant)`, `reset(&mut self)`, `failures(&self) -> u32`.

- [ ] **Step 1: Pruebas** (en el mismo archivo, `#[cfg(test)]`): `new_is_ready_immediately`; `fail_waits_base_then_doubles` (base 5 s: tras el 1.º fallo `ready(t0+4s)` es `false` y `ready(t0+5s)` `true`; tras el 2.º, espera 10 s); `never_exceeds_max` (base 5 s, max 60 s, 20 fallos → espera 60 s); `reset_makes_ready_and_clears_failures`.
- [ ] **Step 2:** `cargo test -p boveda-desktop backoff` — FAIL (no compila).
- [ ] **Step 3:** Implementar con `Instant` recibido, sin `SystemTime` ni dormir; duplicar con `saturating` y recortar a `max`.
- [ ] **Step 4:** Las pruebas pasan.
- [ ] **Step 5: Commit** — `feat: espera creciente y acotada para reintentos (#100)`.

### Task 3: Detector de red: trait, filtro y normalización (`network.rs`)

**Files:** Create `desktop/src-tauri/src/network.rs`; Modify `lib.rs` (`pub mod network;`).

**Interfaces:**
- Produces: `pub trait NetworkProbe: Send + 'static { fn private_addresses(&mut self) -> Result<BTreeSet<Ipv4Addr>, ProbeError>; }`, `pub struct ProbeError` (`Debug, Clone, Copy, PartialEq, Eq`, sin datos, con `Display` fijo), `pub fn is_usable(ip: Ipv4Addr) -> bool`, `pub fn normalize(ips: impl IntoIterator<Item = Ipv4Addr>) -> BTreeSet<Ipv4Addr>`, y `pub struct FakeProbe` (`#[cfg(test)]`) con `FakeProbe::new()` y un `Arc<Mutex<VecDeque<Result<BTreeSet<Ipv4Addr>, ProbeError>>>>` que se llena desde la prueba (`script(...)`); cuando se agota repite el último resultado.

- [ ] **Step 1: Pruebas:** `is_usable` rechaza `127.0.0.1`, `169.254.1.2`, `0.0.0.0`, `224.0.0.251`, `255.255.255.255` y acepta `192.168.1.20`, `10.0.0.8`, `172.16.0.5`; `normalize` quita los no utilizables y los duplicados; `normalize([a,b]) == normalize([b,a])`; `ProbeError` no contiene datos (`format!("{:?}")` no cambia con el contexto).
- [ ] **Step 2:** `cargo test -p boveda-desktop network` — FAIL.
- [ ] **Step 3:** Implementar. `is_usable` usa `Ipv4Addr::{is_loopback, is_link_local, is_unspecified, is_multicast, is_broadcast}`.
- [ ] **Step 4:** PASS.
- [ ] **Step 5: Commit** — `feat: contrato del detector de redes privadas con filtro de direcciones (#100)`.

### Task 4: Backend real con `INetworkListManager` (`network/nlm.rs`)

**Files:** Create `desktop/src-tauri/src/network/nlm.rs`; Modify `desktop/src-tauri/Cargo.toml`; Modify `network.rs` (`#[cfg(windows)] mod nlm; pub use nlm::NlmProbe;`).

**Interfaces:**
- Consumes: `NetworkProbe`, `ProbeError`, `normalize` (Task 3).
- Produces: `pub struct NlmProbe` con `NlmProbe::new() -> Self` (infalible; no toca COM) e `impl NetworkProbe`. Inicializa COM (MTA, `CoInitializeEx`) en la **primera** llamada de `private_addresses` y lo libera en `Drop`; el supervisor mueve la sonda a su hilo antes de usarla.

- [ ] **Step 1: Cargo.toml.** Agregar `windows = { version = "0.62", features = ["Win32_Networking_NetworkListManager", "Win32_System_Com"] }` en `cfg(windows)`; en `windows-sys` sumar `Win32_NetworkManagement_IpHelper` (y `Win32_Networking_WinSock`, `Win32_NetworkManagement_Ndis` si el compilador los pide). Anotar en un comentario por qué cada una. Confirmar que el lock no suma crates nuevos de `windows`.
- [ ] **Step 2: Prueba** `#[ignore]` en `desktop/src-tauri/tests/network_windows.rs`: `real_probe_returns_only_usable_addresses` — `NlmProbe::new().private_addresses()` da `Ok` y todas las direcciones pasan `is_usable`; se imprime el conjunto para la comprobación manual con una red marcada como privada y otra como pública.
- [ ] **Step 3:** Implementar: `GetNetworks(NLM_ENUM_NETWORK_CONNECTED)` → por cada `INetwork` con `GetCategory() == NLM_NETWORK_CATEGORY_PRIVATE` → `GetNetworkConnections()` → `GetAdapterId()`; cruzar el GUID con `AdapterName` de `GetAdaptersAddresses` (familia `AF_INET`) y reunir sus IPv4 unicast; pasar por `normalize`. Todo error de COM, de la API o un GUID sin adaptador es `Err(ProbeError)`. El `unsafe` queda solo en este archivo, con un comentario de seguridad por bloque.
- [ ] **Step 4:** `cargo test -p boveda-desktop --test network_windows -- --ignored --nocapture`. Esperado: PASS; revisar a mano el conjunto con una red privada y con una pública.
- [ ] **Step 5: Commit** — `feat: detectar redes privadas con el Administrador de listas de redes (#100)`.

### Task 5: Enlazar todas las IPs al mismo puerto (`server/bind.rs`)

**Files:** Create `desktop/src-tauri/src/server.rs` (raíz mínima: `mod bind;`) y `desktop/src-tauri/src/server/bind.rs`; Modify `lib.rs` (`pub mod server;`).

**Interfaces:**
- Produces: `pub(crate) struct Bound { pub listeners: Vec<TcpListener>, pub port: u16 }`, `pub(crate) enum BindError { Io(io::ErrorKind), NoPort }` (`Debug, Clone, Copy`, sin direcciones), `pub(crate) fn bind_all(ips: &BTreeSet<Ipv4Addr>, preferred: u16, attempts: u32) -> Result<Bound, BindError>`. Listeners en modo no bloqueante.

- [ ] **Step 1: Pruebas** (sobre `127.0.0.1` y `127.0.0.2`, que Windows enruta a loopback): `uses_preferred_port_when_free`; `falls_back_to_one_shared_port_when_preferred_is_busy` (ocupar el preferido con otro `TcpListener`; todos los `listeners[i].local_addr().port()` son iguales y distintos del preferido, y `Bound.port` coincide); `fails_if_any_ip_cannot_bind_and_leaves_nothing_open` (una IP no asignada a la máquina, p. ej. `203.0.113.1`: `Err`, y los puertos probados quedan libres: un `TcpListener::bind` posterior en la IP válida al mismo puerto funciona); `empty_set_is_error`.
- [ ] **Step 2:** `cargo test -p boveda-desktop bind` — FAIL.
- [ ] **Step 3:** Implementar el algoritmo del diseño: (1) todas las IPs al `preferred`; (2) ante «dirección en uso», cerrar todo, enlazar una vez a `primera_ip:0`, leer el puerto, **soltar el sondeo** e intentar todas las IPs a ese puerto; (3) si una falla, cerrar todo y repetir con un candidato nuevo hasta `attempts`; (4) agotados, `BindError::NoPort`. Cualquier otro error de E/S termina de inmediato.
- [ ] **Step 4:** PASS.
- [ ] **Step 5: Commit** — `feat: enlazar todas las IPs privadas al mismo puerto (#100)`.

### Task 6: Generación: aceptar, limitar, autorizar y entregar (`server/generation.rs`)

**Files:** Create `desktop/src-tauri/src/server/generation.rs`; Modify `server.rs`.

**Interfaces:**
- Consumes: `bind_all`/`Bound` (Task 5); de `vault-core`: `receive_send`, `SendTimeouts`, `CancelToken`, `KeyPair`, `PublicKey`; `PairedDeviceStore`.
- Produces en `server.rs`:
  - `pub trait PasswordSink: Send + Sync + 'static { fn deliver(&self, password: &SecretString) -> Result<(), DeliverFailed>; }` y `pub struct DeliverFailed;`.
  - `pub struct Config { pub preferred_port: u16, pub max_connections: usize, pub accept_poll: Duration, pub timeouts: SendTimeouts, pub bind_attempts: u32, pub stop_wait: Duration, pub check_interval: Duration }` con `Default` = `47615`, `4`, `100 ms`, `SendTimeouts::default()`, `5`, `2 s`, `5 s`.
  - `pub struct Deps { pub identity: Arc<KeyPair>, pub paired: Arc<Mutex<PairedDeviceStore>>, pub sink: Arc<dyn PasswordSink> }` (`Clone`).
- Produces en `generation.rs`: `pub(crate) struct Generation`; `Generation::start(config: &Config, ips: &BTreeSet<Ipv4Addr>, deps: &Deps) -> Result<Self, BindError>`; `port(&self) -> u16`; `is_alive(&self) -> bool` (el hilo de `accept` sigue); `stop(self)` (cancela el token propio, cierra, espera trabajadores hasta `stop_wait`); `token(&self) -> &CancelToken`.

- [ ] **Step 1: Pruebas** (loopback real, núcleo como cliente con `send_to_pc`, `PasswordSink` doble que guarda lo recibido y puede fallar a pedido):
  - `paired_phone_gets_copied_after_sink_ok`: el cliente recibe `AckOutcome::Copied` y el sink recibió la contraseña exacta.
  - `sink_failure_answers_failed`: el sink devuelve `Err` → cliente recibe `AckOutcome::Failed`.
  - `unpaired_phone_is_closed_and_sink_untouched`: el cliente obtiene `SendError::Closed` y el sink no recibió nada.
  - `fifth_simultaneous_connection_is_closed_without_reading`: abrir 4 conexiones que no envían nada y una 5.ª; la 5.ª ve EOF de inmediato sin que se le escriba nada.
  - `silent_client_times_out_and_frees_its_slot`: con `timeouts` cortos (`step` 200 ms), tras el plazo una conexión nueva se atiende (Review Focus).
  - `slot_is_released_when_worker_errors`: tras un envío no emparejado, el contador vuelve a 0 y se pueden abrir 4 conexiones más (Review Focus).
  - `poisoned_paired_mutex_rejects_everyone`: envenenar el `Mutex` desde otro hilo; un celular emparejado recibe `Closed` y no hay panic (Review Focus).
  - `stop_cancels_active_workers_and_is_bounded`: con un cliente a medio apretón de manos, `stop()` vuelve en menos de `stop_wait + 500 ms` y las conexiones se cierran; con `stop_wait` de 300 ms un sink colgado no impide que `stop()` regrese (Review Focus).
  - `new_generation_has_a_fresh_token`: el token de una generación detenida está cancelado y el de la siguiente no.
- [ ] **Step 2:** `cargo test -p boveda-desktop generation` — FAIL.
- [ ] **Step 3:** Implementar. Un hilo de `accept` que sondea todos los listeners cada `accept_poll` y comprueba el token; contador de ranuras con guarda `Drop`; un hilo por conexión que llama a `receive_send(stream, &identity, is_paired, &token, &timeouts)`; `is_paired` toma el candado y, si está envenenado o no está la llave, devuelve `false`; con `IncomingSend`, `sink.deliver(msg.password())` y `copied()`/`failed()`. Los errores se registran con `eprintln!` solo con el tipo del error. Si el hilo de `accept` muere por un error no recuperable, `is_alive` pasa a `false`.
- [ ] **Step 4:** PASS (repetir tres veces para descartar carreras).
- [ ] **Step 5: Commit** — `feat: servidor TCP por generación con límite de conexiones y rechazo en Noise (#100)`.

### Task 7: Anuncio con reintento independiente (`announce.rs`) e integración en la generación

**Files:** Create `desktop/src-tauri/src/announce.rs`; Modify `server/generation.rs`, `server.rs`, `lib.rs` (`pub mod announce;`).

**Interfaces:**
- Consumes: `Backoff` (Task 2), `Generation` (Task 6), `Fingerprint` de `vault-core`.
- Produces: `pub struct Service { pub instance: String, pub host: String, pub addrs: BTreeSet<Ipv4Addr>, pub port: u16, pub id: String }`; `pub trait Announcer: Send + 'static { fn announce(&mut self, service: &Service) -> Result<(), AnnounceError>; fn withdraw(&mut self); }`; `pub struct AnnounceError` (sin datos); `pub type AnnouncerFactory = Arc<dyn Fn() -> Box<dyn Announcer> + Send + Sync>`; `Service::new(addrs, port, fingerprint: &Fingerprint) -> Service` (instancia y host con 8 hex aleatorios de `RandomState`; `id = fingerprint.to_string()`); `Service::txt(&self) -> [(&'static str, String); 2]` = `v=1`, `id=…`; `pub(crate) struct AnnounceSlot` con `AnnounceSlot::new(announcer: Box<dyn Announcer>, service: Service, backoff: Backoff)`, `poll(&mut self, now: Instant)`, `is_announced(&self) -> bool`, `withdraw(&mut self)`. `Deps` suma `pub announcer: AnnouncerFactory`; `Generation::poll_announce(&mut self, now: Instant)`.

- [ ] **Step 1: Pruebas** (con un `Announcer` doble que cuenta llamadas y puede fallar a pedido): `service_never_contains_host_name_and_txt_is_exact` (instancia empieza por `boveda-`, 8 hex; TXT = `v=1` e `id=` con la huella); `announces_only_after_listeners_are_bound` (la generación llama a `announce` una vez y el `Service` lleva el puerto de `Generation::port()` y solo las IPs del conjunto); `withdraw_happens_before_listeners_close` (el doble registra el orden de eventos: `withdraw` antes de que el puerto deje de aceptar); `mdns_failure_does_not_stop_the_listener` (con `announce` fallando, un celular emparejado sigue recibiendo `Copied` y `is_alive()` es `true`); `mdns_retries_without_rebuilding_generation` (con `announce` fallando 2 veces y luego bien, `poll_announce` llamado con instantes crecientes logra anunciar y el número de generaciones y el puerto no cambian; antes de `next_attempt` no hay llamada); `no_announcement_without_valid_listener` (si `start` falla por bind, el doble no recibe ninguna llamada).
- [ ] **Step 2:** `cargo test -p boveda-desktop announce` — FAIL.
- [ ] **Step 3:** Implementar `AnnounceSlot` con estado `Announced` o `Retrying` apoyado en `Backoff`. `Generation::start` crea el anuncio con la fábrica y hace el primer intento tras enlazar; `stop` llama a `withdraw()` **antes** de cancelar el token. `poll_announce` solo avanza el reintento.
- [ ] **Step 4:** PASS.
- [ ] **Step 5: Commit** — `feat: anuncio mDNS con reintento independiente del listener (#100)`.

### Task 8: Anunciante real con `mdns-sd` (`announce/mdns.rs`)

**Files:** Create `desktop/src-tauri/src/announce/mdns.rs`; Modify `announce.rs`.

**Interfaces:**
- Consumes: `Announcer`, `Service`, `AnnounceError`; la configuración fijada en Task 1.
- Produces: `pub struct MdnsAnnouncer` con `MdnsAnnouncer::new() -> Self` (infalible; el `ServiceDaemon` se crea en el primer `announce`) e `impl Announcer`.

- [ ] **Step 1: Prueba** `#[ignore]` en `tests/mdns_interfaces.rs` (ampliar la de Task 1): `mdns_announcer_is_visible_over_ipv4_and_invisible_over_ipv6` con `MdnsAnnouncer`; y `withdraw_makes_service_disappear` (el consultante IPv4 ve la baja).
- [ ] **Step 2:** FAIL.
- [ ] **Step 3:** Implementar: `ServiceInfo::new("_boveda._tcp.local.", &instance, &host, ips, port, &txt)` con las direcciones explícitas, la combinación de interfaces fijada en Task 1 y sin `enable_addr_auto`; `withdraw` hace `unregister` (esperar su estado hasta ~1 s) y `shutdown` del daemon, sin entrar en pánico ni bloquear sin plazo.
- [ ] **Step 4:** `cargo test -p boveda-desktop --test mdns_interfaces -- --ignored --nocapture` — PASS.
- [ ] **Step 5: Commit** — `feat: anunciante mDNS real restringido a IPv4 privadas (#100)`.

### Task 9: Supervisor (`server/supervisor.rs`)

**Files:** Create `desktop/src-tauri/src/server/supervisor.rs`; Modify `server.rs` (`pub use supervisor::ServerSupervisor;`).

**Interfaces:**
- Consumes: `NetworkProbe`/`FakeProbe`, `Generation`, `Config`, `Deps`, `Backoff`.
- Produces: `pub(crate) struct Supervisor` con `Supervisor::new(probe: Box<dyn NetworkProbe>, deps: Deps, config: Config) -> Self`, `tick(&mut self, now: Instant)`, `generations_started(&self) -> u32`, `current_port(&self) -> Option<u16>`, `shutdown(&mut self)`; y `pub struct ServerSupervisor` con `ServerSupervisor::start(probe: impl NetworkProbe, deps: Deps, config: Config) -> Self` (hilo que llama a `tick(Instant::now())` cada `config.check_interval` hasta `stop`), `local_port(&self) -> Option<u16>`, `stop(&self)` (idempotente, acotado).

- [ ] **Step 1: Pruebas** (`FakeProbe` + `Deps` dobles, IPs `127.0.0.1`/`127.0.0.2`): `empty_to_one_starts_a_generation`; `one_to_empty_stops_and_port_is_released`; `same_set_in_other_order_does_nothing` (`generations_started` no cambia); `added_ip_restarts_generation`; `replaced_ip_restarts_generation`; `private_public_private_cycle_gets_a_new_token` (la segunda generación acepta conexiones y la primera ya no); `probe_error_stops_a_running_server` (fail closed); `first_tick_with_probe_error_or_empty_set_starts_nothing` (sin panic, `current_port() == None`) (Review Focus); `dead_listener_is_rebuilt_with_backoff` (matar el hilo de `accept` y comprobar que se reconstruye tras `next_attempt`, no antes); `bind_failure_retries_next_ticks_without_partial_listening`; `server_supervisor_stop_is_idempotent_and_bounded`.
- [ ] **Step 2:** `cargo test -p boveda-desktop supervisor` — FAIL.
- [ ] **Step 3:** Implementar. Cada `tick`: leer el conjunto (error → vacío); si difiere del vigente, bajar la generación (`stop()`) y, si no está vacío, subir otra; si coincide, solo revisar `is_alive` (reconstruir con `Backoff`) y llamar a `poll_announce(now)`. `ServerSupervisor` expone el puerto en un `Mutex<Option<u16>>` y `stop` une el hilo con plazo.
- [ ] **Step 4:** PASS (tres ejecuciones seguidas).
- [ ] **Step 5: Commit** — `feat: supervisor que sube y baja el servidor según las redes privadas (#100)`.

### Task 10: Conexión con Tauri (`lib.rs`)

**Files:** Modify `desktop/src-tauri/src/lib.rs`; Create `desktop/src-tauri/src/server/tauri_sink.rs`.

**Interfaces:**
- Consumes: `ServerSupervisor::start`, `Deps`, `Config`, `NlmProbe`, `MdnsAnnouncer`, `ClipboardManager`, `notify_ready`, `IdentityStore`, `PairedDeviceStore`.
- Produces: `pub struct TauriSink` (`impl PasswordSink` con `app: tauri::AppHandle`: `app.state::<ClipboardManager>().deliver_and_notify(password, || notify_ready(&app))`, mapea error a `DeliverFailed`); estado de Tauri `Arc<Mutex<PairedDeviceStore>>` y `ServerSupervisor`.

- [ ] **Step 1: Prueba** en `server/tauri_sink.rs` sobre un `ClipboardManager::with_backend(fake)`: `ok_delivery_writes_to_clipboard` y `clipboard_error_maps_to_deliver_failed` (aviso inyectable para no depender de `AppHandle`: extraer `fn deliver_with(manager, password, notify)` y probar esa).
- [ ] **Step 2:** FAIL.
- [ ] **Step 3:** Implementar. En `setup`, tras `ensure_identity`, `start_server(app)`: abrir `paired-devices.bin` (error → registrar el tipo y **no** arrancar), cargar la identidad con `IdentityStore::load` (ausente o ilegible → registrar y no arrancar, sin regenerar), `app.manage(paired.clone())`, arrancar el supervisor con `NlmProbe::new()` y `MdnsAnnouncer::new`, y `app.manage(supervisor)`. `ensure_identity` devuelve el `Result` para reutilizarlo. En `RunEvent::Exit`: `supervisor.stop()` **antes** de `clipboard.shutdown(..)`. Actualizar el comentario que dice «#100 lo usará».
- [ ] **Step 4:** `cargo test -p boveda-desktop` — PASS.
- [ ] **Step 5: Commit** — `feat: arrancar y detener el servidor con el ciclo de vida de la app (#100)`.

### Task 11: Reglas de Firewall del instalador

**Files:** Create `desktop/src-tauri/windows/hooks.nsh`; Modify `desktop/src-tauri/tauri.conf.json` (`bundle.windows.nsis.installerHooks`, `targets` solo `nsis` si MSI queda sin regla); Modify `desktop/README.md`.

**Interfaces:** Produces las reglas `Bóveda TCP (red privada)` y `Bóveda mDNS (red privada)` (nombres fijos, resolver en Step 1 si llevan tilde).

- [ ] **Step 1:** Comprobar en la documentación de NSIS/Tauri (context7 `/tauri-apps/tauri-docs`) que `installerHooks` y `ExecShellWait "runas"` están soportados por la versión de NSIS que usa `tauri-bundler`; elegir nombres ASCII para las reglas si las tildes dan problemas.
- [ ] **Step 2:** Escribir `NSIS_HOOK_POSTINSTALL`: elevar solo `netsh advfirewall firewall add rule name=… dir=in action=allow program="$INSTDIR\<exe>" profile=private protocol=TCP` y la regla `protocol=UDP localport=5353`; `NSIS_HOOK_POSTUNINSTALL`: `delete rule name=…` también elevado. Si el UAC se rechaza o el comando falla, la instalación continúa y se muestra un aviso. Si `runas` no es soportado o mantenible, cambiar a `installMode: "perMachine"` y anotar el motivo en el diseño.
- [ ] **Step 3:** `bun run tauri build` y revisar que el `.nsh` se incluye. Manual (a registrar en la PR): instalar → `netsh advfirewall firewall show rule name=all dir=in` muestra ambas reglas con `Profiles: Private`; desinstalar → desaparecen; rechazar el UAC → la app funciona y el servidor sigue limitado por la comprobación de red.
- [ ] **Step 4:** En `desktop/README.md`, sección «Desarrollo»: `tauri dev` no crea reglas; comando manual con administrador (`netsh advfirewall firewall add rule … profile=private`) y cómo borrarlas.
- [ ] **Step 5: Commit** — `feat: reglas de Firewall solo para redes privadas en el instalador (#100)`.

### Task 12: Documentación y verificación final

**Files:** Modify `ARCHITECTURE.md` (mapa de `desktop/src/` y descripción del servidor), `docs/especificacion.md` (sección del servidor: puerto, nombre mDNS, límites, fail closed, IPv6 fuera, reglas), `docs/roadmap.md` (marcar el servidor de la Etapa 3, indicando lo pendiente: pruebas manuales de Firewall y de cambio de red).

- [ ] **Step 1:** Actualizar los tres documentos con lo implementado y lo decidido (valores exactos de *Global Constraints*), y anotar los crates nuevos del lock.
- [ ] **Step 2:** Ejecutar y confirmar que pasan: `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check`, `cargo audit`, `cargo deny check`. Ejecutar además las pruebas `--ignored` de Tasks 1, 4 y 8 y la comprobación manual de cambio de red (privada → pública → privada) con la app abierta.
- [ ] **Step 3:** Comparar `grep '^name = ' Cargo.lock` con el de antes; listar los crates nuevos en la descripción de la PR. Confirmar que `core/` no tiene cambios (`git diff dev --stat -- core`).
- [ ] **Step 4: Commit** — `docs: documentar el servidor de Windows (#100)`.

---

## Self-review

- **Cobertura del diseño:** detección fail closed y filtro (3, 4); bind por IP y puerto único (5); límite, autorización, entrega y `copied`/`failed` (6); orden de arranque/parada, mDNS independiente y reintento (7, 8); supervisor y cambios de conjunto (9); Tauri, lista compartida y orden al salir (10); Firewall, UAC y `tauri dev` (11); IPv6 y compuerta (1); documentación y verificación (12).
- **Tipos coherentes:** `Config`, `Deps`, `PasswordSink`, `Generation`, `AnnounceSlot`, `Announcer`, `Backoff` se definen una vez y se citan igual en las tareas posteriores. `Deps.announcer` se agrega en Task 7 y las pruebas de Task 6 usan un anunciante nulo hasta entonces.
- **Proporción:** las tareas fijan firmas, pruebas y valores; los cuerpos los decide quien implementa.
