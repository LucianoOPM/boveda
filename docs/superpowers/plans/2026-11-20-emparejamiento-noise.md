# Emparejamiento Noise con código de un solo uso (#95) — plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `vault-core` empareja al celular y a la PC con Noise (`snow`) y un código de un solo uso, con confirmación obligatoria del usuario en la PC, probado con ambos extremos por `TcpStream` en `127.0.0.1`.

**Architecture:** `protocol/pairing.rs` tiene las máquinas de estado por tipos (sin sockets): `PairingSession` (estados del código) en la PC e `Initiator` en el celular. `protocol/transport.rs` tiene lo mínimo del conductor síncrono: `TimedStream`, `CancelToken`, `PairingTimeouts` y lectura y escritura de tramas con plazos, cancelación y tamaño máximo. Los conductores `serve_pairing` y `pair_with_pc` unen ambos. Los mensajes nuevos (`PairingRequest`, `PairingReply`, `DeviceName`) se agregan a `message.rs` con la codificación estricta de la #94.

**Tech Stack:** Rust, `snow` 0.10 (nueva, aprobada, sin `std`), `serde` + `postcard`, `secrecy`, `zeroize`, `thiserror`, `proptest`. Sin tokio ni otras dependencias de red.

**Spec:** `docs/superpowers/specs/2026-11-20-emparejamiento-noise-design.md` (los dos documentos se leen juntos).

## Global Constraints

- Identificadores en inglés; comentarios, documentación y commits en español, Conventional Commits con `(#95)`.
- Rama `feat/95-emparejamiento` (desde `feat/94-protocolo-tipos`; el PR contra `dev` se abre cuando la #94 esté fusionada, sin repetir sus cambios).
- Pruebas primero: cada función nueva empieza con una prueba que falla y se ve fallar por la razón esperada (`core/CLAUDE.md`). Las unitarias van en `modulo/tests.rs` (`modulo.rs` termina con `#[cfg(test)] mod tests;`); las de la API pública, en `core/tests/protocol.rs`.
- Única dependencia nueva: `snow = { version = "0.10", default-features = false, features = ["use-curve25519", "use-chacha20poly1305", "use-blake2", "use-getrandom"] }`. **Sin la feature `std`** (en 0.10.0 activa `ring`, `sha2` y `blake2/std`). Cualquier otra dependencia criptográfica o de red se pregunta antes.
- Patrón `Noise_XKpsk0_25519_ChaChaPoly_BLAKE2s`; prólogo `boveda/emparejamiento/v1`; `PAIRING_CODE_TTL` 120 s; `MAX_FAILED_ATTEMPTS` 3; `FIRST_FRAME_LEN` 48; `MAX_FRAME_LEN` 512; `CANCEL_POLL` 250 ms; plazos por defecto: 10 s por paso, 20 s hasta la trama 3, 60 s para confirmar, 65 s de espera del celular. `PROTOCOL_VERSION` sigue en 1.
- Trama: `longitud (u16 big-endian) | bytes`; la longitud no incluye sus 2 bytes; 0 no se acepta; se comprueba **antes de reservar**.
- El núcleo **nunca persiste** una relación de confianza. La confirmación del usuario es obligatoria: sin `accept` no hay `PairedDevice`; sin `Accepted` autenticado no hay `PairedPc`.
- Código incorrecto, caducado, reutilizado o agotado: la PC cierra sin escribir una sola trama; el celular recibe el mismo `Closed` que ante un corte de red.
- Ningún secreto en logs, errores, panics ni `Debug`; los errores no llevan datos ni texto de `snow`. La API pública no hace panic (sin `unwrap`/`expect` fuera de pruebas). La llave privada y el código se pasan a `snow` como `&[u8]` desde su `SecretBox`, sin copias propias. No se presenta esto como borrado seguro: `snow` no borra nada (excepción en el diseño).
- No tocar el formato de la bóveda, `vault::Error` ni `mobile/rust`.
- Antes de cada commit: `cargo test -p vault-core` en verde. Nunca encadenar `fmt`, `clippy` y `commit` con `;` ni con `| grep`: correr cada uno aparte y comprobar su código de salida.
- No hay Python: los scripts auxiliares, si hacen falta, en Node o bash, dentro del scratchpad.

## Review Focus

Condiciones que el diseño implica y que ninguna tarea debe dejar sin prueba, de más a menos probable:

- Un nombre con U+202E, con NUL, solo espacios, de 128 bytes con un carácter de 2 bytes que lo parta (127 + 2 = 129) o de 128 bytes exactos con letras no ASCII: solo el último se acepta (Task 1).
- Un cliente que declara una longitud de 0xFFFF o que manda la longitud y se queda mudo: sin reservar memoria y sin colgarse (Task 6 y 8).
- Una trama 4 válida de **otra** sesión (misma forma, otras llaves) o con un bit cambiado: no completa (Task 5 y 8).
- Dos hilos con el código correcto a la vez: solo uno llega a `PendingPairing` (Task 4 y 8).
- Un celular que se desconecta mientras el diálogo está abierto: `confirm` falla y no hay `PairedDevice` (Task 7 y 8).

---

### Task 1: `DeviceName`

**Files:**
- Modify: `core/src/protocol/error.rs` (variante `InvalidField::DeviceName`, mensaje "el nombre del dispositivo no es válido")
- Modify: `core/src/protocol/message.rs`, `core/src/protocol/mod.rs` (reexportar)
- Test: `core/src/protocol/message/tests.rs`

**Interfaces:**
- Produces: `pub struct DeviceName(String)` (`Clone`, `PartialEq`, `Eq`; `Debug` solo con el largo); `DeviceName::new(&str) -> Result<DeviceName>`; `as_str(&self) -> &str`; `pub const MAX_DEVICE_NAME_BYTES: usize = 128`.

- [ ] **Step 1: Escribir las pruebas que fallan**, con estos nombres: `device_name_trims`, `device_name_accepts_1_and_128_bytes`, `device_name_rejects_empty_and_129_bytes`, `device_name_rejects_control_chars` (U+0000, U+0007, U+007F), `device_name_rejects_direction_chars` (una prueba que recorre U+061C, U+200E, U+200F, U+2028, U+2029, U+202A a U+202E y U+2066 a U+2069), `device_name_128_bytes_multibyte_ok` (64 × `é`) y `device_name_127_plus_2_bytes_rejected`. Todos los fallos son `ProtocolError::InvalidField(InvalidField::DeviceName)`.
- [ ] **Step 2: Correr** `cargo test -p vault-core device_name` — Esperado: no compila (`DeviceName` no existe).
- [ ] **Step 3: Implementar** `DeviceName::new` con `str::trim`, el límite en bytes y el conjunto de caracteres prohibidos del diseño (`char::is_control` más la lista de dirección y separación de texto, en una función privada `is_forbidden`).
- [ ] **Step 4: Correr** `cargo test -p vault-core device_name` — Esperado: PASS.
- [ ] **Step 5: Commit** `feat: nombre del dispositivo validado para el emparejamiento (#95)`

### Task 2: `PairingRequest` y `PairingReply`

**Files:**
- Modify: `core/src/protocol/message.rs` (`Kind::PairingRequest = 4`, `Kind::PairingReply = 5`), `core/src/protocol/mod.rs`
- Test: `core/src/protocol/message/tests.rs`

**Interfaces:**
- Consumes: `DeviceName` (Task 1), `encode_body` / `decode_body` existentes.
- Produces:
  - `pub struct PairingRequest { name: DeviceName }` con `new(DeviceName) -> Self`, `name(&self) -> &DeviceName`, `encode(&self) -> Result<Zeroizing<Vec<u8>>>`, `decode(&[u8]) -> Result<Self>` (valida el nombre al decodificar y exige que ya venga recortado).
  - `pub enum AckDecision { Accepted, Rejected }` (`Copy`, `PartialEq`, `Eq`, `Debug`, `Serialize`, `Deserialize`; `Accepted` = `0x00`, `Rejected` = `0x01`).
  - `pub struct PairingReply { decision: AckDecision }` con `new(AckDecision)`, `decision()`, `encode()`, `decode()`.

- [ ] **Step 1: Escribir las pruebas que fallan:** `pairing_reply_exact_bytes` (`Accepted` → `[1, 5, 0]`, `Rejected` → `[1, 5, 1]`), `pairing_reply_rejects_third_value` (`[1, 5, 2]` → `Malformed`), `pairing_reply_rejects_trailing_bytes`, `pairing_request_roundtrip`, `pairing_request_max_len_is_132` (nombre de 128 bytes → 132 bytes codificados), `pairing_request_rejects_untrimmed_name` (bytes armados a mano con `" ab"` → `InvalidField::DeviceName`), `pairing_request_rejects_wrong_kind` (bytes de un `Ack` → `UnexpectedKind(3)`) y `pairing_request_rejects_noncanonical_varint`.
- [ ] **Step 2: Correr** `cargo test -p vault-core pairing_re` — Esperado: no compila.
- [ ] **Step 3: Implementar** los dos mensajes sobre `encode_body` y `decode_body`, con los tipos 4 y 5; `PairingRequest::decode` valida con `DeviceName::new` y compara con el texto recibido.
- [ ] **Step 4: Correr** `cargo test -p vault-core pairing_re` — Esperado: PASS.
- [ ] **Step 5: Commit** `feat: mensajes PairingRequest y PairingReply (#95)`

### Task 3: dependencia `snow`, errores y tipos de resultado

**Files:**
- Modify: `core/Cargo.toml` (con un comentario en español: por qué sin `std`, qué entra y la excepción de borrado), `Cargo.lock`
- Create: `core/src/protocol/pairing.rs`, `core/src/protocol/pairing/tests.rs`
- Modify: `core/src/protocol/mod.rs` (`mod pairing;` y reexportes)

**Interfaces:**
- Produces:
  - Constantes públicas: `PAIRING_PATTERN: &str`, `PAIRING_PROLOGUE: &[u8]`, `PAIRING_CODE_TTL: Duration`, `MAX_FAILED_ATTEMPTS: u8`, `FIRST_FRAME_LEN: usize`, `MAX_FRAME_LEN: usize`.
  - `pub enum Refusal { WrongCode, Malformed, Expired, AlreadyUsed, TooManyAttempts, Cancelled }` (`Copy`, `Debug`, `PartialEq`, `Eq`).
  - `pub enum PairingError` (`#[non_exhaustive]`, `thiserror`, `Debug`, `PartialEq`, `Eq`, `Clone`): `Refused(Refusal)`, `PeerNotAuthentic`, `Rejected`, `ConfirmationTimeout`, `Timeout`, `Cancelled`, `Closed`, `Io(std::io::ErrorKind)`, `FrameTooLarge`, `Handshake`, `Protocol(ProtocolError)`; mensajes sin datos.
  - `pub struct PairedPc { public_key: PublicKey, address: SocketAddr }` con `public_key()`, `address()`, `provisional_name(&self) -> String` (`"PC "` + huella) y `into_new_paired_pc(self, name: Option<String>) -> crate::vault::NewPairedPc`.
  - `pub struct PairedDevice { public_key: PublicKey, name: DeviceName }` con getters; `Debug` solo con la huella.
  - `pub type Frame = Vec<u8>`.

- [ ] **Step 1: Escribir las pruebas que fallan** (`pairing/tests.rs`): `provisional_name_uses_fingerprint` (`"PC A1B2-C3D4-E5F6-0718"` con una llave de bytes fijos), `provisional_name_is_valid_for_the_vault` (entre 1 y 128 bytes y sin cambios al pasar por `vault::NewPairedPc` validado), `into_new_paired_pc_uses_given_name_or_provisional`, `into_new_paired_pc_sets_last_ip_canonical` (una dirección `::ffff:192.168.1.20` queda como `192.168.1.20`), `paired_device_debug_shows_only_fingerprint`, `pairing_error_messages_carry_no_data`.
- [ ] **Step 2: Correr** `cargo test -p vault-core pairing::` — Esperado: no compila.
- [ ] **Step 3: Agregar `snow`** a `core/Cargo.toml` con las features exactas de arriba y escribir los tipos y constantes de la sección Interfaces.
- [ ] **Step 4: Correr** `cargo test -p vault-core pairing::` — Esperado: PASS. Después, cada uno por separado: `cargo tree -p vault-core -i ring` (Esperado: error, el paquete no está), `cargo tree -p vault-core -i sha2` (ídem, o solo vía las dependencias previas del núcleo: anotar el resultado) y `cargo deny check` (Esperado: sin errores; solo avisos de versiones duplicadas).
- [ ] **Step 5: Commit** `feat: snow, errores y tipos de resultado del emparejamiento (#95)`

### Task 4: sesión del código en la PC y los dos primeros mensajes

**Files:**
- Modify: `core/src/protocol/pairing.rs`, `core/src/protocol/pairing/tests.rs`

**Interfaces:**
- Consumes: tipos de la Task 3; `KeyPair`, `PairingCode`, `PairingOffer`, `DeviceName`, `PairingRequest`.
- Produces:
  - `pub struct PairingSession` con `new(identity: KeyPair, code: PairingCode, now: Instant) -> Self`, `accept_first(&mut self, now: Instant, frame: &[u8]) -> Result<AwaitingThird, Refusal>`, `note_malformed(&mut self, now: Instant) -> Refusal`, `cancel(&mut self)`.
  - `pub struct AwaitingThird` con `second_message(&self) -> &[u8]` (la trama 2 sin prefijo de longitud).
  - `pub struct Initiator` con `new(identity: &KeyPair, offer: &PairingOffer, name: DeviceName) -> Result<Initiator, PairingError>` y `first_message(self) -> Result<(AwaitingSecond, Frame), PairingError>`.
  - `pub struct AwaitingSecond` con `read_second(self, frame: &[u8]) -> Result<(AwaitingReply, Frame), PairingError>` (devuelve la trama 3; trama 2 que no descifra → `PeerNotAuthentic`).
  - Función privada `noise_builder` compartida (patrón, prólogo, PSK en la posición 0) para no repetir la configuración.

- [ ] **Step 1: Escribir las pruebas que fallan**, con la sesión y el celular en memoria y `now` inyectado: `first_frame_is_48_bytes`, `correct_code_reaches_awaiting_third`, `wrong_code_is_refused_and_counts` (`WrongCode`), `third_failed_attempt_exhausts` (tres incorrectos y después el correcto → `TooManyAttempts`), `malformed_length_counts_as_attempt` (47 y 49 bytes vía `note_malformed`), `consumed_after_first_success` (segunda llamada con el código correcto → `AlreadyUsed`), `expired_at_exactly_120s` (a 119 s 999 ms aún sirve; a 120 s → `Expired`), `expired_does_not_use_the_code_or_count` (tras caducar, intentos con código incorrecto no cambian el estado), `cancelled_session_refuses`, `wrong_pc_key_is_peer_not_authentic` (el celular con la llave de otra PC en la oferta: `read_second` da `PeerNotAuthentic`), `refusals_do_not_change_after_final_state`.
- [ ] **Step 2: Correr** `cargo test -p vault-core pairing::` — Esperado: no compila o falla por falta de las funciones.
- [ ] **Step 3: Implementar** la sesión con el enum interno de estados (`Active { failed }`, `Consumed`, `Expired`, `Exhausted`, `Cancelled`), evaluando la caducidad primero con `now.saturating_duration_since(created)`. `accept_first` construye el respondedor de `snow` con `local_private_key(identity.private().expose_secret())` y el PSK `code.expose_bytes()`, lee la trama 1 (con éxito pasa a `Consumed`, con fallo suma intento y mapea a `WrongCode`) y escribe la trama 2 en el mismo paso. Los errores de `snow` nunca se reenvían.
- [ ] **Step 4: Correr** `cargo test -p vault-core pairing::` — Esperado: PASS.
- [ ] **Step 5: Commit** `feat: sesión del código de un solo uso y primeros mensajes Noise (#95)`

### Task 5: tercer mensaje, confirmación y respuesta

**Files:**
- Modify: `core/src/protocol/pairing.rs`, `core/src/protocol/pairing/tests.rs`

**Interfaces:**
- Consumes: `AwaitingThird`, `AwaitingSecond` (Task 4).
- Produces:
  - `AwaitingThird::read_third(self, now: Instant, frame: &[u8]) -> Result<PendingPairing, PairingError>` (decodifica el `PairingRequest`, obtiene la llave con `get_remote_static`, pasa a modo de transporte y fija `deadline = now + 60 s`).
  - `pub struct PendingPairing` solo de lectura: `peer_public_key()`, `fingerprint()`, `peer_name() -> &DeviceName`, `deadline() -> Instant`; `accept(self, now: Instant) -> Result<Decision, PairingError>`; `reject(self) -> Frame`.
  - `pub enum Decision { Accepted { device: PairedDevice, frame: Frame }, TimedOut { frame: Frame } }`.
  - `pub struct AwaitingReply` con `read_reply(self, frame: &[u8]) -> Result<PairedPc, PairingError>` (`Rejected` → `Rejected`; cualquier alteración → `Handshake` o `Protocol(..)`).

- [ ] **Step 1: Escribir las pruebas que fallan:** `full_exchange_in_memory` (ambos terminan con la llave del otro y la PC ve el nombre y la huella), `third_frame_max_is_196_bytes` (nombre de 128 bytes), `reply_frame_is_19_bytes`, `all_frames_fit_in_max_frame_len`, `accept_after_deadline_times_out` (`Decision::TimedOut` en `deadline`; `Accepted` un instante antes), `reject_gives_rejected_to_the_phone`, `altered_reply_does_not_pair` (un bit cambiado), `plaintext_reply_does_not_pair` (`[1, 5, 0]` sin cifrar), `reply_from_another_session_does_not_pair`, `reply_with_rejected_is_an_error`, `phone_never_gets_paired_pc_before_reply` (la única vía a `PairedPc` es `read_reply`; prueba que `AwaitingReply` no expone la llave), `third_frame_with_garbage_name_fails` (nombre con U+202E armado a mano dentro de un mensaje Noise válido → `Protocol(InvalidField::DeviceName)`).
- [ ] **Step 2: Correr** `cargo test -p vault-core pairing::` — Esperado: falla por funciones inexistentes.
- [ ] **Step 3: Implementar** las transiciones, soltando el `HandshakeState` y el `TransportState` en cuanto dejan de hacer falta. Un error en cualquier paso consume el valor.
- [ ] **Step 4: Correr** `cargo test -p vault-core pairing::` — Esperado: PASS.
- [ ] **Step 5: Commit** `feat: confirmación del emparejamiento por tipos y respuesta cifrada (#95)`

### Task 6: tramas, plazos y cancelación

**Files:**
- Create: `core/src/protocol/transport.rs`, `core/src/protocol/transport/tests.rs`
- Modify: `core/src/protocol/mod.rs`

**Interfaces:**
- Produces:
  - `pub trait TimedStream: Read + Write` con `set_read_timeout(&mut self, Option<Duration>) -> io::Result<()>` y `set_write_timeout(...)`; `impl TimedStream for std::net::TcpStream`. La documentación del trait fija el contrato (volver con `WouldBlock` o `TimedOut` al cumplirse el tiempo).
  - `pub struct CancelToken` (`Clone`, `Default`): `new()`, `cancel(&self)`, `is_cancelled(&self) -> bool`.
  - `pub struct PairingTimeouts { pub step: Duration, pub handshake: Duration, pub confirmation: Duration, pub reply_wait: Duration }` con `Default` (10 s, 20 s, 60 s, 65 s).
  - Funciones del crate: `read_frame_header(stream, deadline, step, cancel) -> Result<usize, PairingError>` (lee los 2 bytes, rechaza 0 y > 512 con `FrameTooLarge` o `Malformed` sin leer más), `read_frame_body(stream, len, deadline, step, cancel) -> Result<Frame, PairingError>` y `write_frame(stream, frame, deadline, step, cancel) -> Result<(), PairingError>`. La lectura lleva su propia posición (sin `read_exact`) y revisa el token y los plazos al menos cada `CANCEL_POLL`.

- [ ] **Step 1: Escribir las pruebas que fallan**, con un flujo guionizado que implementa `TimedStream` (entrega por trozos, devuelve `TimedOut`, se cierra o registra cuántos bytes le pidieron): `reads_frame_delivered_one_byte_at_a_time`, `reads_frame_split_across_reads`, `rejects_zero_length`, `rejects_over_512_without_reading_the_body` (el flujo falla la prueba si se le piden más de 2 bytes), `eof_mid_frame_is_closed`, `eof_before_header_is_closed`, `silent_peer_times_out_at_step` (con relojes reales y plazos de 200 ms), `slow_drip_hits_total_deadline`, `cancel_interrupts_a_blocked_read_within_poll` (otro hilo cancela a los 100 ms; vuelve `Cancelled` en menos de 1 s), `interrupted_is_retried`, `write_frame_prefixes_the_length`, `write_times_out`.
- [ ] **Step 2: Correr** `cargo test -p vault-core transport::` — Esperado: no compila.
- [ ] **Step 3: Implementar** el bucle de lectura: antes de cada `read`, `set_read_timeout(min(CANCEL_POLL, lo que falte))`; `WouldBlock`/`TimedOut` revisan token y plazos y reintentan; `Interrupted` reintenta; 0 bytes es `Closed`. La escritura, igual con `set_write_timeout`.
- [ ] **Step 4: Correr** `cargo test -p vault-core transport::` — Esperado: PASS.
- [ ] **Step 5: Commit** `feat: tramas con plazos y cancelación para el emparejamiento (#95)`

### Task 7: conductores `serve_pairing` y `pair_with_pc`

**Files:**
- Modify: `core/src/protocol/pairing.rs`, `core/src/protocol/pairing/tests.rs`, `core/src/protocol/mod.rs`

**Interfaces:**
- Consumes: Tasks 3 a 6.
- Produces:
  - `pub fn serve_pairing<S: TimedStream>(stream: S, session: &Mutex<PairingSession>, cancel: &CancelToken, timeouts: &PairingTimeouts) -> Result<PendingConnection<S>, PairingError>`.
  - `pub struct PendingConnection<S>` con `pending(&self) -> &PendingPairing`, `confirm(self) -> Result<PairedDevice, PairingError>` y `reject(self) -> Result<(), PairingError>`. `confirm` escribe la trama 4 completa **antes** de devolver el dispositivo; con `TimedOut` escribe `Rejected` (si puede) y da `ConfirmationTimeout`.
  - `pub fn pair_with_pc<S: TimedStream>(stream: S, identity: &KeyPair, offer: &PairingOffer, name: DeviceName, cancel: &CancelToken, timeouts: &PairingTimeouts) -> Result<PairedPc, PairingError>`.
  - Un `Mutex` envenenado se trata como `Closed` (sin panic).

- [ ] **Step 1: Escribir las pruebas que fallan**, con flujos guionizados en memoria (pares conectados por canales) y los plazos de la tabla del diseño: una prueba por fila de "Qué pasa en cada fallo" (`pc_closed_before_header_does_not_count`, `pc_timeout_does_not_count`, `pc_bad_length_counts_and_does_not_read_body`, `pc_wrong_code_writes_nothing`, `pc_expired_reads_first_frame_and_writes_nothing`, `pc_third_frame_failure_keeps_consumed`, `confirm_after_phone_disconnect_fails`, `confirm_writes_frame_before_returning_device`, `phone_gets_closed_for_refused_and_for_cut` (mismo error), `phone_peer_not_authentic_sends_no_third_frame`, `phone_waits_at_most_reply_wait`, `cancel_on_each_side`).
- [ ] **Step 2: Correr** `cargo test -p vault-core pairing::` — Esperado: falla por funciones inexistentes.
- [ ] **Step 3: Implementar** los conductores según el diseño: el `Mutex` se toma solo durante `accept_first` y `note_malformed`; el encabezado de la trama 1 con longitud distinta de 48 llama a `note_malformed` sin leer el cuerpo; todo camino de error suelta la conexión.
- [ ] **Step 4: Correr** `cargo test -p vault-core pairing::` — Esperado: PASS.
- [ ] **Step 5: Commit** `feat: conductores síncronos del emparejamiento (#95)`

### Task 8: pruebas de la API pública por localhost

**Files:**
- Modify: `core/tests/protocol.rs`

**Interfaces:**
- Consumes: la API pública de las Tasks 1 a 7 (`vault_core::protocol::*`).

- [ ] **Step 1: Escribir las pruebas** (cada una abre un `TcpListener` en `127.0.0.1:0`, la PC en un hilo con `serve_pairing` y el celular con `pair_with_pc`): `pairing_with_correct_code_both_know_each_other_after_confirm`, `wrong_expired_reused_and_exhausted_codes_look_the_same_to_the_phone` (el celular da `Closed` y el flujo de la PC no escribió nada), `pc_reject_gives_rejected_and_no_trust`, `no_decision_in_time_gives_no_device` (con `PairingTimeouts` cortos), `dropping_pending_connection_closes`, `phone_rejects_pc_with_other_private_key_and_sends_no_third_frame` (el servidor de prueba cuenta los bytes recibidos tras la trama 2), `altered_unencrypted_and_foreign_replies_do_not_pair` (un intermediario de prueba que reenvía y altera la trama 4), `two_concurrent_connections_only_one_consumes_the_code` (dos hilos), `silent_client_times_out_and_cancel_returns_quickly`, `connection_cut_mid_frame_does_not_burn_attempts` (después, el código correcto aún funciona), `slow_drip_hits_the_handshake_deadline`, `unpaired_device_without_code_gets_no_bytes`, `provisional_name_builds_a_new_paired_pc`.
- [ ] **Step 2: Correr** `cargo test -p vault-core --test protocol` — Esperado: PASS; las pruebas de plazos con tiempos cortos, sin esperas de más de unos segundos.
- [ ] **Step 3: Comprobar a mano que las pruebas muerden:** quitar temporalmente (cada una por separado, y restaurar) el paso a `Consumed`, el conteo de intentos y la comprobación del tipo de la respuesta; cada una debe romper al menos una prueba concreta. No se commitea ninguna mutación (`git diff` limpio al terminar).
- [ ] **Step 4: Commit** `test: pruebas del emparejamiento por localhost (#95)`

### Task 9: documentación y verificación final

**Files:**
- Modify: `docs/especificacion.md` (sección "Emparejamiento y envío": decisiones, formato, estados del código, separación entre autorización, entrega y persistencia, limitación de la aceptación perdida con su revocación, y la excepción de borrado de `snow`), `ARCHITECTURE.md` (mapa y tabla de módulos con `pairing.rs` y `transport.rs`, y la excepción (5) en la lista de borrado de memoria), `docs/roadmap.md` (marcar la #95), `core/CLAUDE.md` (las features de `snow`), `docs/superpowers/specs/2026-11-20-emparejamiento-noise-design.md` (la firma de `accept` con `Decision` y `AwaitingThird::second_message`).

- [ ] **Step 1: Actualizar** los cinco documentos, sin repetir lo ya fijado en el diseño.
- [ ] **Step 2: Correr, cada uno por separado**, y comprobar el código de salida de cada uno: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo deny check`, `cargo audit`. Esperado: todos en 0 (avisos de versiones duplicadas permitidos).
- [ ] **Step 3: Revisar** que `cargo tree -p vault-core -e normal` no trae `ring` ni AES-GCM y que `git status` solo tiene los archivos de las tareas.
- [ ] **Step 4: Commit** `docs: emparejamiento con Noise en la especificación y la arquitectura (#95)`

## Autorrevisión

- **Cobertura del diseño:** patrón y prólogo (T3, T4), estados del código y consumo (T4), nombre (T1, T2, T5), confirmación por tipos (T5, T7), formatos y tamaños (T2, T5), conductor, plazos y cancelación (T6, T7), tabla de fallos (T7, T8), limitación de la aceptación perdida (T9), excepción de borrado y dependencia (T3, T9), nombre provisional de la PC (T3, T8).
- **Detalle que el diseño dejaba abierto y este plan fija:** `AwaitingThird::second_message` y `Decision` (se anotan en el diseño en la Task 9).
- **No incluido, por diseño:** servidor real, mDNS, límite de conexiones sin autenticar (#100); QR (#101, #103); temporizador del diálogo (#101); envío (#96).
