# Diseño: cifrado por entrada (issue #5)

Fecha: 2026-11-07 · Etapa 1 · Rama `feat/5-cifrado-entrada`

## Objetivo

Cifrar y descifrar con XChaCha20-Poly1305 cada parte de una entrada (datos de búsqueda y secreto). Cada parte queda ligada a su entrada, a su versión de estructura y a su tipo mediante el dato autenticado. Se parte de los tipos de la #4 (`EntryId`, `SearchData`, `Secret`) y de la llave de bóveda de la #3 (`VaultKey`).

### Fuera de alcance

- Guardar las partes en SQLite (#6).
- Bóvedas de ejemplo en `core/tests/fixtures/` (#10). La #10 debe incluir al menos una entrada v1 con todos los campos opcionales, para cubrir lo mismo a través de la API pública ([nota en la #10](https://github.com/LucianoOPM/boveda/issues/10#issuecomment-6711570863)).
- Pasar la cabecera al mismo patrón tipado de dato autenticado (otra issue).
- Detectar el rollback de una entrada a una versión anterior: queda fuera del modelo de amenazas de la v1.

### Criterios de aceptación (de la issue)

- El dato autenticado coincide byte a byte con un vector conocido escrito en la prueba.
- Cifrar y descifrar devuelve exactamente el mismo contenido (con `proptest`).
- Alterar cualquier byte del nonce, del texto cifrado o de la etiqueta se detecta.
- Mover el secreto de una entrada a otra se rechaza.
- Intercambiar la parte de búsqueda y el secreto de una misma entrada se rechaza.
- Descifrar con otra versión de estructura se rechaza.
- Dos cifrados del mismo contenido producen resultados distintos.
- `cargo test -p vault-core` y `clippy` pasan, y la CI está en verde.

## Decisiones

- **Dónde vive cada cosa.** `crypto/aead.rs` arma el dato autenticado y cifra o descifra una parte; recibe `&[u8; 16]` y no `EntryId`, para que `crypto` no dependa de `vault`. El cifrado de la entrada completa vive en `vault/entry/sealed.rs`, que llama a `aead`.
- **Dato autenticado.** Lo arma una función privada en un `[u8; AAD_LEN]` en la pila. Solo `seal_part` y `open_part` son visibles (`pub(crate)`). El `u16` va en little-endian, igual que en la cabecera.
- **`Part`.** `#[repr(u8)]` con valores explícitos: `Search = 1`, `Secret = 2`.
- **Parte cifrada.** Newtype `EncryptedPart(Vec<u8>)` con `nonce (24) | cifrado | etiqueta (16)`, el mismo contenido que se guarda como `BLOB`.
- **Errores.** Ninguna variante nueva. Una parte demasiado corta, un fallo de autenticación o un fallo de deserialización son `Error::CorruptEntry` para esa entrada. Un fallo del generador aleatorio es `Error::Random`. Esa entrada falla sola, sin abortar el desbloqueo de la bóveda: en la #7 `unlock()` la listará como dañada.
- **Una sola versión por entrada.** `SealedEntry::seal` cifra siempre las dos partes con `ENTRY_SCHEMA_VERSION`.
- **API de `aead`.** Se usa `encrypt_inout_detached` (como en `keys.rs`) porque `encrypt_in_place_detached` está deprecada en `aead 0.6.1` y rompería `clippy -D warnings`. Para descifrar se usa `decrypt_in_place`.
- **Sin `postcard::to_extend`.** Escribe con `Vec::extend`, que reasigna sin borrar el búfer anterior. Se acepta una copia extra entre dos búferes `Zeroizing`.

## Componentes

```
core/src/crypto/aead.rs              # dato autenticado, Part, EncryptedPart, seal_part/open_part
core/src/crypto/aead/tests.rs        # vector de 28 bytes, ida y vuelta, manipulación
core/src/crypto/keys.rs              # TAG_LEN pasa a pub(crate)
core/src/vault/entry/sealed.rs       # SealedEntry: cifrar y descifrar una entrada completa
core/src/vault/entry/sealed/tests.rs # vectores v1, secreto movido, partes intercambiadas
```

### `crypto/aead.rs`

```rust
pub(crate) const AAD_LEN: usize = 28;
const AAD_LABEL: &[u8; 9] = b"boveda-v1";
/// Nonce más etiqueta: la parte cifrada más corta posible.
pub(crate) const MIN_SEALED_LEN: usize = NONCE_LEN + TAG_LEN; // 40

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Part {
    Search = 1,
    Secret = 2,
}

/// `nonce (24) | cifrado | etiqueta (16)`. `Debug` propio: solo la longitud.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct EncryptedPart(Vec<u8>);

impl EncryptedPart {
    /// Reconstruye una parte leída del almacenamiento. No valida: la
    /// longitud la comprueba `open_part`.
    pub(crate) fn from_bytes(bytes: Vec<u8>) -> Self;
    pub(crate) fn as_bytes(&self) -> &[u8];
}

fn associated_data(id: &[u8; ENTRY_ID_LEN], entry_schema_version: u16, part: Part)
    -> [u8; AAD_LEN];

pub(crate) fn seal_part(key: &VaultKey, id: &[u8; ENTRY_ID_LEN], entry_schema_version: u16,
                        part: Part, plaintext: &[u8]) -> Result<EncryptedPart>;

pub(crate) fn open_part(key: &VaultKey, id: &[u8; ENTRY_ID_LEN], entry_schema_version: u16,
                        part: Part, sealed: &EncryptedPart) -> Result<Zeroizing<Vec<u8>>>;
```

Para que `crypto` no dependa de `vault`, `aead.rs` define su propia `pub(crate) const ENTRY_ID_LEN: usize = 16`. Una prueba en `vault/entry/sealed/tests.rs` comprueba que coincide con `vault::entry::ENTRY_ID_LEN`.

El comentario de documentación del módulo dice que el rollback de una entrada a una versión anterior queda fuera del modelo de amenazas de la v1.

### Dato autenticado (28 bytes, sin separadores)

| Offset | Campo | Tamaño | Valor |
| --- | --- | --- | --- |
| 0–8 | Etiqueta | 9 bytes | ASCII `boveda-v1` |
| 9–24 | Identificador | 16 bytes | id de la entrada |
| 25–26 | Versión de estructura | 2 bytes | `u16` little-endian |
| 27 | Parte | 1 byte | `1` = datos de búsqueda, `2` = secreto |

### `vault/entry/sealed.rs`

```rust
/// Entrada tal como se guarda: id y versión en claro, dos partes cifradas.
/// `Debug` propio: id, versión y longitudes, sin los bytes cifrados.
pub(crate) struct SealedEntry {
    id: EntryId,
    entry_schema_version: u16,
    search: EncryptedPart,
    secret: EncryptedPart,
}

impl SealedEntry {
    pub(crate) fn seal(key: &VaultKey, id: EntryId,
                       search: &SearchData, secret: &Secret) -> Result<Self>;
    pub(crate) fn from_parts(id: EntryId, entry_schema_version: u16,
                             search: EncryptedPart, secret: EncryptedPart) -> Self;
    pub(crate) fn open_search(&self, key: &VaultKey) -> Result<SearchData>;
    pub(crate) fn open_secret(&self, key: &VaultKey) -> Result<Secret>;
    pub(crate) fn id(&self) -> EntryId;
    pub(crate) fn entry_schema_version(&self) -> u16;
    pub(crate) fn search_bytes(&self) -> &EncryptedPart;
    pub(crate) fn secret_bytes(&self) -> &EncryptedPart;
}
```

- `from_parts` y los métodos `*_bytes` existen para que el almacenamiento (#6) guarde y lea la entrada sin conocer el cifrado.
- Editar solo los datos de búsqueda vuelve a cifrar también el secreto; para el MVP basta.
- `modified_at`, `last_used_at` y la marca de eliminada pertenecen a la #6.

## Flujo de datos

### Cifrar una parte (`seal_part`)

1. Se genera el nonce con `XNonce::try_generate()`; si falla, `Error::Random`.
2. Se crea un `Zeroizing<Vec<u8>>` con capacidad `NONCE_LEN + plaintext.len() + TAG_LEN`, se escribe el nonce y se copia el texto en claro detrás.
3. Se cifra `buf[NONCE_LEN..]` con `encrypt_inout_detached(&nonce, &aad, ...)` y se agrega la etiqueta con `extend_from_slice`. La capacidad ya alcanza, así que no hay reasignación.
4. Solo si el cifrado terminó bien: `EncryptedPart(mem::take(&mut *buf))`. Si algo falla antes, el búfer `Zeroizing` se borra al soltarse y nunca queda texto en claro en un `Vec` normal.

El cifrador se crea en cada llamada con `XChaCha20Poly1305::new_from_slice`, como en `keys.rs`.

### Descifrar una parte (`open_part`)

1. Si `sealed.len() < MIN_SEALED_LEN`, devuelve `CorruptEntry` sin panic.
2. Se lee el nonce de `[..NONCE_LEN]` y se copia `[NONCE_LEN..]` (cifrado y etiqueta) a un `Zeroizing<Vec<u8>>`.
3. `decrypt_in_place(&nonce, &aad, &mut *buf)`; si falla la autenticación, `CorruptEntry`.
4. Se devuelve el búfer, que ya contiene solo el texto en claro.

### Entrada completa

- **Cifrar:** `encode()` da un `Zeroizing<Vec<u8>>`, que va a `seal_part(..., Part::Search | Part::Secret, &buf)`. Al terminar, el búfer en claro se borra.
- **Descifrar:** `open_part` da un `Zeroizing<Vec<u8>>`, que va a `decode(self.entry_schema_version, &buf)`. Cualquier error es `CorruptEntry`.

## Pruebas

### `crypto/aead/tests.rs`

| Criterio | Prueba |
| --- | --- |
| Vector de 28 bytes | `associated_data(&[0x00..=0x0f], 1, Part::Secret)` coincide con un arreglo escrito a mano: `b"boveda-v1"`, el id, `01 00` y `02` |
| Valores de `Part` | `Part::Search as u8 == 1` y `Part::Secret as u8 == 2` |
| Ida y vuelta | `proptest` con texto arbitrario de 0 a 4 KiB, cualquier id, versión y parte: `open_part(seal_part(x)) == x` |
| Manipulación | `proptest`: invertir un bit en cualquier posición del nonce, el cifrado o la etiqueta devuelve `CorruptEntry` |
| Contexto distinto | Abrir con otro id, otra versión u otra `Part` devuelve `CorruptEntry` |
| Otra llave | Abrir con otra `VaultKey` devuelve `CorruptEntry` |
| No determinista | Dos `seal_part` del mismo contenido dan nonces y bytes distintos |
| Demasiado corta | Longitudes de 0, 1, 39 y 40 bytes (40 es un texto vacío con etiqueta falsa): siempre `CorruptEntry`, sin panic |
| `Debug` | `EncryptedPart` no muestra sus bytes |

### `vault/entry/sealed/tests.rs`

| Criterio | Prueba |
| --- | --- |
| Vectores v1 | En hexadecimal: llave `00..1f`, un id fijo, y una parte de búsqueda y un secreto cifrados con todos los campos opcionales. `open_search` y `open_secret` devuelven el contenido conocido. Llevan el comentario "Formato v1. No modificar: si esta prueba falla, cambió el formato en disco." |
| Ida y vuelta de la entrada | `proptest` con entradas válidas: `open_search(seal(x)) == x` y el secreto se compara con `expose_secret()`. La estrategia de texto de `entry/tests.rs` es privada a su módulo `props`, así que `sealed/tests.rs` define la suya, sin tocar las pruebas de la #4 |
| Longitud del id | `aead::ENTRY_ID_LEN == entry::ENTRY_ID_LEN` |
| Secreto movido | El secreto de la entrada A, puesto en la entrada B con `from_parts`, hace fallar `open_secret` con `CorruptEntry` |
| Partes intercambiadas | Con `search` como `secret` y al revés, ambos `open_*` fallan con `CorruptEntry` |
| Otra versión | `from_parts` con versión 2 sobre partes de la v1 da `CorruptEntry` |
| `Debug` | `SealedEntry` no muestra los bytes cifrados |

Los vectores v1 se generan una sola vez con una prueba temporal que imprime el hexadecimal (sin Python). Se copian a la prueba y la prueba temporal se borra antes del commit.

## Documentación

- `docs/especificacion.md`, "Formato de la bóveda → Entradas":
  - Columna de offsets en la tabla del dato autenticado.
  - Una parte de menos de 40 bytes o que no se autentica cuenta como entrada dañada.
  - Las dos partes de una entrada se cifran siempre con la misma versión de estructura.
- `ARCHITECTURE.md`:
  - En la fila de `crypto/`: "cifrar y descifrar cada parte de una entrada (`aead.rs`)".
  - En la fila de `vault/`: mencionar `entry/sealed.rs`.
- `docs/roadmap.md`: marcar "Cifrado por entrada" como terminado.

## Dependencias nuevas

Ninguna. `chacha20poly1305`, `zeroize` y `proptest` ya están en `core/Cargo.toml`.

## Commits previstos

1. `docs:` diseño del cifrado por entrada
2. `feat:` dato autenticado de las partes de una entrada
3. `feat:` cifrar y descifrar una parte con XChaCha20-Poly1305
4. `test:` pruebas de manipulación y propiedades de `aead`
5. `feat:` `SealedEntry` para cifrar y descifrar una entrada completa
6. `test:` vectores de descifrado v1 y pruebas de la entrada cifrada
7. `docs:` especificación, arquitectura y roadmap
