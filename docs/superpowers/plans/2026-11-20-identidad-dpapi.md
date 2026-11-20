# Identidad con DPAPI y lista de emparejados (#99) — plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** La PC genera su identidad X25519 en el primer arranque y guarda, protegidas con DPAPI de usuario, la llave privada y la lista de celulares emparejados, con escrituras atómicas y sin regenerar ni vaciar nunca un archivo dañado.

**Architecture:** Cuatro módulos pequeños en `desktop/src-tauri/src/`: `dpapi.rs` (único `unsafe` de DPAPI), `atomic_file.rs` (escritura atómica), `protected_file.rs` (sobre `magic | versión | cuerpo` cifrado con DPAPI, compartido) y los dos almacenes (`identity.rs`, `paired_devices.rs`). `lib.rs` solo llama a `load_or_create` al arrancar.

**Tech Stack:** Rust 2024 (1.100), `windows-sys` 0.61 (`CryptProtectData`, `CryptUnprotectData`, `LocalFree`, `MoveFileExW`), `vault-core` (`KeyPair`), `serde` + `postcard`, `zeroize`, `tempfile` (solo pruebas).

**Spec:** `docs/superpowers/specs/2026-11-20-identidad-dpapi-design.md`

## Global Constraints

- Alcance DPAPI de **usuario** (sin `CRYPTPROTECT_LOCAL_MACHINE`) y con `CRYPTPROTECT_UI_FORBIDDEN`. Entropías fijas: `boveda-desktop identity v1` y `boveda-desktop paired-devices v1`.
- Contenido en claro: identidad = `boveda-pc-identity` (19 bytes) | versión `u16` LE (=1) | 32 bytes; lista = `boveda-pc-devices` (17 bytes) | versión `u16` LE (=1) | cuerpo `postcard`.
- Solo se genera la identidad si el archivo **no existe** (`ErrorKind::NotFound`); cualquier otro fallo se devuelve y no se toca nada. La lista nunca se reemplaza por una vacía.
- Nombre del celular: recortado, de 1 a 128 bytes UTF-8; máximo 64 celulares; llave repetida rechazada.
- Sin secretos ni nombres en errores, `Debug`, `Display`, logs o panics. Sin `#[allow(dead_code)]` (módulos `pub`).
- Escritura atómica: temporal en el mismo directorio, `sync_all`, `MoveFileExW(REPLACE_EXISTING | WRITE_THROUGH)`, sin borrar antes el original; no se promete durabilidad absoluta.
- Sin crates nuevos en `Cargo.lock` (comprobar con `grep '^name = ' Cargo.lock` antes y después). No cambiar `core/`.
- Comentarios y commits en español (Conventional Commits); identificadores en inglés. Sin Python.
- Ni IPC, ni interfaz, ni emparejamiento, ni revocación.

## Review Focus

- Archivo vacío (0 bytes) en lugar de ausente: debe dar error y no regenerar (Tasks 3 y 4).
- La ruta del archivo es una carpeta o está bloqueada por otro proceso: error de E/S, no regeneración (Task 4).
- Archivo de la lista con muchísimos bytes o con una cuenta enorme dentro: rechazado sin reservar memoria sin tope (Task 5).
- Nombre con espacios alrededor, solo espacios o multibyte de más de 128 bytes (Task 5).
- Quitar el único celular y reabrir: lista vacía válida, no «archivo dañado» (Task 5).

---

### Task 1: Dependencias y envoltura de DPAPI

**Files:**
- Modify: `desktop/src-tauri/Cargo.toml`
- Create: `desktop/src-tauri/src/dpapi.rs`
- Modify: `desktop/src-tauri/src/lib.rs` (agregar `#[cfg(windows)] pub mod dpapi;`)
- Test: `desktop/src-tauri/src/dpapi.rs` (módulo `tests` al final)

**Interfaces:**
- Produces: `pub fn protect(plain: &[u8], entropy: &[u8]) -> Result<Vec<u8>, DpapiError>`; `pub fn unprotect(blob: &[u8], entropy: &[u8]) -> Result<Zeroizing<Vec<u8>>, DpapiError>`; `pub struct DpapiError` (sin datos, con `code()` del error de Windows, `Debug`/`Display` manuales).

- [ ] **Step 1: Dependencias.** En `Cargo.toml` agregar `vault-core = { path = "../../core" }`, `serde = { version = "1", default-features = false, features = ["derive"] }`, `postcard = { version = "1", default-features = false, features = ["alloc"] }` (mismas features que `core/Cargo.toml`), `[dev-dependencies] tempfile = "3"`, y a `windows-sys` agregar las features `Win32_Security_Cryptography` y `Win32_Storage_FileSystem`. Comprobar el nombre real del paquete de `core/` en su `Cargo.toml`.
- [ ] **Step 2: Pruebas que fallan** (`dpapi::tests`): `roundtrip_returns_original`; `blob_does_not_contain_plaintext` (texto de 32 bytes reconocibles; `blob.windows(32).all(|w| w != plain)`); `wrong_entropy_fails`; `flipped_byte_fails` (cambiar un byte en el medio); `empty_and_truncated_blob_fail` (`b""` y el blob sin su último byte).
- [ ] **Step 3: Ver fallar.** Run: `cargo test -p boveda-desktop dpapi` → Expected: no compila (funciones sin definir).
- [ ] **Step 4: Implementar** `protect`/`unprotect` con `CRYPTPROTECT_UI_FORBIDDEN` (`1`), descripción nula, `CRYPT_INTEGER_BLOB { cbData, pbData }`. `unprotect` copia la salida a `Zeroizing<Vec<u8>>`, sobrescribe con ceros el búfer de Windows y llama a `LocalFree`; `protect` libera con `LocalFree` el suyo. Rechazar entradas de más de `u32::MAX` bytes con error. Todo `unsafe` con un comentario `// SAFETY:` que diga por qué los punteros son válidos.
- [ ] **Step 5: Ver pasar.** Run: `cargo test -p boveda-desktop dpapi` → Expected: 5 pruebas OK.
- [ ] **Step 6: Commit** (`cargo clippy -p boveda-desktop --all-targets -- -D warnings` aparte, antes).

```bash
git add desktop/src-tauri docs Cargo.lock
git commit -m "feat: envoltura de DPAPI de usuario para la PC (#99)"
```

---

### Task 2: Escritura atómica

**Files:**
- Create: `desktop/src-tauri/src/atomic_file.rs`
- Modify: `desktop/src-tauri/src/lib.rs` (`#[cfg(windows)] pub mod atomic_file;`)

**Interfaces:**
- Produces: `pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()>`.

- [ ] **Step 1: Pruebas que fallan** (`tempfile::tempdir`): `creates_new_file`; `replaces_existing_content`; `leaves_no_temp_file_on_success` (el directorio solo contiene el destino); `failed_replace_keeps_original_and_cleans_temp` (abrir el destino con `OpenOptionsExt::share_mode(FILE_SHARE_READ)` y mantenerlo abierto: el reemplazo falla; el contenido original sigue igual y no queda `.tmp`); `stale_temp_does_not_block` (existe `<nombre>.tmp` con basura: la escritura funciona); `missing_parent_dir_errors` (el padre no existe: error y sin crear nada).
- [ ] **Step 2: Ver fallar.** Run: `cargo test -p boveda-desktop atomic_file` → Expected: no compila.
- [ ] **Step 3: Implementar `write_atomic`** como en la spec: temporal `<nombre>.tmp` en el mismo directorio (si ya existe, borrarlo solo a él y recrearlo con `create_new`), `write_all`, `sync_all`, `MoveFileExW(tmp, path, MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH)` con rutas UTF-16 terminadas en NUL (`OsStrExt::encode_wide`). Ante cualquier error, borrar el temporal (ignorando su fallo) y devolver el error original. Sin borrar nunca el destino.
- [ ] **Step 4: Ver pasar.** Run: `cargo test -p boveda-desktop atomic_file` → Expected: 6 OK.
- [ ] **Step 5: Commit.**

```bash
git add desktop/src-tauri/src
git commit -m "feat: escritura atómica de archivos de la app de Windows (#99)"
```

---

### Task 3: Archivo protegido con DPAPI

**Files:**
- Create: `desktop/src-tauri/src/protected_file.rs`
- Modify: `desktop/src-tauri/src/lib.rs` (`#[cfg(windows)] pub mod protected_file;`)

**Interfaces:**
- Consumes: `dpapi::{protect, unprotect}`, `atomic_file::write_atomic`.
- Produces: `pub enum ProtectedFileError { Io, Protection, Corrupt, UnknownVersion }` (`Copy`, `Debug`/`Display` sin datos); `pub fn write_protected(path: &Path, magic: &[u8], version: u16, body: &[u8], entropy: &[u8]) -> Result<(), ProtectedFileError>`; `pub fn read_protected(path: &Path, magic: &[u8], version: u16, entropy: &[u8]) -> Result<Option<Zeroizing<Vec<u8>>>, ProtectedFileError>` (`Ok(None)` solo si el archivo no existe; `Ok(Some(cuerpo))` sin la cabecera).

- [ ] **Step 1: Pruebas que fallan:** `roundtrip_returns_body`; `missing_file_is_none`; `file_does_not_contain_body_or_magic`; `empty_file_is_protection_error` (0 bytes → `Protection`, no `None`); `flipped_byte_is_protection_error`; `truncated_is_protection_error`; `wrong_magic_is_corrupt` (escrito con otro `magic`, leído con el esperado: descifra pero `Corrupt`); `unknown_version_is_unknown_version` (se escribe con versión 2 y se lee esperando 1); `directory_path_is_io_error` (la ruta es una carpeta); `other_entropy_is_protection_error`; `failed_read_never_modifies_file` (bytes iguales tras cada error).
- [ ] **Step 2: Ver fallar.** Run: `cargo test -p boveda-desktop protected_file` → Expected: no compila.
- [ ] **Step 3: Implementar.** Claro = `magic | version.to_le_bytes() | body`; `write_protected` lo protege y llama a `write_atomic`; `read_protected` mapea `NotFound` → `Ok(None)`, otro error de E/S → `Io`, fallo de DPAPI → `Protection`, claro más corto que `magic + 2` o con magic distinto → `Corrupt`, versión ≠ esperada → `UnknownVersion`. El claro va en `Zeroizing`.
- [ ] **Step 4: Ver pasar.** Run: `cargo test -p boveda-desktop protected_file` → Expected: 11 OK.
- [ ] **Step 5: Commit.**

```bash
git add desktop/src-tauri/src
git commit -m "feat: archivo protegido con DPAPI y versión (#99)"
```

---

### Task 4: Identidad de la PC

**Files:**
- Create: `desktop/src-tauri/src/identity.rs`
- Modify: `desktop/src-tauri/src/lib.rs` (`#[cfg(windows)] pub mod identity;`)

**Interfaces:**
- Consumes: `protected_file::{read_protected, write_protected, ProtectedFileError}`; `vault_core::protocol::KeyPair` (`generate() -> Result<KeyPair>`, `from_private(SecretBox<[u8;32]>)`, `private()`, `public()`).
- Produces: `pub struct IdentityStore` con `pub fn new(path: PathBuf) -> Self` y `pub fn load_or_create(&self) -> Result<KeyPair, IdentityError>`; `pub enum IdentityError { Io, Protection, Corrupt, UnknownVersion, Generation }` (`Copy`, sin datos). Constantes privadas `MAGIC = b"boveda-pc-identity"`, `VERSION = 1`, `ENTROPY = b"boveda-desktop identity v1"`. Confirmar en `core/src/protocol/identity.rs` el tipo de error de `generate` y la ruta pública exacta del módulo.

- [ ] **Step 1: Pruebas que fallan** (carpeta temporal): `first_run_creates_file`; `file_does_not_contain_private_key` (los 32 bytes de `private()` no aparecen en el archivo ni en ningún alineamiento: `windows(32)`); `second_open_returns_same_public_key` (un `IdentityStore` nuevo con la misma ruta); `altered_file_errors_without_regenerating`, `truncated_file_...`, `empty_file_...`, `directory_path_...`, `unknown_version_...` (armada con `write_protected` y versión 2), `wrong_length_body_...` (cuerpo de 31 y de 33 bytes): cada una comprueba el error esperado, que el archivo queda igual byte por byte y que una **segunda** llamada vuelve a fallar; `devices_file_is_not_a_valid_identity` (un archivo escrito con la entropía y el magic de la lista da error); `errors_do_not_show_key` (el `Debug` y el `Display` de cada error no contienen los bytes de la llave en hex ni en decimal).
- [ ] **Step 2: Ver fallar.** Run: `cargo test -p boveda-desktop identity::` → Expected: no compila.
- [ ] **Step 3: Implementar `load_or_create`.** `read_protected` → `Some(body)`: debe medir exactamente 32 y se arma `KeyPair::from_private` sin transformar; `None`: `KeyPair::generate`, guardar los 32 bytes de `private()` con `write_protected` (el copiado intermedio va en `Zeroizing`) y devolver el par; errores mapeados 1 a 1. Crear la carpeta padre con `create_dir_all` solo en la rama de generación.
- [ ] **Step 4: Ver pasar.** Run: `cargo test -p boveda-desktop identity::` → Expected: todas OK.
- [ ] **Step 5: Mutación.** Cambiar temporalmente la rama `Err(Protection)` para que genere una identidad nueva y comprobar que `altered_file_errors_without_regenerating` falla; restaurar con el árbol limpio de otros cambios (confirmar antes).
- [ ] **Step 6: Commit.**

```bash
git add desktop/src-tauri/src
git commit -m "feat: identidad X25519 de la PC protegida con DPAPI (#99)"
```

---

### Task 5: Lista de celulares emparejados

**Files:**
- Create: `desktop/src-tauri/src/paired_devices.rs`
- Modify: `desktop/src-tauri/src/lib.rs` (`#[cfg(windows)] pub mod paired_devices;`)

**Interfaces:**
- Consumes: `protected_file::{read_protected, write_protected, ProtectedFileError}`.
- Produces: `pub struct PairedDevice { pub name: String, pub public_key: [u8; 32], pub paired_at: i64 }` (`Clone`, `PartialEq`, `Serialize`/`Deserialize`; `Debug` manual que oculta nombre y llave); `pub struct PairedDeviceStore` con `pub fn open(path: PathBuf) -> Result<Self, PairedDevicesError>`, `pub fn list(&self) -> &[PairedDevice]`, `pub fn add(&mut self, device: PairedDevice) -> Result<(), PairedDevicesError>`, `pub fn remove(&mut self, public_key: &[u8; 32]) -> Result<(), PairedDevicesError>`; `pub enum PairedDevicesError { Io, Protection, Corrupt, UnknownVersion, AlreadyPaired, NotFound, InvalidName, TooMany }` (`Copy`, sin datos). Constantes: `MAGIC = b"boveda-pc-devices"`, `VERSION = 1`, `ENTROPY = b"boveda-desktop paired-devices v1"`, `MAX_DEVICES = 64`, `MAX_NAME_BYTES = 128`.

- [ ] **Step 1: Pruebas que fallan:** `open_missing_file_is_empty_and_writes_nothing`; `add_list_remove`; `survives_reopen`; `remove_last_then_reopen_is_empty_valid_list`; `rejects_duplicate_key`; `name_is_trimmed`; `rejects_empty_whitespace_only_and_over_128_bytes_name` (incluye 43 caracteres de 3 bytes); `rejects_65th_device`; `remove_unknown_is_not_found`; `file_does_not_contain_name_or_public_key`; `altered_truncated_empty_wrong_magic_unknown_version_errors_keep_file_identical` (cada una con su error y el archivo igual); `trailing_bytes_after_body_is_corrupt`; `body_claiming_huge_count_is_rejected_without_allocating` (cuerpo `postcard` con un varint de longitud enorme: `Corrupt`, sin pánico ni reserva gigante); `oversized_file_is_rejected` (un cuerpo protegido de 1 MiB → `Corrupt`; el tope: `64 * (128 + 32 + 10) + 16` bytes de cuerpo); `failed_add_keeps_memory_and_disk` y `failed_remove_keeps_memory_and_disk` (bloquear el destino con `share_mode(FILE_SHARE_READ)`, como en Task 2: tras el error, `list()` igual y archivo igual); `identity_file_is_not_a_valid_list` (entropía y magic de la identidad); `debug_hides_name_and_key`.
- [ ] **Step 2: Ver fallar.** Run: `cargo test -p boveda-desktop paired_devices` → Expected: no compila.
- [ ] **Step 3: Implementar.** `open`: `read_protected` (`None` → vacía); comprobar el tope del cuerpo antes de deserializar; `postcard::take_from_bytes::<Vec<PairedDevice>>` y exigir que no sobren bytes; revalidar cada nombre (1–128 bytes, ya recortado), unicidad de llaves y `≤ 64`, o `Corrupt`. `add`/`remove` validan, arman la lista nueva, `write_protected` y **solo si termina** reemplazan `self.devices`.
- [ ] **Step 4: Ver pasar.** Run: `cargo test -p boveda-desktop paired_devices` → Expected: todas OK.
- [ ] **Step 5: Mutación.** Hacer que `open` devuelva lista vacía ante `Protection` y comprobar que la prueba de alteración falla; restaurar.
- [ ] **Step 6: Commit.**

```bash
git add desktop/src-tauri/src
git commit -m "feat: lista de celulares emparejados protegida con DPAPI (#99)"
```

---

### Task 6: Arranque, documentación y verificación

**Files:**
- Modify: `desktop/src-tauri/src/lib.rs` (`setup`)
- Modify: `ARCHITECTURE.md` (sección de `desktop/`, línea 130 y el mapa), `docs/especificacion.md` (párrafo «Decidido el 2026-11-20 (#99)» tras el de «Identidades»), `docs/roadmap.md:75`
- Create: `desktop/src-tauri/examples/identity_probe.rs` (solo para la validación manual con otra cuenta; imprime la huella corta o el tipo de error, nunca la llave)

**Interfaces:**
- Consumes: `IdentityStore::new(PathBuf)`, `load_or_create`, `tauri::Manager::path().app_data_dir()`.

- [ ] **Step 1: `setup`.** Tras `tray::setup`, resolver `app.path().app_data_dir()`, `create_dir_all`, y `IdentityStore::new(dir.join("identity.bin")).load_or_create()`; en error, `eprintln!` con solo el tipo de error (`{err}` sin datos) y seguir; descartar el `KeyPair`. Comentario que explique por qué se descarta y que #100/#101 abrirán los almacenes con la misma carpeta.
- [ ] **Step 2: Ejemplo `identity_probe`.** Recibe la carpeta como argumento, abre `identity.bin` y `paired-devices.bin` sin crear nada si faltan (usar `read_protected` para no generar identidad), e imprime `ok` o el tipo de error por archivo. Sirve para que el usuario copie los archivos a otra cuenta y compruebe `Protection`.
- [ ] **Step 3: Documentación.** `ARCHITECTURE.md`: módulos nuevos, `%APPDATA%\<identificador>\identity.bin` y `paired-devices.bin`, la política de archivos dañados y que la identidad ya no figura como faltante. `docs/especificacion.md`: política, limitaciones aceptadas (mismo usuario, copia antigua válida), entropía no secreta, validación manual con otra cuenta pendiente. `docs/roadmap.md:75`: marcar hecha la llave con DPAPI y el guardado de la lista; el flujo sigue en #100, #101 y #107.
- [ ] **Step 4: Verificación completa**, cada comando por separado y mirando su código de salida:
  - `cargo fmt --all -- --check` → sin diferencias
  - `cargo clippy --workspace --all-targets -- -D warnings` → 0 advertencias
  - `cargo test --workspace` → todas OK
  - `cargo audit` y `cargo deny check` → sin errores nuevos
  - `grep '^name = ' Cargo.lock` comparado con el de antes de la Task 1 → sin crates nuevos
  - `git status` → sin archivos generados por las pruebas en el repositorio
- [ ] **Step 5: Revisión final** de toda la rama con un revisor independiente y corrección de hallazgos baratos antes del PR (PR contra `dev` con el formato del #11; la validación con otra cuenta de Windows se declara como pendiente del usuario).
- [ ] **Step 6: Commit.**

```bash
git add desktop ARCHITECTURE.md docs
git commit -m "feat: generar la identidad de la PC al primer arranque y documentar (#99)"
```
