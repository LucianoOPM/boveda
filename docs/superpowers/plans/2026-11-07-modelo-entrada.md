# Plan de implementación: modelo de entrada (issue #4)

> **Para agentes:** SUB-SKILL REQUERIDA: usa superpowers:subagent-driven-development (recomendada) o superpowers:executing-plans para implementar este plan tarea por tarea. Los pasos usan casillas (`- [ ]`) para el seguimiento.

**Objetivo:** tipos `EntryId`, `SearchData` y `Secret` en `vault-core`, con validación por versión y serialización postcard que no deja copias sin borrar.

**Arquitectura:** `vault/entry.rs` define los tipos validados y las reglas v1. `entry/raw.rs` define estructuras serde que toman prestado (`&str`) en ambos sentidos. `entry/flavor.rs` es un flavor de postcard que escribe en `Zeroizing<Vec<u8>>` y borra el búfer anterior al crecer. `new()` y `decode()` comparten las mismas funciones `build_*_v1`.

**Tecnologías:** Rust 2024, `serde` 1, `postcard` 1 (`alloc`), `secrecy` 0.10, `zeroize` 1.9, `proptest` (desarrollo).

**Especificación:** `docs/superpowers/specs/2026-11-07-modelo-entrada-design.md`. Léela antes de empezar: este plan no repite sus tablas de reglas.

## Restricciones globales

- Identificadores en inglés; comentarios, documentación y commits en español (Conventional Commits).
- Las pruebas unitarias van en `core/src/vault/entry/tests.rs`; `entry.rs` termina con `#[cfg(test)] mod tests;`.
- Todo es `pub(crate)`, salvo `InvalidEntryField` (es `pub` y se reexporta desde `lib.rs`).
- Ningún tipo con secretos o datos de usuario deriva `Debug` ni implementa `Display`. Ningún error incluye valores ni el detalle de postcard.
- La API no hace panic: nada de `unwrap`, `expect` ni indexación que pueda fallar fuera de las pruebas.
- Límites en bytes UTF-8 tras normalizar: servicio 256, usuario 256, URL 2048, contraseña 1024, notas 16384, máximo 32 etiquetas de 64.
- `ENTRY_SCHEMA_VERSION: u16 = 1`. Una versión desconocida → `Error::CorruptEntry`.
- Recortar con `str::trim()` (espacios Unicode). La contraseña y el contenido de las notas no se recortan nunca.
- Los `String` se construyen desde `&str` con `to_owned()`/`String::from` (capacidad exacta), para que `SecretString::from(String)` → `into_boxed_str` no reasigne.
- Antes de cada commit: `cargo test -p vault-core` y `cargo clippy --workspace --all-targets -- -D warnings`. `cargo fmt --all` siempre.

## Foco de revisión

1. Espacios Unicode: un servicio que solo contiene `"\u{00A0}"` (espacio de no separación) debe dar `ServiceEmpty`, no guardarse como un servicio invisible. → Tarea 2.
2. El límite se mide después de recortar: `"  " + 256×"a" + "  "` es válido. → Tarea 2.
3. El límite es en bytes, no en caracteres: 128×`"ñ"` (256 B) pasa y 129×`"ñ"` falla con `ServiceTooLong`. → Tarea 2.
4. El máximo de 32 etiquetas se cuenta después de quitar duplicados: 40 etiquetas que se reducen a 32 distintas son válidas. → Tarea 2.
5. Bytes dañados con un prefijo de longitud enorme (por ejemplo, el número de etiquetas codificado como varint `u32::MAX`) devuelven `CorruptEntry` sin reservar memoria gigante ni hacer panic. → Tarea 5.

---

### Tarea 1: dependencias e identificador de entrada

**Archivos:**
- Modificar: `core/Cargo.toml`
- Crear: `core/src/vault/entry.rs`, `core/src/vault/entry/tests.rs`
- Modificar: `core/src/vault/mod.rs` (agregar `pub(crate) mod entry;` y actualizar el comentario del módulo)

**Interfaces:**
- Produce: `EntryId` (`Clone, Copy, PartialEq, Eq, Hash, Debug`), `EntryId::generate() -> Result<EntryId>`, `EntryId::from_bytes([u8; 16]) -> EntryId`, `EntryId::as_bytes(&self) -> &[u8; 16]`, `pub(crate) const ENTRY_ID_LEN: usize = 16`.

- [ ] **Paso 1: agregar las dependencias**

En `core/Cargo.toml`:
```toml
[dependencies]
postcard = { version = "1", default-features = false, features = ["alloc"] }
serde = { version = "1", default-features = false, features = ["derive"] }

[dev-dependencies]
proptest = "1"
```
Ejecuta `cargo deny check` y `cargo audit`. Ambos deben pasar sin errores. Si una licencia transitiva no está en `deny.toml`, detente y pregunta; no la agregues por tu cuenta.

- [ ] **Paso 2: commit**

```bash
git add core/Cargo.toml Cargo.lock
git commit -m "chore: agregar serde, postcard y proptest al núcleo"
```

- [ ] **Paso 3: escribir las pruebas que fallan** en `entry/tests.rs`

```rust
#[test]
fn generated_ids_are_16_bytes_and_distinct() {
    let a = EntryId::generate().unwrap();
    let b = EntryId::generate().unwrap();
    assert_eq!(a.as_bytes().len(), ENTRY_ID_LEN);
    assert_ne!(a, b);
}

#[test]
fn id_round_trips_through_bytes() {
    let bytes: [u8; 16] = core::array::from_fn(|i| i as u8);
    assert_eq!(EntryId::from_bytes(bytes).as_bytes(), &bytes);
}
```

- [ ] **Paso 4:** `cargo test -p vault-core entry` → FALLA (no compila: falta `EntryId`).

- [ ] **Paso 5: implementar `EntryId`** en `entry.rs`. `generate()` usa `getrandom::fill`; el error se mapea a `Error::Random`, como en `crypto/keys.rs`.

- [ ] **Paso 6:** `cargo test -p vault-core entry` → PASA.

- [ ] **Paso 7: commit**: `feat: agregar el identificador de entrada`

---

### Tarea 2: errores y `SearchData` con las reglas v1

**Archivos:**
- Modificar: `core/src/error.rs`, `core/src/lib.rs` (`pub use error::{Error, InvalidEntryField, Result};`)
- Modificar: `core/src/vault/entry.rs`, `core/src/vault/entry/tests.rs`

**Interfaces:**
- Consume: nada de tareas anteriores además del módulo.
- Produce:
  - `Error::CorruptEntry` (`#[error("entrada dañada")]`) y `Error::InvalidEntry(InvalidEntryField)` (`#[error("entrada no válida: {0}")]`).
  - `#[non_exhaustive] pub enum InvalidEntryField { ServiceEmpty, ServiceTooLong, UrlTooLong, UsernameTooLong, PasswordEmpty, PasswordTooLong, NotesTooLong, TooManyTags, TagTooLong }`, que deriva `Debug, Clone, Copy, PartialEq, Eq, thiserror::Error` y tiene un mensaje en español por variante (p. ej. `"el servicio está vacío"`).
  - Constantes `MAX_SERVICE_LEN = 256`, `MAX_URL_LEN = 2048`, `MAX_USERNAME_LEN = 256`, `MAX_PASSWORD_LEN = 1024`, `MAX_NOTES_LEN = 16384`, `MAX_TAGS = 32`, `MAX_TAG_LEN = 64` (`usize`, `pub(crate)`).
  - `ENTRY_SCHEMA_VERSION: u16 = 1`.
  - `SearchData` (`Clone, PartialEq, Eq, Zeroize, ZeroizeOnDrop`, `Debug` propio) con `new(service: &str, url: Option<&str>, username: Option<&str>, tags: &[&str]) -> Result<SearchData>` y los métodos de acceso `service() -> &str`, `url() -> Option<&str>`, `username() -> Option<&str>`, `tags() -> &[String]`.
  - Privada: `fn build_search_data_v1(service: &str, url: Option<&str>, username: Option<&str>, tags: &[&str]) -> std::result::Result<SearchData, InvalidEntryField>`. `new()` = `build_search_data_v1(..).map_err(Error::InvalidEntry)`.

- [ ] **Paso 1: escribir las pruebas que fallan.** Usa un helper `fn search(service: &str) -> Result<SearchData>` que llame a `new(service, None, None, &[])`. Cada prueba lleva su aserción exacta:

| Prueba | Aserción |
| --- | --- |
| `service_is_trimmed` | `new(" GitHub ", ..)` → `service() == "GitHub"` |
| `blank_service_is_rejected` | `"   "` → `Err(InvalidEntry(ServiceEmpty))` |
| `nbsp_only_service_is_rejected` | `"\u{00A0}"` → `Err(InvalidEntry(ServiceEmpty))` |
| `service_limit_is_measured_after_trim` | `format!("  {}  ", "a".repeat(256))` → `Ok` |
| `service_limit_is_in_bytes` | `"ñ".repeat(128)` → `Ok`; `"ñ".repeat(129)` → `Err(InvalidEntry(ServiceTooLong))` |
| `blank_url_and_username_become_none` | `Some("")`, `Some("  ")` → `url() == None`, `username() == None` |
| `url_and_username_are_trimmed` | `Some(" a ")` → `Some("a")` |
| `url_limit` | 2048 B → `Ok`; 2049 → `UrlTooLong` |
| `username_limit` | 256 B → `Ok`; 257 → `UsernameTooLong` |
| `tags_are_trimmed_and_blank_dropped` | `[" a ", "", "  ", "b"]` → `["a", "b"]` |
| `duplicate_tags_ignore_case_keep_first_form_and_order` | `["Trabajo", "casa", "trabajo", " TRABAJO "]` → `["Trabajo", "casa"]` |
| `accented_tags_are_distinct` | `["Médico", "Medico"]` → las dos |
| `tag_limit` | etiqueta de 64 B → `Ok`; de 65 → `TagTooLong` |
| `tag_count_limit` | 32 distintas → `Ok`; 33 → `TooManyTags` |
| `tag_count_is_measured_after_dedup` | 32 distintas más 8 repetidas en mayúsculas (40 en total) → `Ok` con 32 |
| `search_data_debug_hides_values` | `format!("{:?}", d)` no contiene `"GitHub"`, `"alice"`, `"example.com"` ni el texto de las etiquetas |
| `search_data_zeroizes_on_drop` | `assert_zeroize_on_drop::<SearchData>()` (helper genérico `fn assert_zeroize_on_drop<T: ZeroizeOnDrop>() {}`) |

- [ ] **Paso 2:** `cargo test -p vault-core entry` → FALLA (no compila).

- [ ] **Paso 3: implementar los errores, las constantes, `SearchData` y `build_search_data_v1`.**
  - Orden de las comprobaciones: servicio, URL, usuario, etiquetas. Se devuelve el primer error.
  - Para los duplicados: compara `tag.to_lowercase()` contra las claves ya vistas en un `Vec<String>` local; con 32 como máximo no hace falta un `HashSet`. Envuelve esas claves en `Zeroizing`: también son datos del usuario.
  - El límite de cantidad se aplica después de quitar duplicados.
  - El `Debug` propio sigue el formato de la especificación: `SearchData { service: "[oculto]", url: Some("[oculto]"), username: None, tags: 3 }`.

- [ ] **Paso 4:** `cargo test -p vault-core entry` y clippy → PASAN.

- [ ] **Paso 5: commit**: `feat: agregar los datos de búsqueda de la entrada con sus reglas`

---

### Tarea 3: `Secret` con las reglas v1

**Archivos:**
- Modificar: `core/src/vault/entry.rs`, `core/src/vault/entry/tests.rs`

**Interfaces:**
- Consume: `InvalidEntryField`, `MAX_PASSWORD_LEN` y `MAX_NOTES_LEN` (tarea 2).
- Produce:
  - `Secret` (sin `Clone`, `PartialEq` ni `Debug` derivado; `Debug` propio). `impl ZeroizeOnDrop for Secret {}`, con un comentario que dice que la garantía la dan los campos `SecretString`.
  - `Secret::new(password: SecretString, notes: Option<SecretString>) -> Result<Secret>`, `password() -> &SecretString`, `notes() -> Option<&SecretString>`.
  - Privada: `fn build_secret_v1(password: SecretString, notes: Option<SecretString>) -> std::result::Result<Secret, InvalidEntryField>`.

- [ ] **Paso 1: escribir las pruebas que fallan**

| Prueba | Aserción |
| --- | --- |
| `empty_password_is_rejected` | `""` → `Err(InvalidEntry(PasswordEmpty))` |
| `password_is_not_trimmed` | `" x "` → `password().expose_secret() == " x "` |
| `spaces_only_password_is_kept` | `"   "` → `Ok`, se conserva igual |
| `password_limit` | 1024 B → `Ok`; 1025 → `PasswordTooLong` |
| `blank_notes_become_none` | `Some("")`, `Some(" \n ")` → `notes().is_none()` |
| `notes_are_not_trimmed` | `Some(" n ")` → `" n "` |
| `notes_limit` | 16384 B → `Ok`; 16385 → `NotesTooLong` |
| `secret_debug_hides_values` | `{:?}` no contiene la contraseña ni las notas |
| `secret_does_not_implement_display` | ver el truco de abajo |
| `secret_zeroizes_on_drop` | `assert_zeroize_on_drop::<Secret>()` |

Truco para comprobar que no hay `Display`, sin dependencias nuevas:
```rust
trait NoDisplay { const IMPLS_DISPLAY: bool = false; }
impl<T: ?Sized> NoDisplay for T {}
struct Probe<T: ?Sized>(core::marker::PhantomData<T>);
#[allow(dead_code)]
impl<T: ?Sized + core::fmt::Display> Probe<T> { const IMPLS_DISPLAY: bool = true; }

assert!(!<Probe<Secret>>::IMPLS_DISPLAY);
assert!(!<Probe<SearchData>>::IMPLS_DISPLAY);
```

- [ ] **Paso 2:** `cargo test -p vault-core entry` → FALLA.
- [ ] **Paso 3: implementar `Secret` y `build_secret_v1`.** Las longitudes se leen con `expose_secret().len()`. Las notas en blanco (`trim().is_empty()`) se convierten en `None`, sin copiar el valor.
- [ ] **Paso 4:** pruebas y clippy → PASAN.
- [ ] **Paso 5: commit**: `feat: agregar el secreto de la entrada con sus reglas`

---

### Tarea 4: flavor `ZeroizingVec`

**Archivos:**
- Crear: `core/src/vault/entry/flavor.rs` (declarado en `entry.rs` como `mod flavor;`)
- Modificar: `core/src/vault/entry/tests.rs`

**Interfaces:**
- Produce: `pub(super) struct ZeroizingVec(Zeroizing<Vec<u8>>)`, `ZeroizingVec::with_capacity(cap: usize) -> Self` e `impl postcard::ser_flavors::Flavor for ZeroizingVec { type Output = Zeroizing<Vec<u8>>; }`.

Firma del trait (verificada en la documentación de postcard 1.x): `fn try_push(&mut self, data: u8) -> postcard::Result<()>`, `fn try_extend(&mut self, data: &[u8]) -> postcard::Result<()>` (opcional) y `fn finalize(self) -> postcard::Result<Self::Output>`.

- [ ] **Paso 1: escribir las pruebas que fallan**
  - `flavor_keeps_content_across_growth`: `ZeroizingVec::with_capacity(1)`; se llama `try_extend` con 1, 3, 10 y 100 bytes conocidos y `try_push` con 1 byte; `finalize()` devuelve exactamente esos 115 bytes concatenados.
  - `flavor_serializes_like_to_allocvec`: `postcard::serialize_with_flavor(&("abc", Some(7u32), [1u8, 2]), ZeroizingVec::with_capacity(1))` es igual a `postcard::to_allocvec` del mismo valor.
- [ ] **Paso 2:** `cargo test -p vault-core flavor` → FALLA.
- [ ] **Paso 3: implementar.** `try_extend`: si `len + data.len() > capacity`, reserva un `Zeroizing<Vec<u8>>` nuevo con `Vec::with_capacity(max(2 * cap, len + data.len()))`, copia el contenido actual, asígnalo a `self.0` (el anterior se borra al soltarse, incluida toda su capacidad) y después `extend_from_slice`, que ya no reasigna. Usa `checked_add` y devuelve `postcard::Error::SerializeBufferFull` si se desborda. `try_push` delega en `try_extend`. Explica en un comentario por qué nunca se deja crecer al `Vec` por su cuenta.
- [ ] **Paso 4:** pruebas y clippy → PASAN.
- [ ] **Paso 5: commit**: `feat: agregar un flavor de postcard que borra el búfer al crecer`

---

### Tarea 5: `encode`/`decode` con versión de estructura

**Archivos:**
- Crear: `core/src/vault/entry/raw.rs` (`mod raw;` en `entry.rs`)
- Modificar: `core/src/vault/entry.rs`, `core/src/vault/entry/tests.rs`

**Interfaces:**
- Consume: `ZeroizingVec` (tarea 4); `build_search_data_v1` y `build_secret_v1` (tareas 2 y 3).
- Produce:
  - `SearchData::encode(&self) -> Result<Zeroizing<Vec<u8>>>` y `SearchData::decode(entry_schema_version: u16, bytes: &[u8]) -> Result<SearchData>`.
  - `Secret::encode(&self) -> Result<Zeroizing<Vec<u8>>>` y `Secret::decode(entry_schema_version: u16, bytes: &[u8]) -> Result<Secret>`.
  - En `raw.rs`, `#[derive(Serialize, Deserialize)]`:
    - `RawSearchDataV1<'a> { service: &'a str, url: Option<&'a str>, username: Option<&'a str>, tags: Vec<&'a str> }`
    - `RawSecretV1<'a> { password: &'a str, notes: Option<&'a str> }`
    - `#[serde(borrow)]` en los campos `Option<&str>` y `Vec<&str>`. **El orden de los campos es el formato en disco v1: no se cambia.**

- [ ] **Paso 1: escribir las pruebas que fallan**

| Prueba | Aserción |
| --- | --- |
| `search_data_v1_bytes_are_fixed` | `new("a", None, Some("b"), &["x"])` → `encode() == [0x01, 0x61, 0x00, 0x01, 0x01, 0x62, 0x01, 0x01, 0x78]` |
| `secret_v1_bytes_are_fixed` | `new("p", None)` → `encode() == [0x01, 0x70, 0x00]` |
| `search_data_round_trips` | ejemplo con los cuatro campos y etiquetas Unicode: `decode(ENTRY_SCHEMA_VERSION, &encode()) == original` |
| `secret_round_trips` | contraseña y notas iguales tras `expose_secret()` |
| `unknown_schema_version_is_corrupt` | versiones `0`, `2` y `u16::MAX` con bytes válidos → `Err(CorruptEntry)` en ambas partes |
| `trailing_bytes_are_corrupt` | `encode()` + `[0x00]` → `Err(CorruptEntry)` |
| `truncated_bytes_are_corrupt` | `encode()` sin su último byte → `Err(CorruptEntry)` |
| `rule_violating_bytes_do_not_produce_entry` | `postcard::to_allocvec` de un `RawSearchDataV1` con servicio `"  "`; de otro con 33 etiquetas distintas; de un `RawSecretV1` con contraseña de 1025 B → `Err(CorruptEntry)`, nunca `InvalidEntry` |
| `decode_normalizes_like_new` | `RawSearchDataV1` con `" GitHub "`, url `Some(" ")` y etiquetas `["a", "A"]` → `service() == "GitHub"`, `url() == None`, `tags() == ["a"]` |
| `huge_length_prefix_is_corrupt` | `[0x01, 0x61, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0x0F]` (número de etiquetas = `u32::MAX`) → `Err(CorruptEntry)` sin panic |

- [ ] **Paso 2:** `cargo test -p vault-core entry` → FALLA.
- [ ] **Paso 3: implementar.**
  - `encode`: arma el `Raw*V1` prestado (en el secreto, con `expose_secret()`) y llama a `postcard::serialize_with_flavor(&raw, ZeroizingVec::with_capacity(256))`. El error de postcard se convierte en `CorruptEntry`; en la práctica no ocurre.
  - `decode`: hace `match entry_schema_version { 1 => decode_v1(bytes), _ => Err(Error::CorruptEntry) }`. `decode_v1` usa `postcard::take_from_bytes::<Raw*V1>`; si falla o el resto no está vacío → `CorruptEntry`. Después llama a `build_*_v1(...)` y convierte el error en `CorruptEntry`. Para el secreto: `SecretString::from(raw.password)`, que internamente usa `String::from(&str)` con capacidad exacta, así que `into_boxed_str` no reasigna. Déjalo explicado en un comentario.
  - `build_secret_v1` recibe `SecretString` ya construidos, así que la conversión ocurre en `decode_v1`.
- [ ] **Paso 4:** pruebas y clippy → PASAN.
- [ ] **Paso 5: commit**: `feat: serializar las partes de la entrada con versión de estructura`

---

### Tarea 6: pruebas de propiedades

**Archivos:**
- Modificar: `core/src/vault/entry/tests.rs`

**Interfaces:**
- Consume: `new`, `encode` y `decode` de `SearchData` y `Secret`.

- [ ] **Paso 1: escribir las propiedades** con `proptest!`:
  - `search_data_round_trip_prop`: entradas `any::<String>()` (servicio), `option::of(any::<String>())` (URL y usuario) y `vec(any::<String>(), 0..40)` (etiquetas), más casos con espacios y mayúsculas mezcladas: `prop_oneof![any::<String>(), "[ \\t]{0,3}[a-zA-ZñÑ]{1,8}[ \\t]{0,3}"]`. Si `new(...)` devuelve `Ok(d)`, debe cumplirse `decode(ENTRY_SCHEMA_VERSION, &d.encode()?) == d`.
  - `search_data_new_is_idempotent_prop`: con las mismas entradas, si `new` devuelve `Ok(d)`, reconstruir desde los campos de `d` devuelve un valor igual a `d`.
  - `secret_round_trip_prop`: contraseña `any::<String>()` y notas `option::of(any::<String>())`; si `new` devuelve `Ok`, la ida y vuelta conserva `expose_secret()` de ambos campos.
  - `decode_never_panics_prop`: `vec(any::<u8>(), 0..512)` y `any::<u16>()` como versión. `SearchData::decode` y `Secret::decode` devuelven `Ok` o `Err`; la prueba solo comprueba que terminan sin panic.
- [ ] **Paso 2:** `cargo test -p vault-core entry` → PASAN. Estas pruebas cubren código ya hecho; si alguna falla, es un error de las tareas 2 a 5 y se corrige ahí.
- [ ] **Paso 3: commit**: `test: agregar pruebas de propiedades del modelo de entrada`

---

### Tarea 7: documentación y cierre

**Archivos:**
- Modificar: `ARCHITECTURE.md`, `docs/especificacion.md`, `docs/roadmap.md`, `core/src/vault/mod.rs` (comentario del módulo, si no se hizo en la tarea 1)

- [ ] **Paso 1: `ARCHITECTURE.md`.**
  - En "Decisiones clave", o en una nota bajo `core/`: el borrado de memoria está garantizado dentro de `vault-core` y es de mejor esfuerzo fuera, porque Dart no permite borrar sus cadenas.
  - En el mapa del repositorio, bajo `docs/`: `superpowers/specs/` (diseños) y `superpowers/plans/` (planes de implementación).
- [ ] **Paso 2: `docs/especificacion.md`, sección "Entradas".**
  - Agrega un punto con los campos obligatorios y opcionales, las reglas de normalización, los límites y las etiquetas sin distinguir mayúsculas; remite al documento de diseño para el detalle.
  - Agrega "sin normalización NFC" como limitación conocida del MVP.
  - Agrega la nota para la sub-issue de cifrado: reservar `len + 16` y usar `encrypt_in_place`/`decrypt_in_place` sobre `Zeroizing<Vec<u8>>`.
- [ ] **Paso 3: `docs/roadmap.md`.** Marca `[x]` en "Modelo de entrada…".
- [ ] **Paso 4: verificación completa.** `cargo fmt --all --check`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo audit` y `cargo deny check`: todos deben terminar sin errores.
- [ ] **Paso 5: commit**: `docs: registrar el modelo de entrada en la especificación y la arquitectura`
