# Diseño: modelo de entrada (issue #4)

Fecha: 2026-11-07 · Etapa 1 · Rama `feat/4-modelo-entrada`

## Objetivo

Definir en `vault-core` los tipos de una entrada de la bóveda y su serialización a texto plano, lista para que la sub-issue de cifrado por entrada la cifre. Cada entrada tiene dos partes:

- **Datos de búsqueda** (servicio, URL, usuario, etiquetas): se descifran al desbloquear.
- **Secreto** (contraseña, notas): se descifra solo al copiar, mostrar o enviar.

### Fuera de alcance

- Dato autenticado de 28 bytes y cifrado XChaCha20-Poly1305 de cada parte.
- Almacenamiento SQLite.
- Exponer los tipos fuera del crate: quedan `pub(crate)` hasta que exista `Vault`.
- Entradas de solo notas (sin contraseña).
- Normalización Unicode (NFC) y búsqueda sin acentos.

### Criterios de aceptación (de la issue)

- Serializar y deserializar los datos de búsqueda y el secreto devuelve el mismo contenido (con `proptest`).
- Dos identificadores generados son distintos y miden 16 bytes.
- Una versión de estructura desconocida devuelve un error, no un panic.
- El secreto no expone su valor en `Debug` ni `Display` y se borra al soltarse.
- `cargo test -p vault-core` y `clippy` pasan, y la CI está en verde.

## Decisiones

### Versión de estructura

- La tabla de entradas ya guarda la versión de estructura como columna propia, en claro, fuera de las partes cifradas (`docs/especificacion.md`, "Almacenamiento"). No se cambia la especificación por esto.
- Una sola versión por entrada, compartida por ambas partes.
- Nombres distintos para no confundirla con la versión de formato de la cabecera: `format_version` (cabecera) y `entry_schema_version` (entrada). Constante: `ENTRY_SCHEMA_VERSION: u16 = 1`.
- `decode(entry_schema_version, bytes)` acepta todas las versiones conocidas y aplica las reglas **de esa versión**, no las de la actual. Al escribir se usa siempre `ENTRY_SCHEMA_VERSION`.
- Una versión desconocida devuelve `Error::CorruptEntry`, sin panic. Encaja con `unlock()` (lista las entradas dañadas) y `reveal()` (devuelve error de entrada dañada).

### Campos y reglas de validación (v1)

| Campo | Tipo | Regla | Límite (bytes UTF-8, tras normalizar) |
| --- | --- | --- | --- |
| `service` | `String` | Se recorta; no puede quedar vacío | 256 B |
| `url` | `Option<String>` | Se recorta; `Some("")` o solo espacios → `None` | 2048 B |
| `username` | `Option<String>` | Se recorta; `Some("")` o solo espacios → `None` | 256 B |
| `tags` | `Vec<String>` | Se recortan, se descartan vacías, sin duplicados | Máx. 32 etiquetas de 64 B |
| `password` | `SecretString` | Obligatoria y no vacía; **no se recorta** | 1024 B |
| `notes` | `Option<SecretString>` | Vacía o solo espacios → `None`; el contenido no se recorta | 16384 B |

Etiquetas:

- Duplicados detectados comparando con `to_lowercase()` (Unicode): `["Trabajo", "trabajo", " TRABAJO "]` → `["Trabajo"]`.
- Se conserva la primera forma escrita y el orden de entrada.
- El límite de 32 se cuenta después de recortar, descartar vacías y quitar duplicados.
- "Médico" y "Medico" son etiquetas distintas. La búsqueda sin acentos se decide en el punto de búsqueda.
- Limitación conocida del MVP: sin normalización NFC, una misma palabra con acentos compuestos y descompuestos cuenta como dos etiquetas distintas.

### Errores

```rust
/// La entrada no se pudo leer: versión de estructura desconocida, bytes mal
/// formados o contenido que viola las reglas de su versión.
#[error("entrada dañada")]
CorruptEntry,

/// Los datos para crear o editar una entrada no cumplen las reglas.
#[error("entrada no válida: {0}")]
InvalidEntry(InvalidEntryField),
```

```rust
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum InvalidEntryField {
    ServiceEmpty, ServiceTooLong, UrlTooLong, UsernameTooLong,
    PasswordEmpty, PasswordTooLong, NotesTooLong, TooManyTags, TagTooLong,
}
```

Cada variante de `InvalidEntryField` tiene su mensaje `#[error(...)]` en español, sin valores. `InvalidEntryField` es `pub` porque forma parte de `Error`, y se reexporta desde `lib.rs` junto con `Error`.

Ningún error incluye valores de la entrada ni el detalle del error de postcard.

## Componentes

```
core/src/vault/entry.rs          # tipos, new(), encode/decode, reglas por versión
core/src/vault/entry/raw.rs      # RawSearchDataV1<'a>, RawSecretV1<'a> (serde, privado)
core/src/vault/entry/flavor.rs   # ZeroizingVec: flavor de postcard que borra al crecer
core/src/vault/entry/tests.rs    # pruebas unitarias y de propiedades
core/src/error.rs                # CorruptEntry, InvalidEntry, InvalidEntryField
```

### Tipos y firmas

```rust
pub(crate) const ENTRY_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct EntryId([u8; 16]);
impl EntryId {
    pub(crate) fn generate() -> Result<Self>;            // getrandom; falla → Error::Random
    pub(crate) fn from_bytes(bytes: [u8; 16]) -> Self;
    pub(crate) fn as_bytes(&self) -> &[u8; 16];
}

#[derive(Clone, PartialEq, Eq, Zeroize, ZeroizeOnDrop)]  // Debug propio
pub(crate) struct SearchData {
    service: String,
    url: Option<String>,
    username: Option<String>,
    tags: Vec<String>,
}
impl SearchData {
    pub(crate) fn new(service: &str, url: Option<&str>, username: Option<&str>,
                      tags: &[&str]) -> Result<Self>;
    pub(crate) fn encode(&self) -> Result<Zeroizing<Vec<u8>>>;
    pub(crate) fn decode(entry_schema_version: u16, bytes: &[u8]) -> Result<Self>;
    pub(crate) fn service(&self) -> &str;
    pub(crate) fn url(&self) -> Option<&str>;
    pub(crate) fn username(&self) -> Option<&str>;
    pub(crate) fn tags(&self) -> &[String];
}

// Sin Clone ni PartialEq; Debug propio.
pub(crate) struct Secret {
    password: SecretString,
    notes: Option<SecretString>,
}
impl Secret {
    pub(crate) fn new(password: SecretString, notes: Option<SecretString>) -> Result<Self>;
    pub(crate) fn encode(&self) -> Result<Zeroizing<Vec<u8>>>;
    pub(crate) fn decode(entry_schema_version: u16, bytes: &[u8]) -> Result<Self>;
    pub(crate) fn password(&self) -> &SecretString;
    pub(crate) fn notes(&self) -> Option<&SecretString>;
}
```

`Secret::new` recibe `SecretString`, no `&str`, para que la contraseña cruce la API siempre envuelta (`core/CLAUDE.md`).

`SearchData` y `Secret` **no** derivan `Serialize` ni `Deserialize`. Solo se construyen con `new()` o `decode()`, y los campos son privados: no existe una entrada sin validar.

### Reglas compartidas por versión

```rust
fn build_search_data_v1(service: &str, url: Option<&str>, username: Option<&str>,
                        tags: &[&str]) -> std::result::Result<SearchData, InvalidEntryField>;
fn build_secret_v1(password: SecretString, notes: Option<SecretString>)
                        -> std::result::Result<Secret, InvalidEntryField>;
```

- `new()` mapea el error a `Error::InvalidEntry(campo)`.
- `decode()` lo mapea a `Error::CorruptEntry`.
- Al decodificar se aplica la misma normalización que en `new()` (un solo camino de código). Lo que viola una regla, como un servicio vacío o demasiado largo, es `CorruptEntry`.
- Una v2 futura agrega `build_*_v2` sin tocar las de v1.

## Flujo de datos

### Codificar (siempre con `ENTRY_SCHEMA_VERSION`)

1. Se arma `RawSearchDataV1<'_>` / `RawSecretV1<'_>` tomando prestado del tipo validado (`&str`, `Option<&str>`, `Vec<&str>`; en el secreto, mediante `expose_secret()`). El `Vec<&str>` de etiquetas solo contiene punteros.
2. `postcard::serialize_with_flavor(&raw, ZeroizingVec::with_capacity(256))`.
3. Se devuelve `Zeroizing<Vec<u8>>`.

### Decodificar

1. Versión: `1` → `decode_v1`; cualquier otra → `CorruptEntry` sin leer los bytes.
2. `postcard::take_from_bytes::<RawSearchDataV1>(bytes)`: error → `CorruptEntry`; bytes sobrantes → `CorruptEntry`.
3. Los campos crudos se toman prestados del búfer `Zeroizing<Vec<u8>>` de quien llama (`#[serde(borrow)]` en `Option<&str>` y `Vec<&str>`, para que postcard no copie a `String`).
4. `build_*_v1` crea los `String` con `str::to_owned()`, que reserva exactamente la longitud. Para el secreto: `SecretString::from(raw.password.to_owned())`. Un comentario en el código explica que `into_boxed_str` no reasigna porque la capacidad es igual a la longitud, así que no queda una copia sin borrar.

### Flavor `ZeroizingVec`

Flavor de serialización de postcard (`postcard::ser_flavors::Flavor`) que escribe en un `Zeroizing<Vec<u8>>`:

- `try_extend`: si los datos no caben, reserva un búfer nuevo con capacidad `max(2·cap, len + data.len())`, copia y reemplaza. El búfer anterior se borra al soltarse (`Zeroizing` borra toda la capacidad). Nunca deja que `Vec` reasigne por su cuenta.
- `try_push` delega en `try_extend`.
- `finalize` devuelve el `Zeroizing<Vec<u8>>`.

La firma exacta del trait se verifica con la documentación de postcard 1.x antes de implementarlo.

## Debug, Display y borrado de memoria

- `SearchData`: `Debug` propio que oculta los valores, por ejemplo `SearchData { service: "[oculto]", url: Some("[oculto]"), username: None, tags: 3 }`. Muestra qué campos hay y cuántas etiquetas, nunca su contenido.
- `Secret`: `Debug` propio, por ejemplo `Secret { password: "[oculto]", notes: Some("[oculto]") }`.
- `EntryId` deriva `Debug`: el identificador no es secreto.
- Ningún tipo implementa `Display`.
- `SearchData` deriva `Zeroize` y `ZeroizeOnDrop`.
- `Secret` declara `impl ZeroizeOnDrop for Secret {}` como marca. La garantía la dan sus campos `SecretString`, y queda comentado en el código.

## Dependencias nuevas

No son criptográficas.

- `serde` (`default-features = false`, `features = ["derive"]`)
- `postcard` (`default-features = false`, `features = ["alloc"]`)
- `proptest` (solo de desarrollo)

Se verifican licencias y avisos con `cargo deny check` y `cargo audit`.

## Pruebas

| Criterio | Prueba |
| --- | --- |
| Ida y vuelta | `proptest` con estrategias de entradas válidas (Unicode, espacios, etiquetas repetidas con distintas mayúsculas): `decode(ENTRY_SCHEMA_VERSION, encode(new(x))) == new(x)`. El secreto se compara con `expose_secret()` |
| Identificador | Dos `generate()` son distintos; `as_bytes().len() == 16`; `from_bytes`/`as_bytes` ida y vuelta |
| Versión desconocida | `decode(0, …)`, `decode(2, …)` y `decode(u16::MAX, …)` → `CorruptEntry` |
| `Debug` | `format!("{:?}")` de `Secret` y `SearchData` no contiene ningún valor |
| Sin `Display` | Truco de constantes inherentes en la prueba, sin dependencias nuevas |
| Borrado al soltar | Comprobación en compilación con `fn assert_zod<T: ZeroizeOnDrop>()` para `SearchData` y `Secret`. No se lee memoria liberada: sería comportamiento indefinido |
| Reglas de validación | Una prueba por regla. Por cada límite: el valor justo en el límite pasa y un byte más falla con su `InvalidEntryField` |
| Normalización | Recorte del servicio; `url`/`username` vacíos → `None`; etiquetas: recorte, vacías descartadas, duplicados sin distinguir mayúsculas, primera forma y orden conservados; contraseña con espacios intacta; notas vacías → `None` |
| Bytes inválidos no producen entrada | Bytes válidos para postcard pero inválidos para las reglas (servicio vacío, 33 etiquetas, contraseña de 1025 B) → `CorruptEntry` |
| Bytes sobrantes | `encode` + un byte extra → `CorruptEntry` |
| Robustez | `proptest`: un búfer arbitrario pasado a `decode` devuelve `Ok` o `Err`, nunca panic |
| Flavor | `ZeroizingVec` con capacidad inicial 1 conserva el contenido tras crecer varias veces |

## Documentación

- `ARCHITECTURE.md`: el borrado de memoria está garantizado dentro de `vault-core` y es de mejor esfuerzo fuera (Dart no permite borrar sus cadenas).
- `docs/especificacion.md`, sección "Entradas": campos obligatorios y opcionales, reglas de normalización, límites, etiquetas sin distinguir mayúsculas, NFC como limitación conocida.
- `docs/roadmap.md`: marcar "Modelo de entrada" como terminado.

## Para la siguiente issue (cifrado por entrada)

- Reservar `len + 16` en el búfer `Zeroizing` antes de `encrypt_in_place`, para que la etiqueta quepa sin reasignar.
- Descifrar con `decrypt_in_place` sobre `Zeroizing<Vec<u8>>` y pasar ese búfer a `decode`.

## Commits previstos

1. `chore:` agregar `serde`, `postcard` y `proptest`
2. `feat:` identificador de entrada
3. `feat:` errores de entrada y reglas de validación de la v1
4. `feat:` tipos `SearchData` y `Secret` con `new()`, `Debug` propio y borrado
5. `feat:` flavor `ZeroizingVec` para postcard
6. `feat:` `encode`/`decode` con versión de estructura
7. `test:` pruebas de propiedades
8. `docs:` especificación, arquitectura y roadmap
