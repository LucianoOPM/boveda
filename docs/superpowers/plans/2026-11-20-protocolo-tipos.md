# Identidades X25519 y mensajes del protocolo (#94) — plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `vault-core` expone `vault_core::protocol` con el par de llaves X25519, la llave pública con su huella corta y los mensajes del protocolo (oferta del QR, envío de 30 s y confirmación), con codificación binaria estricta, sin red.

**Architecture:** Módulo `protocol/` con `error.rs`, `identity.rs` y `message.rs`. Cada mensaje viaja como `versión (u8) | tipo (u8) | cuerpo postcard`; decodificar comprueba tamaño, versión y tipo antes del cuerpo, usa `take_from_bytes` y exige que no sobre nada. La contraseña se serializa con un módulo auxiliar sobre el `ZeroizingVec` ya existente, con capacidad exacta, y se deserializa directamente a `SecretString`. La llave pública se deriva con `x25519-dalek` desde los 32 bytes de la llave privada, sin transformarlos.

**Tech Stack:** Rust, `x25519-dalek` 3 (nuevo, aprobado), `serde` + `postcard`, `secrecy`, `zeroize`, `getrandom`, `thiserror`, `proptest`.

**Spec:** `docs/superpowers/specs/2026-11-20-protocolo-tipos-design.md`

## Global Constraints

- Identificadores en inglés; comentarios, documentación y commits en español, Conventional Commits con `(#94)`.
- Rama `feat/94-protocolo-tipos` desde `dev` (ya creada). PR contra `dev` con `Closes #94`.
- Pruebas primero: cada función nueva empieza con una prueba que falla, y se ve fallar por la razón esperada (`core/CLAUDE.md`). Las unitarias van en `modulo/tests.rs` (`mod.rs`/`modulo.rs` termina con `#[cfg(test)] mod tests;`); las de la API pública, en `core/tests/protocol.rs`.
- Única dependencia nueva: `x25519-dalek` (`default-features = false`, `features = ["static_secrets", "zeroize"]`). Cualquier otra, antes se pregunta.
- Ningún secreto en logs, errores, panics ni `Debug`. La llave privada y el código van en `SecretBox`; los búferes con la contraseña, en `Zeroizing`. `Debug` de los tipos con secretos se implementa a mano.
- Los 32 bytes de la llave privada **nunca** se recortan ni se transforman: ni al construir, ni al leerlos, ni al derivar la pública.
- Los bytes de la contraseña nunca se recortan ni se normalizan.
- La API pública no hace panic: `Result` con `ProtocolError`. Nada de `unwrap`/`expect` fuera de pruebas.
- No cambiar `vault::Error`, `UnlockReport`, la fachada (`mobile/rust`) ni nada del formato de la bóveda.
- Antes de cada commit: `cargo test -p vault-core` en verde. Nunca encadenar `fmt`, `clippy` y `commit` con `;` ni con `| grep`: correr cada uno aparte y comprobar su código de salida.
- No hay Python: los scripts auxiliares, si hacen falta, en Node o bash, dentro del scratchpad.

## Review Focus

Condiciones que el diseño implica y que ninguna tarea debe dejar sin prueba, de más a menos probable:

- `from_bytes` ignora los bytes sobrantes: un mensaje con basura al final debe dar `TrailingBytes` en los tres tipos (Task 3, 5 y 6).
- Una versión desconocida con un cuerpo que no es de la versión 1 debe dar `UnsupportedVersion`, no `Truncated` ni `Malformed` (Task 3).
- `ttl_secs` de 29 o 31 debe rechazarse al **decodificar** (armando los bytes a mano), no solo al construir (Task 6).
- Una contraseña de 1024 bytes se acepta y una de 1025 no; con el tope de 4 KiB del mensaje no debe haber ninguna ruta que dé `TooLarge` por una contraseña válida (Task 6).
- La llave privada que se entrega y la que se lee son idénticas byte por byte aun sin recortar (Task 2).
- El búfer de `encode` no se reubica: `capacity == len` (Task 5 y 6).
- Una mutación comprobada de cada arreglo: quitar el chequeo de sobrantes, el de versión o el de TTL debe romper una prueba concreta (Task 8).

---

### Task 1: Dependencia, módulo y errores

**Files:**
- Modify: `core/Cargo.toml`, `core/src/lib.rs`, `core/CLAUDE.md`
- Create: `core/src/protocol/mod.rs`, `core/src/protocol/error.rs`, `core/src/protocol/error/tests.rs`

**Interfaces:**
- Produces:
  - `vault_core::protocol::{ProtocolError, InvalidField}` y `pub type Result<T> = std::result::Result<T, ProtocolError>` (privado al módulo `protocol`, reexportado como `protocol::Result`).
  - `ProtocolError` (`#[non_exhaustive]`, `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, `thiserror::Error`): `TooLarge`, `Truncated`, `UnsupportedVersion(u8)`, `UnexpectedKind(u8)`, `Malformed`, `TrailingBytes`, `InvalidField(InvalidField)`, `Random`, `Encode`.
  - `InvalidField` (`#[non_exhaustive]`, `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, `thiserror::Error`): `Ttl`, `PasswordEmpty`, `PasswordTooLong`, `Port`, `Address`. Cada mensaje de `#[error(...)]` está en español y no incluye datos.

- [ ] **Step 1: Verificar la dependencia antes de agregarla.** `cargo info x25519-dalek@3.0.0` (BSD-3-Clause, features `static_secrets` y `zeroize`) y revisar en `~/.cargo/registry/src/*/x25519-dalek-3.0.0/src/x25519.rs` que `StaticSecret` implementa `ZeroizeOnDrop` y no deriva `Debug`. Anotar el resultado en el mensaje de commit.
- [ ] **Step 2: Prueba que falla** (`error/tests.rs`): que cada variante tenga un `Display` no vacío; que ningún `Display` de `UnsupportedVersion(7)` ni `UnexpectedKind(9)` repita más que el número que ya es parte de la variante (los demás no llevan datos); que `ProtocolError` sea `Copy`.
- [ ] **Step 3:** `cargo test -p vault-core --lib protocol` y comprobar que no compila (el módulo no existe).
- [ ] **Step 4: Implementar.** En `core/Cargo.toml`, orden alfabético, con el comentario de la finalidad:

```toml
# Deriva la llave pública X25519 desde los 32 bytes de la llave privada (#94).
# `static_secrets` da `StaticSecret::from([u8; 32])`; `zeroize` la borra al
# soltarla. Sin `precomputed-tables` (se deriva una vez por emparejamiento).
# (BSD-3-Clause)
x25519-dalek = { version = "3", default-features = false, features = ["static_secrets", "zeroize"] }
```

  `lib.rs` agrega `pub mod protocol;` y actualiza el comentario ("y, más adelante, `protocol`" ya existe). `protocol/mod.rs` declara `mod error;` y reexporta. `core/CLAUDE.md`: en la lista de crates, `x25519-dalek` con su finalidad (derivar la llave pública) y sus features.
- [ ] **Step 5:** `cargo test -p vault-core`, `cargo deny check` y `cargo audit` (sin que aparezcan avisos ni licencias nuevas; si `curve25519-dalek` duplica una versión, queda como advertencia).
- [ ] **Step 6: Commit**

```bash
git add core/Cargo.toml Cargo.lock core/CLAUDE.md core/src
git commit -m "feat: módulo protocol con errores y x25519-dalek (#94)"
```

---

### Task 2: Llaves X25519 y huella

**Files:**
- Create: `core/src/protocol/identity.rs`, `core/src/protocol/identity/tests.rs`
- Modify: `core/src/protocol/mod.rs`

**Interfaces:**
- Consumes: Task 1; `crate::vault::PUBLIC_KEY_LEN` (32).
- Produces:
  - `PublicKey` (`Clone`, `Copy`, `PartialEq`, `Eq`, `Hash`, `Debug` con la huella): `from_bytes([u8; 32])`, `as_bytes(&self) -> &[u8; 32]`, `from_private(&SecretBox<[u8; 32]>) -> PublicKey`, `fingerprint(&self) -> Fingerprint`.
  - `Fingerprint` (`Clone`, `Copy`, `PartialEq`, `Eq`, `Debug`, `Display`): `as_bytes(&self) -> &[u8; 8]`; `Display` da `A1B2-C3D4-E5F6-0718`.
  - `KeyPair` (`ZeroizeOnDrop`, `Debug` a mano con la huella, sin `Clone`): `generate() -> Result<KeyPair>`, `from_private(SecretBox<[u8; 32]>) -> KeyPair`, `private(&self) -> &SecretBox<[u8; 32]>`, `public(&self) -> PublicKey`.

- [ ] **Step 1: Pruebas que fallan** (`identity/tests.rs`):
  - `rfc7748_alice_and_bob_public_keys`: las dos llaves privadas del RFC 7748 §6.1 (`77076d0a…2c2a` y `5dab087e…eb0a`) dan las públicas `8520f009…1a2a` y `de9edb7d…3f4f` (copiar los 64 hexadecimales completos del RFC al escribir la prueba).
  - `public_key_ignores_clamping`: para una `k` cualquiera, la pública de `k` es igual a la de `k` con el recorte ya aplicado (`k[0] &= 248; k[31] &= 127; k[31] |= 64`).
  - `keypair_keeps_the_private_bytes_unchanged`: con `k = [0xFF; 32]` (sin recortar) y con `[0; 32]`, `private().expose_secret()` devuelve exactamente `k`.
  - `generate_returns_different_32_byte_keys` (dos llamadas distintas; la pública coincide con `from_private`).
  - `fingerprint_is_the_first_8_bytes_in_uppercase_groups`: con una llave de bytes conocidos, `to_string()` es `A1B2-C3D4-E5F6-0718` (elegir los bytes para que el resultado sea ese texto) y `as_bytes()` son los primeros 8.
  - `keypair_debug_hides_the_private_key`: con una llave de relleno reconocible (`0xAB`), `format!("{:?}", pair)` no contiene `ab`, `171`, ni `[` con bytes; sí contiene la huella.
  - `keypair_is_zeroize_on_drop` (`assert_zeroize_on_drop::<KeyPair>()`).
- [ ] **Step 2:** `cargo test -p vault-core --lib protocol::identity` y comprobar que fallan por lo esperado (no compila: los tipos no existen).
- [ ] **Step 3: Implementar.** Derivación:

```rust
use x25519_dalek::{PublicKey as DalekPublic, StaticSecret};

pub fn from_private(private: &SecretBox<[u8; 32]>) -> Self {
    // Se copia a la pila justo antes de derivar y no se retiene: `StaticSecret`
    // se borra al soltarse, pero esta copia del argumento no (excepción
    // documentada en la especificación).
    let secret = StaticSecret::from(*private.expose_secret());
    Self(DalekPublic::from(&secret).to_bytes())
}
```

  `generate` escribe `getrandom::fill` directamente en el `SecretBox` (`SecretBox::<[u8; 32]>::default()` y `expose_secret_mut`), con `ProtocolError::Random` si falla, como `vault::identity::generate`. La huella usa un arreglo de 8 bytes y un formateador manual (sin crates). La documentación de `Fingerprint` y de `fingerprint()` dice que es un identificador visual corto, que no autentica nada y no sustituye a Noise, y que 64 bits no impiden fabricar una llave con la misma huella.
- [ ] **Step 4:** `cargo test -p vault-core --lib protocol::identity` en verde; comprobar que `rfc7748_*` falla si se rompe la derivación (cambiar temporalmente `to_bytes()` por `[0; 32]`, ver el fallo y restaurar desde una copia, no con `git checkout`).
- [ ] **Step 5: Commit**

```bash
git add core/src/protocol
git commit -m "feat: par de llaves X25519 y huella de la llave pública (#94)"
```

---

### Task 3: Codificación común de los mensajes

**Files:**
- Create: `core/src/protocol/message.rs`, `core/src/protocol/message/tests.rs`
- Modify: `core/src/protocol/mod.rs`, `core/src/vault/entry.rs` (solo si hace falta ampliar la visibilidad de `flavor`: ya es `pub(crate) mod flavor`, así que no debería)

**Interfaces:**
- Consumes: Task 1; `crate::vault::entry::flavor::ZeroizingVec`.
- Produces (todo `pub(super)`/`pub(crate)` salvo las constantes):
  - `pub const PROTOCOL_VERSION: u8 = 1; MAX_MESSAGE_LEN: usize = 4096; SEND_TTL_SECS: u8 = 30; PAIRING_CODE_LEN: usize = 32; MAX_PASSWORD_BYTES: usize = 1024;`
  - `enum Kind { PairingOffer = 1, Send = 2, Ack = 3 }` (privado).
  - `fn encode_body<T: Serialize>(kind: Kind, body: &T) -> Result<Zeroizing<Vec<u8>>>` y `fn decode_body<'a, T: Deserialize<'a>>(kind: Kind, bytes: &'a [u8]) -> Result<T>`, con el orden de comprobaciones del diseño ("Decodificar", pasos 1 a 6; la validación de campos, paso 7, la hace cada tipo).
  - Un tipo de prueba interno (solo `#[cfg(test)]`): `struct Probe { a: u8, b: String }` para ejercitar la codificación sin depender de los mensajes reales.

- [ ] **Step 1: Pruebas que fallan** (`message/tests.rs`, con `Probe` y `Kind::Ack` para no depender de las tareas siguientes):
  - `roundtrip`: `decode_body(Kind::Ack, &encode_body(Kind::Ack, &probe)?)` devuelve `probe`.
  - `frame_starts_with_version_and_kind`: los dos primeros bytes son `[1, 3]`.
  - `empty_and_one_byte_inputs_are_truncated` (`[]` y `[1]`).
  - `unknown_version_wins_over_everything`: versiones 0 y 2 con un cuerpo vacío, con un cuerpo de basura y con un cuerpo válido de la versión 1 dan `UnsupportedVersion(v)`.
  - `wrong_kind_is_unexpected_kind`: tipo 9 y tipo 1 al pedir el 3 dan `UnexpectedKind`.
  - `trailing_bytes_are_rejected`: un mensaje válido más `[0]`, y más 100 bytes, dan `TrailingBytes`.
  - `every_proper_prefix_fails_with_truncated_or_malformed`: para cada prefijo propio, error (nunca `Ok`, nunca pánico).
  - `exactly_max_len_is_not_too_large`: un `Probe` de relleno armado con 4096 bytes en total se decodifica (o falla con un error distinto de `TooLarge`); 4097 bytes dan `TooLarge` sin importar el contenido (incluso si empieza con una versión desconocida).
  - `encode_over_max_len_is_too_large`: un `Probe` con una cadena de 4100 bytes.
  - `encode_buffer_does_not_reallocate`: `capacity() == len()` del resultado.
  - `bad_utf8_is_malformed`: un cuerpo con una cadena de 2 bytes inválidos en UTF-8.
- [ ] **Step 2:** `cargo test -p vault-core --lib protocol::message` y comprobar que fallan.
- [ ] **Step 3: Implementar.** Codificar:

```rust
fn encode_body<T: Serialize>(kind: Kind, body: &T) -> Result<Zeroizing<Vec<u8>>> {
    let size = postcard::experimental::serialized_size(body) // o postcard::serialized_size según la versión
        .map_err(|_| ProtocolError::Encode)?;
    let total = size.checked_add(2).filter(|n| *n <= MAX_MESSAGE_LEN).ok_or(ProtocolError::TooLarge)?;
    let mut flavor = ZeroizingVec::with_capacity(total);
    flavor.try_push(PROTOCOL_VERSION).map_err(|_| ProtocolError::Encode)?;
    flavor.try_push(kind as u8).map_err(|_| ProtocolError::Encode)?;
    postcard::serialize_with_flavor(body, flavor).map_err(|_| ProtocolError::Encode)
}
```

  (En postcard 1.1.3 la función es `postcard::serialized_size`, en `ser/mod.rs:490`: comprobar la ruta pública exacta antes de escribirla.) Decodificar: los pasos 1 a 6 del diseño, con `postcard::take_from_bytes`, mapeando `DeserializeUnexpectedEnd` a `Truncated` y el resto a `Malformed`. El error de postcard nunca se reenvía.
- [ ] **Step 4:** pruebas en verde; `cargo clippy -p vault-core --all-targets -- -D warnings`.
- [ ] **Step 5: Commit**

```bash
git add core/src/protocol
git commit -m "feat: codificación estricta de los mensajes del protocolo (#94)"
```

---

### Task 4: `PairingCode` y la contraseña con serialización explícita

**Files:**
- Modify: `core/src/protocol/message.rs`, `core/src/protocol/message/tests.rs`

**Interfaces:**
- Consumes: Task 3.
- Produces:
  - `PairingCode` (`ZeroizeOnDrop`, `Debug` a mano sin el valor, sin `Clone`): `generate() -> Result<PairingCode>`, `from_bytes(SecretBox<[u8; 32]>)`, `expose_bytes(&self) -> &[u8; 32]`. Implementa `Serialize` (los 32 bytes como arreglo, para el cuerpo del QR) y `Deserialize` (con el `[u8; 32]` que se pasa de inmediato a un `SecretBox`).
  - Módulo privado `secret_str` con `pub(super) fn serialize<S: Serializer>(value: &SecretString, s: S) -> Result<S::Ok, S::Error>` (`s.serialize_str(value.expose_secret())`) y `pub(super) fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<SecretString, D::Error>` (`String::deserialize(d).map(SecretString::from)`). Es la **única** ruta de serialización de un secreto del módulo y lo dice su documentación.

- [ ] **Step 1: Pruebas que fallan:**
  - `pairing_code_generate_is_random_and_32_bytes`.
  - `pairing_code_debug_hides_the_value` (sin hexadecimal ni bytes; no implementa `Display`).
  - `pairing_code_is_zeroize_on_drop`.
  - `secret_str_roundtrip_keeps_bytes`: una estructura de prueba con `#[serde(with = "secret_str")]` (`password: SecretString`) hace la ida y vuelta para: `""`, `"a"`, espacios al inicio y al final (`"  x  "`), `"é"` (NFC) y `"e\u{0301}"` (NFD) — los dos distintos entre sí —, un emoji `"🔐"`, un NUL incrustado `"a\0b"` y 1024 bytes.
  - `secret_str_serialization_does_not_reallocate`: `encode_body` de esa estructura con 1000 bytes da `capacity == len`.
  - `secret_str_decode_with_trailing_bytes_still_errors_and_drops_secret`: mensaje válido más un byte da `TrailingBytes` (la contraseña ya decodificada se suelta; no se puede observar el borrado, queda cubierto por la revisión de `SecretBox`).
- [ ] **Step 2:** comprobar que fallan.
- [ ] **Step 3: Implementar.** `deserialize` documenta que el `String` de postcard tiene capacidad exacta (`visit_str` con `to_owned`) y que `into_boxed_str` no copia, con la referencia al código de `postcard` 1.1.3 y `secrecy` 0.10.3 verificados en el diseño. Documentar también que `PairingCode` pasa por un `[u8; 32]` en la pila al decodificar y que es la excepción aceptada de la especificación.
- [ ] **Step 4:** pruebas en verde.
- [ ] **Step 5: Commit**

```bash
git add core/src/protocol
git commit -m "feat: código de emparejamiento y contraseña serializada sin copias extra (#94)"
```

---

### Task 5: `PairingOffer`

**Files:**
- Modify: `core/src/protocol/message.rs`, `core/src/protocol/message/tests.rs`, `core/src/protocol/mod.rs`

**Interfaces:**
- Consumes: Tasks 2, 3 y 4.
- Produces: `PairingOffer { public_key: PublicKey, address: SocketAddr, code: PairingCode }` con `new(public_key, address, code) -> Result<Self>` (valida), `public_key()`, `address()`, `code() -> &PairingCode`, `encode(&self) -> Result<Zeroizing<Vec<u8>>>` y `decode(&[u8]) -> Result<Self>`; `Debug` a mano con la huella y la dirección pero sin el código. Constante `pub const MAX_PAIRING_OFFER_LEN: usize = 86;`.
  - Cuerpo postcard: `public_key` (`[u8; 32]`) | `address` (`SocketAddr`) | `code` (`[u8; 32]`). `PublicKey` serializa como un arreglo de 32 bytes (implementar `Serialize`/`Deserialize` a mano para que no agregue longitud).
  - Reglas (`InvalidField`): puerto 0 → `Port`; dirección no especificada o multicast → `Address`; loopback y direcciones privadas se aceptan.

- [ ] **Step 1: Pruebas que fallan:**
  - Ida y vuelta con IPv4 (`192.168.1.20:50000`) e IPv6 (`[fd00::1]:50000`); la llave y el código decodificados son los mismos.
  - `pairing_offer_wire_bytes_ipv4`: vector fijo con bytes escritos a mano (versión 1, tipo 1, llave de `0x11`, variante IPv4, `192.168.1.20`, puerto 50000 en varint, código de `0x22`) para detectar cualquier cambio del formato; y el equivalente con IPv6.
  - `pairing_offer_max_len_is_86`: IPv6 con puerto 65535 mide **exactamente** 86 y es el máximo (IPv4 con el mismo puerto, 74); `MAX_PAIRING_OFFER_LEN` coincide.
  - `port_zero_is_invalid`, `unspecified_ipv4_and_ipv6_are_invalid`, `multicast_ipv4_and_ipv6_are_invalid`, `loopback_is_accepted`. Cada una, tanto en `new`/`encode` como al decodificar bytes armados a mano.
  - `truncated_prefixes_fail` y `trailing_bytes_are_rejected` para este tipo; `wrong_kind` (un `Ack` codificado no decodifica como oferta).
  - `debug_does_not_contain_the_code`.
  - `encode_buffer_does_not_reallocate`.
- [ ] **Step 2:** comprobar que fallan.
- [ ] **Step 3: Implementar** con `encode_body`/`decode_body` y la validación compartida (`validate_address(&SocketAddr)`), llamada desde `new`, `encode` y `decode`.
- [ ] **Step 4:** pruebas en verde.
- [ ] **Step 5: Commit**

```bash
git add core/src/protocol
git commit -m "feat: oferta de emparejamiento del QR (#94)"
```

---

### Task 6: `SendMessage` y `Ack`

**Files:**
- Modify: `core/src/protocol/message.rs`, `core/src/protocol/message/tests.rs`, `core/src/protocol/mod.rs`

**Interfaces:**
- Consumes: Tasks 3 y 4.
- Produces:
  - `SendMessage { ttl_secs: u8, password: SecretString }`: `new(password: SecretString) -> Result<Self>` (fija 30 y valida), `ttl_secs()`, `password() -> &SecretString`, `encode` y `decode`; `Debug` a mano solo con `ttl_secs`; `ZeroizeOnDrop` (marcador, como `vault::Identity`).
  - `Ack { outcome: AckOutcome }`, `AckOutcome { Copied, Failed }` (`Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`): `Ack::new(outcome)`, `outcome()`, `encode`, `decode`.
  - Reglas: `ttl_secs == 30` → si no, `InvalidField::Ttl`; contraseña vacía → `PasswordEmpty`; más de 1024 bytes → `PasswordTooLong`.

- [ ] **Step 1: Pruebas que fallan:**
  - Ida y vuelta de `SendMessage` y de `Ack` (los dos resultados).
  - `send_message_wire_bytes`: vector fijo (`[1, 2, 30, len, bytes…]`) para `"clave"` y para otra con acento; `ack_wire_bytes` (`[1, 3, 0]` y `[1, 3, 1]`).
  - `new_sets_ttl_to_30` y `decode_rejects_ttl_0_29_31_and_255` (cuerpos armados a mano con esos valores y una contraseña válida): `InvalidField::Ttl`. Un TTL de 30 con contraseña válida se acepta.
  - `empty_password_is_rejected` (`new` y decodificando `[1, 2, 30, 0]`); `password_of_1024_bytes_is_accepted_and_1025_rejected` (`PasswordTooLong`), al construir y al decodificar; un mensaje de 1024 bytes de contraseña mide menos de 4096 (nunca `TooLarge`).
  - `unicode_passwords_are_unchanged`: la lista de Task 4 (espacios, NFC y NFD distintos, emoji, combinantes, NUL incrustado) pasa por `new` → `encode` → `decode` y devuelve los mismos bytes; los NFC y NFD siguen siendo distintos.
  - `truncated_prefixes_fail`, `trailing_bytes_are_rejected`, `unknown_ack_outcome_is_malformed` (`[1, 3, 9]`), `wrong_kind`.
  - `debug_does_not_contain_the_password`, y `send_message_is_zeroize_on_drop`.
  - `encode_buffer_does_not_reallocate` con una contraseña de 1000 bytes.
- [ ] **Step 2:** comprobar que fallan.
- [ ] **Step 3: Implementar.** `SendMessage` usa `#[serde(with = "secret_str")]` en el campo de la contraseña; `decode` valida después de decodificar y antes de devolver (el `SendMessage` fallido se suelta y se borra).
- [ ] **Step 4:** pruebas en verde.
- [ ] **Step 5: Commit**

```bash
git add core/src/protocol
git commit -m "feat: mensaje de envío con TTL de 30 s y confirmación (#94)"
```

---

### Task 7: Pruebas de la API pública y de propiedades

**Files:**
- Create: `core/tests/protocol.rs`

**Interfaces:**
- Consumes: Tasks 1 a 6, solo con la API pública `vault_core::protocol`.

- [ ] **Step 1: Pruebas** (`core/tests/protocol.rs`):
  - Ida y vuelta de punta a punta de los tres mensajes usando solo la API pública, con una oferta armada con `KeyPair::generate()` y `PairingCode::generate()`.
  - `proptest`: cualquier `SendMessage` con una contraseña de 1 a 1024 bytes UTF-8 (cadenas arbitrarias, `".{1,300}"` y de bytes grandes) hace la ida y vuelta; cualquier `PairingOffer` con IP y puerto válidos también.
  - `proptest`: cualquier secuencia de bytes de 0 a 5000 bytes, y cualquier mensaje válido con un byte cambiado o con un sufijo aleatorio, se decodifica sin pánico para los tres tipos (con éxito o con un error tipado); y, para el sufijo no vacío, **siempre** da error.
  - La huella de una `PublicKey` y su `Display` desde la API pública.
  - `KeyPair::generate` + `PublicKey::from_private(pair.private()) == pair.public()`.
  - `ProtocolError` es `std::error::Error + Send + Sync + 'static`.
- [ ] **Step 2:** comprobar que fallan por lo esperado antes de ajustar reexportaciones (`mod.rs` reexporta `KeyPair`, `PublicKey`, `Fingerprint`, `PairingOffer`, `PairingCode`, `SendMessage`, `Ack`, `AckOutcome`, las constantes y los errores; **no** reexporta `Kind` ni `secret_str`).
- [ ] **Step 3:** ajustar `mod.rs`; `cargo test -p vault-core` completo en verde.
- [ ] **Step 4: Commit**

```bash
git add core/tests/protocol.rs core/src/protocol
git commit -m "test: pruebas de la API pública y de propiedades del protocolo (#94)"
```

---

### Task 8: Mutaciones, documentación y verificación

**Files:**
- Modify: `docs/especificacion.md`, `ARCHITECTURE.md`, `docs/roadmap.md`

- [ ] **Step 1: Mutaciones.** Para cada una, hacer el cambio, ver que falla **una prueba concreta con datos realistas**, anotar cuál, y restaurar desde una copia sacada antes (no `git checkout` con trabajo sin confirmar). Confirmar primero todo lo pendiente.
  - Cambiar `take_from_bytes` por `from_bytes`: debe fallar `trailing_bytes_are_rejected`.
  - Quitar la comprobación de versión: debe fallar `unknown_version_wins_over_everything`.
  - Aceptar cualquier `ttl_secs`: debe fallar `decode_rejects_ttl_0_29_31_and_255`.
  - Usar `to_allocvec` en `encode_body`: debe fallar `encode_buffer_does_not_reallocate`.
  - Recortar (`trim`) la contraseña al decodificar: debe fallar `unicode_passwords_are_unchanged`.
  - Aplicar el recorte (`k[0] &= 248`…) a la llave privada guardada en `KeyPair::from_private`: debe fallar `keypair_keeps_the_private_bytes_unchanged`.
- [ ] **Step 2: Documentación.**
  - `docs/especificacion.md`: en "Emparejamiento y envío", subsección "Tipos del protocolo (#94)" que resuma el diseño (formato del cable, constantes, reglas, tamaño del QR con la corrección de la versión 6 y 7, huella, errores); `x25519-dalek` en la tabla de crates; la excepción de borrado nueva junto a las del "Formato de la bóveda" y de `ARCHITECTURE.md`. Actualizar la fecha de "Actualizado".
  - `ARCHITECTURE.md`: la fila de `protocol/` (`identity.rs`, `message.rs`, `error.rs`), el mapa de `core/src/protocol/` y la excepción de borrado (nº 4) en "Decisiones clave".
  - `docs/roadmap.md`: sub-casilla de la #94 dentro del punto del protocolo, de la etapa 3.
- [ ] **Step 3: Verificación final**, cada comando por separado y con su código de salida:
  - `cargo fmt --all --check`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace`
  - `cargo deny check`
  - `cargo audit`
  - Comprobar con el generador que el puente de `mobile/` no cambió: la fachada no usa nada de `protocol`.
- [ ] **Step 4: Commit**

```bash
git add docs ARCHITECTURE.md
git commit -m "docs: tipos del protocolo y excepción de borrado de la llave pública (#94)"
```

- [ ] **Step 5:** Revisión final con un subagente en opus, corrección de los menores baratos y PR contra `dev` con el formato del PR #11 (Resumen, Decisiones de implementación, Criterios de aceptación con casillas, Verificación local, `Closes #94`), sin esperar la CI.

## Criterios de aceptación de la issue, y dónde los cubre este plan

| Criterio | Tarea |
| --- | --- |
| Ida y vuelta de cada mensaje; truncado, versión desconocida o campos fuera de rango dan error tipado | Tasks 3, 5, 6, 7 |
| Ningún tipo con secretos implementa `Debug` ni `Display` con el valor | Tasks 2, 4, 5, 6 |
| `cargo test --workspace`, `clippy`, `fmt` y `deny` pasan | Task 8 |
