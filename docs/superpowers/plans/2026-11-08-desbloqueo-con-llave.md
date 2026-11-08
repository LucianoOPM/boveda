# Desbloqueo con la llave de bóveda: plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `Vault::export_vault_key` y `Vault::unlock_with_key`, con un verificador `key_check` en la cabecera (formato v2) y migración desde la v1.

**Architecture:** `crypto/keys.rs` crea y comprueba `key_check` (XChaCha20-Poly1305 sobre texto vacío). `VaultHeader` pasa a la v2 y sabe desbloquear una v1 devolviendo la cabecera v2 ya armada con la misma llave derivada. `Storage` lee las dos versiones y migra con el procedimiento de 12 pasos de SQLite en una transacción. `Vault::unlock` y `backup::import` migran; `export_vault_key` repara el verificador si está dañado.

**Tech Stack:** Rust (toolchain de `rust-toolchain.toml`), `chacha20poly1305`, `secrecy`, `zeroize`, `rusqlite` (SQLite empaquetado).

**Spec:** `docs/superpowers/specs/2026-11-08-desbloqueo-con-llave-design.md`

## Global Constraints

- Sin dependencias nuevas.
- Identificadores en inglés; comentarios, documentación y commits en español; Conventional Commits.
- Ningún secreto en errores, logs, panics ni `Debug`. Ningún tipo nuevo deriva `Debug`/`Display` con su valor.
- La llave de bóveda cruza la API como `SecretBox<[u8; 32]>`; búferes internos en `Zeroizing` o dentro del `SecretBox` (sin copias en la pila).
- Nonce aleatorio nuevo de 24 bytes en cada cifrado, incluido cada `key_check`.
- Dato autenticado de `key_check`: ASCII `boveda-key-check` (16 bytes) seguido de `format_version` `u16` little-endian (18 bytes).
- `key_check`: `nonce (24) | etiqueta (16)` = 40 bytes; columna `key_check BLOB NOT NULL CHECK (length(key_check) = 40)`.
- `Error::WrongVaultKey`, mensaje `"la llave no corresponde a esta bóveda"`.
- `FORMAT_VERSION = 2`. `open` y la apertura en solo lectura aceptan 1 y 2; otra versión da `UnsupportedFormatVersion`.
- `vault-v1.db` nunca se modifica; las pruebas trabajan sobre una copia. `vault-v2.db` se genera una sola vez con `KdfParams::MIN`, contraseña `prueba-bóveda-v2`.
- Pruebas unitarias en `modulo/tests.rs`; de la API pública en `core/tests/`.
- Antes de cada commit: `cargo test -p vault-core`. Al final: `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`.

## Review Focus

1. Contraseña incorrecta sobre una v1: no debe escribir nada (archivo idéntico byte por byte) → Task 4.
2. `unlock_with_key` fallido seguido de `unlock` con contraseña: el estado queda limpio y desbloquea normal → Task 5.
3. `export_vault_key` con el verificador válido no escribe en disco y dos llamadas entregan la misma llave → Task 5.
4. Bóveda migrada desde la v1 que después cambia de contraseña: la llave exportada sigue sirviendo → Task 7.
5. Importar un respaldo v1 con contraseña incorrecta: `dest` intacta y sin copia temporal sobrante → Task 6.

---

### Task 1: Especificación

**Files:**
- Modify: `docs/especificacion.md`

**Interfaces:** ninguna (documentación previa al código, como exige la issue).

- [ ] **Step 1: Actualizar "Formato de la bóveda"**
  - Nueva decisión fechada (2026-11-08, #34, detalle en la spec de diseño): formato v2 con `key_check`, su tabla de offsets (la de la Global Constraints, con el formato de la tabla de las entradas), qué lo crea (`rekey`, así que también `create` y `change_master_password`), quién lo comprueba (`unlock_with_key`; `unlock` con contraseña no lo lee) y la reparación en `export_vault_key` con su justificación.
  - La línea sobre XChaCha20-Poly1305: no compromete la llave, pero construir un texto que se autentique con dos llaves exige conocer ambas.
  - Agregar "Esquema de la versión de formato 2" debajo del de la 1 (que no cambia), con el DDL de la spec y el comentario sobre `"header"` entre comillas.
  - Migración: dentro de `unlock` y de `import`, una sola derivación de Argon2id, el procedimiento de 12 pasos en una transacción, error de almacenamiento si falla y la bóveda sigue bloqueada.
- [ ] **Step 2: Actualizar "API"**
  - `export_vault_key` y `unlock_with_key`, la llave por valor que se descarta si falla, `WrongVaultKey`.
  - Abrir sigue sin modificar el archivo, pero `unlock` con contraseña sobre una v1 escribe (la migra), como `reveal` escribe `last_used_at`; en un medio de solo lectura una v1 no se desbloquea.
- [ ] **Step 3: Viaje de la llave fuera del núcleo** en "Frontera entre Flutter y Rust": bytes (`Vec<u8>`/`Uint8List`/`ByteArray`), nunca `String` ni base64; la fachada pasa a `SecretBox` y aplica `zeroize` al `Vec`; Dart y Kotlin sobrescriben con ceros (mejor esfuerzo); por omisión pasa por Dart; JNI se evalúa en la #38.
- [ ] **Step 4: Bóvedas de ejemplo** en "Estrategia de pruebas": mencionar `vault-v2.db` (2026-11-08) junto a la regla de una por versión. Actualizar "Actualizado:" a 2026-11-08.
- [ ] **Step 5: Commit**

```bash
git add docs/especificacion.md
git commit -m "docs: especificación del formato v2 y el desbloqueo con llave"
```

---

### Task 2: Verificador de la llave y error nuevo

**Files:**
- Modify: `core/src/error.rs`, `core/src/crypto/keys.rs`
- Test: `core/src/error/tests.rs`, `core/src/crypto/keys/tests.rs`

**Interfaces:**
- Produces:
  - `Error::WrongVaultKey`
  - `pub(crate) const KEY_CHECK_LEN: usize = 40;` y `pub(crate) const KEY_CHECK_AAD_LEN: usize = 18;` en `keys.rs`
  - `pub(crate) fn key_check_associated_data(format_version: u16) -> [u8; KEY_CHECK_AAD_LEN]`
  - `pub(crate) fn seal_key_check(key: &VaultKey, format_version: u16) -> Result<[u8; KEY_CHECK_LEN]>` (un fallo del generador da `Error::Random`)
  - `pub(crate) fn verify_key_check(key: &VaultKey, format_version: u16, key_check: &[u8; KEY_CHECK_LEN]) -> Result<()>` (cualquier fallo da `Error::WrongVaultKey`)
  - `pub(crate) fn open_vault_key_keeping_derived(encrypted: &EncryptedKey, password: &SecretString, salt: &[u8; SALT_LEN], params: &KdfParams, associated_data: &[u8]) -> Result<(VaultKey, DerivedKey)>`: igual que `open_vault_key`, pero devuelve también la llave derivada para la migración. `open_vault_key` pasa a llamarla y soltar la derivada.

- [ ] **Step 1: Pruebas que fallan**

```rust
// error/tests.rs
#[test]
fn wrong_vault_key_message() {
    assert_eq!(Error::WrongVaultKey.to_string(), "la llave no corresponde a esta bóveda");
}

// crypto/keys/tests.rs
#[test]
fn key_check_associated_data_layout() {
    let ad = key_check_associated_data(0x0102);
    assert_eq!(ad.len(), 18);
    assert_eq!(&ad[..16], b"boveda-key-check");
    assert_eq!(&ad[16..], &[0x02, 0x01]);
}

#[test]
fn key_check_round_trip() {
    let key = generate_vault_key().unwrap();
    let check = seal_key_check(&key, 2).unwrap();
    assert_eq!(check.len(), KEY_CHECK_LEN);
    assert_eq!(verify_key_check(&key, 2, &check), Ok(()));
}

#[test]
fn key_check_rejects_other_key_version_or_tamper() {
    let key = generate_vault_key().unwrap();
    let other = generate_vault_key().unwrap();
    let check = seal_key_check(&key, 2).unwrap();
    assert_eq!(verify_key_check(&other, 2, &check), Err(Error::WrongVaultKey));
    assert_eq!(verify_key_check(&key, 3, &check), Err(Error::WrongVaultKey));
    for i in [0, 23, 24, 39] {
        let mut bad = check;
        bad[i] ^= 0x01;
        assert_eq!(verify_key_check(&key, 2, &bad), Err(Error::WrongVaultKey));
    }
}

#[test]
fn key_check_uses_fresh_nonce() {
    let key = generate_vault_key().unwrap();
    let a = seal_key_check(&key, 2).unwrap();
    let b = seal_key_check(&key, 2).unwrap();
    assert_ne!(a[..24], b[..24]);
}

#[test]
fn open_keeping_derived_matches_open() {
    // Con KdfParams::MIN: sellar con `seal_vault_key`, abrir con ambas
    // funciones; mismas llaves de bóveda, y la derivada coincide con
    // `kdf::derive_key(password, salt, params)`.
}
```

- [ ] **Step 2: Ejecutar** `cargo test -p vault-core --lib crypto::keys error` → FAIL (no compilan: faltan los símbolos).
- [ ] **Step 3: Implementar** en `error.rs` la variante (con doc: "la llave recibida no autentica el verificador de la cabecera: es de otra bóveda, está alterada, el verificador está dañado o la bóveda es v1") y en `keys.rs` las funciones. `seal_key_check` cifra un texto vacío con `encrypt_inout_detached` (cuerpo vacío) y un cifrador construido con `XChaCha20Poly1305::new_from_slice(key.expose_secret())`; el resultado es `nonce | tag`. `verify_key_check` separa nonce y etiqueta y usa `decrypt_inout_detached` sobre un cuerpo vacío.
- [ ] **Step 4: Ejecutar** `cargo test -p vault-core` → PASS.
- [ ] **Step 5: Commit**

```bash
git add core/src/error.rs core/src/error/tests.rs core/src/crypto/keys.rs core/src/crypto/keys/tests.rs
git commit -m "feat: verificador de la llave de bóveda en crypto"
```

---

### Task 3: Cabecera v2 en memoria y en disco

Juntas porque `from_parts` exige `key_check` en la v2: la cabecera sin el almacenamiento rompe todas las pruebas.

**Files:**
- Modify: `core/src/vault/header.rs`, `core/src/vault/storage.rs`
- Test: `core/src/vault/header/tests.rs`, `core/src/vault/storage/tests.rs`, `core/src/vault/backup/tests.rs` (versión futura)

**Interfaces:**
- Consumes: Task 2 (`seal_key_check`, `verify_key_check`, `KEY_CHECK_LEN`).
- Produces:
  - `pub(crate) const FORMAT_VERSION: u16 = 2;` y `pub(crate) const FORMAT_VERSION_V1: u16 = 1;`
  - `VaultHeader` con el campo `key_check: Option<[u8; KEY_CHECK_LEN]>` (`None` solo en una v1).
  - `VaultHeader::from_parts(format_version, algorithm, params, salt, encrypted_key, key_check: Option<[u8; KEY_CHECK_LEN]>) -> Result<Self>`: acepta 1 con `None` y 2 con `Some`; versión distinta → `UnsupportedFormatVersion`; versión conocida con la presencia equivocada → `Storage(Corrupt)`.
  - `VaultHeader::rekey` calcula `key_check` con `seal_key_check(vault_key, FORMAT_VERSION)`.
  - `VaultHeader::key_check(&self) -> Option<&[u8; KEY_CHECK_LEN]>`
  - `VaultHeader::verify_key(&self, key: &VaultKey) -> Result<()>`: v1 o verificador que no autentica → `WrongVaultKey`.
  - `Storage::read_header` por versión (la v1 sin la columna); `Storage::create`/`initialize` y `Storage::replace_header` escriben `key_check`.
  - Esquema v2 en `storage.rs`: el cuerpo de columnas de `header` escrito una sola vez (por ejemplo, un `macro_rules!` que devuelve el literal para `concat!`), porque la Task 4 lo reutiliza con el nombre `header_v2`. `SCHEMA` crea `CREATE TABLE "header" ...` con el comentario de la spec junto a él (por qué las comillas y que lo comprueba la prueba de igualdad de `sqlite_schema` de la Task 4). La tabla `entries` no cambia.

- [ ] **Step 1: Pruebas que fallan**

```rust
// header/tests.rs
#[test]
fn new_header_is_v2_with_valid_key_check() {
    let (header, key) = VaultHeader::create(&password("p"), KdfParams::MIN).unwrap();
    assert_eq!(header.format_version(), 2);
    assert_eq!(header.verify_key(&key), Ok(()));
}

#[test]
fn verify_key_rejects_other_key() {
    let (header, _) = VaultHeader::create(&password("p"), KdfParams::MIN).unwrap();
    let other = keys::generate_vault_key().unwrap();
    assert_eq!(header.verify_key(&other), Err(Error::WrongVaultKey));
}

#[test]
fn from_parts_versions() {
    // Con las partes de una cabecera creada: (1, None) → Ok y verify_key da
    // WrongVaultKey; (2, Some) → Ok; (1, Some) y (2, None) →
    // Storage(Corrupt); (3, Some) → UnsupportedFormatVersion(3).
}
```

En `header/tests.rs` (línea ~151), `storage/tests.rs` (líneas ~173–227 y ~790–817) y `backup/tests.rs` (~289–298), las pruebas de "versión futura" usan `format_version = 2`: pasan a `3` (y `UnsupportedFormatVersion(3)`), sin cambiar lo que prueban.

```rust
// storage/tests.rs
#[test]
fn created_vault_stores_key_check() {
    // create_with_params; `raw(&path)`: SELECT format_version,
    // length(key_check) FROM header → (2, 40); read_header().verify_key
    // con la llave de la Vault → Ok.
}

#[test]
fn replace_header_writes_key_check() {
    // rekey con la misma llave → replace_header → el key_check en disco
    // cambió de bytes y verify_key sigue Ok.
}
```

- [ ] **Step 2: Ejecutar** `cargo test -p vault-core` → FAIL.
- [ ] **Step 3: Implementar** las interfaces de arriba. `associated_data()` sigue usando la versión de la cabecera, así que una v1 en memoria se descifra con su propio dato autenticado.
- [ ] **Step 4: Ejecutar** `cargo test -p vault-core` → PASS, incluidas las pruebas de punta a punta de `vault-v1.db` (abren la v1 sin migrarla todavía: `unlock` usa `VaultHeader::unlock`, que funciona con la v1).
- [ ] **Step 5: Commit**

```bash
git add core/src/vault/header.rs core/src/vault/header/tests.rs core/src/vault/storage.rs core/src/vault/storage/tests.rs core/src/vault/backup/tests.rs
git commit -m "feat: formato v2 de la cabecera con key_check"
```

---

### Task 4: Migración de la v1 dentro de `unlock`

**Files:**
- Modify: `core/src/vault/header.rs`, `core/src/vault/storage.rs`, `core/src/vault/mod.rs`, `core/src/crypto/kdf.rs`, `core/src/vault/test_support.rs`
- Test: `core/src/vault/header/tests.rs`, `core/src/vault/storage/tests.rs`, `core/src/vault/tests.rs`

**Interfaces:**
- Consumes: Task 2 (`open_vault_key_keeping_derived`, `encrypt_vault_key`), Task 3.
- Produces:
  - `VaultHeader::unlock_upgrading(&self, password: &SecretString) -> Result<(VaultKey, Option<VaultHeader>)>`: una sola derivación. En una v2 devuelve `(llave, None)`. En una v1 devuelve la cabecera v2: misma sal y parámetros, `encrypt_vault_key` con la derivada conservada y el dato autenticado de la versión 2, `key_check` nuevo.
  - `Storage::migrate_header(&mut self, header: &VaultHeader) -> Result<()>`: una transacción `IMMEDIATE`: `CREATE TABLE header_v2` (mismo cuerpo de la Task 3), `INSERT` de la cabecera, `DROP TABLE header`, `ALTER TABLE header_v2 RENAME TO header`, y verificación `SELECT format_version, length(key_check) FROM header WHERE id = 1` = `(2, 40)` (si no, `Storage(Corrupt)`), y `commit`.
  - `Vault::unlock` usa `unlock_upgrading` y, si hay cabecera nueva, `migrate_header` antes de `load_entries`.
  - En `kdf.rs`, solo con `#[cfg(test)]`: `thread_local! { pub(crate) static DERIVE_CALLS: Cell<usize> }`, que `derive_key` incrementa.
  - En `test_support.rs`: `FIXTURE_V1_PASSWORD` (se mueve desde `vault/tests.rs`), `fixture_v1_copy() -> (tempfile::TempDir, PathBuf)` (copia `tests/fixtures/vault-v1.db` a un directorio temporal) y `schema_of(path) -> Vec<(String, String, String, String)>` (`SELECT type, name, tbl_name, sql FROM sqlite_schema ORDER BY name`).

- [ ] **Step 1: Pruebas que fallan**

```rust
// vault/tests.rs
#[test]
fn unlock_migrates_v1_with_one_derivation() {
    let (_dir, path) = fixture_v1_copy();
    let mut vault = Vault::open(&path).unwrap();
    DERIVE_CALLS.with(|c| c.set(0));
    let report = vault.unlock(&password(FIXTURE_V1_PASSWORD)).unwrap();
    assert_eq!(DERIVE_CALLS.with(|c| c.get()), 1);
    assert!(report.corrupt.is_empty());
    assert_eq!(vault.entries().unwrap().len(), 3);
    let header = vault.storage.read_header().unwrap();
    assert_eq!(header.format_version(), 2);
    assert_eq!(header.verify_key(&vault.unlocked().unwrap().key), Ok(()));
}

#[test]
fn migrated_schema_equals_created_schema() {
    // fixture_v1_copy + unlock; create_with_params en otra ruta;
    // assert_eq!(schema_of(&migrated), schema_of(&created)); y
    // `PRAGMA integrity_check` de la migrada = "ok".
}

#[test]
fn failed_migration_keeps_v1_byte_for_byte() {
    let (_dir, path) = fixture_v1_copy();
    let mut vault = Vault::open(&path).unwrap();
    let before = fs::read(&path).unwrap();
    vault.storage.deny_writes_for_test();
    let err = vault.unlock(&password(FIXTURE_V1_PASSWORD)).unwrap_err();
    assert!(matches!(err, Error::Storage(_)));
    assert!(vault.is_locked());
    drop(vault);
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn wrong_password_on_v1_writes_nothing() {
    // Igual que la anterior, sin deny_writes y con otra contraseña:
    // WrongPassword, bloqueada y el archivo idéntico.
}

#[test]
fn v2_unlock_writes_nothing() {
    // Bóveda creada, lock, leer bytes, unlock, soltar: bytes iguales.
}
```

En `header/tests.rs`: `unlock_upgrading` sobre una v2 devuelve `None`; sobre una v1 (armada con `from_parts(1, ..., None)` a partir de una cabecera sellada con el dato autenticado de la versión 1) devuelve la misma llave y una cabecera v2 con la misma sal y parámetros, cuyo `unlock` con la misma contraseña da la misma llave.

- [ ] **Step 2: Ejecutar** `cargo test -p vault-core --lib vault` → FAIL.
- [ ] **Step 3: Implementar** las interfaces de arriba.
- [ ] **Step 4: Ejecutar** `cargo test -p vault-core` → PASS. Si `query_only` no hace fallar la migración (y la prueba pasa sin el arreglo o falla por otra razón), detenerse y avisar antes de cambiar el método de la prueba.
- [ ] **Step 5: Commit**

```bash
git add core/src
git commit -m "feat: migrar la cabecera v1 a v2 al desbloquear"
```

---

### Task 5: `export_vault_key` y `unlock_with_key`

**Files:**
- Modify: `core/src/vault/mod.rs`, `core/src/vault/storage.rs`
- Test: `core/src/vault/tests.rs`, `core/src/vault/storage/tests.rs`

**Interfaces:**
- Consumes: Task 3 (`verify_key`, `key_check`), Task 2 (`seal_key_check`).
- Produces:
  - `pub fn export_vault_key(&mut self) -> Result<SecretBox<[u8; 32]>>`: `Locked` si está bloqueada; lee la cabecera y la comprueba con `verify_key`; si da `WrongVaultKey`, `seal_key_check` nuevo y `Storage::replace_key_check`; si la escritura falla, devuelve ese error sin la llave. La copia se escribe dentro de un `SecretBox` nuevo (`VaultKey::default()` y `copy_from_slice`, como `generate_vault_key`). Doc con la justificación de por qué regenerar es seguro (de la spec).
  - `pub fn unlock_with_key(&mut self, key: SecretBox<[u8; 32]>) -> Result<UnlockReport>`: `AlreadyUnlocked`; `read_header()?.verify_key(&key)?`; `load_entries(&self.storage, key)`. Doc: si falla, la llave se descarta y se borra al soltarse; para reintentar hay que pedirla de nuevo al Keystore.
  - `Storage::replace_key_check(&mut self, key_check: &[u8; KEY_CHECK_LEN]) -> Result<()>`: `UPDATE header SET key_check = ?1 WHERE id = 1` en una transacción; si no cambia exactamente 1 fila, `Storage(Corrupt)`.

- [ ] **Step 1: Pruebas que fallan** (en `vault/tests.rs`, con `new_vault()` y `add_sample()`):

```rust
#[test]
fn export_lock_unlock_with_key_round_trip() {
    // add_sample x2 → reveal de una (last_used) → export_vault_key → lock
    // → unlock_with_key: report vacío; snapshot(&vault) igual al de antes;
    // reveal da los mismos secretos.
}

#[test]
fn unlock_with_wrong_key_stays_locked() { /* llave al azar → WrongVaultKey, is_locked */ }

#[test]
fn empty_vault_rejects_wrong_key() { /* bóveda sin entradas, llave al azar → WrongVaultKey */ }

#[test]
fn unlock_with_key_of_other_vault() { /* export de otra bóveda → WrongVaultKey */ }

#[test]
fn export_vault_key_locked_is_locked() { /* lock → Locked */ }

#[test]
fn unlock_with_key_when_unlocked_is_already_unlocked() { /* → AlreadyUnlocked */ }

#[test]
fn old_key_survives_change_master_password() {
    // export; key_check en disco (raw) antes; change_master_password_with_params;
    // key_check cambió de bytes; lock; unlock_with_key(llave vieja) → Ok.
}

#[test]
fn damaged_key_check_is_repaired_by_export() {
    // export (llave A); lock; alterar un byte de key_check con raw();
    // unlock_with_key(A) → WrongVaultKey; unlock(contraseña) → Ok;
    // export → Ok (misma llave); lock; unlock_with_key → Ok.
}

#[test]
fn export_fails_without_key_when_repair_cannot_write() {
    // key_check alterado; unlock(contraseña); deny_writes_for_test;
    // export_vault_key → Err(Storage(_)).
}

#[test]
fn export_with_valid_check_writes_nothing() {
    // bytes del archivo antes; dos export_vault_key (mismos bytes de llave);
    // soltar la Vault; bytes iguales.
}

#[test]
fn failed_unlock_with_key_then_password_works() {
    // llave al azar → WrongVaultKey; unlock(contraseña) → Ok con las entradas.
}
```

- [ ] **Step 2: Ejecutar** `cargo test -p vault-core --lib vault` → FAIL.
- [ ] **Step 3: Implementar** las interfaces de arriba.
- [ ] **Step 4: Ejecutar** `cargo test -p vault-core` → PASS.
- [ ] **Step 5: Commit**

```bash
git add core/src
git commit -m "feat: export_vault_key y unlock_with_key"
```

---

### Task 6: Migrar los respaldos v1 al importar

**Files:**
- Modify: `core/src/vault/backup.rs`
- Test: `core/src/vault/backup/tests.rs`

**Interfaces:**
- Consumes: Task 4 (`unlock_upgrading`, `migrate_header`, `DERIVE_CALLS`, `fixture_v1_copy`, `FIXTURE_V1_PASSWORD`).
- Produces: `validate(path, password) -> Result<(Unlocked, UnlockReport, Option<VaultHeader>)>` usa `unlock_upgrading`. En `import`, después de `validate` y antes de `settle_dest`: si hay cabecera v2, `Storage::open(&temp.0)?`, `migrate_header`, `close()`. El resto de `import` no cambia.

- [ ] **Step 1: Pruebas que fallan**

```rust
#[test]
fn import_v1_backup_installs_v2_with_one_derivation() {
    let (_dir, backup) = fixture_v1_copy();
    let (_d2, dest) = temp_path();
    DERIVE_CALLS.with(|c| c.set(0));
    let (vault, report) = Vault::import(&backup, &dest, &password(FIXTURE_V1_PASSWORD)).unwrap();
    assert_eq!(DERIVE_CALLS.with(|c| c.get()), 1);
    assert!(report.corrupt.is_empty());
    assert_eq!(vault.storage.read_header().unwrap().format_version(), 2);
    drop(vault);
    // El respaldo original no cambió: sigue siendo v1.
    let conn = raw(&backup);
    let v: i64 = conn.query_row("SELECT format_version FROM header", [], |r| r.get(0)).unwrap();
    assert_eq!(v, 1);
}

#[test]
fn import_v1_with_wrong_password_keeps_dest() {
    // dest = bóveda existente; bytes antes; import con contraseña mala →
    // WrongPassword; bytes de dest iguales; no existe la copia temporal
    // (with_suffix(&dest, IMPORT_SUFFIX)).
}
```

- [ ] **Step 2: Ejecutar** `cargo test -p vault-core --lib vault::backup` → FAIL.
- [ ] **Step 3: Implementar.**
- [ ] **Step 4: Ejecutar** `cargo test -p vault-core` → PASS.
- [ ] **Step 5: Commit**

```bash
git add core/src/vault/backup.rs core/src/vault/backup/tests.rs
git commit -m "feat: migrar los respaldos v1 al importar"
```

---

### Task 7: Bóveda de ejemplo `vault-v2.db` y pruebas de punta a punta

**Files:**
- Create: `core/tests/fixtures/vault-v2.db` (generado)
- Modify: `core/src/vault/tests.rs`, `core/tests/vault.rs`, `core/tests/fixtures/README.md`

**Interfaces:**
- Consumes: todo lo anterior, solo por la API pública en `core/tests/`.
- Produces: `generate_fixture_v2` (`#[ignore]`) y `fill_fixture(vault: &mut Vault)` en `vault/tests.rs`, con el contenido que hoy arma `generate_fixture_v1`; ambos generadores la usan. En la doc de `generate_fixture_v1`: desde la v2, `create_with_params` crea v2, así que ya no reproduce `vault-v1.db`; se conserva como registro.

- [ ] **Step 1: Generar la bóveda una sola vez**

```bash
cargo test -p vault-core --lib -- --ignored --exact vault::tests::generate_fixture_v2
```

Esperado: 1 passed y existe `core/tests/fixtures/vault-v2.db`. `git status` no muestra cambios en `vault-v1.db`.

- [ ] **Step 2: Pruebas de punta a punta** en `core/tests/vault.rs`. `fixture_copy` recibe el nombre del archivo; las comprobaciones de contenido de `fixture_v1_opens_with_known_contents` pasan a un ayudante compartido.

```rust
const FIXTURE_V2_PASSWORD: &str = "prueba-bóveda-v2";

#[test]
fn fixture_v2_opens_with_known_contents() { /* mismas comprobaciones que la v1 */ }

#[test]
fn fixture_v1_migrates_and_keeps_working() {
    // copia de la v1; unlock (migra); servicios y secretos esperados;
    // export_vault_key; drop; Vault::open; unlock_with_key → Ok (prueba
    // que quedó en v2); mismos servicios y secretos; lock; unlock con
    // FIXTURE_V1_PASSWORD → Ok.
}

#[test]
fn unlock_with_key_on_unmigrated_v1_is_wrong_vault_key() {
    // copia de la v1 → open → unlock_with_key(llave al azar) →
    // WrongVaultKey; y con la llave real (exportada de otra copia ya
    // migrada) también → WrongVaultKey.
}

#[test]
fn migrated_vault_key_survives_password_change() {
    // copia de la v1; unlock; export; change_master_password (calibra: va
    // con el sufijo `_with_calibration` si la prueba tarda, como las
    // existentes); lock; unlock_with_key(llave) → Ok.
}
```

- [ ] **Step 3: Ejecutar** `cargo test -p vault-core` → PASS (sin `--ignored`).
- [ ] **Step 4: README de las bóvedas de ejemplo**: sección `vault-v2.db` con la contraseña `prueba-bóveda-v2`, `KdfParams::MIN`, el comando del generador con `--exact`, "mismo contenido que `vault-v1.db`" y la nota de que abrir la v1 con contraseña la migra (por eso las pruebas usan una copia).
- [ ] **Step 5: Commit**

```bash
git add core/tests/fixtures/vault-v2.db core/tests/fixtures/README.md core/tests/vault.rs core/src/vault/tests.rs
git commit -m "test: bóveda de ejemplo vault-v2.db y pruebas de migración"
```

---

### Task 8: ARCHITECTURE.md y verificación final

**Files:**
- Modify: `ARCHITECTURE.md`

- [ ] **Step 1: Actualizar la tabla de componentes**: en `crypto/`, `keys.rs` también crea y comprueba el verificador de la llave de bóveda; en `vault/`, la cabecera v2 con `key_check` y la migración desde la v1 (`header.rs`, `storage.rs`).
- [ ] **Step 2: Verificación completa**

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check
```

Esperado: sin diferencias de formato, sin advertencias y todas las pruebas en verde (las `#[ignore]` no corren). `git diff dev --stat -- core/tests/fixtures/vault-v1.db` vacío.

- [ ] **Step 3: Commit**

```bash
git add ARCHITECTURE.md
git commit -m "docs: verificador de la llave y formato v2 en ARCHITECTURE.md"
```

- [ ] **Step 4: Al cerrar (con el visto bueno del usuario):** comentario en la #38 sobre JNI: el crate `jni` sería una dependencia nueva que se decide allí con aprobación; por omisión, la llave pasa por Dart como mejor esfuerzo.
