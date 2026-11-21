# Fachada del protocolo (#102) — Plan de implementación

> **Para quien ejecute:** SUB-SKILL REQUERIDA: `superpowers:executing-plans` (o `superpowers:subagent-driven-development`, según decida el usuario). Los pasos usan casillas `- [ ]`. Cada tarea termina con pruebas y lint en verde y un commit pequeño.

**Objetivo:** `mobile/rust` expone a Dart la identidad del celular, emparejar con el texto de un QR, enviar una entrada a una PC y listar o quitar PCs, sin que una operación de red retenga el candado de la bóveda y con el bloqueo como prioridad absoluta.

**Arquitectura:** una fachada delgada sobre `vault-core`. Un `NetState` (época, `CancelToken`, operación activa) vive junto al `Mutex<Option<Vault>>`; `lock()` e `import_backup` lo invalidan con los dos candados tomados (orden `vault` → `net`); cada operación de red extrae sus datos bajo `vault`, suelta el candado, conecta y vuelve a comprobar la época antes de guardar o devolver. El núcleo gana una sola función: `PairingOffer::decode_qr_text`.

**Tecnologías:** Rust (`vault-core`, `flutter_rust_bridge` 2.13.0, `secrecy`, `zeroize`, solo `std` para la conexión), Android (`INTERNET`).

**Diseño:** `docs/superpowers/specs/2026-11-20-fachada-protocolo-design.md` (léelo antes de empezar; este plan solo fija lo que el diseño deja a quien construye).

## Restricciones globales

- Código y nombres en inglés; comentarios, documentación y mensajes de commit en español. Commits Conventional Commits, pequeños.
- Antes de cada commit: `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` y `cargo fmt --all`, **cada uno por separado y comprobando su código de salida** (nunca encadenar con `|` ni `;` antes de un commit).
- Dependencias: **ninguna nueva** (ni de red ni criptográfica). La conexión usa `std::net::TcpStream::connect_timeout`. No se agrega `mdns-sd`. Si algo parece necesitar una, se pregunta.
- Toda la criptografía y la lógica del protocolo viven en `vault-core`; `mobile/rust` solo convierte tipos y llama.
- Ninguna función de `mobile/rust/src/api/` lleva `#[frb(sync)]`.
- Nada de secretos en logs, errores, `Debug` ni valores devueltos a Dart. `NewPairedPc`, `PairedPc` y `PairingOffer` ya ocultan su contenido; los tipos nuevos de cruce no derivan `Debug` con datos sensibles.
- **Orden de candados: siempre `vault` y después `net`.** Nadie toma `vault` mientras conserva `net`. La red nunca corre con ninguno de los dos tomados.
- Valores fijos: `CONNECT_TIMEOUT` = 5 s; plazos del núcleo `PairingTimeouts::default()` y `SendTimeouts::default()`; huella `XXXX-XXXX-XXXX-XXXX` (`Fingerprint`); ids en 32 caracteres hexadecimales en minúsculas.
- Cualquier cambio en `mobile/rust/src/api/` (hasta un comentario `///`) exige regenerar el puente antes del commit: desde `mobile/`, `flutter_rust_bridge_codegen generate` (necesita `flutter_rust_bridge_codegen` 2.13.0 y `cargo-expand` 1.0.127; si faltan, **detenerse y avisar al usuario**: instalar herramientas exige su visto bueno). Si cambias un nombre en `api/`, regenera antes de `cargo test -p rust_lib_boveda`. Los `.dart` de un archivo eliminado de `api/` se borran a mano.
- Las pruebas de la fachada usan siempre copias en un directorio temporal de las bóvedas de `core/tests/fixtures/`, nunca los originales, y servidores en `127.0.0.1` con puerto asignado por el sistema. Toda espera de una prueba tiene tope (nada de bloqueos indefinidos).
- No se lanzan ejecutables de la app ni se toca el celular o el emulador sin avisar al usuario; las pruebas manuales se declaran pendientes de él.

## Review Focus

Condiciones que el diseño implica y que ninguna tarea cubriría por sí sola; cada una tiene su prueba en la tarea indicada:

1. Texto del QR con salto de línea o espacio al final (lo que añaden algunos lectores): se rechaza con `InvalidPairingOffer`, sin pánico y sin tocar la red (Tarea 1 y Tarea 5; el recorte es de la #103).
2. Direcciones de envío raras (`"[fe80::1%5]:47615"` con zona, puerto 0, `"pc.local:47615"`, texto vacío): `InvalidAddress`, sin pánico, sin conectar y con `Busy` libre (Tarea 6).
3. Enviar a una PC que ya se quitó (id válido, ya no está): `PairedPcNotFound`, sin intento de conexión y con `Busy` libre (Tarea 6).
4. Una PC que contesta con una trama gigante (longitud 65535) o basura y otra que se queda muda: `ProtocolFailure`/`NetworkTimeout`, sin colgarse, y `Busy` libre después (Tareas 5 y 6).
5. El mismo QR escaneado dos veces (código ya consumido): la segunda da `ConnectionClosed` y no duplica ni cambia la PC guardada (Tarea 5).

## Mapa de archivos

| Archivo | Responsabilidad |
| --- | --- |
| `core/src/protocol/message.rs` (+ `message/tests.rs`) | `PairingOffer::decode_qr_text` |
| `core/tests/protocol.rs` | prueba de la API pública de `decode_qr_text` |
| `desktop/src-tauri/src/qr/tests.rs` | cruce: `offer_text` de la PC se decodifica con `decode_qr_text` |
| `mobile/rust/src/api/error.rs` (+ `error/tests.rs`) | variantes nuevas de `VaultError`, mapeo definitivo de las PCs y de `PairingError`/`SendError` |
| `mobile/rust/src/api/net.rs` (+ `net/tests.rs`) | `NetState`, `OpGuard`, `begin_op`, invalidación, `connect` |
| `mobile/rust/src/api/session.rs` | campo `net`; `lock()` con los dos candados |
| `mobile/rust/src/api/backup.rs` | `import_backup` invalida la época |
| `mobile/rust/src/api/entries.rs` | ayudantes de ids hexadecimales compartidos |
| `mobile/rust/src/api/pcs.rs` (+ `pcs/tests.rs`) | `ensure_identity`, `paired_pcs`, `remove_paired_pc` y sus tipos |
| `mobile/rust/src/api/pairing.rs` (+ `pairing/tests.rs`) | `pair` |
| `mobile/rust/src/api/sending.rs` (+ `sending/tests.rs`) | `send_to_pc` |
| `mobile/rust/src/test_support.rs` | copia del fixture v3 y `FakePc` (servidor de pruebas) |
| `mobile/rust/src/frb_generated.rs`, `mobile/lib/src/rust/` | generados |
| `mobile/android/app/src/main/AndroidManifest.xml` | `INTERNET` |
| `ARCHITECTURE.md`, `docs/especificacion.md`, `docs/roadmap.md`, `mobile/CLAUDE.md` | documentación |

---

### Task 1: Núcleo — `PairingOffer::decode_qr_text`

**Files:**
- Modify: `core/src/protocol/message.rs` (junto a `PairingOffer::decode`), `core/src/protocol/message/tests.rs`
- Modify: `core/tests/protocol.rs` (prueba de la API pública)
- Modify: `desktop/src-tauri/src/qr/tests.rs` (cruce con `offer_text`)

**Interfaces:**
- Produces: `impl PairingOffer { pub fn decode_qr_text(text: &str) -> Result<PairingOffer> }` (con `Result` = `protocol::Result`).

Reglas fijas (del diseño): base64url **sin relleno**, alfabeto `A-Z a-z 0-9 - _`; ni espacios, ni saltos de línea, ni `=`, ni `+` ni `/`; tope de largo `(MAX_PAIRING_OFFER_LEN * 4).div_ceil(3)` caracteres comprobado **antes** de reservar (`ProtocolError::TooLarge`); largo módulo 4 igual a 1, carácter fuera del alfabeto o bits sobrantes del último carácter distintos de cero → `ProtocolError::Malformed`; decodifica a un búfer `Zeroizing<Vec<u8>>` con capacidad exacta y delega en `PairingOffer::decode` (que aporta el resto de errores). Sin dependencias nuevas; la decodificación se escribe a mano, como el codificador de `desktop/src-tauri/src/qr.rs` (`ALPHABET`).

- [ ] **Step 1: Escribe las pruebas que fallan** en `message/tests.rs`, con un codificador de prueba local (copia del de `qr.rs`) llamado `qr_text(&PairingOffer) -> String`:
  - `qr_text_roundtrip`: oferta con IPv4 y con IPv6 → `decode_qr_text(&qr_text(&o))` devuelve la misma llave, dirección y código.
  - `qr_text_rejects_padding_whitespace_and_foreign_characters`: añadir `=`, `\n`, `" "`, `+`, `/` o un carácter no ASCII → `Err(Malformed)`.
  - `qr_text_rejects_length_mod_4_equal_1` → `Malformed`.
  - `qr_text_rejects_non_canonical_trailing_bits`: cambiar el último carácter de un texto cuya longitud no es múltiplo de 4 por otro que deje bits sobrantes distintos de cero → `Malformed`.
  - `qr_text_too_long_is_rejected_before_decoding`: texto de tope + 1 caracteres → `Err(TooLarge)`.
  - `qr_text_of_other_message_kind_is_unexpected_kind`: el texto de un `Ack` codificado da `Err(UnexpectedKind(3))`.
  - `qr_text_empty_is_truncated_or_malformed`: `""` no entra en pánico y es `Err(_)`.
- [ ] **Step 2: Comprueba que fallan:** `cargo test -p vault-core --lib protocol::message` → no compila (`decode_qr_text` no existe).
- [ ] **Step 3: Implementa** `decode_qr_text` en `message.rs` (con su documentación: formato, que **no** recorta espacios y por qué —un QR válido no los lleva; recortar lo que añada el lector es de la #103— y que el código viaja en el texto).
- [ ] **Step 4: Comprueba que pasan:** misma orden; todas verdes.
- [ ] **Step 5: Prueba de la API pública** en `core/tests/protocol.rs`: `qr_text_roundtrip_through_public_api` (oferta generada con `KeyPair::generate` y `PairingCode::generate`, texto armado con un codificador local) y `qr_text_never_panics_on_arbitrary_input` con `proptest` (cadenas arbitrarias: `decode_qr_text` devuelve `Ok` o `Err`, nunca pánico).
- [ ] **Step 6: Cruce con la PC** en `desktop/src-tauri/src/qr/tests.rs`: `offer_text_decodes_with_the_core` (`offer_text(&offer)` → `PairingOffer::decode_qr_text` devuelve la misma llave y dirección). Ejecuta `cargo test -p <paquete de desktop> qr`.
- [ ] **Step 7: Mutación.** Cambia el alfabeto de `-` y `_` por `+` y `/` en `decode_qr_text` y comprueba que `qr_text_roundtrip` o el cruce con la PC fallan; restaura (`git diff` limpio sobre el archivo antes de seguir).
- [ ] **Step 8: Verificación y commit.** `cargo test --workspace`, `clippy` y `fmt` (cada uno aparte, con su código de salida). Commit: `feat: decodificar el texto del QR de emparejamiento en el núcleo (#102)`.

---

### Task 2: Errores de la fachada

**Files:**
- Modify: `mobile/rust/src/api/error.rs`, `mobile/rust/src/api/error/tests.rs`
- Regenerate: `mobile/rust/src/frb_generated.rs`, `mobile/lib/src/rust/`

**Interfaces:**
- Produces: variantes nuevas de `VaultError` (sin datos, todas `Copy`):
  - Estado: `Busy`, `Cancelled`.
  - Entrada: `InvalidPairingOffer`, `InvalidDeviceName`, `InvalidAddress`, `InvalidPairedPcId`.
  - PCs: `CorruptIdentity`, `CorruptPairedPc`, `PairedPcNotFound`, `PairedPcExists`, `InvalidPairedPcNameEmpty`, `InvalidPairedPcNameTooLong`, `InvalidPairedPcOther`.
  - Red: `ConnectFailed`, `NetworkTimeout`, `ConnectionClosed`, `NetworkIo`, `PeerNotAuthentic`, `PairingRejected`, `PairingConfirmationTimeout`, `SendExpired`, `ProtocolFailure`.
- Produces: `impl From<vault_core::protocol::PairingError> for VaultError` y `impl From<vault_core::protocol::SendError> for VaultError`.

Mapeo fijo:

| `PairingError` | `VaultError` | `SendError` | `VaultError` |
| --- | --- | --- | --- |
| `Refused(_)` | `ProtocolFailure` | `NotPaired` | `ProtocolFailure` |
| `PeerNotAuthentic` | `PeerNotAuthentic` | `Expired` | `SendExpired` |
| `Rejected` | `PairingRejected` | `PeerNotAuthentic` | `PeerNotAuthentic` |
| `ConfirmationTimeout` | `PairingConfirmationTimeout` | `Timeout` | `NetworkTimeout` |
| `Timeout` | `NetworkTimeout` | `Cancelled` | `Cancelled` |
| `Cancelled` | `Cancelled` | `Closed` | `ConnectionClosed` |
| `Closed` | `ConnectionClosed` | `Io(_)` | `NetworkIo` |
| `Io(_)` | `NetworkIo` | `FrameTooLarge`, `Handshake`, `Protocol(_)`, `_` | `ProtocolFailure` |
| `FrameTooLarge`, `Handshake`, `Protocol(_)`, `_` | `ProtocolFailure` | | |

Las dos enumeraciones del núcleo son `#[non_exhaustive]`: el brazo `_` es obligatorio. El mapeo del núcleo `vault::Error` **sí es exhaustivo** y deja de ser provisional: `CorruptIdentity → CorruptIdentity`, `CorruptPairedPc → CorruptPairedPc`, `PairedPcNotFound → PairedPcNotFound`, `PairedPcExists → PairedPcExists`, `InvalidPairedPc(NameEmpty) → InvalidPairedPcNameEmpty`, `InvalidPairedPc(NameTooLong) → InvalidPairedPcNameTooLong`, otro campo (`_`) `→ InvalidPairedPcOther`. Quita el comentario «provisional de la #93».

- [ ] **Step 1: Pruebas que fallan** en `error/tests.rs`: actualizar `maps_every_core_error` con el mapeo definitivo de las PCs; agregar `maps_every_pairing_error` y `maps_every_send_error` con una fila por cada variante de la tabla (incluidos `Io(ErrorKind::TimedOut)` y `Protocol(ProtocolError::Malformed)`).
- [ ] **Step 2: Comprueba que fallan:** `cargo test -p rust_lib_boveda error` → no compila.
- [ ] **Step 3: Implementa** las variantes (con una línea de documentación cada una; `ConnectionClosed` dice que no distingue un corte de red de «la PC no reconoce al celular») y los mapeos.
- [ ] **Step 4: Regenera el puente** (`flutter_rust_bridge_codegen generate` desde `mobile/`) y comprueba que `lib/src/rust/api/error.dart` lista las variantes nuevas. Ejecuta `flutter analyze` (desde `mobile/`) y, si hay pruebas de Dart que enumeran `VaultError` con `switch` exhaustivo, ajústalas.
- [ ] **Step 5: Verificación** (`cargo test -p rust_lib_boveda`, `cargo test --workspace`, `clippy`, `fmt`, `flutter test` desde `mobile/`; cada uno aparte). Commit: `feat: errores de la fachada para el protocolo (#102)`.

---

### Task 3: Estado de red, época y bloqueo

**Files:**
- Create: `mobile/rust/src/api/net.rs`, `mobile/rust/src/api/net/tests.rs`
- Modify: `mobile/rust/src/api/mod.rs`, `mobile/rust/src/api/session.rs` (campo `net`, `lock()`), `mobile/rust/src/api/backup.rs` (`import_backup`)
- Regenerate: puente

**Interfaces:**
- Produces (todo `pub(crate)`, nada visible a Dart; usa `#[frb(ignore)]` o mantén los tipos fuera de firmas públicas):
  - `struct NetState { epoch: u64, token: CancelToken, active: Option<u64>, next_op: u64 }` con `NetState::new()` y `fn invalidate(&mut self)` (`epoch += 1`, cancela el token, token nuevo, `active = None`).
  - `struct OpGuard<'a> { session: &'a VaultSession, op: u64, epoch: u64, token: CancelToken }` con `fn token(&self) -> &CancelToken`, `fn epoch(&self) -> u64`, `fn check(&self) -> FacadeResult<()>` (`Cancelled` si el token está cancelado o la época de la sesión ya no es la capturada) y `Drop` (libera `active` **solo si** sigue siendo `Some(su id)`).
  - `VaultSession::begin_op(&self) -> FacadeResult<OpGuard<'_>>` (`Busy` si `active` es `Some`; toma solo `net`).
  - `VaultSession::net_epoch(&self) -> u64` y `VaultSession::invalidate_net(&self)` (toma `net` un instante; se llama con `vault` ya tomado).
  - `fn connect(op: &OpGuard<'_>, address: SocketAddr) -> FacadeResult<TcpStream>`: `TcpStream::connect_timeout(&address, CONNECT_TIMEOUT)` (`CONNECT_TIMEOUT = Duration::from_secs(5)`); un error de conexión es `ConnectFailed`; **al volver llama a `op.check()`** y devuelve su error antes de devolver el flujo.
- Consumes: `VaultError::{Busy, Cancelled, ConnectFailed}` (Tarea 2); `vault_core::protocol::CancelToken`.
- `net` se toma con `unwrap_or_else(PoisonError::into_inner)`.

- [ ] **Step 1: Pruebas que fallan** en `net/tests.rs` (todas deterministas, sin depender de tiempos salvo topes de espera):
  - `begin_op_then_second_is_busy`; `drop_frees_active`; `old_guard_drop_does_not_free_a_newer_operation` (begin A, `invalidate_net`, begin B, soltar A → una tercera sigue dando `Busy`).
  - `lock_invalidates_epoch_cancels_token_and_frees_active` con una operación abierta.
  - `lock_takes_vault_first_then_net` (**orden de candados**): el hilo de la prueba toma `vault` con `slot()`; otro hilo llama a `lock()`; mientras `vault` sigue tomado, la época no cambia, el token no se cancela y `active` sigue igual (espera acotada con sondeos de ≤ 2 s); al soltar `vault`, `lock()` termina y los tres cambios ocurren a la vez.
  - `extraction_never_sees_new_epoch_with_unlocked_vault`: con `vault` tomado por la prueba (desbloqueada, época e0), `lock()` pendiente en otro hilo; la «extracción» (leer `net_epoch()` bajo `vault`) da e0; tras soltar y completar `lock()`, una extracción da `Locked`.
  - `import_backup_invalidates_on_success_and_on_failure`: con una operación capturada antes, una importación con contraseña incorrecta y otra correcta (respaldo exportado de otra sesión) dejan su token cancelado, `active` libre y `check()` en `Cancelled`.
  - `import_backup_invalidates_again_at_the_end`: la invalidación final es independiente de la inicial. Con una operación capturada **después** de la primera invalidación (se simula llamando a `begin_op` desde un gancho de prueba `#[cfg(test)]` que `import_backup` invoca entre el `take()` y la reapertura), tras terminar la importación su `check()` da `Cancelled`. Si el gancho resulta demasiado intrusivo, basta con la mutación del paso 7(b) como prueba de la invalidación final.
  - `connect_refused_is_connect_failed` (puerto cerrado en `127.0.0.1`); `connect_after_cancel_returns_cancelled` (servidor que acepta; se invalida antes de `connect`: el servidor no recibe bytes); `lock_while_connecting_does_not_wait` (dirección que no responde, p. ej. `192.0.2.1:9` documentada como no enrutable; `lock()` desde otro hilo vuelve en < 1 s y `active` ya está libre. Si el entorno de pruebas rechaza la dirección al instante, la prueba acepta `ConnectFailed` y valida solo lo de `lock()`).
- [ ] **Step 2: Comprueba que fallan:** `cargo test -p rust_lib_boveda net` → no compila.
- [ ] **Step 3: Implementa** `NetState`, `OpGuard`, `begin_op`, `net_epoch`, `invalidate_net` y `connect` en `net.rs`; agrega el campo `net: Mutex<NetState>` a `VaultSession` (`VaultSession::new` lo inicia).
- [ ] **Step 4: `lock()`** toma `vault` (con `unwrap_or_else(PoisonError::into_inner)`) y, **sin soltarlo**, `net`; con ambos: bloquea la `Vault`, llama a `NetState::invalidate`; suelta. Sigue sin fallar. Actualiza el comentario de `lock()`.
- [ ] **Step 5: `import_backup`** (que ya conserva `vault`): llama a `invalidate_net()` antes del `take()` y otra vez al terminar —con éxito o con error, ya con la bóveda final en su sitio—; ningún `return` temprano puede saltarse la segunda (usa una estructura que lo garantice, p. ej. calcular el resultado en un bloque y invalidar después de él).
- [ ] **Step 6: Comprueba que pasan** las pruebas de `net` y las existentes de `session` y `backup`.
- [ ] **Step 7: Mutaciones.** (a) En `lock()`, invierte el orden (`net` primero, soltarlo, luego `vault`): `lock_takes_vault_first_then_net` debe fallar. (b) Quita el `invalidate_net` final de `import_backup`: debe fallar su prueba. (c) Haz que el `Drop` libere `active` sin comparar el id: debe fallar `old_guard_drop_does_not_free_a_newer_operation`. Restaura cada una con el árbol limpio (`git diff` antes y después).
- [ ] **Step 7b: Regenera el puente** y comprueba que no cambió nada inesperado en `lib/src/rust/`.
- [ ] **Step 8: Verificación y commit:** `feat: estado de red con época y bloqueo que cancela las operaciones (#102)`.

---

### Task 4: Identidad y lista de PCs

**Files:**
- Create: `mobile/rust/src/api/pcs.rs`, `mobile/rust/src/api/pcs/tests.rs`
- Modify: `mobile/rust/src/api/entries.rs` (ayudantes de ids), `mobile/rust/src/api/mod.rs`, `mobile/rust/src/test_support.rs`
- Regenerate: puente

**Interfaces:**
- Produces (cruzan a Dart):
  - `pub struct IdentityInfo { pub fingerprint: String }`
  - `pub struct PairedPcSummary { pub id: String, pub name: String, pub fingerprint: String, pub last_ip: Option<String>, pub paired_at: i64, pub last_seen_at: Option<i64> }` — sin llaves; `fingerprint` es la de la llave pública de la PC.
  - `pub struct PairedPcList { pub pcs: Vec<PairedPcSummary>, pub corrupt: Vec<String>, pub unidentified_corrupt: u32 }`
  - `VaultSession::ensure_identity(&self) -> FacadeResult<IdentityInfo>`, `paired_pcs(&self) -> FacadeResult<PairedPcList>`, `remove_paired_pc(&self, id: String) -> FacadeResult<()>`. Sin `#[frb(sync)]`; usan `with_vault`.
- Produces (`pub(crate)`): `fn pc_summary(&vault::PairedPc) -> PairedPcSummary`; en `entries.rs`, `bytes16_to_hex([u8; 16]) -> String` y `hex_to_bytes16(&str) -> FacadeResult<[u8; 16]>` (misma validación que hoy: 32 caracteres `0-9a-f`, error `InvalidEntryId`); `entry_id_to_hex`/`entry_id_from_hex` pasan a usarlos; y `fn pc_id_from_hex(&str) -> FacadeResult<PairedPcId>` que traduce su error a `InvalidPairedPcId`.
- Produces (`test_support.rs`, `pub(crate)`): `V3_PASSWORD = "prueba-bóveda-v3"` y `fixture_v3_copy() -> (TempDir, PathBuf)` (copia de `vault-v3.db`).
- Consumes: errores de la Tarea 2.

- [ ] **Step 1: Pruebas que fallan** en `pcs/tests.rs` con la copia v3 abierta y desbloqueada:
  - `paired_pcs_lists_the_two_fixture_pcs_without_keys`: `Escritorio` (IP `192.168.1.50`, con `last_seen_at`) y `Portátil` (sin IP ni `last_seen_at`); las huellas son las de las llaves `[0x11; 32]` y `[0x22; 32]`; ningún campo contiene la llave en hexadecimal ni en bytes.
  - `ensure_identity_returns_the_fixture_fingerprint` (llave `[7u8; 32]`): igual a `PublicKey::from_private(...).fingerprint().to_string()`; llamarla dos veces da lo mismo.
  - `ensure_identity_creates_it_lazily_on_a_vault_without_one` (copia v2, desbloqueada): la primera crea, la segunda repite.
  - `remove_paired_pc_removes_it_and_unknown_id_is_not_found` (`PairedPcNotFound`), `invalid_pc_id_is_rejected` (largo, mayúsculas, no hex → `InvalidPairedPcId`).
  - `locked_or_missing_vault`: `Locked` con la bóveda bloqueada y `NoVault` sin bóveda en las tres funciones.
  - `entry_hex_helpers_still_validate` (los casos existentes de `entries` siguen pasando).
- [ ] **Step 2: Comprueba que fallan:** `cargo test -p rust_lib_boveda pcs` → no compila.
- [ ] **Step 3: Implementa** los tipos y funciones; `paired_pcs` convierte `PairedPcs` del núcleo (`valid`, `corrupt`, `unidentified_corrupt` saturado a `u32`) y no toca la red. Mueve los ayudantes de ids a la forma de arriba sin cambiar su comportamiento.
- [ ] **Step 4: Regenera el puente** y ejecuta `flutter analyze`.
- [ ] **Step 5: Verificación y commit:** `feat: identidad y lista de PCs en la fachada (#102)`.

---

### Task 5: `pair`

**Files:**
- Create: `mobile/rust/src/api/pairing.rs`, `mobile/rust/src/api/pairing/tests.rs`
- Modify: `mobile/rust/src/api/mod.rs`, `mobile/rust/src/test_support.rs` (`FakePc`)
- Regenerate: puente

**Interfaces:**
- Produces: `VaultSession::pair(&self, offer_text: String, device_name: String) -> FacadeResult<PairedPcSummary>` (sin `#[frb(sync)]`).
- Produces (`pub(crate)`, para probar las carreras sin tiempos):
  - `VaultSession::pairing_prepare(&self, op: &OpGuard<'_>) -> FacadeResult<KeyPair>`: bajo `vault`, `unlocked` y época vigente (`op.check()` leyendo `net_epoch` **dentro** del candado de `vault`); `ensure_identity()`; copia la llave privada a un `KeyPair` con `SecretBox::<[u8; 32]>::default()` + `copy_from_slice` (sin pasar por la pila), como `unlock_with_key`.
  - `VaultSession::pairing_persist(&self, op: &OpGuard<'_>, paired: &vault_core::protocol::PairedPc) -> FacadeResult<PairedPcSummary>`: bajo `vault` comprueba que la bóveda siga desbloqueada (`Locked`) y la época (`Locked` si cambió; **no guarda nada**); guarda con `paired.into_new_paired_pc(None)` o, si la llave ya está en la lista, `update_paired_pc` con la IP de `paired.address().ip().to_canonical()` y devuelve esa PC (id, nombre y `paired_at` intactos).
- Produces (`test_support.rs`, `pub(crate)`): `FakePc`, la PC de las pruebas: `FakePc::new()` (oyente en `127.0.0.1:0`, llaves de la PC con `KeyPair::from_private(SecretBox::new(Box::new([1; 32])))`, `PairingSession` con código `[7; 32]`), `addr()`, `offer_text()` (la oferta de esa PC como texto base64url, con un codificador local igual al de `desktop/src-tauri/src/qr.rs`), `serve_pairing(timeouts) -> JoinHandle<Served>` y `serve_send(timeouts) -> JoinHandle<Result<IncomingSend<TcpStream>, SendError>>` (acepta **cualquier** llave de celular: `|_| true`; la prueba verifica `incoming.peer()`), y un modo «mudo» (acepta y no escribe) y uno «basura» (acepta y escribe una longitud 65535).
- Consumes: `begin_op`, `OpGuard`, `connect` (Tarea 3); `pc_summary`, tipos y errores (Tareas 2 y 4); del núcleo `PairingOffer::decode_qr_text` (Tarea 1), `DeviceName::new`, `pair_with_pc`, `PairingTimeouts::default()`.

Flujo de `pair` (fijado por el diseño): decodificar oferta y nombre **sin tocar la bóveda** (`InvalidPairingOffer`, `InvalidDeviceName`; el `String` recibido con la oferta se borra con `zeroize` en todos los caminos) → `begin_op()` → `pairing_prepare` → soltar candado → `connect(&op, offer.address())` → `pair_with_pc(stream, &keys, &offer, name, op.token(), &PairingTimeouts::default())` mapeado con `VaultError::from` → `pairing_persist` → `op.check()` antes de devolver (`Cancelled` si la época cambió).

- [ ] **Step 1: Pruebas que fallan** en `pairing/tests.rs`, con una copia v2 o v3 desbloqueada y `FakePc`; la PC usa `PendingConnection::confirm()` para aceptar:
  - `pair_end_to_end_saves_the_pc_with_provisional_name`: devuelve un `PairedPcSummary` con nombre `PC <huella>`, `last_ip` `127.0.0.1`, y `paired_pcs()` lo lista; la PC vio la llave del celular y el nombre `Pixel de Ana`.
  - `pair_with_pc_already_paired_updates_ip_and_keeps_identity`: id, nombre y `paired_at` iguales; `last_ip` nueva (usar una PC preexistente con la llave `[1;32]` y otra IP vía `with_vault`).
  - `pair_rejected_by_pc_is_pairing_rejected_and_saves_nothing`; `pair_when_pc_does_not_decide_is_confirmation_timeout` (la `FakePc` suelta la conexión: `ConnectionClosed`).
  - `same_qr_twice_second_is_connection_closed_and_nothing_changes` (Review Focus 5).
  - `bad_inputs_do_not_touch_the_network`: texto con salto de línea final, vacío, con `=`, nombre vacío o con caracteres de control → `InvalidPairingOffer`/`InvalidDeviceName`; `FakePc` no recibe conexión; `Busy` libre (Review Focus 1).
  - `silent_pc_and_garbage_pc_do_not_hang`: «muda» → `NetworkTimeout` o `PairingConfirmationTimeout` dentro del tope de la prueba (si es necesario, la prueba usa plazos cortos mediante una función interna que reciba `PairingTimeouts`; `pair` la llama con los de `Default`); «basura» → `ProtocolFailure`; en ambos, `Busy` libre (Review Focus 4).
  - `lock_during_pairing_cancels_without_waiting`: la PC acepta y calla; `lock()` desde otro hilo; `pair` devuelve `Cancelled` en < 1 s, la bóveda queda bloqueada y `lock()` no esperó (mide el tiempo de `lock()`).
  - `accepted_then_lock_before_persist_gives_locked_and_saves_nothing` (**determinista**): `begin_op`, `pairing_prepare`, `pair_with_pc` completo contra `FakePc` con `confirm()`, luego `lock()` y entonces `pairing_persist` → `Locked`; tras desbloquear, `paired_pcs()` no lo contiene.
  - `lock_then_unlock_does_not_revive_the_old_operation` (**determinista**): igual, pero con `unlock` entre el `lock()` y `pairing_persist` → sigue `Locked`; y una `pair` nueva sí funciona.
  - `import_backup_during_pairing_cancels`: con una operación preparada, `import_backup` (éxito y error) deja `pairing_persist` en `Locked`/`Cancelled`.
  - `second_network_operation_is_busy_and_busy_is_released`: con una `pair` detenida contra la PC muda, otra `pair` da `Busy` al instante; tras `lock()`, tras éxito y tras error, la siguiente corre.
  - `no_secret_crosses_in_results_or_errors`: el `Debug` de `PairedPcSummary` y de cada error no contiene el nombre del celular, el código ni la llave.
- [ ] **Step 2: Comprueba que fallan:** `cargo test -p rust_lib_boveda pairing` → no compila.
- [ ] **Step 3: Implementa** `FakePc` y luego `pair`, `pairing_prepare` y `pairing_persist`. La lógica es solo orquestación: nada de Noise ni de reglas de la lista más allá de «existe → actualiza IP».
- [ ] **Step 4: Comprueba que pasan.** Corre la suite de `pairing` **cinco veces seguidas** para detectar pruebas dependientes de tiempos.
- [ ] **Step 5: Mutaciones.** (a) Quita la comprobación de época de `pairing_persist`: fallan las dos pruebas deterministas. (b) Quita `op.check()` de `connect`: falla la prueba de cancelación tras `connect`. (c) Haz que `pair` tome `vault` durante `pair_with_pc`: falla `lock_during_pairing_cancels_without_waiting`. Restaura con el árbol limpio.
- [ ] **Step 6: Regenera el puente, `flutter analyze` y verificación completa.** Commit: `feat: emparejar con el QR desde la fachada (#102)`.

---

### Task 6: `send_to_pc`

**Files:**
- Create: `mobile/rust/src/api/sending.rs`, `mobile/rust/src/api/sending/tests.rs`
- Modify: `mobile/rust/src/api/mod.rs`
- Regenerate: puente

**Interfaces:**
- Produces (cruza a Dart): `pub enum SendOutcome { Copied, Failed }` (`Failed`: la PC no pudo copiar; no es un error); `VaultSession::send_to_pc(&self, pc_id: String, entry_id: String, address: String) -> FacadeResult<SendOutcome>` (sin `#[frb(sync)]`).
- Produces (`pub(crate)`): `struct SendJob { keys: KeyPair, pc_key: PublicKey, message: SendMessage }` y `VaultSession::send_prepare(&self, op: &OpGuard<'_>, pc: PairedPcId, entry: EntryId) -> FacadeResult<SendJob>`: bajo `vault`, desbloqueada y época vigente; busca la PC en `paired_pcs().valid` (`PairedPcNotFound`; si su id está en `corrupt`, `CorruptPairedPc`); `ensure_identity()`; llave privada copiada a un `KeyPair` de vida corta (como en la Tarea 5); `vault.reveal(entry)` y copia **solo** la contraseña a un `SecretString` de largo exacto (`SecretString::from(expose_secret())`, sin pasar por `String`) para `SendMessage::new` (las notas se descartan al soltar el `Secret`); `PublicKey::from_bytes(*pc.public_key())`.
- Consumes: `begin_op`, `OpGuard`, `connect`; `pc_id_from_hex`, `entry_id_from_hex` (Tarea 4); `FakePc` (Tarea 5); del núcleo `send_to_pc`, `SendTimeouts::default()`, `AckOutcome`.

Flujo: validar textos (`InvalidPairedPcId`, `InvalidEntryId`, `InvalidAddress` con `address.parse::<SocketAddr>()`) → `begin_op()` → `send_prepare` → soltar candado → `connect` → `send_to_pc` del núcleo con `op.token()` mapeado con `VaultError::from` → `op.check()` antes de devolver (`Cancelled` aunque el `Ack` haya llegado, si la época cambió) → `Copied`/`Failed`.

- [ ] **Step 1: Pruebas que fallan** en `sending/tests.rs`; la bóveda v3 desbloqueada con la PC de `FakePc` agregada vía `with_vault` (llave `[1;32]`, mediante `NewPairedPc`) y una entrada del fixture:
  - `send_copied_end_to_end`: la PC recibe exactamente la contraseña de la entrada (`incoming.message().password()`), `incoming.peer()` es la llave de la identidad del celular, responde `copied()` → `Ok(Copied)`.
  - `send_failed_is_ok_failed`: la PC responde `failed()` → `Ok(Failed)`.
  - `password_never_crosses_to_dart`: con una contraseña distintiva en la entrada, ni el `SendOutcome` ni ningún error posible (`Debug`) la contienen; sí la recibió la PC.
  - `pc_that_does_not_recognise_the_phone_is_connection_closed` (la PC cierra sin escribir) y `unreachable_address_is_connect_failed`.
  - `bad_inputs_do_not_touch_the_network_and_free_busy`: direcciones `"[fe80::1%5]:47615"`, `":47615"`, `"1.2.3.4:0"`, `"pc.local:47615"`, `""` → `InvalidAddress`; ids mal formados → `InvalidPairedPcId`/`InvalidEntryId` (Review Focus 2); PC ya quitada con id válido → `PairedPcNotFound` sin conexión (Review Focus 3); entrada eliminada → `EntryNotFound`; siempre `Busy` libre al terminar.
  - `silent_and_garbage_pc` (Review Focus 4): «muda» → `NetworkTimeout` dentro del tope (mediante función interna con `SendTimeouts` cortos); «basura» → `ProtocolFailure`; `Busy` libre.
  - `lock_during_send_cancels_without_waiting`: la PC acepta y calla; `lock()` desde otro hilo; `send_to_pc` devuelve `Cancelled` en < 1 s y la bóveda queda bloqueada.
  - `lock_between_prepare_and_connect_sends_nothing` (**determinista**): `begin_op`, `send_prepare`, `lock()`, `connect(&op, addr)` → `Cancelled`; la PC no recibió un solo byte.
  - `late_success_after_lock_is_cancelled` (**determinista**): una operación completa contra `FakePc` con `copied()`, pero `lock()` antes del `op.check()` final → `Cancelled`, no `Copied`.
  - `second_network_operation_is_busy_in_both_orders`: una `send_to_pc` detenida y luego `pair` → `Busy`; y al revés.
  - `busy_is_released_after_success_error_and_cancel`.
  - `reveal_updates_last_used_at_like_any_reveal` (documenta el efecto del diseño: tras enviar, `entry(id).last_used_at` es `Some`).
- [ ] **Step 2: Comprueba que fallan:** `cargo test -p rust_lib_boveda sending` → no compila.
- [ ] **Step 3: Implementa** `send_to_pc` y `send_prepare`.
- [ ] **Step 4: Comprueba que pasan**; corre `sending` y `pairing` cinco veces seguidas.
- [ ] **Step 5: Mutaciones.** (a) Haz que `send_to_pc` devuelva `Ok(outcome)` sin el `op.check()` final: falla `late_success_after_lock_is_cancelled`. (b) Haz que `send_prepare` incluya las notas en el mensaje (p. ej. concatenarlas): falla la comprobación de la contraseña exacta. (c) Quita la liberación de `Busy` en el `Drop`: fallan las pruebas de `Busy`. Restaura con el árbol limpio.
- [ ] **Step 6: Regenera el puente, `flutter analyze` y verificación completa.** Commit: `feat: enviar una entrada a una PC desde la fachada (#102)`.

---

### Task 7: Permiso, documentación y verificación final

**Files:**
- Modify: `mobile/android/app/src/main/AndroidManifest.xml`
- Modify: `ARCHITECTURE.md`, `docs/especificacion.md`, `docs/roadmap.md`, `mobile/CLAUDE.md`

- [ ] **Step 1: Manifiesto.** Agrega `<uses-permission android:name="android.permission.INTERNET"/>` junto a `USE_BIOMETRIC`; nada de permisos de multidifusión (son de la #104).
- [ ] **Step 2: `mobile/CLAUDE.md`.** Reglas nuevas: ninguna operación de red de `rust/` mantiene tomado el candado de la sesión; el orden de candados es `vault` y luego `net`; `lock()` e `import_backup` invalidan la época y cancelan; una sola operación de red (`Busy`); la contraseña de un envío nunca cruza a Dart; `pair` recibe el texto del QR sin recortar (lo recorta la pantalla, #103) y conecta a la dirección del QR, que **no autentica** a la PC (la autentica Noise).
- [ ] **Step 3: `ARCHITECTURE.md`.** Archivos nuevos de `mobile/rust/src/api/` (`net.rs`, `pcs.rs`, `pairing.rs`, `sending.rs`), `decode_qr_text` en la fila de `protocol/` y una línea en «Decisiones clave» sobre la época y los dos candados; el texto de la excepción de borrado de los búferes de cruce de frb incluye la oferta (código de un solo uso) que llega de Dart.
- [ ] **Step 4: `docs/especificacion.md`.** Decisión del 2026-11-20 (#102, detalle en el diseño): operaciones de red de la fachada, bloqueo, época, `Busy`, `decode_qr_text`, PC repetida (actualiza la IP), el caso (c) cuando `lock()` llega entre `Accepted` y guardar, `ConnectionClosed` sin distinguir «la PC no reconoce al celular», y que el borrado de la oferta recibida de Dart es de mejor esfuerzo.
- [ ] **Step 5: `docs/roadmap.md`.** Marca la #102 (formato de las entradas vecinas).
- [ ] **Step 6: Verificación completa**, cada comando aparte y con su código de salida: `cargo test --workspace`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo fmt --all` y `cargo fmt --all -- --check`; `cargo audit` y `cargo deny check` (sin dependencias nuevas: deben seguir igual); desde `mobile/`: `flutter_rust_bridge_codegen generate` y `git diff --exit-code lib/src/rust rust/src/frb_generated.rs` (el puente está al día), `flutter analyze` y `flutter test`. Anota qué salió.
- [ ] **Step 7: Pendiente del usuario** (declararlo en el PR, no ejecutarlo): confirmar en el celular que la app sigue arrancando con el permiso nuevo; las pruebas de punta a punta con una PC real son de #103 y #104.
- [ ] **Step 8: Commit:** `docs: documentar la fachada del protocolo y el permiso de red (#102)` (y `chore: permiso INTERNET` aparte si el manifiesto se commitea por separado).

---

## Autorrevisión

- **Cobertura del diseño:** decisiones 1–10 → Tareas 1 (9), 3 (2, 3, 4), 4–6 (5, 7, 10), 7 (6); pruebas obligatorias (8): bloqueo durante envío y emparejamiento y la carrera `Accepted`/`lock()` en Tareas 5 y 6; nuevo desbloqueo, dos operaciones y liberación de `Busy` en 3, 5 y 6; contraseña que no cruza en la 6; invalidación determinista de la época contra el bloqueo real en la 3. Correcciones del usuario: `lock()` con `vault` primero (Tarea 3, con mutación), `import_backup` con invalidación al inicio y al final (Tarea 3), comprobación tras `connect` (Tarea 3, probada en 5 y 6), sin éxito tardío (5 y 6), memoria de mejor esfuerzo (Tarea 7, paso 3–4).
- **Consistencia de tipos:** `OpGuard::{token, epoch, check}`, `begin_op`, `net_epoch`, `invalidate_net`, `connect`, `pairing_prepare/persist`, `send_prepare`, `PairedPcSummary`, `FakePc` se definen en la tarea que los produce y se usan con esos nombres.
- **Proporción:** el plan fija firmas, valores y pruebas; los cuerpos los escribe quien ejecuta.
