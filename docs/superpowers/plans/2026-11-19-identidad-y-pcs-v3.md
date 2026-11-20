# Identidad y PCs emparejadas en la bóveda, formato v3 (#93) — plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** La bóveda guarda la llave privada X25519 del celular y la lista de PCs emparejadas, cifradas; sube a formato v3 con migración desde v1 y v2, y el respaldo importado nunca conserva identidad ni PCs.

**Architecture:** Dos tablas nuevas (`identity`, `paired_pcs`) sin tocar `header` ni `entries`. El dato autenticado de `encrypted_key` pasa a usar una versión de la cabecera (`HEADER_VERSION`) independiente de `format_version`, para que `unlock_with_key` pueda migrar. Identidad y PCs se cifran como las partes de una entrada, con dato autenticado propio, y se leen bajo demanda.

**Tech Stack:** Rust, `rusqlite`, `chacha20poly1305`, `postcard`/`serde`, `secrecy`, `zeroize`, `getrandom`. Sin dependencias nuevas.

**Spec:** `docs/superpowers/specs/2026-11-19-identidad-y-pcs-v3-design.md`

## Global Constraints

- Identificadores en inglés; comentarios, documentación y commits en español, Conventional Commits con `(#93)`.
- Rama `feat/93-formato-v3` desde `dev`. PR contra `dev` con `Closes #93`.
- Pruebas primero: cada función nueva empieza con una prueba que falla (`core/CLAUDE.md`). Las unitarias van en `modulo/tests.rs`; las de la API pública, en `core/tests/`.
- Nunca modificar ni borrar `core/tests/fixtures/vault-v1.db` ni `vault-v2.db`. Las pruebas trabajan sobre copias.
- Ningún secreto en logs, errores, panics ni `Debug`. La llave privada va en `SecretBox`; los búferes con texto en claro, en `Zeroizing`.
- Sin dependencias nuevas, y `x25519-dalek` **no** se agrega (lo trae la #94).
- Nonce aleatorio de 24 bytes nuevo en cada cifrado. Valores fijos: identidad = 32 bytes sin serializar; id de PC = 16 bytes aleatorios; nombre de PC de 1 a 128 bytes UTF-8.
- Rutas absolutas en toda la API pública (ya lo exige `Storage`).
- No cambiar nada de `export_vault_key` ni del tipo `UnlockReport`.
- Antes de cada commit: `cargo test -p vault-core`. Nunca encadenar `clippy | grep` antes de un commit (oculta el fallo).
- Cambios en `mobile/rust/src/api/`: solo comentarios `//` (los `///` los copia el generador al Dart).

## Review Focus

Entradas o condiciones que el diseño implica y que ninguna tarea debe dejar sin prueba, de más a menos probable:

- Una v3 con `format_version` bajado a 2 a mano: con contraseña, error `Storage(Corrupt)`, bóveda bloqueada y archivo igual (Task 2); con llave, `WrongVaultKey`.
- Un respaldo v3 con identidad y PCs reales: la bóveda instalada queda sin ellas y sus bytes cifrados no están en el archivo (Task 3 y Task 7).
- Una fila de `paired_pcs` alterada fuera de SQLite (id de 15 bytes, `data` corta, versión fuera de `u16`): aparece como dañada y no impide listar las demás (Task 6).
- Dos conexiones desbloqueando una v2 a la vez, con y sin cambio de contraseña en medio (Task 2).
- Migración v1→v2 confirmada y v2→v3 fallida: archivo en v2 válida, bóveda bloqueada (Task 2).

---

### Task 1: Cabecera y esquema v3 (sin migración)

**Files:**
- Modify: `core/src/vault/header.rs`, `core/src/vault/header/tests.rs`
- Modify: `core/src/vault/storage.rs`, `core/src/vault/storage/tests.rs`
- Modify: `core/src/vault/tests.rs`, `core/src/vault/backup/tests.rs`, `core/src/crypto/keys/tests.rs` (solo donde una prueba asuma que una bóveda nueva es v2)

**Interfaces:**
- Produces (en `header.rs`):
  - `pub(crate) const FORMAT_VERSION_V1: u16 = 1`, `FORMAT_VERSION_V2: u16 = 2`, `FORMAT_VERSION: u16 = 3`.
  - `pub(crate) const HEADER_VERSION: u16 = 2` y `pub(crate) const fn header_aad_version(format_version: u16) -> u16` (1 para la v1; 2 para la v2 y las siguientes).
  - `VaultHeader::create`/`rekey` producen una cabecera v3: `key_check` sellado con `FORMAT_VERSION`, dato autenticado con `header_aad_version(FORMAT_VERSION)`.
  - `VaultHeader::from_parts` acepta 1 (sin `key_check`), 2 y 3 (con él).
  - `VaultHeader::unlock_upgrading` sigue migrando solo v1→v2: devuelve `Some(cabecera v2)` para una v1 y `None` para una v2 o v3.
- Produces (en `storage.rs`): la constante `SCHEMA` incluye `identity` y `paired_pcs` con el SQL de la especificación; el texto de las dos tablas vive en macros o constantes compartidas con la migración de la Task 2. `read_header` lee v1, v2 y v3. `insert_header` y `current_key_check` aceptan una cabecera con `key_check` de la v2 o la v3 (la migración v1→v2 inserta una v2).

- [ ] **Step 1: Pruebas que fallan** en `header/tests.rs`: `header_aad_version_is_1_for_v1_and_2_from_v2` (1→1, 2→2, 3→2, 4→2); `v2_associated_data_bytes_are_unchanged` (los bytes del dato autenticado de una cabecera v2 son los de antes: `versión = 2`); `new_header_is_v3_with_valid_key_check` (`format_version() == 3`, `verify_key` pasa, y `keys::verify_key_check(key, 2, check)` falla); `from_parts_accepts_versions_1_2_3_and_rejects_4` (`UnsupportedFormatVersion(4)`). En `storage/tests.rs`: `created_vault_has_identity_and_paired_pcs_tables` (ambas en `sqlite_schema`, vacías) y `read_header_reads_v3`.
- [ ] **Step 2:** `cargo test -p vault-core --lib header storage` y comprobar que fallan por lo esperado.
- [ ] **Step 3: Implementar** las constantes y `header_aad_version` en `header.rs`; `encode_associated_data` recibe la versión de la cabecera, no `format_version`; `rekey`, `from_parts` y `unlock_upgrading` según Interfaces (la rama v1→v2 usa `FORMAT_VERSION_V2` explícito). En `storage.rs`, agregar las tablas a `SCHEMA` y la rama v3 de `read_header`, y ajustar `current_key_check` y la verificación de `migrate_header` (`FORMAT_VERSION_V2`).
- [ ] **Step 4:** Corregir las pruebas existentes que asumían que una bóveda nueva es v2 (buscar `FORMAT_VERSION` y `format_version` en los archivos de Files). `cargo test -p vault-core` en verde.
- [ ] **Step 5: Commit**

```bash
git add core/src
git commit -m "feat: cabecera y esquema del formato v3 (#93)"
```

---

### Task 2: Migración v2→v3 al desbloquear

**Files:**
- Modify: `core/src/vault/storage.rs`, `core/src/vault/mod.rs`, `core/src/vault/header.rs`
- Test: `core/src/vault/tests.rs`, `core/src/vault/storage/tests.rs`, `core/tests/vault.rs`

**Interfaces:**
- Consumes: Task 1.
- Produces:
  - `Storage::migrate_v2_to_v3(&mut self, key_check: &[u8; KEY_CHECK_LEN]) -> Result<Migration>`: una transacción `IMMEDIATE` con los cuatro pasos de "Migración v2→v3" del diseño. Devuelve `Migration::Done` o `NotNeeded` (relee `format_version` dentro de la transacción; 3 → `NotNeeded`; distinto de 2 y 3 → `Storage(Corrupt)`). Si `identity` o `paired_pcs` ya existen en `sqlite_schema`, `Storage(Corrupt)`.
  - `Vault::finish_unlock` encadena v1→v2 y v2→v3; `unlock_with_key` verifica con la cabecera leída y migra si es v2.

- [ ] **Step 1: Pruebas que fallan** (unitarias, `vault/tests.rs`, con copias de los fixtures):
  - `unlock_migrates_v2_to_v3_without_identity` (contraseña): `format_version == 3`, `identity` y `paired_pcs` vacías, `key_check` verifica con la versión 3, `export_vault_key` entrega una llave con la que `unlock_with_key` abre.
  - `unlock_with_key_migrates_v2_to_v3_and_keystore_copy_still_works`: se desbloquea una copia de `vault-v2.db` con contraseña y se exporta la llave `K` (el Keystore guardaría esa copia); con una segunda copia, aún v2, `unlock_with_key(K)` abre, deja el archivo en v3 y `K` sigue valiendo después (`unlock_with_key` otra vez tras bloquear).
  - `unlock_migrates_v1_to_v3_with_one_derivation`.
  - `unlock_with_key_on_v1_is_wrong_vault_key_and_writes_nothing`.
  - `wrong_vault_key_on_v2_writes_nothing`.
  - `migrated_schema_equals_created_schema_from_v1_and_from_v2` (`sqlite_schema` igual al de `Vault::create`).
  - `v1_to_v2_committed_then_v2_to_v3_failed_leaves_valid_v2_locked` (hacer fallar la segunda transacción con `deny_writes_for_test` o un `TRIGGER ... RAISE(ABORT)` en `header` después de la primera; archivo en v2, `integrity_check` ok, `Vault` bloqueada, error de almacenamiento).
  - **Degradación:** `downgraded_v3_to_v2_with_password_fails_cleanly`: tomar una v3 recién creada y ejecutar `UPDATE header SET format_version = 2`; `unlock` con la contraseña devuelve `Storage(Corrupt)`, `is_locked()`, archivo igual byte por byte (`fs::read` antes y después) y sin pánico. `downgraded_v3_to_v2_with_key_is_wrong_vault_key`: `unlock_with_key` con la llave exportada da `WrongVaultKey`. Comentario en la prueba: el caso lo cubre `key_check` (versión 3), no `encrypted_key`.
  - **Carrera:** `concurrent_v2_to_v3_migration_does_not_revert_password_change` y `concurrent_v2_to_v3_migration_without_password_change_unlocks` (mismo molde que las pruebas de v1→v2, intercalando otra `Vault` entre la derivación y `finish_unlock`; una contraseña cambiada da `WrongPassword`).
  - Reescribir `v2_unlock_writes_nothing` en `vault/tests.rs` y `fixture_v2_unlock_does_not_write` en `core/tests/vault.rs`: ahora una **v3** no escribe al desbloquear, y una v2 sí (migra).
  - `open_never_modifies_any_fixture_version` (`core/tests/vault.rs`): para `vault-v1.db`, `vault-v2.db` y `vault-v3.db` (esta última, tras la Task 7; hasta entonces, una v3 creada en la prueba), `Vault::open` y `Storage::open_read_only` dejan los bytes iguales.
- [ ] **Step 2:** `cargo test -p vault-core` y comprobar que fallan por lo esperado.
- [ ] **Step 3: Implementar** `Storage::migrate_v2_to_v3` (reutilizar el patrón de `migrate_header` y el texto compartido de las tablas); `Vault::finish_unlock`: tras la v1→v2 (si había), calcular `keys::seal_key_check(&key, FORMAT_VERSION)` y llamar a `migrate_v2_to_v3`; en `NotNeeded`, releer la cabecera y volver a verificar la contraseña. `unlock_with_key`: leer la cabecera, `verify_key` (con la versión que tenga), y si es v2, migrar; en `NotNeeded`, volver a leer y verificar.
- [ ] **Step 4:** `cargo test -p vault-core` en verde.
- [ ] **Step 5: Commit**

```bash
git add core
git commit -m "feat: migrar la bóveda de v2 a v3 al desbloquear (#93)"
```

---

### Task 3: Importar migra y descarta identidad y PCs

**Files:**
- Modify: `core/src/vault/storage.rs`, `core/src/vault/backup.rs`
- Test: `core/src/vault/backup/tests.rs`, `core/src/vault/storage/tests.rs`

**Interfaces:**
- Consumes: Task 1 y Task 2.
- Produces:
  - `Storage::prepare_import_copy(&mut self, from_v2: Option<&[u8; KEY_CHECK_LEN]>) -> Result<()>`: una transacción `IMMEDIATE`. `Some(key_check)`: espera la versión 2, aplica la migración v2→v3 con ese `key_check`; `None`: espera la 3. Si la versión leída dentro de la transacción no es la esperada, `Storage(Corrupt)`. Termina con `DELETE FROM identity` y `DELETE FROM paired_pcs`. Comparte con la Task 2 las funciones privadas que operan sobre la transacción.
  - `backup::validate` devuelve además la versión de origen (`u16`); `backup::import` encadena: v1→v2 (si aplica) y `prepare_import_copy`, ambos antes de `settle_dest`.

- [ ] **Step 1: Pruebas que fallan** (`backup/tests.rs`, con filas de relleno en `identity` y `paired_pcs` insertadas por SQL directo con blobs de longitud válida):
  - `import_v3_backup_discards_identity_and_pcs`: el respaldo trae filas en ambas tablas; tras importar, `SELECT count(*)` da 0 en `identity` y en `paired_pcs` (la aserción con `Vault::identity() == None` y `paired_pcs()` vacía se añade en la Task 5 y la Task 6).
  - `import_v2_backup_installs_v3_with_empty_identity_and_pcs` (un respaldo v2, las dos tablas existen y están vacías, `format_version == 3`); `import_v1_backup_installs_v3_with_one_derivation`.
  - `import_failed_discard_installs_nothing`: un `TRIGGER BEFORE DELETE ON paired_pcs BEGIN SELECT RAISE(ABORT, 'x'); END` creado en la copia de prueba del respaldo; `import` devuelve error de almacenamiento, `dest` queda igual byte por byte (o sin crear si no existía) y no queda archivo `.import` sobrante.
  - `import_copy_changed_after_validation_is_corrupt`: la copia cambia de versión entre `validate` y `prepare_import_copy` (probar `prepare_import_copy` directamente con la versión equivocada): `Storage(Corrupt)` y nada escrito.
  - No se prueba ni se afirma la eliminación física de bytes residuales: SQLite no la garantiza. La garantía es lógica (tablas vacías en la bóveda instalada).
- [ ] **Step 2:** `cargo test -p vault-core --lib backup` y comprobar que fallan.
- [ ] **Step 3: Implementar** `prepare_import_copy` y el cambio en `validate`/`import`. Calcular `seal_key_check(&unlocked.key, FORMAT_VERSION)` solo si el origen es v1 o v2.
- [ ] **Step 4:** `cargo test -p vault-core` en verde.
- [ ] **Step 5: Commit**

```bash
git add core
git commit -m "feat: importar migra a v3 y descarta identidad y PCs (#93)"
```

---

### Task 4: Errores nuevos, mapeo en la fachada y cifrado con dato autenticado propio

**Files:**
- Modify: `core/src/error.rs`, `core/src/error/tests.rs`, `core/src/crypto/aead.rs`, `core/src/crypto/aead/tests.rs`
- Modify: `mobile/rust/src/api/error.rs` (y sus pruebas)

**Interfaces:**
- Produces:
  - `Error::{CorruptIdentity, CorruptPairedPc, PairedPcNotFound, PairedPcExists, InvalidPairedPc(InvalidPairedPcField)}` y `InvalidPairedPcField::{NameEmpty, NameTooLong}` (`#[non_exhaustive]`), con mensajes sin datos.
  - En `aead.rs`: `pub(crate) fn seal_with_ad(key: &VaultKey, ad: &[u8], plaintext: &[u8]) -> Result<EncryptedPart>` y `pub(crate) fn open_with_ad(key: &VaultKey, ad: &[u8], sealed: &EncryptedPart) -> Result<Zeroizing<Vec<u8>>>`, con el mismo manejo de búferes `Zeroizing` y de capacidad exacta que `seal_part` y `open_part`, que pasan a llamarlas. Un fallo de `open_with_ad` devuelve `Error::CorruptEntry` (el llamador lo traduce al error de su pieza).
  - Mapeo provisional en la fachada, con comentarios `//`: `CorruptIdentity` y `CorruptPairedPc` → `StorageCorrupt`; `PairedPcNotFound` → `EntryNotFound`; `PairedPcExists` → `AlreadyExists`; `InvalidPairedPc(_)` → `StorageOther`.

- [ ] **Step 1: Pruebas que fallan:** en `aead/tests.rs`, `seal_with_ad_round_trips`, `open_with_ad_rejects_other_ad`, `open_with_ad_rejects_a_flipped_byte`, `seal_part_output_is_unchanged` (las pruebas existentes de entradas siguen pasando sin tocarlas); en `error/tests.rs`, los mensajes nuevos no contienen datos; en la fachada, una prueba por variante nueva con su mapeo.
- [ ] **Step 2:** `cargo test -p vault-core` y `cargo test -p rust_lib_boveda`; comprobar que fallan.
- [ ] **Step 3: Implementar.** Después, regenerar el puente (`flutter_rust_bridge_codegen generate`, desde `mobile/`) y comprobar que `git diff` no cambia nada generado.
- [ ] **Step 4:** `cargo test --workspace` en verde.
- [ ] **Step 5: Commit**

```bash
git add core mobile/rust
git commit -m "feat: errores de identidad y PCs y cifrado con dato autenticado propio (#93)"
```

---

### Task 5: Identidad perezosa

**Files:**
- Create: `core/src/vault/identity.rs`, `core/src/vault/identity/tests.rs`
- Modify: `core/src/vault/mod.rs`, `core/src/vault/storage.rs`, `core/src/vault/test_support.rs`
- Test: `core/src/vault/tests.rs`, `core/tests/vault.rs`

**Interfaces:**
- Consumes: Task 4 (`seal_with_ad`, `open_with_ad`, errores).
- Produces:
  - `pub struct Identity` con `pub fn private_key(&self) -> &SecretBox<[u8; 32]>`; sin `Debug` con el valor (implementación manual que no lo muestra); se borra al soltarse.
  - `Vault::identity(&self) -> Result<Option<Identity>>` y `Vault::ensure_identity(&mut self) -> Result<Identity>`.
  - `Storage::get_identity(&self) -> Result<Option<StoredIdentity>>` y `Storage::insert_identity(&mut self, version: u16, data: &EncryptedPart) -> Result<()>` (inserta con `id = 1`; si ya existe, `Storage(Corrupt)`).
  - Dato autenticado: `boveda-identity` (15 bytes) | versión de estructura `u16` LE (17 bytes). Texto en claro: los 32 bytes sin serializar, en `Zeroizing`.
  - Ayudante de pruebas `Vault::set_identity_for_test(&mut self, key: [u8; 32])` (`#[cfg(test)]`) para fijar una identidad conocida.

- [ ] **Step 1: Pruebas que fallan:** `identity_is_none_on_a_new_vault` (ausente = `Ok(None)`, no error); `ensure_identity_creates_the_first_when_absent`; `absent_and_damaged_identity_are_distinct` (`identity()` da `Ok(None)` sin fila y `Err(CorruptIdentity)` con la fila alterada); `ensure_identity_creates_once_and_returns_the_same` (dos llamadas, mismos bytes); `identity_survives_lock_and_reopen`; `identity_requires_unlocked` (`Locked`); `flipped_byte_in_identity_data_is_corrupt`; `changed_identity_schema_version_is_corrupt`; `identity_swapped_with_a_pc_row_is_corrupt` (tras la Task 6, se completa); `damaged_identity_does_not_block_unlock_or_entries`; `ensure_identity_on_a_damaged_one_never_overwrites_it` (`CorruptIdentity`, y `data` e `identity_schema_version` iguales byte por byte; repetir tras reabrir); `identity_data_is_not_plaintext_in_the_file` (los 32 bytes no están en el archivo con `test_support::contains`); `identity_debug_does_not_show_the_key`; y en `core/tests/vault.rs` el recorrido por la API pública: crear, bloquear, reabrir y leer la misma identidad. Añadir a las pruebas de importar de la Task 3 la aserción `identity() == None` tras importar un respaldo v3 con identidad.
- [ ] **Step 2:** `cargo test -p vault-core` y comprobar que fallan.
- [ ] **Step 3: Implementar.** Generar los 32 bytes con `getrandom` directamente en un `Zeroizing<[u8; 32]>` y mover al `SecretBox` sin copias intermedias; documentar en el módulo que la #94 no debe validar ni transformar esos bytes al leerlos.
- [ ] **Step 4:** `cargo test -p vault-core` en verde.
- [ ] **Step 5: Commit**

```bash
git add core
git commit -m "feat: identidad X25519 del celular cifrada en la bóveda (#93)"
```

---

### Task 6: PCs emparejadas

**Files:**
- Create: `core/src/vault/paired_pc.rs`, `core/src/vault/paired_pc/tests.rs`
- Modify: `core/src/vault/mod.rs`, `core/src/vault/storage.rs`
- Test: `core/src/vault/tests.rs`, `core/tests/vault.rs`

**Interfaces:**
- Consumes: Task 4 y Task 5 (para la prueba del intercambio).
- Produces:
  - `pub struct PairedPcId` (16 bytes; `Display` y `Debug` en hexadecimal, como `EntryId`), `pub struct PairedPc { id, public_key: [u8; 32], name: String, last_ip: Option<IpAddr>, paired_at: i64, last_seen_at: Option<i64> }` con accesores, `Debug` que solo muestra el id y `ZeroizeOnDrop` del nombre; `pub struct NewPairedPc { pub public_key: [u8; 32], pub name: String, pub last_ip: Option<IpAddr> }`; `pub struct PairedPcUpdate { pub name: Option<String>, pub last_ip: Option<IpAddr> }`; `pub struct PairedPcs { pub valid: Vec<PairedPc>, pub corrupt: Vec<PairedPcId>, pub unidentified_corrupt: usize }`.
  - `Vault::paired_pcs(&self) -> Result<PairedPcs>`, `add_paired_pc(&mut self, new: NewPairedPc) -> Result<PairedPcId>`, `update_paired_pc(&mut self, id: PairedPcId, update: PairedPcUpdate) -> Result<()>`, `remove_paired_pc(&mut self, id: PairedPcId) -> Result<()>`.
  - `Storage`: `list_paired_pcs`, `insert_paired_pc`, `update_paired_pc`, `delete_paired_pc` (este último, `DELETE` real; si no cambia una fila, `PairedPcNotFound`).
  - Dato autenticado: `boveda-pc` (9) | id (16) | versión de estructura `u16` LE (27 bytes). Texto en claro: `postcard` de la estructura privada de la versión 1, con `last_ip` como `Option<Vec<u8>>` de 4 o 16 bytes.
  - Reglas: nombre recortado, de 1 a 128 bytes UTF-8 (`InvalidPairedPc`); `add` con una `public_key` ya emparejada da `PairedPcExists` (ignorando las filas dañadas); `paired_at` con el reloj de `Vault` (`Error::Clock` si no es válido); un `last_ip` nuevo en `update` fija `last_seen_at`; `update` y `remove` de un id inexistente dan `PairedPcNotFound`; `update` sobre una fila dañada da `CorruptPairedPc` sin escribir.

- [ ] **Step 1: Pruebas que fallan:** `add_list_update_remove_round_trip`; `public_key_already_paired_is_rejected`; `empty_or_blank_name_is_invalid`; `name_of_129_bytes_is_too_long_and_128_is_accepted`; `name_is_trimmed`; `update_ip_sets_last_seen_at`; `update_without_ip_keeps_last_seen_at`; `remove_unknown_id_is_not_found`; `paired_pcs_require_unlocked`; `pcs_survive_lock_and_reopen`; `name_and_ip_are_not_plaintext_in_the_file`; **filas dañadas** (`list_reports_corrupt_pcs_and_still_lists_the_rest`): un byte alterado en `data`, `pc_schema_version` fuera de `u16`, una PC movida a otro id, una fila con `id` de 15 bytes (insertada con el `CHECK` desactivado en una copia, `PRAGMA ignore_check_constraints = ON`, que cuenta en `unidentified_corrupt`) y una con `last_ip` de 5 bytes; `remove_paired_pc_deletes_a_corrupt_row`; `corrupt_pc_does_not_block_unlock`; `paired_pc_debug_shows_only_the_id`.
- [ ] **Step 2:** `cargo test -p vault-core` y comprobar que fallan.
- [ ] **Step 3: Implementar.** La comprobación de duplicados descifra la lista una vez; ninguna operación guarda PCs en la caché de `Unlocked`.
- [ ] **Step 4:** `cargo test -p vault-core` en verde.
- [ ] **Step 5: Commit**

```bash
git add core
git commit -m "feat: lista de PCs emparejadas cifrada en la bóveda (#93)"
```

---

### Task 7: Bóveda de ejemplo `vault-v3.db` y pruebas de integración

**Files:**
- Create: `core/tests/fixtures/vault-v3.db` (generada una sola vez)
- Modify: `core/src/vault/tests.rs` (generador), `core/tests/vault.rs`, `core/tests/fixtures/README.md`

**Interfaces:**
- Consumes: Tasks 1–6.
- Produces: `generate_fixture_v3` (`#[ignore]`), contraseña maestra `prueba-bóveda-v3` (con "ó"), `KdfParams::MIN`, el mismo contenido que v1 y v2 (`fill_fixture`), una identidad fija (`set_identity_for_test([7u8; 32])`) y dos PCs: una con IP y fechas, otra sin IP.

- [ ] **Step 1: Pruebas que fallan** (en `core/tests/vault.rs`, sobre copias): `fixture_v3_opens_with_known_contents` (las cuatro entradas, la identidad `[7u8; 32]`, las dos PCs con sus campos); `fixture_v3_unlock_does_not_write`; `fixture_v1_and_v2_still_open_and_migrate_to_v3` (las dos terminan en v3 y conservan su contenido); `import_of_fixture_v3_leaves_no_pairing` (importar `vault-v3.db`: `identity() == None`, `paired_pcs()` vacía y las tablas con 0 filas, entradas intactas) e `import_of_fixture_v2_leaves_no_pairing`; `fixture_v3_wrong_password`; `fixture_v3_tampered_identity_is_detected`.
- [ ] **Step 2:** `cargo test -p vault-core` y comprobar que fallan (falta el archivo).
- [ ] **Step 3: Generar el fixture una sola vez**, con `cargo test -p vault-core --lib -- --ignored --exact vault::tests::generate_fixture_v3`. Nunca `--ignored` sin filtro. Confirmar con `git status` que no cambió ningún otro `.db`.
- [ ] **Step 4:** Documentar `vault-v3.db` en el README de fixtures (contraseña, contenido, generador y cómo se ejecutó) y actualizar la regla de que desbloquear una bóveda de la versión 1 o 2 la migra a la 3. `cargo test -p vault-core` en verde.
- [ ] **Step 5: Commit**

```bash
git add core/tests core/src
git commit -m "test: bóveda de ejemplo v3 con identidad y PCs (#93)"
```

---

### Task 8: Documentación y cierre

**Files:**
- Modify: `docs/especificacion.md`, `docs/roadmap.md`, `ARCHITECTURE.md`

- [ ] **Step 1: `docs/especificacion.md`:** nueva subsección "Formato v3" bajo "Formato de la bóveda" (esquema, datos autenticados, `HEADER_VERSION`, el motivo y la consecuencia para el futuro, la degradación y quién la cubre); la descripción del dato autenticado de la cabecera ("versión" es la versión de la cabecera); la migración (v1→v2→v3, el archivo "sigue siendo v1" pasa a "queda en la última versión confirmada"); "Respaldos" (el borrado ya ocurre en `Vault::import`, dentro de la copia temporal, y la #106 solo avisa); la API de identidad y PCs; y "Emparejamiento y envío" (identidad perezosa ya en el formato v3). Quitar "Aplica a partir de la etapa 3" donde ya aplique.
- [ ] **Step 1b: `docs/roadmap.md` (etapa 3):** el punto del protocolo/identidad refleja el formato v3 con sus migraciones (v1→v2→v3) y la identidad y PCs dentro de la bóveda (#93); el punto de restaurar un respaldo pasa a decir que la limpieza de identidad y PCs ocurre en `Vault::import` (#93) y que la #106 pone el aviso de emparejar de nuevo y las pruebas de interfaz. Sin marcar casillas que dependan de otras issues.
- [ ] **Step 2: `ARCHITECTURE.md`:** fila de `vault/` (cabecera formato 3, `identity.rs`, `paired_pc.rs`, migración v1→v2→v3 y el borrado al importar) y el árbol del mapa del repositorio.
- [ ] **Step 3: Verificación completa**, cada comando por separado y comprobando el código de salida: `cargo fmt --all --check`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo deny check`, `cargo audit`. Comprobar `git status`: ningún `.db` de ejemplo distinto de `vault-v3.db`.
- [ ] **Step 4: Commit**

```bash
git add docs ARCHITECTURE.md
git commit -m "docs: formato v3, identidad y PCs emparejadas (#93)"
```

- [ ] **Step 5:** Revisión final independiente y PR contra `dev` con el formato del PR #11 (Resumen, Decisiones de implementación, Criterios de aceptación con casillas, Verificación local, `Closes #93`). Con tu visto bueno, ajustar el texto de la #106.
