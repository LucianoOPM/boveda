# Envío con Noise IK (#96) — plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Un celular emparejado envía una contraseña a la PC por Noise IK con plazo de 30 s y confirmación, y la PC rechaza en el apretón de manos, sin escribir nada, a cualquier dispositivo no emparejado; probado con ambos extremos por `TcpStream` en `127.0.0.1`.

**Architecture:** `protocol/send.rs` tiene las máquinas de estado por tipos (sin sockets: `Responder` en la PC y `Sender` en el celular) y los conductores síncronos `receive_send` y `send_to_pc`. `protocol/transport.rs` conserva tramas, plazos y cancelación, ahora con máximo de trama configurable (512 en el emparejamiento, 2048 en el envío). Los mensajes `SendMessage` y `Ack` ya existen (#94): no hay mensajes nuevos ni cambio de versión.

**Tech Stack:** Rust, `snow` 0.10 (ya en el núcleo, sin dependencias nuevas), `secrecy`, `zeroize`, `thiserror`. Sin tokio ni crates de red.

**Spec:** `docs/superpowers/specs/2026-11-20-envio-noise-design.md` (los dos documentos se leen juntos; en caso de duda manda la spec).

## Global Constraints

- Identificadores en inglés; comentarios, documentación y commits en español, Conventional Commits con `(#96)`.
- Rama `feat/96-envio-noise` desde `feat/95-emparejamiento`; el PR contra `dev` se abre cuando la #95 esté fusionada, sin repetir sus cambios.
- Pruebas primero: cada función nueva empieza con una prueba que se ve fallar por la razón esperada (`core/CLAUDE.md`). Unitarias en `modulo/tests.rs` (`modulo.rs` termina con `#[cfg(test)] mod tests;`); de la API pública, en `core/tests/protocol.rs`.
- **Sin dependencias nuevas** ni mecanismos criptográficos adicionales (ni ids de mensaje, ni caché de repeticiones, ni marcas de hora, ni PSK). Solo `snow`.
- Patrón `Noise_IK_25519_ChaChaPoly_BLAKE2s`; prólogo `boveda/envio/v1`; tramas 1 de 96 bytes y 2 de 48; trama 3 (envío) de hasta 1045; trama 4 (`Ack`) de 19; `MAX_FRAME_LEN` sigue en 512 y `MAX_SEND_FRAME_LEN` es 2048; `SEND_TTL` 30 s; `PROTOCOL_VERSION` sigue en 1.
- Plazos por defecto (`SendTimeouts`): 10 s por trama, 30 s en total, 10 s del celular para el `Ack`. `CANCEL_POLL` 250 ms (sin cambios).
- La PC consulta `is_paired` **después** de autenticar la trama 1 y con la llave que recupera `snow` (`get_remote_static`), antes de escribir la trama 2. No emparejado: cierra sin escribir nada. Las tramas 1 y 2 no llevan datos de aplicación.
- El `SendMessage` solo viaja tras completar el apretón de manos. Una conexión acepta un solo envío: tras el `Ack`, la PC cierra sin leer más.
- Caducidad: 30 s con `Instant` desde que la PC acepta la conexión; si vence antes de tener el mensaje completo y autenticado, `Expired` y el secreto no llega a la app. No se afirma nada sobre cuándo se generó la contraseña.
- El núcleo no toca el portapapeles ni persiste la lista de emparejados.
- Ningún secreto en logs, errores, panics ni `Debug`; `SendError` no lleva datos ni texto de `snow`. La API pública no hace panic (sin `unwrap`/`expect` fuera de pruebas). Los búferes que reciben la contraseña en claro van en `Zeroizing`. Las llaves se pasan a `snow` como `&[u8]` desde su `SecretBox`.
- No tocar el formato de la bóveda, `vault::Error` ni `mobile/rust`.
- Antes de cada commit: `cargo test -p vault-core` en verde. Nunca encadenar `fmt`, `clippy` y `commit` con `;` ni con `| grep`: correr cada uno aparte y comprobar su código de salida.
- No hay Python: los scripts auxiliares, si hacen falta, en Node o bash, dentro del scratchpad.

## Review Focus

- Una llave no emparejada: ni un byte de la PC y `is_paired` recibe la llave recuperada por `snow` (Task 3 y 6).
- La trama 1 reenviada de una captura y una trama 3 de **otra** sesión: nunca producen un `SendMessage` (Task 4 y 6).
- Una trama 3 que llega justo en los 30 s o después, aunque descifre bien: `Expired` sin entregar el mensaje (Task 4 y 6).
- Una trama de largo 0, de 2049 o declarada y seguida de silencio: sin reservar memoria y sin colgarse (Task 1 y 6).
- Una PC con otra llave privada o una trama 2 falsa: el celular no manda la contraseña (Task 3 y 6).

---

### Task 0: Rama y documentos de diseño

**Files:**
- Create (ya escritos): `docs/superpowers/specs/2026-11-20-envio-noise-design.md`, `docs/superpowers/plans/2026-11-20-envio-noise.md`

- [ ] **Step 1:** `git switch -c feat/96-envio-noise` (desde `feat/95-emparejamiento`; el árbol está limpio salvo estos dos documentos).
- [ ] **Step 2: Commit** `docs: diseño y plan del envío con Noise IK (#96)`

### Task 1: Máximo de trama configurable en `transport.rs`

**Files:**
- Modify: `core/src/protocol/transport.rs`, `core/src/protocol/pairing.rs` (llamadas), `core/src/protocol/mod.rs`
- Test: `core/src/protocol/transport/tests.rs`

**Interfaces:**
- Produces: `pub const MAX_SEND_FRAME_LEN: usize = 2048` (en `transport.rs`, reexportada). `read_frame_body`, `write_frame` y `check_len` reciben un `max: usize` extra (antes del plazo en las dos primeras); `read_frame_header` no cambia.

- [ ] **Step 1: Escribir las pruebas que fallan** en `transport/tests.rs`: `accepts_a_2048_frame_with_the_send_limit` (cuerpo de 2048 con `max = MAX_SEND_FRAME_LEN`: `Ok`), `rejects_2049_without_reading_the_body_with_the_send_limit` (el lector falla si se le piden más de 2 bytes, como la prueba existente de 512), `pairing_limit_stays_at_512` (513 con `max = MAX_FRAME_LEN`: `FrameTooLarge`) y `write_frame_honors_the_given_max`. Adaptar las pruebas existentes a la firma nueva con `MAX_FRAME_LEN`.
- [ ] **Step 2: Correr** `cargo test -p vault-core transport` — Esperado: no compila.
- [ ] **Step 3: Implementar** el parámetro `max` en `check_len`, `read_frame_body` y `write_frame`; actualizar las llamadas de `pairing.rs` con `MAX_FRAME_LEN` (comportamiento idéntico). Reexportar `MAX_SEND_FRAME_LEN` desde `mod.rs`.
- [ ] **Step 4: Correr** `cargo test -p vault-core` — Esperado: PASS completo (el emparejamiento no cambia).
- [ ] **Step 5: Commit** `refactor: máximo de trama configurable en el transporte (#96)`

### Task 2: `SendError`, constantes y `SendTimeouts`

**Files:**
- Create: `core/src/protocol/send.rs`, `core/src/protocol/send/tests.rs`
- Modify: `core/src/protocol/mod.rs` (`mod send;` y reexportar)

**Interfaces:**
- Produces: `pub const SEND_PATTERN: &str = "Noise_IK_25519_ChaChaPoly_BLAKE2s"`, `pub const SEND_PROLOGUE: &[u8] = b"boveda/envio/v1"`, `pub const SEND_TTL: Duration` (30 s), `pub const SEND_FIRST_FRAME_LEN: usize = 96`, `pub const SEND_SECOND_FRAME_LEN: usize = 48`.
- Produces: `#[non_exhaustive] pub enum SendError { NotPaired, Expired, PeerNotAuthentic, Timeout, Cancelled, Closed, Io(std::io::ErrorKind), FrameTooLarge, Handshake, Protocol(ProtocolError) }` (`Debug, Clone, Copy, PartialEq, Eq, thiserror::Error`; mensajes en español, sin datos) con `From<ProtocolError>` y `From<PairingError>` (`Timeout`, `Cancelled`, `Closed`, `Io`, `FrameTooLarge` y `Protocol` pasan igual; `PeerNotAuthentic` también; el resto, `Handshake`).
- Produces: `pub struct SendTimeouts { pub step: Duration, pub total: Duration, pub ack_wait: Duration }` con `Default` 10 s / `SEND_TTL` / 10 s.

- [ ] **Step 1: Escribir las pruebas que fallan**: `send_timeouts_default_match_the_spec`, `send_error_from_pairing_error_maps_each_variant`, `send_error_messages_carry_no_data` (el `Display` y el `Debug` de cada variante no contienen ningún byte de entrada: se construyen con valores conocidos y se comprueba que no aparecen).
- [ ] **Step 2: Correr** `cargo test -p vault-core send::` — Esperado: no compila.
- [ ] **Step 3: Implementar** lo anterior; `noise_params()` privado que parsea `SEND_PATTERN` y devuelve `SendError::Handshake` si falla (sin panic).
- [ ] **Step 4: Correr** `cargo test -p vault-core send::` — Esperado: PASS.
- [ ] **Step 5: Commit** `feat: errores, constantes y plazos del envío (#96)`

### Task 3: Apretón de manos IK y autorización (tramas 1 y 2)

**Files:**
- Modify: `core/src/protocol/send.rs`
- Test: `core/src/protocol/send/tests.rs`

**Interfaces:**
- Consumes: `KeyPair`, `PublicKey` (`identity.rs`), `SendError`, constantes de la Task 2, `Frame` (`pairing.rs`), `MAX_SEND_FRAME_LEN` como tamaño de los búferes de trabajo.
- Produces: `Responder::new(identity: &KeyPair) -> Responder`; `Responder::read_first(self, now: Instant, frame: &[u8], is_paired: impl Fn(&PublicKey) -> bool) -> Result<(AwaitingSend, Frame), SendError>`; `Sender::new(identity: &KeyPair, pc_key: &PublicKey) -> Result<Sender, SendError>`; `Sender::first_message(self) -> Result<(AwaitingResponse, Frame), SendError>`; `AwaitingSend` (con `peer() -> PublicKey` y el `deadline: Instant = now + SEND_TTL`) y `AwaitingResponse`. Los pasos de transporte (`read_send`, `read_second`) llegan en la Task 4.

- [ ] **Step 1: Escribir las pruebas que fallan** (claves `[1;32]` PC y `[2;32]` celular, como el emparejamiento):
  - `first_frame_is_96_bytes_and_second_is_48`.
  - `paired_phone_gets_a_second_frame`: `is_paired` recibe exactamente `phone.public()` y devuelve `true`.
  - `unpaired_phone_gets_no_second_frame`: `is_paired` devuelve `false` → `Err(SendError::NotPaired)`, y la prueba comprueba que `is_paired` se llamó **una** vez (con un `Cell`) y con la llave recuperada por `snow`, no con la del celular declarada por otro canal.
  - `first_frame_for_another_pc_key_is_handshake_error` (el celular cifra hacia la llave 3; la PC 1 no descifra: `Handshake`, y `is_paired` **no** se llama).
  - `first_frame_with_wrong_length_or_garbage_is_handshake_error` (95, 97 bytes y 96 de ceros).
  - `first_frame_with_application_payload_is_handshake_error` (trama 1 armada a mano con `snow` y una carga de 1 byte).
  - `second_frame_from_another_key_is_peer_not_authentic` (una PC con la llave 3 contesta; el celular no produce la trama 3: `read_second` aún no existe, así que esta prueba se completa en la Task 4).
- [ ] **Step 2: Correr** `cargo test -p vault-core send::` — Esperado: no compila.
- [ ] **Step 3: Implementar** con `snow::Builder`: iniciador con `local_private_key`, `remote_public_key(pc_key)` y `prologue(SEND_PROLOGUE)`; respondedor con `local_private_key` y el prólogo. `read_first` verifica largo 96, `read_message` con carga vacía, obtiene la llave con `get_remote_static()`, consulta `is_paired` y **solo si es `true`** escribe la trama 2 (carga vacía). Pasar las llaves como `&[u8]` desde el `SecretBox`; ningún error lleva texto de `snow`.
- [ ] **Step 4: Correr** `cargo test -p vault-core send::` — Esperado: PASS.
- [ ] **Step 5: Commit** `feat: apretón de manos IK y autorización de la llave del celular (#96)`

### Task 4: Envío y confirmación por el canal de transporte (tramas 3 y 4)

**Files:**
- Modify: `core/src/protocol/send.rs`
- Test: `core/src/protocol/send/tests.rs`

**Interfaces:**
- Consumes: Task 3, `SendMessage::{new, encode, decode, password}`, `Ack::{new, encode, decode, outcome}`, `AckOutcome`.
- Produces: `AwaitingResponse::read_second(self, frame: &[u8], message: &SendMessage) -> Result<(AwaitingAck, Frame), SendError>` (trama 3; `PeerNotAuthentic` si la 2 no se autentica, sin producir trama 3); `AwaitingAck::read_ack(self, frame: &[u8]) -> Result<AckOutcome, SendError>`; `AwaitingSend::read_send(self, now: Instant, frame: &[u8]) -> Result<ReceivedSend, SendError>`; `ReceivedSend::{ peer() -> PublicKey, message() -> &SendMessage, ack(self, outcome: AckOutcome) -> Result<Frame, SendError> }`.

- [ ] **Step 1: Escribir las pruebas que fallan**:
  - `full_exchange_in_memory_delivers_the_password_and_copied` (la contraseña llega igual; `ack(Copied)` y `read_ack` da `Copied`).
  - `ack_failed_roundtrips`.
  - `send_frame_with_a_1024_byte_password_is_1045_bytes_and_fits_2048`; `ack_frame_is_19_bytes`.
  - `second_frame_from_a_pc_with_another_key_is_peer_not_authentic_and_sends_no_third_frame` (completa la prueba de la Task 3).
  - `send_just_before_the_deadline_is_accepted_and_at_the_deadline_expired` (`now` inyectado: `deadline - 1 ns` y `deadline`; con `Expired` no hay `ReceivedSend`).
  - `replayed_first_frame_gets_a_second_frame_but_no_valid_send`: la misma trama 1 en dos `Responder` distintos produce dos tramas 2 **distintas**; un atacante que no tiene la llave privada del celular no puede armar la trama 3 (se le da una trama 3 de la sesión 1 a la sesión 2: `Handshake`, sin mensaje).
  - `send_frame_from_another_session_is_rejected` (mismas llaves, otra efímera).
  - `altered_send_frame_is_rejected` (un bit cambiado en cada posición de una muestra: primer byte, mitad, etiqueta).
  - `unencrypted_or_empty_send_frame_is_rejected`.
  - `altered_or_foreign_ack_is_rejected`.
  - `second_send_frame_after_the_first_is_not_part_of_the_api` — documentada en un comentario: tras `read_send`, `AwaitingSend` ya no existe (se consumió), así que no hay forma de leer una segunda trama.
  - `send_debug_and_errors_do_not_show_the_password` (contraseña centinela; `format!("{:?}")` de `ReceivedSend` y de cada error no la contiene).
- [ ] **Step 2: Correr** `cargo test -p vault-core send::` — Esperado: no compila.
- [ ] **Step 3: Implementar** con `into_transport_mode` tras la trama 2 en cada extremo. La trama 3 se cifra con `TransportState::write_message` sobre el resultado de `message.encode()` (ya `Zeroizing`) hacia un búfer `Zeroizing`; en la PC se descifra a un `Zeroizing<Vec<u8>>` de 2048 y se decodifica con `SendMessage::decode`; `read_send` comprueba `now >= deadline` **después** de decodificar y, si venció, deja caer el mensaje y da `Expired`. `ReceivedSend` no deriva `Debug` con el valor (solo la huella del celular). Antes de implementar, **leer `resolvers/default.rs` de `snow` 0.10.0** (en `~/.cargo/registry/src/`) para saber si el cifrado de transporte deja una copia del texto plano en su estado; anotarlo en el diseño (sección "Borrado de memoria") sea cual sea el resultado.
- [ ] **Step 4: Correr** `cargo test -p vault-core send::` — Esperado: PASS.
- [ ] **Step 5: Commit** `feat: envío y confirmación por el canal Noise con caducidad (#96)`

### Task 5: Conductores síncronos

**Files:**
- Modify: `core/src/protocol/send.rs`, `core/src/protocol/mod.rs` (reexportar el API público: `receive_send`, `send_to_pc`, `IncomingSend`, `SendError`, `SendTimeouts`, constantes)
- Test: `core/src/protocol/send/tests.rs` (con un `Script` o un par de `TcpStream` locales, como `transport/tests.rs`)

**Interfaces:**
- Consumes: Tasks 1 a 4, `TimedStream`, `CancelToken`, `read_frame_header`, `read_frame_body`, `write_frame`.
- Produces: `pub fn receive_send<S: TimedStream>(stream: S, identity: &KeyPair, is_paired: impl Fn(&PublicKey) -> bool, cancel: &CancelToken, timeouts: &SendTimeouts) -> Result<IncomingSend<S>, SendError>`; `IncomingSend<S>::{ peer() -> PublicKey, message() -> &SendMessage, copied(self) -> Result<(), SendError>, failed(self) -> Result<(), SendError> }`; `pub fn send_to_pc<S: TimedStream>(stream: S, identity: &KeyPair, pc_key: &PublicKey, message: &SendMessage, cancel: &CancelToken, timeouts: &SendTimeouts) -> Result<AckOutcome, SendError>`.

- [ ] **Step 1: Escribir las pruebas que fallan** (sobre `TcpStream` local o `Script`): `receive_send_and_send_to_pc_complete`, `unpaired_phone_gets_zero_bytes_from_the_pc`, `first_frame_of_wrong_length_is_rejected_without_reading_the_body`, `step_timeout_before_the_deadline_is_timeout`, `total_deadline_passed_is_expired_and_delivers_nothing`, `cancel_returns_cancelled_quickly_on_both_sides`, `dropping_incoming_send_closes_without_ack` (el celular da `Closed`), `copied_and_failed_write_the_ack_and_close`, `a_second_frame_after_the_ack_is_never_read`.
- [ ] **Step 2: Correr** `cargo test -p vault-core send::` — Esperado: no compila.
- [ ] **Step 3: Implementar.** `receive_send`: `deadline = Instant::now() + timeouts.total`; leer solo el encabezado de la trama 1 y exigir 96 **antes** de leer el cuerpo; `Responder::read_first`; `write_frame` de la trama 2 solo si está emparejada; leer la trama 3 con `MAX_SEND_FRAME_LEN`; ante `Timeout`, si `Instant::now() >= deadline` es `Expired`. `IncomingSend` guarda `stream`, `ReceivedSend`, `step` y `cancel`; `copied`/`failed` escriben la trama 4 y sueltan la conexión. `send_to_pc`: trama 1, leer la 2 (`read_second` con `message`), escribir la 3, esperar la 4 con `timeouts.ack_wait`; `Timeout` pasa tal cual.
- [ ] **Step 4: Correr** `cargo test -p vault-core send::` — Esperado: PASS.
- [ ] **Step 5: Commit** `feat: conductores síncronos del envío (#96)`

### Task 6: Pruebas de la API pública por localhost

**Files:**
- Modify: `core/tests/protocol.rs` (sección nueva "Envío con Noise (#96)", reutilizando `secret_keys`, `framed`, `read_until_close` y `read_exact_frame`)

**Interfaces:**
- Consumes: solo la API pública de la Task 5. Un `Pc` de pruebas con `TcpListener` en `127.0.0.1:0`, `serve(is_paired, timeouts)` en otro hilo y `phone(message, timeouts)`.

- [ ] **Step 1: Escribir las pruebas** (cada una falla antes de la Task 5 por no compilar; después deben pasar sin cambiar código): `send_succeeds_and_phone_gets_copied`, `send_with_a_1024_byte_password_succeeds`, `unpaired_phone_gets_closed_and_the_pc_writes_nothing`, `replayed_first_frame_gets_a_reply_but_never_a_message`, `send_frame_from_another_session_delivers_nothing`, `late_send_expires_with_short_timeouts_and_delivers_nothing`, `oversized_and_zero_length_frames_are_rejected`, `altered_send_frame_delivers_nothing`, `phone_disconnecting_mid_send_closes_the_pc`, `pc_disconnecting_mid_send_closes_the_phone`, `ack_failed_reaches_the_phone`, `second_send_in_the_same_connection_is_not_processed`, `pc_with_another_private_key_never_gets_the_password`, `forged_second_frame_is_peer_not_authentic_and_phone_sends_nothing_more`, `cancel_is_effective_on_both_sides`, `password_never_appears_in_debug_or_errors`.
- [ ] **Step 2: Correr** `cargo test -p vault-core --test protocol` — Esperado: PASS (si una falla, el código de las tasks anteriores tiene un hueco: corregirlo ahí, con su prueba unitaria).
- [ ] **Step 3: Commit** `test: pruebas del envío por localhost (#96)`

### Task 7: Documentación

**Files:**
- Modify: `docs/especificacion.md` (subsección "Envío con Noise" tras "Emparejamiento con Noise"), `ARCHITECTURE.md` (árbol: `send.rs`; fila de `protocol/` en la tabla de módulos; fila de decisiones "Envío: IK, un solo envío y plazo de 30 s sin marcas de hora"; excepción (5): "#95 y #96" y lo que se averiguó en la Task 4), `docs/roadmap.md` (casilla de la #96 bajo "Protocolo en `vault-core`")

- [ ] **Step 1:** Copiar a `especificacion.md` las decisiones 1 a 6, el cuadro de tamaños, el de fallos y la limitación de los 30 s (solo limita la recepción; no hay marcas de hora) de la spec; marcar en la #94 que el envío "se rechaza en el apretón de manos (#96)" ya está hecho.
- [ ] **Step 2:** Actualizar `ARCHITECTURE.md` y `roadmap.md`.
- [ ] **Step 3: Commit** `docs: envío con Noise IK en la especificación y la arquitectura (#96)`

### Task 8: Verificación final

- [ ] **Step 1:** Cada comando aparte, comprobando su código de salida: `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`, `cargo deny check`, `cargo audit`.
- [ ] **Step 2: Mutaciones** (con el árbol limpio, y revirtiendo cada una antes de la siguiente): (a) quitar la consulta a `is_paired`; (b) escribir la trama 2 antes de consultarla; (c) no comprobar `now >= deadline` en `read_send`; (d) dejar el máximo de trama en 512. Cada una debe hacer fallar al menos una prueba **por la razón esperada**; si pasa en verde, la prueba es insuficiente.
- [ ] **Step 3:** Revisar el diseño: cada criterio de aceptación de la issue tiene su prueba, y la sección "Borrado de memoria" refleja lo que se averiguó. Marcar las casillas de la spec.
- [ ] **Step 4:** Revisión final de la rama y PR contra `dev` (cuando la #95 esté fusionada) con el formato del PR #11; sin esperar la CI.
