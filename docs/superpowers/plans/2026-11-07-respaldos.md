# Plan de implementación: respaldos, exportar e importar (issue #9)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `Vault::export(&self, dest)` y `Vault::import(backup, dest, password) -> Result<(Vault, UnlockReport)>`, con la lógica de archivos en `core/src/vault/backup.rs`.

**Architecture:** Exportar crea `dest` vacío con `create_new` y ejecuta `VACUUM INTO`. Importar copia el respaldo a `<dest>.import`, lo valida en solo lectura (versión, `integrity_check`, contraseña y descifrado de los datos de búsqueda), revierte un `-journal` caliente de `dest` y renombra la copia encima. `backup.rs` es hijo de `vault`, así que usa los privados de `Vault` (`load_entries` y la construcción de `Vault { .. }`) sin hacerlos públicos.

**Tech Stack:** Rust 2024, `rusqlite` 0.41 (SQLite empaquetado con `-DSQLITE_USE_URI`), `secrecy`, `tempfile` (pruebas).

**Spec:** `docs/superpowers/specs/2026-11-07-respaldos-design.md`

## Global Constraints

- Identificadores en inglés; comentarios, documentación y commits en español (Conventional Commits).
- Pruebas unitarias en archivo aparte: `backup.rs` termina con `#[cfg(test)] mod tests;` y las pruebas viven en `core/src/vault/backup/tests.rs` con `use super::*;`.
- La API pública no hace panic. Sin variantes nuevas de `Error`.
- Ningún error, `Debug` ni `Display` contiene secretos, rutas ni mensajes de SQLite (`map_sqlite_error` los descarta). Los mensajes de `integrity_check` se descartan sin leerlos.
- Exportar con la bóveda bloqueada → `Error::Locked` antes de tocar el disco.
- `backup` y `dest` no absolutas → `Error::Storage(StorageErrorKind::Io)` sin crear nada. Una ruta que no es UTF-8 en `VACUUM INTO` → `Storage(Io)`.
- `VACUUM INTO ?1` con la ruta como parámetro, nunca formateada en el SQL.
- Sin `sync_all()` propio del respaldo exportado: lo hace SQLite porque `synchronous = FULL`.
- Si falla cualquier paso de la importación antes del renombrado, `dest` queda igual byte por byte (o sigue sin existir) y no queda `<dest>.import`.
- Sin dependencias nuevas.
- Antes de cada commit: `cargo test -p vault-core`. Antes de terminar: `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all`.

## Review Focus

1. **`backup` y `dest` son la misma ruta** (el usuario elige como respaldo su propia bóveda): la copia temporal hace que funcione; debe importar bien y no dejar `.import`. Prueba en la Task 4.
2. **Rutas con espacios y caracteres no ASCII** (`Respaldos/bóveda ñ 2026.db`): `VACUUM INTO` recibe UTF-8 y debe funcionar en Windows y Linux. Prueba en las Tasks 2 y 4.
3. **Directorio inexistente** para `dest`, o `backup` inexistente: `Storage(Io)`, sin crear nada ni dejar `.import`. Prueba en las Tasks 2 y 4.
4. **Exportar dos veces al mismo `dest`**: la segunda da `AlreadyExists` y el primer respaldo queda igual byte por byte (la limpieza por error no debe borrar un archivo que no creó). Prueba en la Task 2.
5. **Ciclo completo**: la bóveda devuelta por `import` se puede usar de inmediato para escribir y volver a exportar; comprueba que la conexión final es de lectura y escritura con sus pragmas. Prueba en la Task 4.

---

### Task 1: Ayudantes de prueba compartidos

**Files:**
- Create: `core/src/vault/test_support.rs`
- Modify: `core/src/vault/mod.rs` (declarar el módulo), `core/src/vault/storage/tests.rs`, `core/src/vault/tests.rs`

**Interfaces:**
- Produces (todas `pub(crate)`, en `crate::vault::test_support`, módulo `#[cfg(test)]`):
  - `fn password(text: &str) -> SecretString`
  - `fn temp_path() -> (tempfile::TempDir, PathBuf)` (ruta `boveda.db` que no existe)
  - `fn raw(path: &Path) -> rusqlite::Connection`
  - `fn sibling(path: &Path, suffix: &str) -> PathBuf`
  - `fn contains(haystack: &[u8], needle: &[u8]) -> bool`
  - `fn sample(service: &str) -> (SearchData, Secret)` (contraseña `clave-{service}`, notas `notas de prueba`)
  - `fn tamper(path: &Path, id: EntryId, column: &str)`
  - `fn make_hot_journal_copy(src: &Path, dst: &Path)`
  - `fn damage_entries_root_page(path: &Path)`: extraída del cuerpo de `damaged_entries_page_is_storage_corrupt` (localiza la página raíz de `entries` y llena sus 8 primeros bytes con `0xFF`).

- [ ] **Step 1: Mover los ayudantes** de `storage/tests.rs` y `vault/tests.rs` a `test_support.rs`, sin cambiar sus cuerpos. Declarar en `vault/mod.rs`: `#[cfg(test)] pub(crate) mod test_support;`. Ambos archivos de pruebas importan con `use crate::vault::test_support::*;` y borran sus copias locales.
- [ ] **Step 2: Verificar que nada cambió**

Run: `cargo test -p vault-core`
Expected: PASS, con el mismo número de pruebas que antes del cambio.

- [ ] **Step 3: Commit**

```bash
git add core/src/vault
git commit -m "refactor: ayudantes de prueba compartidos en test_support"
```

### Task 2: Exportar con `VACUUM INTO`

**Files:**
- Create: `core/src/vault/backup.rs`, `core/src/vault/backup/tests.rs`
- Modify: `core/src/vault/storage.rs`, `core/src/vault/storage/tests.rs`, `core/src/vault/mod.rs`

**Interfaces:**
- Consumes: `test_support::*` (Task 1).
- Produces:
  - `Storage::vacuum_into(&self, dest: &Path) -> Result<()>`: `dest.to_str()` o `Storage(Io)`; `self.conn.execute("VACUUM INTO ?1", [dest_str])`, con el error por `map_sqlite_error`.
  - `backup::export(storage: &Storage, dest: &Path) -> Result<()>` (`pub(super)`).
  - `backup::require_absolute(path: &Path) -> Result<()>` (privada; la usa la Task 4).
  - `Vault::export(&self, dest: &Path) -> Result<()>`.

- [ ] **Step 1: Pruebas que fallan**

En `storage/tests.rs`:
- `vacuum_into_copies_with_synchronous_full`: tras `new_vault()` e insertar 2 entradas, `PRAGMA synchronous` en `storage.conn` vale `2`; `vacuum_into(dest)` sobre un archivo vacío creado antes; `Storage::open_read_only(dest)` lista las 2 entradas con las mismas partes.

En `backup/tests.rs` (con un `new_vault()` local sobre `Vault::create_with_params(.., KdfParams::MIN)`):
- `export_creates_openable_backup`: tras exportar, `Storage::open_read_only(dest)` lee la cabecera, y `Vault::open(dest)` + `unlock(password("correcta"))` da las mismas entradas.
- `export_contains_no_known_plaintext`: entradas con cadenas `canario-…` en servicio, URL, usuario, etiqueta, contraseña y notas, y la contraseña maestra `canario-maestra`. `!contains(&bytes, b"canario-")`.
- `export_purges_deleted_entry_parts`: se leen los bytes de `search` y `secret` de una entrada con `raw`, se elimina la entrada y se exporta. El respaldo no contiene ninguno de los dos blobs; el archivo original puede contenerlos o no (no se afirma).
- `export_locked_is_locked`: `vault.lock()`; `export` da `Locked` y `!dest.exists()`.
- `export_existing_dest_is_already_exists`: `dest` con contenido `b"previo"` → `AlreadyExists` y el contenido no cambia.
- `export_twice_keeps_first_backup` (Review Focus 4): exportar, guardar los bytes, exportar de nuevo al mismo `dest` → `AlreadyExists` y los bytes siguen iguales.
- `export_relative_path_is_io`: `Path::new("respaldo.db")` → `Storage(Io)` y no existe en el directorio actual.
- `export_missing_directory_is_io` (Review Focus 3): `dir/no-existe/respaldo.db` → `Storage(Io)`.
- `export_non_ascii_path` (Review Focus 2): `dir/"bóveda ñ 2026.db"` se exporta y abre.
- `#[cfg(unix)] export_non_utf8_path_is_io`: un nombre con `OsStr::from_bytes(b"\xFF.db")` → `Storage(Io)` y el archivo no queda.

- [ ] **Step 2: Ejecutar y ver que fallan**

Run: `cargo test -p vault-core export vacuum_into`
Expected: FAIL de compilación (`export` y `vacuum_into` no existen).

- [ ] **Step 3: Implementar**
  - `Storage::vacuum_into` como en Interfaces.
  - `backup::export`: `require_absolute(dest)` → `OpenOptions::new().write(true).create_new(true).open(dest)` (`AlreadyExists` → `Error::AlreadyExists`, otro → `Storage(Io)`), soltar el `File` → `storage.vacuum_into(dest)`. Si falla, `fs::remove_file(dest)` ignorando el error y se devuelve el original. Solo se borra lo que creó `create_new` en esta llamada.
  - `Vault::export`: `self.unlocked()?;` y delega.
  - Comentario de módulo de `backup.rs` (en español): qué hace, que `VACUUM INTO` no deja restos de lo eliminado y que SQLite hace el `fsync` porque `synchronous = FULL` (cita de `lang_vacuum.html`), y por qué se exigen rutas absolutas (`SQLITE_USE_URI`).
  - En `vault/mod.rs`, `mod backup;` y quitar "Los respaldos llegan más adelante." del comentario del módulo.

- [ ] **Step 4: Ejecutar las pruebas**

Run: `cargo test -p vault-core`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add core/src/vault
git commit -m "feat: exportar la bóveda con VACUUM INTO"
```

### Task 3: Extraer la carga de entradas de `unlock`

**Files:**
- Modify: `core/src/vault/mod.rs`

**Interfaces:**
- Produces: `fn load_entries(storage: &Storage, key: VaultKey) -> Result<(Unlocked, UnlockReport)>`, función asociada privada de `Vault`. Lista con `list_entries` y descifra con `open_search`; las que fallan van a `corrupt`. Es el cuerpo actual de `unlock` después de obtener la llave.

- [ ] **Step 1: Refactorizar** `unlock` para que haga: `AlreadyUnlocked` → `read_header()?.unlock(password)?` → `load_entries(&self.storage, key)?` → instalar `Unlocked` → devolver el reporte.
- [ ] **Step 2: Verificar**

Run: `cargo test -p vault-core`
Expected: PASS sin cambios en las pruebas (las de `unlock` cubren el comportamiento).

- [ ] **Step 3: Commit**

```bash
git add core/src/vault/mod.rs
git commit -m "refactor: extraer la carga de entradas de unlock"
```

### Task 4: Importar con validación en una copia temporal

**Files:**
- Modify: `core/src/vault/backup.rs`, `core/src/vault/backup/tests.rs`, `core/src/vault/storage.rs`, `core/src/vault/storage/tests.rs`, `core/src/vault/mod.rs`

**Interfaces:**
- Consumes: `Vault::load_entries` (Task 3), `require_absolute` y `Vault::export` (Task 2), `test_support::*` (Task 1).
- Produces:
  - `Storage::check_integrity(&self) -> Result<()>`: `PRAGMA integrity_check`; recoge las filas como `String` y exige exactamente una igual a `"ok"`, si no `Storage(Corrupt)`. Los errores de SQLite pasan por `map_sqlite_error` (`SQLITE_CORRUPT` → `Storage(Corrupt)`).
  - `backup::import(backup: &Path, dest: &Path, password: &SecretString) -> Result<(Vault, UnlockReport)>` (`pub(super)`).
  - `Vault::import(backup: &Path, dest: &Path, password: &SecretString) -> Result<(Vault, UnlockReport)>`, que delega. Su comentario documenta que quien llama suelta antes su `Vault` sobre `dest`, que la contraseña es la del respaldo y que un fallo de la última apertura deja ya instalado el respaldo validado.
  - `backup::IMPORT_SUFFIX: &str = ".import"`.

- [ ] **Step 1: Pruebas que fallan**

En `storage/tests.rs`:
- `check_integrity_ok_on_new_vault`: `Ok(())`.
- `check_integrity_detects_damaged_page`: con 3 entradas, `damage_entries_root_page`; `Storage::open_read_only` sale bien y `check_integrity()` da `Storage(Corrupt)`.

En `backup/tests.rs`. Ayudante local `assert_import_fails_intact(backup, dest, pw, expected: Error)`: guarda los bytes de `dest` (o `None` si no existe), importa, compara el error, comprueba que los bytes o la ausencia siguen iguales y que `!sibling(dest, ".import").exists()`.
- `import_round_trip`: 3 entradas (una con notas), eliminar una, exportar, borrar el archivo original e importar sobre esa misma ruta. `report == UnlockReport { corrupt: vec![], unidentified_corrupt: 0 }`, `entries()` tiene los 2 ids, `reveal` da `clave-{service}` y las notas, y el id eliminado no está.
- `import_wrong_password_keeps_dest`: `WrongPassword`, con `dest` siendo otra bóveda.
- `import_future_version_keeps_dest`: en la copia exportada, `UPDATE header SET format_version = 2` con `raw` → `UnsupportedFormatVersion(2)`.
- `import_not_a_vault_keeps_dest`: respaldos de bytes `[0x5A; 4096]`, vacío, texto `"hola"` y un SQLite ajeno (`CREATE TABLE t(x)`) → `NotAVault` cada uno, sin panic.
- `import_damaged_backup_is_corrupt`: `damage_entries_root_page` sobre el respaldo → `Storage(Corrupt)`.
- `import_reports_corrupt_entry`: `tamper(respaldo, id, "search")` → importa bien y `report.corrupt == vec![id]`.
- `import_into_missing_dest`: `dest` no existe → se crea y abre. Con contraseña incorrecta, `dest` sigue sin existir.
- `import_uses_backup_password`: respaldo creado con `respaldo`, `dest` con `actual`. Importa con `respaldo`; después `Vault::open(dest)` + `unlock(actual)` da `WrongPassword`.
- `import_replaces_leftover_temp`: `<dest>.import` con `b"basura"` antes → importa bien y no queda.
- `import_relative_paths_are_io`: `backup` relativo y `dest` relativo → `Storage(Io)` cada uno.
- `import_missing_backup_or_directory_is_io` (Review Focus 3): `backup` inexistente, y `dest` en un directorio inexistente → `Storage(Io)`, sin `.import`.
- `import_same_path_as_dest` (Review Focus 1): `Vault::import(&path, &path, ..)` sobre una bóveda cerrada → sale bien, mismas entradas, sin `.import`.
- `import_non_ascii_paths` (Review Focus 2): respaldo y `dest` con `"bóveda ñ"` en el nombre.
- `imported_vault_is_writable_and_exportable` (Review Focus 5): tras importar, `add`, `export` a otro archivo e importar ese → tiene la entrada nueva.

- [ ] **Step 2: Ejecutar y ver que fallan**

Run: `cargo test -p vault-core import check_integrity`
Expected: FAIL de compilación (`import` y `check_integrity` no existen).

- [ ] **Step 3: Implementar**

`backup::import`, en este orden (la spec numera los pasos 1 a 11; el 9 es la Task 5):
1. `require_absolute` de `backup` y `dest`.
2. `temp = sibling(dest, IMPORT_SUFFIX)`. Un guardián local (`struct TempFile(PathBuf)` con `Drop` que hace `fs::remove_file` ignorando el error) se crea **antes** de abrir cualquier conexión sobre `temp`, para que se suelte después de ella (en Windows no se borra un archivo abierto).
3. `fs::copy(backup, &temp)` (sobrescribe un `.import` sobrante) → `Storage(Io)`; luego `OpenOptions::new().write(true).open(&temp)?.sync_all()` → `Storage(Io)`. En Windows, `FlushFileBuffers` exige permiso de escritura.
4. `Storage::open_read_only(&temp)`, `check_integrity()`, `read_header()?.unlock(password)?`, `Vault::load_entries(&storage, key)?` y `storage.close()?`.
5. `fs::rename(&temp, dest)` → `Storage(Io)`. En `#[cfg(unix)]`, `fsync` del directorio padre como mejor esfuerzo (`File::open(parent).and_then(|d| d.sync_all())`, ignorando el error).
6. `Storage::open(dest)?` → `Vault { storage, unlocked: Some(unlocked) }` y el reporte.

Quitar los dos `#[cfg_attr(not(test), expect(dead_code, ...))]` de `open_read_only` y `close` en `storage.rs`, que ya se usan.

- [ ] **Step 4: Ejecutar las pruebas**

Run: `cargo test -p vault-core`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add core/src/vault
git commit -m "feat: importar un respaldo validándolo en una copia temporal"
```

### Task 5: Revertir el `-journal` de la bóveda actual antes de reemplazarla

**Files:**
- Modify: `core/src/vault/backup.rs`, `core/src/vault/backup/tests.rs`

**Interfaces:**
- Consumes: `backup::import` (Task 4), `make_hot_journal_copy` y `sibling` (Task 1).
- Produces: `fn settle_dest(dest: &Path) -> Result<()>` (privada): si existe `sibling(dest, "-journal")` o `sibling(dest, "-wal")`, `Storage::open(dest)?.close()`. Se llama entre la validación y el `rename`.

- [ ] **Step 1: Pruebas que fallan**
- `import_rolls_back_hot_journal_of_dest`: `make_hot_journal_copy(bóveda_vieja, dest)`; importar un respaldo → sale bien, `!sibling(dest, "-journal").exists()`, y `Vault::open(dest)` + `unlock` da las entradas del respaldo y ninguna fila de las 200 insertadas por el ayudante. Sin el arreglo falla porque SQLite aplica el diario viejo sobre el respaldo: el error esperado es `Storage(Corrupt)` o entradas incorrectas.
- `import_keeps_unopenable_dest_with_journal`: `dest` con texto `"no soy bóveda"` y `<dest>-journal` con `[0xAA; 1024]` → `NotAVault`, y los dos archivos quedan igual byte por byte, sin `.import`.
- `#[cfg(windows)] import_fails_while_dest_is_open`: conexión `raw(dest)` abierta con una consulta ejecutada (`SELECT count(*) FROM entries`) → la importación da `Storage(Io)` o `Storage(Busy)`, y `dest` queda igual byte por byte tras soltar la conexión.

- [ ] **Step 2: Ejecutar y ver que fallan**

Run: `cargo test -p vault-core import_rolls_back import_keeps_unopenable`
Expected: FAIL en `import_rolls_back_hot_journal_of_dest` y en `import_keeps_unopenable_dest_with_journal` (hoy reemplaza el archivo).

- [ ] **Step 3: Implementar `settle_dest`** y llamarla en `import` justo antes de `fs::rename`. Comentario con la cita de "How To Corrupt Your Database Files" §1.4 y por qué no se borra el diario.
- [ ] **Step 4: Ejecutar las pruebas**

Run: `cargo test -p vault-core`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add core/src/vault/backup.rs core/src/vault/backup/tests.rs
git commit -m "feat: revertir el -journal de la bóveda actual antes de reemplazarla"
```

### Task 6: Errores sin secretos y prueba de punta a punta

**Files:**
- Modify: `core/src/vault/backup/tests.rs`, `core/tests/vault.rs`

**Interfaces:**
- Consumes: `Vault::export` y `Vault::import` públicas.

- [ ] **Step 1: Pruebas**
- `backup_errors_do_not_leak`: se juntan los errores de exportar bloqueada, exportar sobre un archivo existente, importar con contraseña incorrecta, importar algo que no es bóveda e importar un respaldo dañado. Ni `Display` ni `Debug` contienen la contraseña maestra, `clave-`, el servicio, ni el nombre del directorio temporal (`dir.path().file_name()`).
- En `core/tests/vault.rs`, `export_delete_import_with_calibration`: `Vault::create` con calibración real, 2 entradas, `export`, soltar la bóveda, `fs::remove_file` de la original, `Vault::import` → mismas entradas por id y servicio, y `reveal` da cada contraseña. Solo usa `vault_core::vault::*`.

- [ ] **Step 2: Ejecutar**

Run: `cargo test -p vault-core`
Expected: PASS (son pruebas de comportamiento ya implementado; si alguna falla, es un defecto de las Tasks 2 a 5).

- [ ] **Step 3: Commit**

```bash
git add core/src/vault/backup/tests.rs core/tests/vault.rs
git commit -m "test: errores sin secretos y respaldo de punta a punta"
```

### Task 7: Documentación

**Files:**
- Modify: `docs/especificacion.md`, `ARCHITECTURE.md`, `docs/roadmap.md`

- [ ] **Step 1: Especificación**, "Formato de la bóveda → Respaldos": agregar "Decisiones tomadas el 2026-11-07 (detalle en `docs/superpowers/specs/2026-11-07-respaldos-design.md`)" con viñetas para:
  - exportar exige la bóveda desbloqueada;
  - `Vault::import(backup, dest, password)` como función asociada, y quien llama suelta antes la bóveda abierta;
  - la validación en una copia temporal (`integrity_check` y desbloqueo real), y que las entradas dañadas se reportan sin impedir importar;
  - el `-journal` de `dest` se revierte antes de reemplazar;
  - rutas absolutas por `SQLITE_USE_URI`;
  - SQLite hace el `fsync` del respaldo con `synchronous = FULL`.
- [ ] **Step 2: `ARCHITECTURE.md`**: en la fila de `vault/`, cambiar "respaldos" por "respaldos: exportar con `VACUUM INTO` e importar validando una copia temporal (`backup.rs`)". **Roadmap**: marcar `- [x] Exportar e importar el respaldo cifrado.`
- [ ] **Step 3: Verificación final**

Run: `cargo fmt --all && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`
Expected: todo pasa, sin advertencias.

- [ ] **Step 4: Commit**

```bash
git add docs ARCHITECTURE.md
git commit -m "docs: registrar los respaldos en la especificación, la arquitectura y el roadmap"
```
