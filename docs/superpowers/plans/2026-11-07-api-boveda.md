# Plan de implementación: API de la bóveda (issue #7)

> **Para agentes:** SUB-SKILL REQUERIDA: usa superpowers:subagent-driven-development (recomendada) o superpowers:executing-plans para implementar este plan tarea por tarea. Los pasos usan casillas (`- [ ]`) para el seguimiento.

**Objetivo:** exponer la estructura pública `Vault` con `create`, `open`, `unlock`, `lock`, `entries`, `add`, `edit`, `delete`, `reveal` y `change_master_password` sobre la cabecera, el cifrado por entrada y el almacenamiento que ya existen.

**Arquitectura:**
- `Vault` guarda un `Storage` abierto y un `Option<Unlocked>` con la llave (`VaultKey`) y la caché (`Vec<EntrySummary>`). `lock()` pone `None`.
- Cada escritura va primero a `Storage` y después a la caché. La única excepción es `reveal`, que actualiza la caché y luego intenta escribir `last_used_at` como mejor esfuerzo.
- La hora se lee en un solo punto (`now_millis`). La calibración solo ocurre en `create` y `change_master_password` públicos, que delegan en funciones `*_with_params` usadas por las pruebas.

**Tecnologías:** Rust 2024, `rusqlite` 0.41, `secrecy` 0.10, `zeroize` 1.9, `tempfile` 3 (desarrollo). Sin dependencias nuevas.

**Especificación:** `docs/superpowers/specs/2026-11-07-api-boveda-design.md`. Léela antes de empezar: este plan no repite sus decisiones ni su tabla de pruebas, que son la referencia exacta.

## Restricciones globales

- Identificadores en inglés; comentarios, documentación y commits en español (Conventional Commits).
- Pruebas unitarias en archivo aparte: `vault/mod.rs` termina con `#[cfg(test)] mod tests;` y las pruebas viven en `vault/tests.rs` con `use super::*;`. Las de la API pública van en `core/tests/vault.rs`.
- La API no hace panic: nada de `unwrap`, `expect` ni indexación que pueda fallar fuera de las pruebas.
- Ningún error contiene secretos, rutas ni valores. Las variantes nuevas no llevan datos.
- Solo `now_millis()` llama a `SystemTime::now()`.
- Las pruebas unitarias usan `KdfParams::MIN`; solo `core/tests/vault.rs` llama a `calibrate()`.
- Con la bóveda bloqueada, todo salvo `unlock`, `lock` e `is_locked` devuelve `Error::Locked` antes de tocar el disco.
- No modificar ni el formato en disco ni los vectores fijos de la #4 y la #5.
- Antes de cada commit: `cargo test -p vault-core`, `cargo clippy --workspace --all-targets -- -D warnings` y `cargo fmt --all`.

## Foco de revisión

1. Una contraseña maestra con acentos y emoji (`"contraseña 🔐"`): `create` y después `unlock` funcionan, y con `"contrasena 🔐"` da `WrongPassword`. → Task 3.
2. Tras un `unlock` con la contraseña incorrecta, `add` da `Locked` y no escribe ninguna fila. → Task 3.
3. Eliminar dos veces la misma entrada: la segunda da `EntryNotFound` y `entries()` no cambia. → Task 4.
4. `edit` con `Replace`, `lock` y `unlock`: `reveal` devuelve la contraseña nueva, no la vieja. → Task 4.
5. `change_master_password` con la nueva igual a la actual: funciona y la bóveda sigue abriendo con ella. → Task 6.

---

### Task 1: `AlreadyExists` como variante propia de `Error`

**Archivos:**
- Modificar: `core/src/error.rs`, `core/src/vault/storage.rs` (`create`, su comentario)
- Prueba: `core/src/error/tests.rs`, `core/src/vault/storage/tests.rs` (`create_on_existing_path_fails_and_keeps_file`)

**Interfaces:**
- Produce: `Error::AlreadyExists` con el mensaje `"el archivo ya existe"`. `StorageErrorKind` pierde `AlreadyExists` (queda con 7 variantes).

- [ ] **Step 1: ajustar las pruebas**

En `storage/tests.rs`, `create_on_existing_path_fails_and_keeps_file` espera `Error::AlreadyExists`. En `error/tests.rs`, `ALL_STORAGE_KINDS` pasa a `[StorageErrorKind; 7]` sin `AlreadyExists`, y se agrega:

```rust
#[test]
fn already_exists_message_is_fixed() {
    assert_eq!(Error::AlreadyExists.to_string(), "el archivo ya existe");
}
```

- [ ] **Step 2: comprobar que falla**

Run: `cargo test -p vault-core`
Expected: error de compilación, porque `Error::AlreadyExists` no existe.

- [ ] **Step 3: mover la variante**

En `error.rs`, agregar `AlreadyExists` a `Error`, con `///` en español que diga que no incluye la ruta, y quitarla de `StorageErrorKind`. En `Storage::create`, `io::ErrorKind::AlreadyExists` → `Error::AlreadyExists`, y actualizar su comentario.

- [ ] **Step 4: comprobar que pasa**

Run: `cargo test -p vault-core`
Expected: PASS.

- [ ] **Step 5: commit**

```bash
git add core/src/error.rs core/src/error/tests.rs core/src/vault/storage.rs core/src/vault/storage/tests.rs
git commit -m "refactor: AlreadyExists como variante propia de Error"
```

---

### Task 2: errores `Locked`, `AlreadyUnlocked` y `Clock`, y lectura del reloj

**Archivos:**
- Modificar: `core/src/error.rs`, `core/src/vault/mod.rs`
- Crear: `core/src/vault/tests.rs` (`mod.rs` termina con `#[cfg(test)] mod tests;`)
- Prueba: `core/src/error/tests.rs`, `core/src/vault/tests.rs`

**Interfaces:**
- Produce: `Error::Locked` (`"la bóveda está bloqueada"`), `Error::AlreadyUnlocked` (`"la bóveda ya está desbloqueada"`) y `Error::Clock` (`"la hora del sistema no es válida"`).
- Produce, en `vault/mod.rs`: `fn now_millis() -> Result<i64>` y `fn millis_since_epoch(time: SystemTime) -> Result<i64>`, privadas.

- [ ] **Step 1: escribir las pruebas que fallan**

En `error/tests.rs`, `new_vault_error_messages_are_fixed`, que compara los tres mensajes exactos. En `vault/tests.rs`:

```rust
#[test]
fn clock_before_epoch_is_an_error() {
    let before = UNIX_EPOCH - Duration::from_secs(1);
    assert_eq!(millis_since_epoch(before), Err(Error::Clock));
}

#[test]
fn clock_at_epoch_is_zero() {
    assert_eq!(millis_since_epoch(UNIX_EPOCH), Ok(0));
    assert_eq!(millis_since_epoch(UNIX_EPOCH + Duration::from_millis(1500)), Ok(1500));
}

#[test]
fn now_is_after_2026() {
    assert!(now_millis().unwrap() > 1_767_225_600_000); // 2026-01-01
}
```

- [ ] **Step 2: comprobar que falla**

Run: `cargo test -p vault-core`
Expected: error de compilación (faltan las variantes y las funciones).

- [ ] **Step 3: implementar**

Las tres variantes en `Error`, con `///` en español. `millis_since_epoch`: `duration_since(UNIX_EPOCH)` → `Clock` si falla; `i64::try_from(d.as_millis())` → `Clock` si no cabe. `now_millis` = `millis_since_epoch(SystemTime::now())`. Mientras `Vault` no exista, `#[allow(dead_code)]` no hace falta: lo cubre el `allow` de `lib.rs`.

- [ ] **Step 4: comprobar que pasa**

Run: `cargo test -p vault-core`
Expected: PASS.

- [ ] **Step 5: commit**

```bash
git add core/src/error.rs core/src/error/tests.rs core/src/vault/mod.rs core/src/vault/tests.rs
git commit -m "feat: errores Locked, AlreadyUnlocked y Clock, y lectura del reloj"
```

---

### Task 3: `Vault` con `create`, `open`, `unlock`, `lock`, `entries` y `add`

**Archivos:**
- Modificar: `core/src/vault/mod.rs`, `core/src/vault/entry.rs`
- Prueba: `core/src/vault/tests.rs`

**Interfaces:**
- Consume: `VaultHeader::create(&SecretString, KdfParams) -> Result<(VaultHeader, VaultKey)>`, `VaultHeader::unlock`, `Storage::{create, open, read_header, list_entries, insert_entry}`, `SealedEntry::{seal, open_search}`, `EntryListing { entries, corrupt, unidentified_corrupt }`, `now_millis`.
- Produce, en `vault/mod.rs` (con `pub`, aunque el módulo siga privado hasta la Task 7):
  - `pub struct Vault { storage: Storage, unlocked: Option<Unlocked> }` sin `Debug`; `struct Unlocked { key: VaultKey, entries: Vec<EntrySummary> }` sin `Debug`.
  - `pub struct EntrySummary { id, search: SearchData, modified_at: i64, last_used_at: Option<i64> }` con `#[derive(Debug)]`, `impl ZeroizeOnDrop for EntrySummary {}` (comentario: la garantía la da `SearchData`) y getters `id() -> EntryId`, `search() -> &SearchData`, `modified_at() -> i64`, `last_used_at() -> Option<i64>`.
  - `pub struct UnlockReport { pub corrupt: Vec<EntryId>, pub unidentified_corrupt: usize }` con `#[derive(Debug, Clone, PartialEq, Eq)]`.
  - `Vault::create(path: &Path, password: &SecretString) -> Result<Self>` (llama a `calibrate()` y delega), `pub(crate) fn create_with_params(path: &Path, password: &SecretString, params: KdfParams) -> Result<Self>`, `open(path: &Path) -> Result<Self>`, `unlock(&mut self, password: &SecretString) -> Result<UnlockReport>`, `lock(&mut self)`, `is_locked(&self) -> bool`, `entries(&self) -> Result<&[EntrySummary]>`, `add(&mut self, search: SearchData, secret: Secret) -> Result<EntryId>`.
  - Privada: `fn unlocked(&self) -> Result<&Unlocked>` y `fn unlocked_mut(&mut self) -> Result<&mut Unlocked>`, que dan `Locked`.
- Produce, en `entry.rs`: `EntryId`, `SearchData` y `Secret` pasan a `pub`, con `pub` en `EntryId::{as_bytes, from_bytes}`, `SearchData::{new, service, url, username, tags}` y `Secret::{new, password, notes}`. `generate`, `encode` y `decode` siguen `pub(crate)`. `SearchData` deja de derivar `Clone`.

- [ ] **Step 1: escribir las pruebas que fallan**

Utilitarios en `vault/tests.rs`: `password(&str) -> SecretString`, `temp_path() -> (TempDir, PathBuf)` como en `storage/tests.rs`, `new_vault() -> (TempDir, PathBuf, Vault)` con `create_with_params(.., password("correcta"), KdfParams::MIN)`, `sample(service: &str) -> (SearchData, Secret)` con URL, usuario, dos etiquetas, contraseña `"clave-{service}"` y notas, y `raw(&Path) -> rusqlite::Connection`.

Pruebas:
- `create_unlock_round_trip`: crear, `add` 3 entradas, `lock`, `unlock`. El `UnlockReport` es `{ corrupt: [], unidentified_corrupt: 0 }`, y `entries()` tiene los mismos ids, servicios, URL, usuarios y etiquetas. Repetir con una bóveda nueva obtenida con `drop` y `Vault::open` sobre la misma ruta.
- `open_returns_locked_vault`: `open` → `is_locked()` y `entries()` da `Err(Locked)`.
- `wrong_password_keeps_vault_locked`: `unlock(password("incorrecta"))` da `WrongPassword` y `entries()` da `Locked`.
- `non_ascii_master_password` (Foco 1): crear con `"contraseña 🔐"`, `lock`, `unlock` con esa contraseña da `Ok`; `unlock` con `"contrasena 🔐"` da `WrongPassword`.
- `add_after_failed_unlock_is_locked` (Foco 2): tras el `WrongPassword`, `add` da `Locked`, y con `raw` `SELECT count(*) FROM entries` es 0.
- `unlock_twice_is_already_unlocked`: `AlreadyUnlocked`, y `entries()` conserva sus elementos.
- `lock_is_idempotent`: `lock()` dos veces y después `entries()` da `Locked`.
- `summary_zeroizes_on_drop`: `assert_zod::<EntrySummary>()` y `assert_zod::<SearchData>()`, con `fn assert_zod<T: ZeroizeOnDrop>() {}`.
- `add_sets_modified_at_and_no_last_used`: `modified_at() > 0` y `last_used_at() == None`.
- `create_on_existing_path_is_already_exists`: escribir `b"previo"` en la ruta; `create_with_params` da `AlreadyExists` y el archivo sigue con esos bytes.
- `create_failure_leaves_path_free`: crear el **directorio** `<ruta>-journal` (`path.with_file_name("boveda.db-journal")`); `create_with_params` da `Err(_)` y `!path.exists()`. Si con eso `create` no falla, reemplazar el truco por un gancho `#[cfg(test)]` en `Storage::initialize` y anotarlo en el PR.
- `unlock_storage_failure_keeps_locked`: crear, agregar, `lock`; con `raw`, `DROP TABLE entries`; `unlock` da `Err(Storage(_))` y `is_locked()` sigue en `true`.

- [ ] **Step 2: comprobar que falla**

Run: `cargo test -p vault-core vault::tests`
Expected: error de compilación (`Vault` no existe).

- [ ] **Step 3: implementar**

- `create_with_params`: `VaultHeader::create` y luego `Storage::create`. Todo lo que puede fallar en memoria ocurre antes de crear el archivo.
- `unlock`: primero `AlreadyUnlocked`; luego `read_header` → `header.unlock` → `list_entries` → `open_search` por entrada (un error va a `corrupt`). `self.unlocked` se asigna solo al final.
- `add`: `unlocked_mut` → `now_millis` → `EntryId::generate` → `SealedEntry::seal` → `insert_entry(now)` → `push` a la caché. El `Secret` se suelta al salir.
- `entry.rs`: los cambios de visibilidad y quitar `Clone` de `SearchData`. Si alguna prueba de la #4 o la #5 usa `.clone()` sobre `SearchData`, ajustarla sin cambiar lo que comprueba.

- [ ] **Step 4: comprobar que pasa**

Run: `cargo test -p vault-core` y `cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS y sin advertencias.

- [ ] **Step 5: commit**

```bash
git add core/src/vault/mod.rs core/src/vault/tests.rs core/src/vault/entry.rs
git commit -m "feat: Vault con create, open, unlock, lock, entries y add"
```

---

### Task 4: `edit`, `delete` y `reveal`

**Archivos:**
- Modificar: `core/src/vault/mod.rs`, `core/src/vault/storage.rs` (gancho de prueba)
- Prueba: `core/src/vault/tests.rs`

**Interfaces:**
- Consume: `Storage::{get_entry, update_entry, delete_entry, touch_entry}`, `StoredEntry { sealed, modified_at, last_used_at }`, `SealedEntry::open_secret`.
- Produce:
  - `#[derive(Debug)] pub enum SecretChange { Keep, Replace(Secret) }`.
  - `Vault::edit(&mut self, id: EntryId, search: SearchData, change: SecretChange) -> Result<()>`, `Vault::delete(&mut self, id: EntryId) -> Result<()>` y `Vault::reveal(&mut self, id: EntryId) -> Result<Secret>`.
  - En `storage.rs`: `#[cfg(test)] pub(crate) fn deny_writes_for_test(&self)`, que ejecuta `PRAGMA query_only = ON`.

- [ ] **Step 1: escribir las pruebas que fallan**

Utilitario `tamper(path, id, column: &str)`: lee el blob de `search` o `secret` con `raw`, invierte el bit 0 del byte 30 (dentro del texto cifrado) y lo escribe de vuelta. Otro, `row_bytes(path, id) -> (Vec<u8>, Vec<u8>, i64, Option<i64>)`, devuelve `search`, `secret`, `modified_at` y `last_used_at`.

- `locked_operations_fail`: con la bóveda bloqueada, `entries`, `add`, `edit`, `delete` y `reveal` dan `Locked`.
- `reveal_returns_secret_and_touches`: `reveal(id)` da la contraseña `"clave-gmail"` y las notas. `last_used_at` es `Some` en `entries()` y en `row_bytes`.
- `reveal_corrupt_secret`: `tamper(.., "secret")`; `reveal` da `CorruptEntry`, y `last_used_at` de la fila sigue en `None`.
- `reveal_missing_id`: `EntryId::from_bytes([7; 16])` da `EntryNotFound`.
- `reveal_survives_write_failure`: `vault.storage.deny_writes_for_test()`; `reveal` devuelve la contraseña correcta, y `add` da `Storage(StorageErrorKind::ReadOnly)`. Si `query_only` produce otro código distinto de `SQLITE_READONLY`, ajustar `map_sqlite_error` solo si el código es claramente de solo lectura; si no, cambiar el gancho por reabrir con `Storage::open_read_only` y anotarlo en el PR.
- `edit_keep_changes_only_search`: `reveal` (fija `last_used_at = t`), `edit(id, search con otra etiqueta, Keep)`. `reveal` sigue dando la contraseña original, la etiqueta nueva aparece en `entries()`, `modified_at` aumentó o es igual (comparar con `>=` y que los bytes de `search` cambiaron), y `last_used_at` sigue en `t` en la fila.
- `edit_keep_with_corrupt_secret`: `tamper(.., "secret")`; `edit(.., Keep)` da `CorruptEntry` y `row_bytes` queda idéntico.
- `edit_replace_persists` (Foco 4): `edit(.., Replace(secret nuevo "nueva"))`, `lock`, `unlock`; `reveal` da `"nueva"`.
- `edit_missing_or_deleted`: un id inexistente da `EntryNotFound`; tras `delete`, `edit` del mismo id da `EntryNotFound`.
- `delete_makes_tombstone`: `delete(id)`; ya no está en `entries()`; con `raw`, `deleted = 1`, `search IS NULL` y `secret IS NULL`.
- `delete_twice` (Foco 3): la segunda da `EntryNotFound` y `entries().len()` no cambia.
- `reveal_and_edit_do_not_leak_in_errors`: para cada `Err` obtenido en estas pruebas, `format!("{e} {e:?}")` no contiene `"correcta"`, `"clave-"` ni el servicio.

- [ ] **Step 2: comprobar que falla**

Run: `cargo test -p vault-core vault::tests`
Expected: error de compilación (`SecretChange`, `edit`, `delete` y `reveal` no existen).

- [ ] **Step 3: implementar**

- `edit`: `unlocked` → `now_millis` → `get_entry(id)`. Con `Keep`, `stored.sealed.open_secret(key)` (error → `CorruptEntry`); con `Replace(s)`, usar `s`. Luego `SealedEntry::seal(key, id, &search, &secret)` → `update_entry(now)`. En la caché, reemplazar el elemento con `id` o hacer `push` si no estaba, con `modified_at = now` y `last_used_at = stored.last_used_at`.
- `delete`: `unlocked` → `now_millis` → `delete_entry(id, now)` → `retain` sin ese id en la caché.
- `reveal`: `unlocked` → `get_entry` → `open_secret`. Si sale bien, intentar `now_millis()`; con la hora, actualizar `last_used_at` en la caché y llamar a `touch_entry`, ignorando su resultado (`let _ =`). Documentar en `///` que `last_used_at` es de mejor esfuerzo, con el texto de la especificación.

- [ ] **Step 4: comprobar que pasa**

Run: `cargo test -p vault-core` y `cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS y sin advertencias.

- [ ] **Step 5: commit**

```bash
git add core/src/vault/mod.rs core/src/vault/tests.rs core/src/vault/storage.rs
git commit -m "feat: edit, delete y reveal en Vault"
```

---

### Task 5: entradas dañadas, lápidas y reparación

Solo pruebas sobre el comportamiento de las Tasks 3 y 4. Si alguna falla, se corrige el código en la misma tarea.

**Archivos:**
- Prueba: `core/src/vault/tests.rs`

- [ ] **Step 1: escribir las pruebas**

- `unlock_reports_corrupt_search`: 3 entradas; `tamper(b, "search")`; `lock`, `unlock`. `entries()` tiene a `a` y `c`, y `report.corrupt == vec![b]`.
- `unlock_counts_unidentified_rows`: con `raw`, insertar una fila con un id de 15 bytes (como en `storage/tests.rs`); `unlock` da `unidentified_corrupt == 1` y las demás entradas abren.
- `tombstone_not_reported`: `delete(b)`, `lock`, `unlock`; `b` no está en `entries()` ni en `report.corrupt`.
- `replace_repairs_corrupt_search`: `tamper(b, "search")`, `lock`, `unlock` (`b` en `corrupt`), `edit(b, sample("b"), Replace(..))`. `b` aparece en `entries()`; tras `lock` y `unlock`, `corrupt` está vacío.
- `replace_repairs_corrupt_secret`: `tamper(b, "secret")`, `edit(b, .., Replace(..))`; `reveal(b)` da la contraseña nueva.
- `delete_corrupt_entry`: `tamper(b, "search")`, `lock`, `unlock`, `delete(b)` da `Ok`; tras `lock` y `unlock`, `corrupt` está vacío.

- [ ] **Step 2: comprobar que pasa**

Run: `cargo test -p vault-core vault::tests`
Expected: PASS.

- [ ] **Step 3: commit**

```bash
git add core/src/vault/tests.rs
git commit -m "test: entradas dañadas, lápidas y reparación en Vault"
```

---

### Task 6: `change_master_password` con `VaultHeader::rekey`

**Archivos:**
- Modificar: `core/src/vault/header.rs`, `core/src/vault/mod.rs`
- Prueba: `core/src/vault/header/tests.rs`, `core/src/vault/tests.rs`

**Interfaces:**
- Consume: `kdf::generate_salt`, `keys::seal_vault_key`, `encode_associated_data`, `Storage::{read_header, replace_header}`.
- Produce:
  - `pub(crate) fn VaultHeader::rekey(vault_key: &VaultKey, password: &SecretString, params: KdfParams) -> Result<Self>`. `VaultHeader::create` pasa a generar la llave y delegar en `rekey`.
  - `Vault::change_master_password(&mut self, current: &SecretString, new: &SecretString) -> Result<()>` (verificar → `calibrate()` → `rekey_with_params`).
  - `pub(crate) fn change_master_password_with_params(&mut self, current: &SecretString, new: &SecretString, params: KdfParams) -> Result<()>` (verificar → `rekey_with_params`).
  - Privadas: `fn verify_current(&self, current: &SecretString) -> Result<()>` y `fn rekey_with_params(&mut self, new: &SecretString, params: KdfParams) -> Result<()>`.

- [ ] **Step 1: escribir las pruebas que fallan**

En `header/tests.rs`:
- `rekey_keeps_vault_key`: `create` → `(h1, k)`; `rekey(&k, nueva, MIN)` → `h2`. `h2.unlock(nueva)` da una llave igual a `k` (comparar `expose_secret()`), `h2.unlock(vieja)` da `WrongPassword`, y la sal y el nonce de `h2` son distintos a los de `h1`.

En `vault/tests.rs`:
- `change_password_keeps_entries`: 2 entradas; guardar `row_bytes` de ambas; `change_master_password_with_params("correcta", "nueva", MIN)`; `lock`. `unlock("correcta")` da `WrongPassword`; `unlock("nueva")` abre las mismas entradas; `row_bytes` (`search` y `secret`) son idénticos a los de antes.
- `change_password_wrong_current`: con `raw`, leer la fila de `header` completa; con `current = "incorrecta"` da `WrongPassword`, y la fila de `header` es idéntica.
- `change_password_locked`: con la bóveda bloqueada da `Locked`.
- `change_password_to_same` (Foco 5): `current == new == "correcta"` da `Ok`; tras `lock`, `unlock("correcta")` abre.

- [ ] **Step 2: comprobar que falla**

Run: `cargo test -p vault-core`
Expected: error de compilación (`rekey` y `change_master_password_with_params` no existen).

- [ ] **Step 3: implementar**

- `rekey`: `params.validate()` → `generate_salt` → dato autenticado → `seal_vault_key`.
- `verify_current`: `unlocked()?` → `read_header` → `header.unlock(current)`; la llave obtenida se suelta.
- `rekey_with_params`: `VaultHeader::rekey(&unlocked.key, new, params)` → `storage.replace_header`.

- [ ] **Step 4: comprobar que pasa**

Run: `cargo test -p vault-core` y `cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS y sin advertencias.

- [ ] **Step 5: commit**

```bash
git add core/src/vault/header.rs core/src/vault/header/tests.rs core/src/vault/mod.rs core/src/vault/tests.rs
git commit -m "feat: change_master_password con VaultHeader::rekey"
```

---

### Task 7: API pública y pruebas de punta a punta

**Archivos:**
- Modificar: `core/src/lib.rs`, `core/src/vault/mod.rs`
- Crear: `core/tests/vault.rs`

**Interfaces:**
- Produce: `vault_core::vault::{Vault, EntrySummary, UnlockReport, SecretChange, EntryId, SearchData, Secret, Error, Result, InvalidEntryField, StorageErrorKind}`.

- [ ] **Step 1: escribir las pruebas que fallan**

En `core/tests/vault.rs`, usando solo `vault_core::vault::*`, `secrecy`, `rusqlite` y `tempfile`:
- `create_lock_unlock_with_calibration`: `Vault::create`, `add` de una entrada, `lock`, `unlock`: `entries()` tiene esa entrada. Con `rusqlite`, `SELECT kdf_memory_kib, kdf_iterations, kdf_parallelism FROM header`: memoria entre `65536` y `131072`, iteraciones `>= 3` y paralelismo `== 1`.
- `change_password_with_calibration`: `create`, `change_master_password("vieja", "nueva")`, `lock`; `unlock("vieja")` da `Err(Error::WrongPassword)` y `unlock("nueva")` da `Ok`.

- [ ] **Step 2: comprobar que falla**

Run: `cargo test -p vault-core --test vault`
Expected: error de compilación (`vault` es privado).

- [ ] **Step 3: exponer la API**

- `lib.rs`: `pub mod vault;`, `mod crypto;` y `mod error;`; quitar `pub use error::...` y `#![cfg_attr(not(test), allow(dead_code))]`. Actualizar el comentario del crate.
- `vault/mod.rs`: `pub use crate::error::{Error, InvalidEntryField, Result, StorageErrorKind};` y `pub use entry::{EntryId, SearchData, Secret};`. Los submódulos siguen `pub(crate)`.
- Si `clippy` o el compilador marcan código sin usar al quitar el `allow`, quitar ese código o, si lo usan solo las pruebas, ponerle `#[cfg(test)]`. No volver a un `allow` general.

- [ ] **Step 4: comprobar que pasa**

Run: `cargo test --workspace` y `cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS y sin advertencias.

- [ ] **Step 5: commit**

```bash
git add core/src/lib.rs core/src/vault/mod.rs core/tests/vault.rs
git commit -m "feat: exponer vault como API pública con pruebas de punta a punta"
```

---

### Task 8: documentación

**Archivos:**
- Modificar: `docs/especificacion.md` ("Formato de la bóveda → API"), `ARCHITECTURE.md` (fila de `vault/`), `docs/roadmap.md`

- [ ] **Step 1: actualizar**

- Especificación, párrafo **API**: la llave vive junto con la caché en `Option<Unlocked>` y `lock()` borra ambas al poner `None`; `unlock()` devuelve `UnlockReport` (ids dañados y conteo sin id) y las entradas se leen con `entries()`; `edit` con `Keep` o `Replace`; `reveal` actualiza `last_used_at` como mejor esfuerzo; `change_master_password` verifica la contraseña actual antes de calibrar. Agregar la referencia al diseño, como en "Almacenamiento".
- `ARCHITECTURE.md`: en la fila de `vault/`, "API de la bóveda (`mod.rs`, estructura `Vault`)".
- Roadmap: marcar "API de la bóveda" y actualizar la fecha.

- [ ] **Step 2: verificación final**

Run: `cargo fmt --all --check`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo audit` y `cargo deny check`
Expected: todo pasa.

- [ ] **Step 3: commit**

```bash
git add docs/especificacion.md ARCHITECTURE.md docs/roadmap.md
git commit -m "docs: registrar la API de la bóveda en la especificación, la arquitectura y el roadmap"
```
