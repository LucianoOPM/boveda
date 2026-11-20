# Diseño: identidades X25519 y mensajes del protocolo (issue #94)

Fecha: 2026-11-20 · Etapa 3 (épica #92) · Rama `feat/94-protocolo-tipos` (desde `dev`, independiente de la #93)

## Objetivo

`vault-core` tiene los tipos del protocolo, sin red: las identidades X25519 y los mensajes con versión (contenido del QR de emparejamiento, envío con tiempo de vida de 30 s y confirmación), con su codificación binaria estricta. Es la base de `core/src/protocol/` (`identity.rs`, `message.rs`) sobre la que construyen el emparejamiento (#95), el transporte (#96), la identidad de la PC (#99) y la fachada (#102).

## Fuera de alcance

- El emparejamiento con Noise y el uso del código como PSK (#95), el transporte, la confirmación de dispositivos conocidos y el rechazo de no emparejados (#96).
- Guardar la identidad: en el celular ya lo hace la #93; en la PC, la #99 (DPAPI).
- La conversión a texto del contenido del QR, el paquete de QR y el escaneo (#101, #103).
- El nombre del celular que muestra la PC al confirmar el emparejamiento (#95).
- La prueba de que `snow` deriva la misma llave pública que este módulo (#95).
- Exponer algo al puente (#102). `vault::Error` y el `match` de `mobile/rust/src/api/error.rs` no cambian.

## Decisiones

Aprobadas por el usuario el 2026-11-20:

1. **Llave pública con `x25519-dalek` directo**, con las features `static_secrets` y `zeroize`. Los 32 bytes de la llave privada se conservan **tal cual**, sin recortar ni transformar: el recorte (clamping) lo hace la multiplicación al derivar la pública (la #93 ya fijó que la identidad guarda 32 bytes sin serializar).
2. **Versión y tamaño.** Versión explícita `u8` antes del cuerpo postcard; máximo de 4 KiB por mensaje; decodificación estricta con `take_from_bytes`, rechazando bytes sobrantes. En la versión 1, `ttl_secs` es **exactamente 30** (`core/CLAUDE.md`).
3. **QR.** Código de un solo uso de 32 bytes (PSK de Noise en la #95); sin fecha de caducidad dentro del QR (la PC controla los 2 minutos); la conversión a texto queda para las issues de interfaz.
4. **Huella.** Los primeros 8 bytes de la llave pública en hexadecimal, sin hash ni dependencias. Es un identificador visual corto, no un mecanismo de autenticación ni un sustituto de Noise.
5. **Contraseña (`SecretString`).** Serialización explícita y limitada al campo de la contraseña, sin copias intermedias innecesarias (ver "Borrado de memoria").

Llamadas mías (el usuario puede cambiarlas):

- **Byte de tipo de mensaje** después de la versión: `versión (u8) | tipo (u8) | cuerpo`. Impide decodificar un mensaje como otro (un `Ack` como `SendMessage`) y da un error propio para un tipo desconocido. Cuesta un byte.
- **Error propio del protocolo** (`protocol::ProtocolError`) en vez de crecer `vault::Error`: así no cambia el `match` exhaustivo de la fachada, y la #102 decide el mapeo.
- **Reglas de los campos de la versión 1** (ver "Mensajes"): contraseña de 1 a 1024 bytes UTF-8 (el mismo límite que una entrada), puerto distinto de 0, dirección que no sea no especificada ni multicast (loopback sí: las pruebas de la #95 y la #96 usan localhost).
- **`Ack` con dos resultados** (`Copied`, `Failed`) para que el celular distinga "quedó en el portapapeles" de "la PC no pudo escribirlo". El rechazo de un dispositivo no emparejado ocurre en el apretón de manos (#96), no en un mensaje.
- **La serialización de `SecretString` no usa la feature `serde` de `secrecy`**: se escribe un módulo auxiliar (`serialize_with` y `deserialize_with`) simétrico, y la feature no se activa.

## Verificaciones hechas (2026-11-20)

Contra el código fuente descargado de cada crate, no de memoria:

- **`secrecy` 0.10.3.** Con la feature `serde`, `SecretBox<T>` recibe `Deserialize` (`T: DeserializeOwned`), y `SecretString` el suyo: `String::deserialize(d).map(Into::into)`. **No** recibe `Serialize` salvo que `T` implemente `SerializableSecret`, y `str` no puede. `From<String> for SecretString` hace `into_boxed_str()` (sin copiar si `len == capacity`).
- **`postcard` 1.1.3, deserializar.** `deserialize_string` toma los bytes de la entrada prestada (`try_take_n_temp`), valida UTF-8 y llama a `visit_str`, que en `serde` hace `to_owned()`: una sola asignación de capacidad exacta, sin búfer intermedio. Con `into_boxed_str` no hay segunda copia. `from_bytes` **ignora** los bytes sobrantes; `take_from_bytes` devuelve el resto, que hay que exigir vacío.
- **`postcard` 1.1.3, serializar.** `to_allocvec` usa `AllocVec`, un `Vec` que crece con `extend_from_slice`: cada reubicación deja una copia sin borrar. El núcleo ya tiene `vault::entry::flavor::ZeroizingVec`, que serializa dentro de un `Zeroizing<Vec<u8>>` y borra el búfer anterior al crecer. `postcard::serialized_size` (flavor `Size`) calcula el tamaño exacto sin copiar el contenido.
- **`x25519-dalek` 3.0.0** (BSD-3-Clause, MSRV 1.85, sobre `curve25519-dalek` 5.0.0). Features: `static_secrets`, `zeroize`, `precomputed-tables` (por defecto). `StaticSecret::from([u8; 32])` guarda los bytes tal cual; `PublicKey::from(&StaticSecret)` hace `mul_base_clamped(...).to_montgomery()`. `StaticSecret` implementa `ZeroizeOnDrop` y no deriva `Debug`. Un `PublicKey` es un valor público (`Copy`, `Debug`).
- **`snow` 0.10.0** no está en `Cargo.lock`: entra en la #95. Si usa otra versión de `curve25519-dalek`, `cargo deny` avisará de versiones duplicadas (`multiple-versions = "warn"`).

## Módulos

```
core/src/protocol/
├── mod.rs        # reexporta; módulo público `vault_core::protocol`
├── error.rs      # ProtocolError, InvalidField
├── identity.rs   # PublicKey, Fingerprint, KeyPair   (+ identity/tests.rs)
└── message.rs    # constantes, codificación, PairingOffer, PairingCode, SendMessage, Ack   (+ message/tests.rs)
```

`lib.rs` agrega `pub mod protocol;`. Sin dependencias de interfaz ni de plataforma.

## Identidades (`protocol::identity`)

```rust
// La longitud (32) es `vault::PUBLIC_KEY_LEN`, que ya existe: no se define otra.
pub struct PublicKey([u8; 32]);                // Copy, Clone, PartialEq, Eq, Hash
pub struct Fingerprint([u8; 8]);               // Display: `A1B2-C3D4-E5F6-0718`
pub struct KeyPair { private: SecretBox<[u8; 32]>, public: PublicKey }
```

- `PublicKey::from_bytes([u8; 32])` y `as_bytes()`. Cualquier valor de 32 bytes es una llave pública válida a este nivel (X25519 no tiene puntos inválidos en la entrada); la prueba de que la llave sirve la hace el apretón de manos.
- `PublicKey::from_private(&SecretBox<[u8; 32]>) -> PublicKey`: deriva la pública con `x25519-dalek`. Es lo que llamará la fachada con `vault::Identity::private_key()` (la #93 no cambia).
- `PublicKey::fingerprint() -> Fingerprint`: los primeros 8 bytes de la llave, en mayúsculas y agrupados de a dos bytes (`A1B2-C3D4-E5F6-0718`). Su documentación dice, textualmente, que es un identificador visual corto para que una persona compare dos pantallas: **no autentica nada y no sustituye a Noise**; con 64 bits, un atacante podría fabricar una llave con la misma huella, así que nunca se usa para decidir en qué confiar.
- `KeyPair::generate() -> Result<KeyPair>`: 32 bytes del generador del sistema (`getrandom`) escritos directamente en el `SecretBox`; un fallo da `ProtocolError::Random`. La usará la PC (#99).
- `KeyPair::from_private(SecretBox<[u8; 32]>) -> KeyPair`, `private() -> &SecretBox<[u8; 32]>` y `public() -> PublicKey`. Los bytes de `private()` son los mismos con los que se construyó: nunca se recortan.
- `KeyPair` no deriva `Debug`; el suyo muestra solo la huella. Se borra al soltarse (`SecretBox`). La llave privada no es `Clone`.

## Mensajes (`protocol::message`)

### Formato en el cable

```
versión (u8) | tipo (u8) | cuerpo (postcard)
```

| Constante | Valor |
| --- | --- |
| `PROTOCOL_VERSION` | `1` |
| `MAX_MESSAGE_LEN` | `4096` bytes, contando versión y tipo |
| `SEND_TTL_SECS` | `30` |
| `PAIRING_CODE_LEN` | `32` |
| `MAX_PASSWORD_BYTES` | `1024` |

Tipos: `1` = `PairingOffer`, `2` = `SendMessage`, `3` = `Ack`.

**Decodificar**, en este orden (cada paso con su error):

1. Más de `MAX_MESSAGE_LEN` bytes: `TooLarge`. Se mira antes de tocar nada.
2. Menos de 2 bytes: `Truncated`.
3. Versión distinta de 1: `UnsupportedVersion(v)`, sin intentar leer el cuerpo.
4. Tipo distinto del esperado: `UnexpectedKind(k)`.
5. `postcard::take_from_bytes` del cuerpo: `DeserializeUnexpectedEnd` es `Truncated`; cualquier otro error de postcard (booleano, enum, UTF-8, varint) es `Malformed`.
6. Quedan bytes después del cuerpo: `TrailingBytes`.
7. Reglas de los campos: `InvalidField(campo)`.

**Codificar**: calcula `serialized_size`, reserva exactamente `2 + tamaño` en un `ZeroizingVec`, escribe versión y tipo con `try_push` y serializa el cuerpo en ese mismo búfer. Devuelve `Zeroizing<Vec<u8>>`. Si el total pasa de 4 KiB, `TooLarge`. Codificar valida los campos igual que decodificar (no se puede emitir lo que no se acepta).

### `PairingOffer` (contenido del QR)

```rust
pub struct PairingOffer { public_key: PublicKey, address: SocketAddr, code: PairingCode }
```

- `public_key`: la llave pública de la PC (32 bytes). `address`: IP y puerto donde escucha (IPv4 o IPv6). `code`: el código de un solo uso.
- `PairingCode`: 32 bytes en un `SecretBox<[u8; 32]>`, sin `Debug` con el valor, borrado al soltarse; `PairingCode::generate()` con `getrandom`. Es un secreto de vida corta (2 minutos y un uso): sirve de PSK en la #95.
- **Sin caducidad dentro del QR.** Los relojes de la PC y del celular no coinciden; los 2 minutos los cuenta la PC desde que muestra el QR (#95, #101).
- Reglas: puerto ≠ 0; la dirección no puede ser la no especificada (`0.0.0.0`, `::`) ni multicast (`InvalidField::Address`, `InvalidField::Port`). Una IPv6 se codifica sin zona (`scope id`): una dirección link-local sin zona no sirve para conectar, y es la #100 quien decide qué dirección anuncia la PC.
- Cuerpo: `public_key` (32 bytes) | `address` (postcard de `SocketAddr`: variante, IP de 4 o 16 bytes y puerto como varint) | `code` (32 bytes).

#### Tamaño del contenido del QR

Cuentas hechas a mano, con el puerto en el peor caso (3 bytes de varint desde el 16384, que es el caso de los puertos efímeros):

| Dirección | Bytes crudos (versión, tipo, llave, dirección, código) | Base64url sin relleno | Base32 en mayúsculas |
| --- | --- | --- | --- |
| IPv4 | 2 + 32 + (1 + 4 + 3) + 32 = **74** | 99 caracteres | 119 |
| IPv6 | 2 + 32 + (1 + 16 + 3) + 32 = **86** | 115 caracteres | 138 |

Capacidades de la norma ISO 18004 con corrección M, de la tabla (no medidas con una biblioteca): modo de bytes, versión 5 = 84, versión 6 = 106, versión 7 = 122; modo alfanumérico, versión 5 = 122, versión 6 = 154. Resultado:

- **Base64url** (modo de bytes): IPv4 pasa de la versión 5 (99 > 84) y necesita la **versión 6**; IPv6 necesita la **versión 7**. La estimación anterior de la versión 5 era errónea.
- Bytes crudos en el QR: 74 caben en la versión 5 y 86 en la 6, pero los lectores de Flutter suelen entregar texto, así que no se recomienda.
- Base32 en mayúsculas (modo alfanumérico): versión 5 con IPv4 y 6 con IPv6, a costa de un alfabeto más largo de codificar.

**La #94 no fija una versión de QR.** Fija el tope del contenido (**86 bytes**, una prueba lo vigila) y deja a la #101 medir el tamaño real con su biblioteca, con IPv4 e IPv6, antes de elegir codificación y nivel de corrección.

### `SendMessage` (envío)

```rust
pub struct SendMessage { ttl_secs: u8, password: SecretString }
```

- `SendMessage::new(password: SecretString) -> Result<Self>` toma la contraseña **por valor** (sin copiarla) y fija `ttl_secs = 30`. `ttl_secs() -> u8` y `password() -> &SecretString`.
- Reglas: `ttl_secs == 30` (`InvalidField::Ttl`); la contraseña de 1 a 1024 bytes UTF-8 (`PasswordEmpty`, `PasswordTooLong`). **Los bytes de la contraseña no se modifican nunca**: sin recortar, sin normalizar a NFC, sin quitar caracteres de control. Una contraseña en NFD llega en NFD.
- Cuerpo: `ttl_secs` (`u8`) | `password` (cadena de postcard: longitud como varint y los bytes UTF-8).
- El tiempo de vida es relativo (30 s desde que la PC lo recibe): no lleva hora, por lo mismo que el QR.
- La PC sigue siendo quien aplica el borrado a los 30 s (#98); este campo solo lo declara.
- `Debug` muestra únicamente `ttl_secs`.

### `Ack` (confirmación)

```rust
pub struct Ack { outcome: AckOutcome }
pub enum AckOutcome { Copied, Failed }
```

Cuerpo: un `enum` de postcard. Un valor desconocido es `Malformed`.

## Errores (`protocol::ProtocolError`)

`#[non_exhaustive]`, `thiserror`, `PartialEq`, `Eq`, sin datos ni secretos en ningún mensaje:

`TooLarge` · `Truncated` · `UnsupportedVersion(u8)` · `UnexpectedKind(u8)` · `Malformed` · `TrailingBytes` · `InvalidField(InvalidField)` · `Random` · `Encode`.

`InvalidField` (`#[non_exhaustive]`, `Copy`): `Ttl`, `PasswordEmpty`, `PasswordTooLong`, `Port`, `Address`.

El error de postcard nunca se reenvía tal cual (podría describir bytes del mensaje).

## Borrado de memoria

Reglas de `core/CLAUDE.md` y de la especificación ("Borrado de memoria garantizado solo dentro de `vault-core`"):

- **Contraseña al codificar.** `serialize_with` llama a `serialize_str(secret.expose_secret())`: postcard escribe la longitud y los bytes directamente en el búfer. El búfer es un `ZeroizingVec` con la capacidad exacta (`serialized_size`), así que **no crece ni reubica**; una prueba comprueba `capacity == len`. El resultado va en `Zeroizing<Vec<u8>>`.
- **Contraseña al decodificar.** La entrada es una `&[u8]` que pone quien llama (la #96 la tendrá en un `Zeroizing`). `deserialize_with` pide una `String` a postcard, que reserva una sola vez con capacidad exacta, y la convierte con `SecretString::from` (`into_boxed_str`, sin copiar). Si un campo posterior falla, o sobran bytes, el `SendMessage` ya construido se suelta y su `SecretString` se borra.
- **Excepción nueva, acotada y aceptada** (a registrar junto a las de `nfc()`, `argon2` y `chacha20poly1305` en `ARCHITECTURE.md` y en la especificación): `StaticSecret::from([u8; 32])` recibe la llave **por valor**, y un arreglo de 32 bytes es `Copy`: queda una copia en la pila de quien llama, que `StaticSecret` no borra; y `mul_base_clamped` deja el escalar recortado en registros y pila de `curve25519-dalek`. Solo se cierra con una primitiva propia (prohibida). Se mitiga leyendo la llave desde el `SecretBox` justo antes de derivar y sin retenerla. Lo mismo ocurre al decodificar un `PairingCode` (`[u8; 32]` en la pila antes de pasar al `SecretBox`): es un secreto de 2 minutos y un uso, que además se muestra en el QR.
- **Fuera de la #94:** las copias que haga `snow` de la llave privada (la #95 las revisará) y los búferes de red o de Noise (#96).

## Dependencias

- Nueva: `x25519-dalek = { version = "3", default-features = false, features = ["static_secrets", "zeroize"] }` en `core/Cargo.toml`. Aprobada por el usuario el 2026-11-20. Sin `precomputed-tables` (se deriva una vez por emparejamiento) y sin `getrandom` (la generación usa el `getrandom` del núcleo). Trae `curve25519-dalek` 5.0.0 y sus dependencias; BSD-3-Clause, ya permitida en `deny.toml`.
- `secrecy` no cambia de features. `postcard`, `serde`, `getrandom`, `zeroize` ya están.
- `core/CLAUDE.md` ya lista `x25519-dalek` entre los crates de criptografía permitidos: solo se agrega la feature fijada y su finalidad.

## Pruebas

Cada una se escribe antes del código y se comprueba que falla (`core/CLAUDE.md`).

- **Identidad.**
  - Derivación con los vectores de RFC 7748 §6.1 (la llave de Alice y la de Bob).
  - Equivalencia con el recorte: la pública de `k` es la de `k` ya recortada.
  - Los 32 bytes originales no cambian: `KeyPair::from_private(k).private()` devuelve `k` byte por byte, también con `k` sin recortar.
  - `generate` entrega llaves distintas y con 32 bytes.
  - Huella: formato `A1B2-C3D4-E5F6-0718`, los 8 primeros bytes, mayúsculas.
  - `Debug` de `KeyPair` no contiene la llave en hexadecimal, en decimal ni en bytes.
  - El `Debug` de `KeyPair`, `PairingCode` y `SendMessage` (que implementan a mano, sin derivarlo) no contiene el valor del secreto, comprobado con una llave y una contraseña reconocibles; ninguno implementa `Display`; y `KeyPair`, `PairingCode` y `SendMessage` son `ZeroizeOnDrop` (`assert_zeroize_on_drop`, como en `entry/tests.rs`).
- **Cada mensaje.**
  - Ida y vuelta de `PairingOffer` (IPv4 e IPv6), `SendMessage` y `Ack` (los dos resultados).
  - Bytes fijos de un mensaje de ejemplo por tipo (vectores en la prueba, para detectar un cambio accidental del formato).
- **Versión y tipo.** Versión 0 y 2 dan `UnsupportedVersion` sin leer el cuerpo (un cuerpo con basura sigue dando `UnsupportedVersion`); un tipo desconocido, o el de otro mensaje, da `UnexpectedKind`.
- **Truncado.** Cada prefijo propio de un mensaje válido da error (`Truncated`, nunca pánico), para cada tipo.
- **Bytes sobrantes.** Un mensaje válido con 1 byte más da `TrailingBytes`.
- **Tamaño.** Exactamente 4096 bytes se procesa (sin dar `TooLarge`); 4097 da `TooLarge` sin mirar el contenido. El contenido del QR con IPv6 y puerto de 3 bytes mide como máximo 86.
- **Campos.** `ttl_secs` 0, 29 y 31 dan `InvalidField::Ttl` (en el decodificador, armando los bytes a mano); contraseña vacía o de 1025 bytes; puerto 0; direcciones no especificadas y multicast. Una contraseña de exactamente 1024 bytes se acepta.
- **Unicode sin modificar.** Contraseñas con espacios al inicio y al final, NFC y NFD de la misma palabra (llegan distintas), emoji, caracteres combinantes y un NUL incrustado: la ida y vuelta devuelve los mismos bytes.
- **Memoria.** El resultado de `encode` de `SendMessage` y de `PairingOffer` cumple `capacity == len` (no hubo reubicación), y el búfer devuelto es `Zeroizing`.
- **Propiedades (`proptest`).** Cualquier valor válido hace la ida y vuelta; cualquier secuencia de bytes aleatoria, y cualquier mensaje válido con un byte cambiado, se decodifica sin pánico (con éxito o con error tipado).

## Documentación que cambia

- `docs/especificacion.md`: en "Emparejamiento y envío", una subsección "Tipos del protocolo (#94)" con el formato del cable, los límites, las reglas de los campos, el tamaño del QR y la huella; la fila de `x25519-dalek` en la tabla de crates; la excepción de borrado nueva.
- `ARCHITECTURE.md`: la fila de `protocol/` (`identity.rs`, `message.rs`, `error.rs`) y la excepción de borrado en "Decisiones clave".
- `core/CLAUDE.md`: la finalidad de `x25519-dalek` y sus features.
- `docs/roadmap.md`: casilla de la #94 dentro del punto del protocolo, de la etapa 3.

## Cambios en otras issues

- **#95:** compara la llave pública de este módulo con la que deriva `snow` (prueba de equivalencia) y revisa las copias de la llave privada dentro de `snow`; usa `PairingCode` como PSK de 32 bytes; decide el nombre del celular.
- **#101:** mide el QR real con su biblioteca, IPv4 e IPv6, antes de fijar codificación y nivel de corrección; el tope es de 86 bytes de contenido.
- **#102:** define el mapeo de `ProtocolError` a `VaultError`.

## Criterios de aceptación

- [ ] Ida y vuelta de cada mensaje; un mensaje truncado, con versión desconocida, con bytes sobrantes o con campos fuera de rango da un error tipado y nunca un pánico.
- [ ] `ttl_secs` distinto de 30 se rechaza al codificar y al decodificar.
- [ ] Ningún tipo con secretos (`KeyPair`, `PairingCode`, `SendMessage`) implementa `Debug` ni `Display` con el valor, y se borran al soltarse.
- [ ] La llave privada se conserva byte por byte; la pública coincide con los vectores de RFC 7748.
- [ ] El contenido del QR mide como máximo 86 bytes.
- [ ] `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`, `cargo deny check` y `cargo audit` pasan.
