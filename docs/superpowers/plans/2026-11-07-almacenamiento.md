# Plan de implementación: almacenamiento SQLite (issue #6)

> **Para agentes:** SUB-SKILL REQUERIDA: usa superpowers:subagent-driven-development (recomendada) o superpowers:executing-plans para implementar este plan tarea por tarea. Los pasos usan casillas (`- [ ]`) para el seguimiento.

**Objetivo:** guardar la cabecera y las entradas cifradas de la bóveda en un archivo SQLite, con lápidas, pragmas fijos y errores que nunca contienen valores.

**Arquitectura:**
- `vault/storage.rs` envuelve una `rusqlite::Connection`, recibe y devuelve `VaultHeader` (#3) y `SealedEntry` (#5), y no cifra ni lee el reloj.
- `map_sqlite_error()`, privada, traduce cada `rusqlite::Error` a un `Error` del núcleo y lo descarta.
- Las filas mal formadas de `entries` se reportan una por una, sin abortar el listado.

**Tecnologías:** Rust 2024, `rusqlite` 0.41 (`bundled`), `tempfile` 3 (desarrollo), `proptest` (ya existe).

**Especificación:** `docs/superpowers/specs/2026-11-07-almacenamiento-design.md`. Léela antes de empezar: este plan no repite su esquema SQL ni sus tablas de comportamiento, que son la referencia exacta.

## Restricciones globales

- Identificadores en inglés; comentarios, documentación y commits en español (Conventional Commits).
- Pruebas unitarias en archivo aparte: `storage.rs` termina con `#[cfg(test)] mod tests;` y las pruebas viven en `storage/tests.rs` con `use super::*;`.
- Todo es `pub(crate)`, salvo `StorageErrorKind`, que es `pub` porque forma parte de `Error`.
- La API no hace panic: nada de `unwrap`, `expect` ni indexación que pueda fallar fuera de las pruebas.
- Ningún `rusqlite::Error` se guarda en un `Error`: ni `#[source]`, ni `#[from]`, ni dentro de un mensaje.
- `storage.rs` nunca llama a `SystemTime`: las fechas llegan como `now: i64` (milisegundos Unix).
- `APPLICATION_ID = 0x4256_4C54` ("BVLT"). Pragmas en cada apertura de lectura y escritura: `secure_delete = ON`, `journal_mode = DELETE`, `synchronous = FULL`.
- `open` y `open_read_only` nunca usan `SQLITE_OPEN_CREATE`.
- Cada escritura en su propia transacción `IMMEDIATE` (`Connection::transaction_with_behavior(TransactionBehavior::Immediate)`).
- Las cabeceras de prueba usan `KdfParams::MIN`.
- Antes de cada commit: `cargo test -p vault-core` y `cargo clippy --workspace --all-targets -- -D warnings`. `cargo fmt --all` siempre.

## Foco de revisión

1. Una ruta con caracteres no ASCII (la carpeta del usuario en Windows puede llevar acentos, como `Jesús`): `create`, `close` y `open` funcionan. → Task 2.
2. `create` en una carpeta que no existe da `Storage(Io)` y no crea nada. → Task 2.
3. Eliminar dos veces la misma entrada: la segunda da `EntryNotFound` y la lápida no cambia su `modified_at`. → Task 4.
4. Fechas extremas (`i64::MIN`, `0`, `i64::MAX`) en `insert`, `update` y `touch` se leen tal cual. → Task 4.
5. Partes vacías (0 bytes) y grandes (20 000 bytes, más que unas notas al límite cifradas) se guardan y se leen iguales. → Task 4 (rango del `proptest`).

---

### Task 1: dependencias y errores de almacenamiento

**Archivos:**
- Modificar: `core/Cargo.toml`, `Cargo.lock`
- Modificar: `core/src/error.rs`, `core/src/lib.rs`
- Prueba: `core/src/error/tests.rs` (nuevo; `error.rs` termina con `#[cfg(test)] mod tests;`)

**Interfaces:**
- Produce: `Error::NotAVault`, `Error::EntryNotFound`, `Error::Storage(StorageErrorKind)` y `pub enum StorageErrorKind { Io, Busy, Corrupt, ReadOnly, HotJournal, JournalMode, AlreadyExists, Other }`, con `#[non_exhaustive]` y `#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]`. Se reexporta en `lib.rs`: `pub use error::{Error, InvalidEntryField, Result, StorageErrorKind};`.

- [ ] **Step 1: agregar las dependencias**

En `core/Cargo.toml`: `rusqlite = { version = "0.41", features = ["bundled"] }` en `[dependencies]` y `tempfile = "3"` en `[dev-dependencies]`, en orden alfabético y con un comentario en español que diga que `bundled` compila SQLite desde C (en Android requiere el NDK).

- [ ] **Step 2: verificar licencias y avisos**

Run: `cargo build -p vault-core && cargo deny check && cargo audit`
Expected: compila; `cargo deny` sin errores (advertencias de versiones múltiples aceptables); `cargo audit` sin vulnerabilidades. Si una licencia falla, detente y avísale al usuario: no edites `deny.toml` por tu cuenta.

- [ ] **Step 3: commit**

```bash
git add core/Cargo.toml Cargo.lock
git commit -m "chore: agregar rusqlite y tempfile"
```

- [ ] **Step 4: escribir las pruebas que fallan**

En `core/src/error/tests.rs`:
- `storage_error_messages_are_spanish_and_fixed`: `Error::NotAVault.to_string() == "el archivo no es una bóveda"`, `Error::EntryNotFound.to_string() == "entrada no encontrada"`, `Error::Storage(StorageErrorKind::Corrupt).to_string()` empieza con `"error de almacenamiento: "`.
- `every_storage_kind_has_a_message`: para las 8 variantes, `to_string()` no está vacío y no hay dos iguales.

- [ ] **Step 5: comprobar que fallan**

Run: `cargo test -p vault-core error::tests`
Expected: FAIL de compilación (no existen las variantes).

- [ ] **Step 6: implementar las variantes**

Mensajes de `StorageErrorKind`, exactos: `Io` "error de lectura o escritura del archivo", `Busy` "el archivo está bloqueado por otro proceso", `Corrupt` "el archivo de la bóveda está dañado", `ReadOnly` "el archivo se abrió en solo lectura", `HotJournal` "el archivo tiene una transacción sin terminar", `JournalMode` "no se pudo fijar el modo del diario", `AlreadyExists` "el archivo ya existe", `Other` "error interno de SQLite". Documenta cada variante con `///` en español.

- [ ] **Step 7: comprobar que pasan, y commit**

Run: `cargo test -p vault-core && cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS, sin advertencias.

```bash
git add core/src/error.rs core/src/error/tests.rs core/src/lib.rs
git commit -m "feat: agregar los errores de almacenamiento"
```

---

### Task 2: crear y abrir el archivo

**Archivos:**
- Crear: `core/src/vault/storage.rs`, `core/src/vault/storage/tests.rs`
- Modificar: `core/src/vault/mod.rs` (`pub(crate) mod storage;` y el comentario del módulo)

**Interfaces:**
- Consume: `VaultHeader::{create, from_parts, format_version, algorithm, params, salt, encrypted_key}`, `EncryptedKey { nonce, ciphertext }`, `KdfParams { memory_kib, iterations, parallelism }` (Task 1 para los errores).
- Produce:
  - `pub(crate) const APPLICATION_ID: i32 = 0x4256_4C54;`
  - `pub(crate) struct Storage { conn: rusqlite::Connection }`
  - `Storage::create(path: &Path, header: &VaultHeader) -> Result<Storage>`
  - `Storage::open(path: &Path) -> Result<Storage>`
  - `Storage::close(self) -> Result<()>`
  - `Storage::read_header(&self) -> Result<VaultHeader>`
  - `fn map_sqlite_error(err: rusqlite::Error) -> Error` (privada)

Ayudantes de prueba en `storage/tests.rs`, que usan las tareas siguientes:
- `fn password(text: &str) -> SecretString`
- `fn new_header() -> (VaultHeader, VaultKey)`: `VaultHeader::create(&password("correcta"), KdfParams::MIN)`.
- `fn temp_path() -> (tempfile::TempDir, PathBuf)`: devuelve el directorio (debe vivir toda la prueba) y `dir/boveda.db`, que no existe.
- `fn new_vault() -> (tempfile::TempDir, PathBuf, Storage)`: `create` con `new_header()`.
- `fn raw(path: &Path) -> rusqlite::Connection`: conexión directa, para leer pragmas y escribir filas anómalas.

- [ ] **Step 1: escribir las pruebas que fallan**

- `create_close_reopen_keeps_header`: `create` → `close` → `open` → `read_header`: misma `format_version`, `algorithm`, `params`, `salt` y `encrypted_key`, y `unlock(&password("correcta"))` devuelve la misma llave que `create`.
- `create_sets_application_id`: `raw(path)` lee `PRAGMA application_id` → `0x4256_4C54`.
- `open_sets_pragmas`: tras `open`, sobre `storage.conn`: `secure_delete == 1`, `journal_mode == "delete"`, `synchronous == 2`.
- `open_converts_wal_back_to_delete`: crear la bóveda, cerrar, `raw(path)` ejecuta `PRAGMA journal_mode = WAL` y se cierra, `open` → `journal_mode == "delete"`.
- `create_on_existing_path_fails_and_keeps_file`: escribir `b"contenido previo"` en la ruta; `create` → `Err(Storage(AlreadyExists))` y el archivo sigue con esos bytes.
- `create_in_missing_directory_fails`: ruta `dir/no-existe/boveda.db` → `Err(Storage(Io))`, y `dir/no-existe` sigue sin existir.
- `create_and_open_non_ascii_path`: ruta `dir/Jesús ñ/bóveda.db` (crear la carpeta antes) → `create`, `close` y `open` funcionan.
- `open_missing_path_fails_without_creating`: `open` → `Err(Storage(Io))` y `!path.exists()`.
- `open_rejects_non_vault_files`, un caso por archivo, cada uno comparando los bytes antes y después con `std::fs::read`:
  - 4096 bytes de texto (`b"no soy sqlite "` repetido) → `NotAVault`.
  - Un SQLite vacío creado con `raw` y `PRAGMA user_version = 1` (para que el archivo no esté vacío) → `NotAVault`.
  - Un SQLite con `CREATE TABLE header (id INTEGER)` → `NotAVault`.
- `open_rejects_future_format_version`: `raw` ejecuta `UPDATE header SET format_version = 2` → `open` da `Err(UnsupportedFormatVersion(2))`, y el archivo no cambia después del intento.

- [ ] **Step 2: comprobar que fallan**

Run: `cargo test -p vault-core storage::tests`
Expected: FAIL de compilación (no existe `Storage`).

- [ ] **Step 3: implementar `map_sqlite_error`**

Por código primario de `rusqlite::Error::SqliteFailure(e, _)`: `NotADatabase` → `NotAVault`; `DatabaseCorrupt` → `Storage(Corrupt)`; `DatabaseBusy` y `DatabaseLocked` → `Storage(Busy)`; `ReadOnly` → `Storage(HotJournal)` si `e.extended_code == rusqlite::ffi::SQLITE_READONLY_ROLLBACK`, si no `Storage(ReadOnly)`; `CannotOpen`, `SystemIoFailure`, `DiskFull` y `PermissionDenied` → `Storage(Io)`. `IntegralValueOutOfRange`, `FromSqlConversionFailure` e `InvalidColumnType` → `Storage(Corrupt)` (la lectura de entradas los intercepta antes; ver Task 4). Todo lo demás → `Storage(Other)`. Confirma los nombres exactos de `ErrorCode` en la documentación de `rusqlite` 0.41 (Context7) antes de escribirlos.

- [ ] **Step 4: implementar `create`, `open`, `close` y `read_header`**

Sigue el orden de "Comportamiento" de la especificación. Detalles que no fija la especificación:
- `create`: `OpenOptions::new().write(true).create_new(true).open(path)`; un `io::ErrorKind::AlreadyExists` → `Storage(AlreadyExists)`, cualquier otro `io::Error` → `Storage(Io)`. Suelta el `File` y abre con `OpenFlags::SQLITE_OPEN_READ_WRITE | SQLITE_OPEN_NO_MUTEX` (el archivo ya existe). Si falla cualquier paso posterior, suelta la conexión **antes** de `fs::remove_file` (en Windows no se puede borrar un archivo abierto) e ignora el error del borrado.
- El esquema va en una constante `SCHEMA: &str`, copiado tal cual de la especificación, y se ejecuta con `execute_batch` dentro de la transacción, junto con `PRAGMA application_id` y el `INSERT` de la cabecera.
- Pragmas en una función privada `fn apply_pragmas(conn: &Connection) -> Result<()>`: `pragma_update` para `secure_delete` y `synchronous`; para `journal_mode`, `pragma_update_and_check` leyendo el `String` resultante; si no es `"delete"` (sin distinguir mayúsculas) → `Storage(JournalMode)`.
- `fn check_application_id(conn: &Connection) -> Result<()>`: lee `PRAGMA application_id` como `i32`; un error pasa por `map_sqlite_error` (así `NOTADB` da `NotAVault`), y un valor distinto de `APPLICATION_ID` da `NotAVault`.
- `read_header`: `SELECT ... FROM header WHERE id = 1`, leyendo cada columna directamente con su tipo (`u16`, `u8`, `u32`, `[u8; 16]` vía `Vec<u8>` + `try_from`, `[u8; 24]`, `[u8; 48]`). Cualquier fallo de conversión o de longitud → `Storage(Corrupt)`; sin fila → `Storage(Corrupt)`. Arma la cabecera con `VaultHeader::from_parts`, que valida versión, algoritmo y parámetros.
- `close`: `self.conn.close().map_err(|(_, e)| map_sqlite_error(e))`.

- [ ] **Step 5: comprobar que pasan, y commit**

Run: `cargo test -p vault-core && cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS, sin advertencias.

```bash
git add core/src/vault/mod.rs core/src/vault/storage.rs core/src/vault/storage/tests.rs
git commit -m "feat: crear y abrir el archivo de la bóveda con su esquema y pragmas"
```

---

### Task 3: reemplazar la cabecera y abrir en solo lectura

**Archivos:**
- Modificar: `core/src/vault/storage.rs`, `core/src/vault/storage/tests.rs`

**Interfaces:**
- Consume: Task 2 (`Storage`, ayudantes de prueba, `apply_pragmas`, `check_application_id`, `read_header`).
- Produce:
  - `Storage::replace_header(&mut self, header: &VaultHeader) -> Result<()>`
  - `Storage::open_read_only(path: &Path) -> Result<Storage>`

- [ ] **Step 1: escribir las pruebas que fallan**

- `replace_header_persists_and_old_password_fails`: crear con la cabecera A (contraseña "correcta"); `replace_header` con una cabecera B nueva de `VaultHeader::create(&password("nueva"), KdfParams::MIN)`; `close` → `open` → `read_header`: la sal es la de B, `unlock("nueva")` funciona y `unlock("correcta")` da `WrongPassword`.
- `header_out_of_range_value_is_corrupt_without_value`: `raw` ejecuta `UPDATE header SET kdf_memory_kib = 987654321987`; `open` → `Err(Storage(Corrupt))`, y ni `format!("{e}")` ni `format!("{e:?}")` contienen `"987654321987"`.
- `open_read_only_reads_header`: tras crear y cerrar, `open_read_only` → `read_header` desbloquea con "correcta".
- `read_only_rejects_writes`: `replace_header` sobre una conexión de `open_read_only` → `Err(Storage(ReadOnly))`, y el archivo no cambia. (En la Task 4 se agregan las escrituras de entradas a esta misma prueba.)
- `open_read_only_rejects_wal`: archivo pasado a WAL con `raw` → `open_read_only` da `Err(Storage(JournalMode))`; ni el `.db` ni ningún `-wal` o `-shm` cambian ni se crean.
- `open_read_only_rejects_hot_journal`: ver el Step 2. `open_read_only` sobre la copia → `Err(Storage(HotJournal))`, y los bytes de la copia y de su `-journal` no cambian.
- `open_read_only_missing_path_fails`: → `Err(Storage(Io))` y la ruta sigue sin existir.

- [ ] **Step 2: armar el `-journal` caliente en la prueba**

Un ayudante `fn make_hot_journal_copy(src: &Path, dst: &Path)`:
1. Abrir `src` con `raw`, `PRAGMA cache_size = 1`, `BEGIN IMMEDIATE`.
2. Insertar con SQL directo unas 200 filas en `entries` con partes de 4 KiB, para que la caché se desborde: SQLite sincroniza el diario y escribe páginas en el `.db` antes de confirmar.
3. Sin confirmar, copiar `src` → `dst` y `src-journal` → `dst-journal` con `fs::copy`.
4. `ROLLBACK` y cerrar.
5. Comprobar que la copia es realmente caliente: `dst-journal` existe y mide más de 512 bytes.

Si en tu plataforma la copia no queda caliente (la prueba de `HotJournal` da otro error), detente y repórtalo: no cambies la prueba para que pase.

- [ ] **Step 3: comprobar que fallan**

Run: `cargo test -p vault-core storage::tests`
Expected: FAIL de compilación (no existen `replace_header` ni `open_read_only`).

- [ ] **Step 4: implementar**

- `replace_header`: `UPDATE header SET ... WHERE id = 1` en una transacción `IMMEDIATE`. Si `changes() != 1` → `Storage(Corrupt)`.
- `open_read_only`: `OpenFlags::SQLITE_OPEN_READ_ONLY | SQLITE_OPEN_NO_MUTEX`, luego `check_application_id` (es la primera lectura: ahí aparece `SQLITE_READONLY_ROLLBACK` → `HotJournal`), luego `PRAGMA journal_mode` solo de lectura (si no es `"delete"` → `Storage(JournalMode)`), luego `read_header`. No ejecuta `apply_pragmas`.

- [ ] **Step 5: comprobar que pasan, y commit**

Run: `cargo test -p vault-core && cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS, sin advertencias.

```bash
git add core/src/vault/storage.rs core/src/vault/storage/tests.rs
git commit -m "feat: reemplazar la cabecera y abrir la bóveda en solo lectura"
```

---

### Task 4: operaciones sobre entradas

**Archivos:**
- Modificar: `core/src/vault/storage.rs`, `core/src/vault/storage/tests.rs`

**Interfaces:**
- Consume: Task 2 y 3; `SealedEntry::{from_parts, id, entry_schema_version, search_bytes, secret_bytes}`, `EncryptedPart::{from_bytes, as_bytes}`, `EntryId::{from_bytes, as_bytes}`, `ENTRY_SCHEMA_VERSION`.
- Produce (firmas exactas en la especificación, "Tipos y firmas"):
  - `pub(crate) struct StoredEntry { sealed, modified_at, last_used_at }` y `pub(crate) struct EntryListing { entries, corrupt, unidentified_corrupt }`, ambas con `#[derive(Debug)]`.
  - `insert_entry`, `update_entry`, `get_entry`, `list_entries`, `delete_entry`, `touch_entry`.
  - Ayudante de prueba `fn fake_entry(id_byte: u8, search: &[u8], secret: &[u8]) -> SealedEntry`: `from_parts` con `EntryId::from_bytes([id_byte; 16])`, `ENTRY_SCHEMA_VERSION` y las partes tal cual (sin cifrar: el almacenamiento no las interpreta).

- [ ] **Step 1: escribir las pruebas que fallan**

Fechas fijas: `T1 = 1_700_000_000_000`, `T2 = T1 + 1_000`, `T3 = T1 + 2_000`.

- `insert_then_get_returns_same_parts`: `insert_entry(&fake_entry(1, b"busqueda", b"secreto"), T1)` → `get_entry`: mismos bytes en ambas partes, misma versión, `modified_at == T1`, `last_used_at == None`.
- `entries_survive_reopen`: insertar, `close`, `open`, `get_entry` igual.
- `touch_does_not_change_modified_at`: `insert(T1)` → `touch(T2)` → `modified_at == T1`, `last_used_at == Some(T2)`.
- `update_replaces_parts_and_keeps_last_used_at`: `insert(T1)` → `touch(T2)` → `update_entry(&fake_entry(1, b"otra", b"otro"), T3)` → partes nuevas, `modified_at == T3`, `last_used_at == Some(T2)`.
- `delete_leaves_tombstone`: insertar dos entradas, `delete_entry(id1, T2)`. `list_entries` solo trae la 2, `corrupt` vacío y `unidentified_corrupt == 0`; `get_entry(id1)` → `EntryNotFound`; con `raw`: la fila existe con `deleted = 1`, `modified_at = T2`, `search IS NULL` y `secret IS NULL`.
- `delete_twice_fails_and_keeps_tombstone`: el segundo `delete_entry(id1, T3)` → `EntryNotFound`; con `raw`, `modified_at` sigue en `T2`.
- `missing_or_deleted_entry_is_not_found`: `update_entry`, `delete_entry` y `touch_entry` sobre un id que nunca existió y sobre una lápida → `EntryNotFound`.
- `tombstone_check_constraint_both_ways`: con `raw`, `INSERT` de una fila viva con `search = NULL` y `UPDATE` de una lápida para ponerle `secret = x'00'` → ambos fallan.
- `extreme_timestamps_round_trip`: con `i64::MIN`, `0` e `i64::MAX` en `insert`, `touch` y `update`, se leen iguales.
- `read_only_rejects_writes` (ampliar la de la Task 3): `insert_entry`, `update_entry`, `delete_entry` y `touch_entry` → `Storage(ReadOnly)`; `get_entry` y `list_entries` funcionan.
- `prop_parts_round_trip` (`proptest`, 32 casos): partes de 0 a 20 000 bytes arbitrarios; `insert` → `get` devuelve los mismos bytes.

- [ ] **Step 2: comprobar que fallan**

Run: `cargo test -p vault-core storage::tests`
Expected: FAIL de compilación.

- [ ] **Step 3: implementar la lectura de una fila**

Una función privada `fn read_row(row: &rusqlite::Row) -> rusqlite::Result<RowOutcome>` con `enum RowOutcome { Ok(StoredEntry), Corrupt(EntryId), Unidentified }`, siguiendo los pasos de "Lectura de una fila de `entries`" en la especificación. Los errores de conversión de la fila (`IntegralValueOutOfRange`, `FromSqlConversionFailure`, `InvalidColumnType`) se convierten en `Corrupt`/`Unidentified` ahí mismo; los demás (`SQLITE_CORRUPT`, E/S) se propagan y pasan por `map_sqlite_error`.

- [ ] **Step 4: implementar las operaciones**

- Las escrituras usan una transacción `IMMEDIATE`, comprueban `changes() == 0` → `EntryNotFound` (salvo `insert_entry`) y confirman.
- `get_entry`: `Corrupt` o `Unidentified` → `Err(CorruptEntry)`; sin fila → `Err(EntryNotFound)`.
- `list_entries`: `SELECT ... WHERE deleted = 0`, sin `ORDER BY` (el orden lo decide la búsqueda), y reparte cada `RowOutcome` en `EntryListing`.

- [ ] **Step 5: comprobar que pasan, y commit**

Run: `cargo test -p vault-core && cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS, sin advertencias.

```bash
git add core/src/vault/storage.rs core/src/vault/storage/tests.rs
git commit -m "feat: insertar, actualizar, leer, listar y eliminar entradas"
```

---

### Task 5: filas dañadas, archivo dañado y texto plano

**Archivos:**
- Modificar: `core/src/vault/storage/tests.rs` (y `storage.rs` solo si una prueba revela un fallo)

**Interfaces:**
- Consume: Task 4.

- [ ] **Step 1: escribir las pruebas**

- `corrupt_rows_are_reported_without_blocking`: insertar dos entradas sanas. Con `raw`: una fila con id de 15 bytes (`x'01...'`, versión 1 y partes cualesquiera) y otra con id `[9; 16]` y `entry_schema_version = 70000`. `list_entries` → `entries.len() == 2`, `unidentified_corrupt == 1` y `corrupt == vec![EntryId::from_bytes([9; 16])]`. `get_entry([9; 16])` → `Err(CorruptEntry)`.
- `row_conversion_error_hides_value`: fila con id `[7; 16]` y versión `987654321`. `get_entry` → `Err(CorruptEntry)`, y ni `Display` ni `Debug` del error contienen `"987654321"`.
- `damaged_entries_page_is_storage_corrupt`: tras insertar entradas y cerrar, con `raw` leer `PRAGMA page_size` y `SELECT rootpage FROM sqlite_schema WHERE name = 'entries'`, cerrar `raw`, y sobrescribir con `0xFF` los primeros 8 bytes de esa página en el archivo (offset `(rootpage - 1) * page_size`). `open` funciona (la cabecera está en otra página) y `list_entries` → `Err(Storage(Corrupt))`.
- `file_contains_no_known_plaintext`: crear con `new_header()`, sellar con `SealedEntry::seal` y la llave devuelta tres entradas cuyos campos (servicio, URL, usuario, etiquetas, contraseña y notas) llevan cadenas como `"canario-servicio-1"`, `"canario-pass-1"`, etc. Insertar, actualizar una, eliminar otra, `close`. Los bytes del archivo no contienen ninguna cadena `"canario-"` (buscar la subcadena común) y no existe `boveda.db-journal`.

- [ ] **Step 2: ejecutar**

Run: `cargo test -p vault-core storage::tests`
Expected: PASS. Si alguna falla, es un fallo de `storage.rs`: corrígelo con la prueba como guía, sin cambiar lo que la prueba exige.

- [ ] **Step 3: commit**

```bash
git add core/src/vault/storage.rs core/src/vault/storage/tests.rs
git commit -m "test: filas dañadas, archivo dañado y ausencia de texto plano"
```

---

### Task 6: documentación y cierre

**Archivos:**
- Modificar: `docs/especificacion.md`, `ARCHITECTURE.md`, `docs/roadmap.md`

- [ ] **Step 1: especificación**

En "Formato de la bóveda → Almacenamiento", agregar lo que enumera la sección "Documentación" del diseño: fechas en milisegundos Unix (`i64`) que el almacenamiento recibe de quien llama, el esquema (las dos tablas en un bloque `sql`), `application_id = 0x42564C54`, los tres pragmas, que las filas eliminadas son lápidas sin partes cifradas, y, como fuera del modelo de amenazas, los restos del `-journal` en la memoria flash y los relojes desfasados.

- [ ] **Step 2: arquitectura y roadmap**

`ARCHITECTURE.md`, fila de `vault/`: reemplazar "almacenamiento SQLite" por "almacenamiento SQLite con lápidas para las eliminadas (`storage.rs`)". `docs/roadmap.md`: marcar `[x]` "Almacenamiento SQLite".

- [ ] **Step 3: verificación completa**

Run: `cargo fmt --all -- --check && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo deny check && cargo audit`
Expected: todo pasa.

- [ ] **Step 4: commit**

```bash
git add docs/especificacion.md ARCHITECTURE.md docs/roadmap.md
git commit -m "docs: registrar el almacenamiento SQLite en la especificación y la arquitectura"
```

- [ ] **Step 5: nota en la issue #7** (pedir confirmación al usuario antes de publicarla)

Comentario con `gh issue comment 7` que recoja los tres puntos de "Documentación → Comentario en la issue #7" del diseño.
