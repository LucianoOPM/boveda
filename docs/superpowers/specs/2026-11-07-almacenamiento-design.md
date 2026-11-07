# Diseño: almacenamiento SQLite (issue #6)

Fecha: 2026-11-07 · Etapa 1 · Rama `feat/6-almacenamiento-sqlite`

## Objetivo

Guardar la bóveda en un archivo SQLite administrado por el núcleo: una tabla de cabecera con una sola fila y una tabla de entradas. El módulo `core/src/vault/storage.rs` solo mueve bytes y fechas. No cifra, no descifra y no conoce la llave: recibe y devuelve `VaultHeader` (#3) y `SealedEntry` (#5).

### Fuera de alcance

- La API pública `Vault` (#7). Este módulo es `pub(crate)`.
- Leer el reloj. Lo hace `Vault` en la #7 (ver "Fechas").
- Exportar e importar respaldos. Aquí solo existe `open_read_only`, que esa issue usará para validar un respaldo.
- Las bóvedas de ejemplo en `core/tests/fixtures/` (#10).
- Relojes que retroceden o desfasados entre dispositivos: los resolverá la sincronización (fase 3).
- Deshacer una eliminación. Confirmar con el usuario antes de eliminar le toca a la app (etapa 2).

### Criterios de aceptación (de la issue)

- Crear, cerrar y reabrir un archivo conserva la cabecera.
- Insertar y leer una entrada devuelve las mismas partes cifradas.
- Las entradas eliminadas no aparecen en el listado, pero siguen en la tabla.
- Actualizar `last_used_at` no cambia `modified_at`.
- Ninguna cadena de texto plano conocida aparece en los bytes del archivo.
- `cargo test -p vault-core` y `clippy` pasan, y la CI está en verde.

## Decisiones

### Fechas

- Se guardan en milisegundos Unix, como `i64`.
- `storage.rs` no lee el reloj: cada escritura recibe la hora como parámetro (`now: i64`). Las pruebas usan valores fijos.
- En la #7, `Vault` lee `SystemTime::now()` en un único punto. Si `duration_since(UNIX_EPOCH)` falla (un reloj anterior a 1970), devuelve un error tipado, sin `unwrap()`, porque la API pública no hace panic.

### Entradas eliminadas: lápidas

- Al eliminar, un solo `UPDATE` pone `deleted = 1`, actualiza `modified_at` y deja las dos partes cifradas en `NULL`. La fila conserva solo el id, la versión de estructura y las fechas, que es lo que necesita la sincronización futura.
- Un `CHECK` en ambos sentidos impide una entrada viva sin partes y una lápida con partes.
- `PRAGMA secure_delete = ON` hace que SQLite llene de ceros las páginas liberadas. No persiste en el archivo, así que se fija en cada apertura de lectura y escritura.
- Las lecturas de entradas vivas filtran `deleted = 0`. Una lápida nunca cuenta como entrada dañada: para `get_entry`, es una entrada que no existe.
- Como es la primera versión del esquema, no hace falta migración.

### Identificación del archivo y versión

- `PRAGMA application_id = 0x42564C54` ("BVLT"), escrito al crear. La constante es fija para siempre, aunque cambie el formato, y no representa una versión.
- `application_id` solo identifica el archivo. No es un control de seguridad, porque no está autenticado. La garantía sigue siendo la cabecera como dato autenticado y el AEAD.
- La versión es únicamente `format_version` de la cabecera. No se usa `user_version`.
- Abrir nunca crea ni modifica el archivo: `open` usa `OpenFlags` sin `SQLITE_OPEN_CREATE`. Así, una ruta inexistente da error y no deja un SQLite vacío.
- Orden al abrir:
  1. Leer `application_id`. `rusqlite` abre de forma perezosa, así que un archivo que no es SQLite falla en esta primera consulta con `SQLITE_NOTADB` → `Error::NotAVault`. Un `application_id` distinto, incluido `0` (un SQLite vacío o ajeno), también da `NotAVault`.
  2. Pragmas (ver abajo).
  3. Leer la cabecera y validarla con `VaultHeader::from_parts`. Una `format_version` desconocida da el error que ya existe, `Error::UnsupportedFormatVersion(u16)`, distinto de `NotAVault`.

### Diario y durabilidad

- `journal_mode = DELETE` y `synchronous = FULL`, fijados en cada apertura de lectura y escritura junto con `secure_delete`.
- `PRAGMA journal_mode = DELETE` devuelve el modo resultante. Si no es `delete`, se devuelve `Error::Storage(JournalMode)` en vez de seguir en silencio.
- El modo WAL persiste en el archivo. Si llegara un archivo en WAL, abrirlo en lectura y escritura lo convierte de vuelta a `delete`.
- `open_read_only` no puede fijar `journal_mode`, porque cambiarlo es escribir. Solo comprueba que el modo sea `delete`.
- Con un `-journal` caliente, una conexión de solo lectura no puede revertir la transacción. Ese archivo se rechaza con `Error::Storage(HotJournal)` y nunca se repara. Los respaldos hechos con `VACUUM INTO` nunca traen un `-journal`.
- **Fuera del modelo de amenazas:** al confirmar una transacción se borra el `-journal`, pero sus bytes (cifrados) pueden seguir en la memoria flash hasta que el sistema reutilice esos bloques. `secure_delete` no cubre eso.

### Errores

Variantes nuevas de `Error`:

```rust
/// El archivo no es una bóveda: no es SQLite o su `application_id` no es el de Bóveda.
#[error("el archivo no es una bóveda")]
NotAVault,

/// No existe una entrada viva con ese identificador.
#[error("entrada no encontrada")]
EntryNotFound,

/// Falló el almacenamiento. Nunca incluye el error original de SQLite.
#[error("error de almacenamiento: {0}")]
Storage(StorageErrorKind),
```

```rust
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum StorageErrorKind {
    Io,            // disco lleno, permisos, error de E/S
    Busy,          // archivo bloqueado
    Corrupt,       // SQLITE_CORRUPT: el archivo entero está dañado
    ReadOnly,      // escritura sobre una conexión o un archivo de solo lectura
    HotJournal,    // -journal caliente al abrir en solo lectura
    JournalMode,   // el pragma no quedó en `delete`
    AlreadyExists, // `create` sobre una ruta que ya existe
    Other,         // códigos no previstos
}
```

Cada variante tiene su mensaje `#[error(...)]` en español, sin valores. `StorageErrorKind` se reexporta desde `lib.rs` junto con `Error` e `InvalidEntryField`. La #7 mueve las tres reexportaciones juntas cuando haga público `vault`.

- Una función privada, `map_sqlite_error()`, convierte el `rusqlite::Error` y lo descarta. Nunca se guarda como `#[source]` ni con `#[from]`, y no aparece en ningún `Display` ni `Debug`, porque puede traer SQL, rutas o valores de columnas.
- Dos niveles de daño:
  - `SQLITE_CORRUPT` significa que el archivo entero está dañado: `Storage(Corrupt)`.
  - `CorruptEntry` es solo para una fila: un id que no mide 16 bytes, una versión fuera de `u16` o un error de conversión de columna (`FromSqlConversionFailure`, que trae el valor). Esa fila falla sola, como pide la especificación.
- `SQLITE_NOTADB` → `NotAVault`.
- `SQLITE_CANTOPEN` (por ejemplo, una ruta inexistente al abrir) → `Storage(Io)`.
- `update_entry`, `delete_entry` y `touch_entry` sobre un id inexistente o una lápida devuelven `EntryNotFound` (se detecta con `changes() == 0`).
- Insertar un id repetido es un error de programación (los ids son aleatorios de 128 bits): `Storage(Other)`.
- Un byte alterado dentro de una parte cifrada no lo detecta el almacenamiento: devuelve los bytes tal cual y el AEAD falla después con `CorruptEntry` (#5).

## Esquema

```sql
-- Al crear, antes de las tablas:
PRAGMA application_id = 0x42564C54;  -- "BVLT", fijo para siempre

CREATE TABLE header (
    id              INTEGER PRIMARY KEY CHECK (id = 1),  -- una sola fila
    format_version  INTEGER NOT NULL,
    kdf_algorithm   INTEGER NOT NULL,
    kdf_memory_kib  INTEGER NOT NULL,
    kdf_iterations  INTEGER NOT NULL,
    kdf_parallelism INTEGER NOT NULL,
    salt            BLOB NOT NULL CHECK (length(salt) = 16),
    key_nonce       BLOB NOT NULL CHECK (length(key_nonce) = 24),
    encrypted_key   BLOB NOT NULL CHECK (length(encrypted_key) = 48)
) STRICT;

CREATE TABLE entries (
    id                   BLOB PRIMARY KEY NOT NULL,
    entry_schema_version INTEGER NOT NULL,
    search               BLOB,
    secret               BLOB,
    modified_at          INTEGER NOT NULL,   -- ms Unix
    last_used_at         INTEGER,            -- NULL = nunca usada
    deleted              INTEGER NOT NULL DEFAULT 0,
    CHECK ((deleted = 0 AND search IS NOT NULL AND secret IS NOT NULL)
        OR (deleted = 1 AND search IS NULL AND secret IS NULL))
) STRICT, WITHOUT ROWID;
```

- `STRICT` hace que SQLite rechace un tipo equivocado en vez de aceptarlo en silencio. Requiere SQLite 3.37 o posterior, que la feature `bundled` cumple.
- `entries` no lleva `CHECK (length(id) = 16)`. Así, un id mal formado escrito desde fuera llega como fila dañada y no rompe toda la tabla. La cabecera sí lleva sus `CHECK` de longitud, porque si está mal formada la bóveda no puede abrir de ninguna forma.
- `WITHOUT ROWID` porque la llave primaria ya es el id de 16 bytes.
- No hay índice sobre `last_used_at`: con cientos de entradas, ordenar en memoria es instantáneo y la búsqueda vive en el núcleo.
- `last_used_at` empieza en `NULL`. El orden de "usadas recientemente" (issue de búsqueda) las pone al final.
- Los valores de la cabecera que leen `u16`, `u8` y `u32` se convierten con `try_from`. Si una conversión falla, la cabecera está dañada y se devuelve `Storage(Corrupt)`, porque sin cabecera no hay bóveda.

## Componentes

```
core/src/vault/storage.rs        # Storage, StoredEntry, EntryListing, map_sqlite_error
core/src/vault/storage/tests.rs  # pruebas unitarias
core/src/vault/mod.rs            # pub(crate) mod storage;
core/src/error.rs                # NotAVault, EntryNotFound, Storage(StorageErrorKind)
core/src/lib.rs                  # reexporta StorageErrorKind
```

### Tipos y firmas

```rust
pub(crate) const APPLICATION_ID: i32 = 0x4256_4C54; // "BVLT"

/// Conexión a la bóveda. No deriva `Debug`.
pub(crate) struct Storage {
    conn: rusqlite::Connection,
}

/// Entrada viva leída del archivo. `SealedEntry` ya tiene un `Debug` sin bytes.
#[derive(Debug)]
pub(crate) struct StoredEntry {
    pub(crate) sealed: SealedEntry,
    pub(crate) modified_at: i64,
    pub(crate) last_used_at: Option<i64>,
}

/// Resultado de listar: las filas válidas y las dañadas, por separado.
#[derive(Debug)]
pub(crate) struct EntryListing {
    pub(crate) entries: Vec<StoredEntry>,
    /// Filas dañadas con un id válido de 16 bytes.
    pub(crate) corrupt: Vec<EntryId>,
    /// Filas dañadas cuyo id no mide 16 bytes: no hay un `EntryId` que reportar.
    pub(crate) unidentified_corrupt: usize,
}

impl Storage {
    pub(crate) fn create(path: &Path, header: &VaultHeader) -> Result<Self>;
    pub(crate) fn open(path: &Path) -> Result<Self>;
    pub(crate) fn open_read_only(path: &Path) -> Result<Self>;
    pub(crate) fn close(self) -> Result<()>;

    pub(crate) fn read_header(&self) -> Result<VaultHeader>;
    pub(crate) fn replace_header(&mut self, header: &VaultHeader) -> Result<()>;

    pub(crate) fn insert_entry(&mut self, entry: &SealedEntry, now: i64) -> Result<()>;
    pub(crate) fn update_entry(&mut self, entry: &SealedEntry, now: i64) -> Result<()>;
    pub(crate) fn get_entry(&self, id: EntryId) -> Result<StoredEntry>;
    pub(crate) fn list_entries(&self) -> Result<EntryListing>;
    pub(crate) fn delete_entry(&mut self, id: EntryId, now: i64) -> Result<()>;
    pub(crate) fn touch_entry(&mut self, id: EntryId, now: i64) -> Result<()>;
}
```

`EntryListing` deriva `Debug` porque `StoredEntry` y `EntryId` no muestran secretos.

### Comportamiento

- **`create`:**
  1. Crea el archivo vacío con `OpenOptions::new().write(true).create_new(true)`, que falla de forma atómica si la ruta ya existe: `Storage(AlreadyExists)`, y el archivo existente no se toca.
  2. Abre SQLite sobre ese archivo y fija los pragmas.
  3. En una sola transacción, fija `application_id`, crea las dos tablas e inserta la cabecera.
  4. Si algo falla después del paso 1, cierra la conexión y borra el archivo a medio crear.
- **`open`:** abre en lectura y escritura sin `SQLITE_OPEN_CREATE`, y luego `application_id` → pragmas → `read_header`.
- **`open_read_only`:** abre con `SQLITE_OPEN_READ_ONLY`, y luego `application_id` → comprobación de `journal_mode` y del `-journal` caliente → `read_header`. Cualquier escritura posterior da `Storage(ReadOnly)`.
- **`close`:** `Connection::close`, para no perder un error al cerrar.
- **`replace_header`:** `UPDATE header ... WHERE id = 1` en una transacción. Lo usará `change_master_password` en la #7.
- **`insert_entry`:** guarda el id, la versión, las dos partes, `modified_at = now`, `last_used_at = NULL` y `deleted = 0`.
- **`update_entry`:** reemplaza la versión y las dos partes y fija `modified_at = now`, con `WHERE id = ? AND deleted = 0`. No toca `last_used_at`.
- **`touch_entry`:** solo cambia `last_used_at = now`, con `WHERE id = ? AND deleted = 0`.
- **`delete_entry`:** convierte la fila en lápida en un solo `UPDATE`, con `WHERE id = ? AND deleted = 0`.
- **`get_entry`:** solo filas con `deleted = 0`. Una lápida o un id inexistente dan `EntryNotFound`, y una fila mal formada da `CorruptEntry`.
- **`list_entries`:** solo filas con `deleted = 0`. Una fila mal formada no aborta el listado: si su id mide 16 bytes, va a `corrupt`; si no, se suma a `unidentified_corrupt`. Un error de SQLite durante el recorrido (por ejemplo, `SQLITE_CORRUPT`) sí aborta todo con `Storage(...)`.
- **Transacciones:** cada escritura usa su propia transacción `IMMEDIATE`. No se exponen transacciones a quien llama porque ninguna operación de la #7 escribe varias filas a la vez. Si hiciera falta, se agrega ahí.

### Lectura de una fila de `entries`

1. Leer el id como `Vec<u8>` y convertirlo con `<[u8; 16]>::try_from`. Si falla, la fila no es identificable.
2. Leer `entry_schema_version` directamente como `u16` con `row.get::<_, u16>`. Un valor fuera de rango produce el `IntegralValueOutOfRange` de `rusqlite`, que trae el valor; se mapea a `CorruptEntry` con el id y el valor se descarta.
3. Leer `search` y `secret` como `Vec<u8>` y armarlos con `EncryptedPart::from_bytes` y `SealedEntry::from_parts`.
4. Cualquier `IntegralValueOutOfRange`, `FromSqlConversionFailure` o `InvalidColumnType` en estos pasos es un daño de la fila, no del archivo, y su valor se descarta.

## Pruebas

En `core/src/vault/storage/tests.rs`, con archivos reales en un directorio de `tempfile` y fechas fijas. Las cabeceras usan `KdfParams::MIN`, como las pruebas de la #3.

| Criterio | Prueba |
| --- | --- |
| Crear, cerrar y reabrir conserva la cabecera | `create` → `close` → `open` → `read_header` devuelve los mismos campos y desbloquea con la contraseña de prueba |
| Insertar y leer da las mismas partes | `insert_entry` → `get_entry`: mismos bytes y versión, `modified_at = now`, `last_used_at = None`. Además, `proptest` con partes de longitud arbitraria |
| Las eliminadas no aparecen, pero siguen en la tabla | `delete_entry` → la entrada no está en `list_entries` (ni en `corrupt`), `get_entry` da `EntryNotFound`, y con SQL directo la fila existe con `deleted = 1`, `modified_at` actualizado y las partes en `NULL` |
| `touch` no cambia `modified_at` | `insert(t1)` → `touch(t2)`: `modified_at = t1` y `last_used_at = Some(t2)`. Además, `update(t3)` deja `last_used_at = Some(t2)` y fija `modified_at = t3` |
| Sin texto plano en el archivo | Entradas cifradas con `SealedEntry::seal` y cadenas reconocibles en todos los campos. Al cerrar, los bytes del archivo no contienen ninguna y no queda un `-journal` |
| `CHECK` de la lápida | Con SQL directo, una fila viva sin partes y una lápida con partes son rechazadas |
| Pragmas | Tras `open`: `secure_delete = 1`, `journal_mode = delete` y `synchronous = 2` (FULL) |
| WAL | Un archivo pasado a WAL dentro de la prueba vuelve a `delete` al abrirlo con `open` |
| Identificación | Un archivo que no es SQLite, un SQLite vacío, un SQLite ajeno con una tabla `header` y una bóveda con `format_version = 2`: `NotAVault`, `NotAVault`, `NotAVault` y `UnsupportedFormatVersion(2)`. El archivo queda igual byte por byte |
| Ruta inexistente | `open` y `open_read_only` dan `Storage(Io)` y la ruta sigue sin existir |
| `create` sobre un archivo existente | `Storage(AlreadyExists)`, y el archivo no cambia |
| Solo lectura | `open_read_only` lee la cabecera y las entradas; `insert`, `update`, `delete`, `touch` y `replace_header` dan `Storage(ReadOnly)` |
| `-journal` caliente | Un archivo con un `-journal` caliente se rechaza en `open_read_only` con `Storage(HotJournal)` y queda igual byte por byte |
| `EntryNotFound` | `update`, `delete` y `touch` sobre un id inexistente y sobre una lápida |
| Filas dañadas | Con SQL directo, un id de 15 bytes y una versión de 70000: `list_entries` devuelve las entradas sanas, `unidentified_corrupt = 1` y `corrupt` contiene el id de la otra. `get_entry` de esta última da `CorruptEntry` |
| Archivo dañado | Se alteran los bytes de la cabecera de la página raíz de `entries` (se ubica con `PRAGMA page_size` y `rootpage` de `sqlite_schema`): `list_entries` da `Storage(Corrupt)` |
| Errores sin valores | Con SQL directo, una entrada con versión `987654321`: `get_entry` da `CorruptEntry`, y ni `Display` ni `Debug` del error contienen `987654321`. Igual con `kdf_memory_kib = 987654321987` en la cabecera (fuera de `u32`): `open` da `Storage(Corrupt)` sin el valor |
| `replace_header` | La cabecera nueva se lee tal cual y la anterior ya no desbloquea |

Las pruebas que necesitan escribir filas anómalas abren una segunda conexión de `rusqlite` sobre el mismo archivo. `STRICT` acepta esos valores porque los tipos son correctos (un `BLOB` de 15 bytes, un `INTEGER` grande): el rechazo ocurre al leer, en el núcleo.

## Dependencias nuevas

- `rusqlite` con `features = ["bundled"]`, en la versión estable más reciente (se confirma con `cargo search` al escribir el plan). Compila SQLite desde C; los runners de Ubuntu y Windows de la CI ya tienen compilador. El NDK para Android queda para la etapa 2.
- `tempfile`, solo de desarrollo.

Ninguna es criptográfica ni de red. Licencias y avisos se verifican con `cargo deny check` y `cargo audit` en el mismo commit que las agrega.

## Documentación

- `docs/especificacion.md`, "Formato de la bóveda → Almacenamiento":
  - Fechas en milisegundos Unix (`i64`), que el almacenamiento recibe de quien llama.
  - El esquema, `application_id` y los tres pragmas.
  - Las filas eliminadas son lápidas sin partes cifradas.
  - Fuera del modelo de amenazas: los restos del `-journal` en la memoria flash y los relojes desfasados.
- `ARCHITECTURE.md`: en la fila de `vault/`, describir `storage.rs`.
- `docs/roadmap.md`: marcar "Almacenamiento SQLite" como terminado.
- Comentario en la issue #7:
  - Prueba: eliminar una entrada, desbloquear y comprobar que no aparece ni en las válidas ni en las dañadas.
  - `Vault` lee el reloj en un solo punto, con un error tipado si es anterior a 1970.
  - Cómo reportar `unidentified_corrupt` en `unlock()`.

## Commits previstos

1. `docs:` diseño del almacenamiento SQLite
2. `chore:` agregar `rusqlite` y `tempfile`
3. `feat:` errores de almacenamiento (`Storage`, `NotAVault`, `EntryNotFound`)
4. `feat:` crear y abrir el archivo con su esquema y pragmas
5. `feat:` leer y reemplazar la cabecera
6. `feat:` operaciones sobre entradas
7. `test:` filas dañadas, archivo dañado y texto plano
8. `docs:` especificación, arquitectura y roadmap
