# Fachada del puente: plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reemplazar el ejemplo `greet` de `mobile/rust` por la fachada `VaultSession`, que expone a Dart las operaciones de la bóveda de la etapa 2, con la ruta en `noBackupFilesDir`.

**Architecture:** `VaultSession` es un tipo opaco de flutter_rust_bridge con `path: PathBuf` y `vault: Mutex<Option<Vault>>`. Sus métodos toman `&self`, son asíncronos en Dart (salvo `new`) y solo convierten tipos y delegan en `vault_core::vault::Vault`. Los errores cruzan como un enum plano `VaultError`. Un `MethodChannel` de Kotlin entrega `noBackupFilesDir` y Dart arma la ruta con `package:path`.

**Tech Stack:** Rust (toolchain de `rust-toolchain.toml`), flutter_rust_bridge 2.13.0 (codegen 2.13.0, cargo-expand 1.0.127), `secrecy` 0.10, `zeroize` 1.9, `tempfile` 3 (desarrollo); Flutter 3.47.9, `path` ^1.9.1; Kotlin (`MethodChannel`).

**Spec:** `docs/superpowers/specs/2026-11-08-fachada-puente-design.md`

## Global Constraints

- La fachada no tiene lógica propia: convierte tipos y delega. Toda regla de la bóveda la aplica `vault-core`.
- Sin dependencias nuevas salvo las de la spec: `secrecy = "0.10"`, `zeroize = "1.9"` y `tempfile = "3"` (desarrollo) en `mobile/rust`; `path: ^1.9.1` en `pubspec.yaml`. Ninguna entra nueva a `Cargo.lock` ni a `pubspec.lock`.
- Ninguna función que toque la bóveda lleva `#[frb(sync)]`. Solo `VaultSession::new` es `#[frb(sync)]`.
- Todos los métodos, salvo `new`, devuelven `Result<_, VaultError>`. Ningún `unwrap`/`expect` fuera de las pruebas; un `PoisonError` da `VaultError::SessionPoisoned`.
- `VaultError` es un enum sin campos; ningún tipo de cruce deriva `Debug` con secretos; `NewSecret` no deriva `Debug`.
- Contraseñas y notas que entran: `SecretString::from(s.as_str())` y después `s.zeroize()`; nunca `SecretString::from(String)`.
- Id de entrada: 32 caracteres hexadecimales en minúsculas; cualquier otra entrada da `InvalidEntryId`.
- Ruta de la bóveda: `p.join(noBackupFilesDir, 'boveda.db')`; canal `boveda/storage`, método `noBackupFilesDir`.
- Pruebas de Rust: unitarias en `area/tests.rs` (el crate es `cdylib`/`staticlib`). Usan una copia de `core/tests/fixtures/vault-v2.db` (contraseña `prueba-bóveda-v2`); nunca abren el original. Solo la prueba de `create` calibra.
- No se edita a mano `mobile/lib/src/rust/` ni `mobile/rust/src/frb_generated.rs`: se regeneran con `flutter_rust_bridge_codegen generate` (desde `mobile/`) cada vez que cambia `mobile/rust/src/api/`.
- Identificadores en inglés; comentarios, documentación y commits en español; Conventional Commits.
- Antes de cada commit: `cargo test -p rust_lib_boveda` y, si cambió Dart, `flutter analyze` y `flutter test` (desde `mobile/`). Al final: `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`.

## Review Focus

1. Importar un archivo que no es una bóveda con la bóveda actual desbloqueada: da `NotAVault`, la anterior queda abierta y bloqueada y se desbloquea con su contraseña → Task 4.
2. Un id de 32 bytes con caracteres no ASCII (16 `é`): da `InvalidEntryId` sin panic al cortar el texto → Task 3.
3. Datos con espacios y etiquetas duplicadas al agregar: lo que devuelve `search` es lo normalizado por el núcleo (`"  Gmail  "` → `"Gmail"`; `["a", "A"]` → `["a"]`) → Task 3.
4. Operaciones después de `lock()` con bóveda cargada: dan `Locked`, no `NoVault` → Task 2.
5. Exportar a un destino que ya existe da `AlreadyExists`, y a una ruta relativa da `StorageIo`, sin tocar la bóveda → Task 4.

---

### Task 1: `VaultError` y su conversión

**Files:**
- Create: `mobile/rust/src/api/error.rs`, `mobile/rust/src/api/error/tests.rs`
- Modify: `mobile/rust/src/api/mod.rs`, `mobile/rust/Cargo.toml`

**Interfaces:**
- Produces: `pub enum VaultError` con exactamente las variantes de la tabla "Errores" de la spec (en el mismo orden), `impl From<vault_core::vault::Error> for VaultError`, y `pub type FacadeResult<T> = Result<T, VaultError>` (alias interno; las firmas públicas lo usan).

- [ ] **Step 1: Dependencias.** En `mobile/rust/Cargo.toml` agregar `secrecy = "0.10"`, `zeroize = "1.9"` y, en `[dev-dependencies]`, `tempfile = "3"`. Comprobar que `Cargo.lock` no suma paquetes: `git diff --stat Cargo.lock` solo muestra las dependencias nuevas de `rust_lib_boveda`.
- [ ] **Step 2: Prueba que falla** en `error/tests.rs`:

```rust
#[test]
fn maps_every_core_error() {
    use vault_core::vault::{Error as E, InvalidEntryField as F, StorageErrorKind as S};
    let cases = [
        (E::WrongPassword, VaultError::WrongPassword),
        (E::UnsupportedFormatVersion(9), VaultError::UnsupportedFormatVersion),
        (E::UnsupportedAlgorithm(7), VaultError::UnsupportedAlgorithm),
        (E::Storage(S::Io), VaultError::StorageIo),
        (E::Storage(S::HotJournal), VaultError::StorageHotJournal),
        (E::InvalidEntry(F::ServiceEmpty), VaultError::InvalidEntryServiceEmpty),
        (E::InvalidEntry(F::TagTooLong), VaultError::InvalidEntryTagTooLong),
        (E::WrongVaultKey, VaultError::WrongVaultKey),
        (E::AlreadyExists, VaultError::AlreadyExists),
        // ... una fila por cada variante restante de Error, StorageErrorKind e InvalidEntryField
    ];
    for (core, expected) in cases {
        assert_eq!(VaultError::from(core), expected);
    }
}
```

  La lista final cubre todas las variantes de las tres enumeraciones del núcleo (ver `core/src/error.rs`).
- [ ] **Step 3: Ejecutar** `cargo test -p rust_lib_boveda error` → falla al compilar (`VaultError` no existe).
- [ ] **Step 4: Implementar** `VaultError` (`#[derive(Debug, Clone, Copy, PartialEq, Eq)]`, sin campos) y el `From`. Brazo `_` hacia `StorageOther` e `InvalidEntryOther` para los dos enums `#[non_exhaustive]`; el `match` sobre `Error` es exhaustivo, sin `_`. En `api/mod.rs`, `pub mod error;`.
- [ ] **Step 5: Ejecutar** `cargo test -p rust_lib_boveda error` → PASS.
- [ ] **Step 6: Commit** `feat: error de la fachada sin datos (#35)`.

### Task 2: `VaultSession` (ciclo de vida), fin de `greet` y comprobación del codegen

**Files:**
- Create: `mobile/rust/src/api/session.rs`, `mobile/rust/src/api/session/tests.rs`, `mobile/rust/src/test_support.rs`
- Delete: `mobile/rust/src/api/simple.rs`, `mobile/integration_test/simple_test.dart`
- Modify: `mobile/rust/src/api/mod.rs`, `mobile/rust/src/lib.rs`, `mobile/lib/main.dart`, `mobile/test/widget_test.dart`
- Regenerate: `mobile/lib/src/rust/`, `mobile/rust/src/frb_generated.rs`

**Interfaces:**
- Consumes: `VaultError`, `FacadeResult` (Task 1).
- Produces:
  - `#[frb(opaque)] pub struct VaultSession { path: PathBuf, vault: Mutex<Option<Vault>> }`
  - `#[frb(sync)] pub fn new(path: String) -> VaultSession`
  - `pub fn create(&self, password: String) -> FacadeResult<()>`
  - `pub fn open(&self) -> FacadeResult<()>`
  - `pub fn unlock(&self, password: String) -> FacadeResult<UnlockReport>`
  - `pub fn lock(&self) -> FacadeResult<()>`
  - `pub fn is_locked(&self) -> FacadeResult<bool>`
  - `pub struct UnlockReport { pub corrupt: Vec<String>, pub unidentified_corrupt: usize }` (`corrupt` en hexadecimal; usa `entry_id_to_hex` de Task 3, que en esta tarea se define en `session.rs` como `pub(crate) fn entry_id_to_hex(id: EntryId) -> String` y Task 3 lo mueve a `entries.rs` sin cambiar la firma)
  - Ayudantes internos: `pub(crate) fn secret_from(s: String) -> SecretString` (copia exacta y `zeroize` del origen) y `fn with_vault<T>(&self, f: impl FnOnce(&mut Vault) -> vault_core::vault::Result<T>) -> FacadeResult<T>` (toma el `Mutex`, `NoVault` si `None`, convierte el error).
  - `test_support.rs` (`#[cfg(test)]`): `pub(crate) const FIXTURE_PASSWORD: &str = "prueba-bóveda-v2";` y `pub(crate) fn fixture_copy() -> (tempfile::TempDir, PathBuf)`, que copia `concat!(env!("CARGO_MANIFEST_DIR"), "/../../core/tests/fixtures/vault-v2.db")` a `tempdir/boveda.db`.

- [ ] **Step 1: Pruebas que fallan** en `session/tests.rs`:
  - `without_vault_is_locked_and_operations_fail`: `VaultSession::new(ruta_temporal)`; `is_locked() == Ok(true)`; `unlock(..) == Err(NoVault)`; `lock() == Ok(())`.
  - `open_leaves_vault_locked`: sobre `fixture_copy()`, `open()` y `is_locked() == Ok(true)`.
  - `unlock_with_wrong_password_stays_locked`: `Err(WrongPassword)` e `is_locked() == Ok(true)`.
  - `unlock_with_fixture_password`: `UnlockReport { corrupt: vec![], unidentified_corrupt: 0 }` e `is_locked() == Ok(false)`; un segundo `unlock` da `Err(AlreadyUnlocked)`.
  - `open_or_create_with_vault_loaded_fails`: tras `open()`, `open()` y `create(..)` dan `Err(VaultAlreadyOpen)`.
  - `lock_after_unlock_locks` (Review Focus 4): tras `unlock` y `lock`, `is_locked() == Ok(true)` y `unlock` vuelve a funcionar.
  - `poisoned_mutex_gives_session_poisoned`: envenenar `session.vault` con un hilo que hace panic mientras tiene el candado (`std::thread::scope`); después `is_locked()` y `lock()` dan `Err(SessionPoisoned)`.
  - `create_on_new_path_is_unlocked_and_empty`: ruta nueva en un `tempdir`, `create(..)`, `is_locked() == Ok(false)`. Es la única prueba que calibra.
- [ ] **Step 2: Ejecutar** `cargo test -p rust_lib_boveda session` → falla al compilar.
- [ ] **Step 3: Implementar** `session.rs` según Interfaces. Delegación directa: `Vault::create(&self.path, &secret)`, `Vault::open(&self.path)`, `vault.unlock(&secret)`, `vault.lock()`, `vault.is_locked()`. `lock` sin bóveda devuelve `Ok(())`; `is_locked` sin bóveda devuelve `Ok(true)`. Mover `init_app` (`#[frb(init)]`) a `api/mod.rs`, borrar `simple.rs` y declarar `#[cfg(test)] mod test_support;` en `lib.rs`.
- [ ] **Step 4: Ejecutar** `cargo test -p rust_lib_boveda` → PASS.
- [ ] **Step 5: Regenerar el puente** desde `mobile/`: `flutter_rust_bridge_codegen generate`. Comprobar en el código generado:
  - `lib/src/rust/api/error.dart` tiene `enum VaultError` (enum de Dart, sin `@freezed`), y en `frb_generated.dart` el decodificador de los errores de `VaultSession` lanza ese valor.
  - `lib/src/rust/api/session.dart` tiene `VaultSession` como clase opaca con constructor `factory VaultSession({required String path})` y métodos `Future`.
  - Si el enum plano **no** sirve como error (no compila o no se lanza), parar: el respaldo es `pub struct VaultException { pub kind: VaultError }` como tipo de error. Consultar con el usuario antes de cambiar la spec.
- [ ] **Step 6: Dart.** `main.dart`: `await RustLib.init()` y un `MaterialApp(title: 'Bóveda')` con un `Scaffold` que muestra `Text('Bóveda')`, sin llamadas a Rust. `widget_test.dart`: `RustLib.initMock(api: MockRustLibApi())` en `setUpAll` y `testWidgets('muestra el marcador', ...)`, que espera `find.text('Bóveda')`. Borrar `integration_test/simple_test.dart` (lo reemplaza Task 5).
- [ ] **Step 7: Ejecutar** desde `mobile/`: `flutter analyze` (sin problemas) y `flutter test` (PASS).
- [ ] **Step 8: Commit** `feat: sesión de la bóveda en la fachada (#35)` con el código generado.

### Task 3: Entradas: búsqueda, alta, edición, borrado y revelado

**Files:**
- Create: `mobile/rust/src/api/entries.rs`, `mobile/rust/src/api/entries/tests.rs`
- Modify: `mobile/rust/src/api/mod.rs`, `mobile/rust/src/api/session.rs` (mover `entry_id_to_hex`)
- Regenerate: `mobile/lib/src/rust/`, `mobile/rust/src/frb_generated.rs`

**Interfaces:**
- Consumes: `VaultSession`, `with_vault`, `secret_from`, `FacadeResult` (Task 2).
- Produces (en un bloque `impl VaultSession` de `entries.rs`):
  - `pub struct EntrySummary { pub id: String, pub service: String, pub url: Option<String>, pub username: Option<String>, pub tags: Vec<String>, pub modified_at: i64, pub last_used_at: Option<i64> }`
  - `pub struct EntryInput { pub service: String, pub url: Option<String>, pub username: Option<String>, pub tags: Vec<String> }`
  - `pub struct NewSecret { pub password: String, pub notes: Option<String> }` (sin `Debug`)
  - `pub fn search(&self, query: String) -> FacadeResult<Vec<EntrySummary>>`
  - `pub fn add(&self, input: EntryInput, secret: NewSecret) -> FacadeResult<String>`
  - `pub fn edit(&self, id: String, input: EntryInput, new_secret: Option<NewSecret>) -> FacadeResult<()>`
  - `pub fn delete(&self, id: String) -> FacadeResult<()>`
  - `pub fn reveal_password(&self, id: String) -> FacadeResult<String>`
  - `pub fn reveal_notes(&self, id: String) -> FacadeResult<Option<String>>`
  - `pub(crate) fn entry_id_to_hex(id: EntryId) -> String` y `pub(crate) fn entry_id_from_hex(hex: &str) -> FacadeResult<EntryId>`

- [ ] **Step 1: Pruebas que fallan** en `entries/tests.rs` (sesión abierta y desbloqueada sobre `fixture_copy()`):
  - `search_empty_lists_live_entries_recent_first`: tres resultados; el primero es `Nube`; `Correo Electrónico` trae `url == Some("https://correo.example.com/entrar")`, `username == Some("ana@example.com")` y `tags == ["Trabajo", "Médico", "Año"]`; todos los `id` miden 32 y son `[0-9a-f]`.
  - `search_finds_by_text`: `search("banco")` devuelve `Banco` primero.
  - `add_returns_hex_id_found_by_search`: `add` de `"Gmail"` devuelve un id de 32 caracteres hexadecimales; `search("gmail")` lo trae con ese id.
  - `add_returns_normalized_data` (Review Focus 3): `add` con servicio `"  Gmail  "` y etiquetas `["a", "A"]` → en `search` aparece `service == "Gmail"` y `tags == ["a"]`.
  - `add_with_empty_service_fails`: `Err(InvalidEntryServiceEmpty)`.
  - `edit_keep_preserves_password` y `edit_replace_changes_password`: con `None`, `reveal_password` sigue dando la anterior; con `Some(NewSecret { password: "nueva".into(), notes: None })`, da `"nueva"` y `reveal_notes` da `None`.
  - `delete_removes_entry`: tras `delete`, `search("banco")` no la trae y `reveal_password` da `Err(EntryNotFound)`.
  - `reveal_returns_fixture_values`: `Correo Electrónico` → contraseña `"clave-completa-ñ"`, notas `Some("Línea uno\nLínea dos")`; `Banco` → `reveal_notes == Ok(None)`.
  - `invalid_ids_are_rejected` (Review Focus 2): `""`, 31 y 33 caracteres, 32 en mayúsculas, 32 con una `g` y `"é".repeat(16)` dan `Err(InvalidEntryId)` en `reveal_password`, `delete` y `edit`.
  - `locked_session_gives_locked`: tras `lock()`, `search("")` da `Err(Locked)`.
- [ ] **Step 2: Ejecutar** `cargo test -p rust_lib_boveda entries` → falla al compilar.
- [ ] **Step 3: Implementar** `entries.rs`. Los datos de búsqueda pasan por `SearchData::new(&service, url.as_deref(), username.as_deref(), &tags_as_str)`, y el secreto por `Secret::new(secret_from(password), notes.map(secret_from))`. `edit` con `None` pasa `SecretChange::Keep`; con `Some`, `SecretChange::Replace`. `reveal_*` llama a `vault.reveal(id)` y copia con `expose_secret().to_owned()` solo la parte pedida. `search` recorre `vault.search(&query)` y copia los campos de `EntrySummary` del núcleo. Hexadecimal a mano: validar primero `hex.len() == 32 && hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))` y recién entonces convertir de a pares de bytes (así no se corta un carácter multibyte).
- [ ] **Step 4: Ejecutar** `cargo test -p rust_lib_boveda` → PASS.
- [ ] **Step 5: Regenerar el puente** y comprobar que los métodos de `entries.rs` aparecen en la clase `VaultSession` de `session.dart`. Si el codegen no acepta el segundo bloque `impl`, mover los métodos a `session.rs` (las estructuras se quedan en `entries.rs`), como prevé la spec.
- [ ] **Step 6: Ejecutar** `flutter analyze` y `flutter test` → sin problemas y PASS.
- [ ] **Step 7: Commit** `feat: entradas en la fachada (#35)`.

### Task 4: Respaldos: exportar e importar con reapertura

**Files:**
- Create: `mobile/rust/src/api/backup.rs`, `mobile/rust/src/api/backup/tests.rs`
- Modify: `mobile/rust/src/api/mod.rs`
- Regenerate: `mobile/lib/src/rust/`, `mobile/rust/src/frb_generated.rs`

**Interfaces:**
- Consumes: `VaultSession`, `UnlockReport`, `with_vault`, `secret_from`, `entry_id_to_hex` (Tasks 2 y 3).
- Produces: `pub fn export(&self, dest: String) -> FacadeResult<()>` y `pub fn import_backup(&self, backup: String, password: String) -> FacadeResult<UnlockReport>`.

- [ ] **Step 1: Pruebas que fallan** en `backup/tests.rs`:
  - `export_then_import_into_new_session_recovers_entries`: exportar desde la copia del fixture desbloqueada a `tempdir/respaldo.db`; en otra sesión sin bóveda (`tempdir2/boveda.db`), `import_backup(respaldo, FIXTURE_PASSWORD)` devuelve un `UnlockReport` vacío, `is_locked() == Ok(false)` y `search("")` trae las tres entradas.
  - `import_over_existing_replaces_it`: sesión con otra bóveda abierta (una segunda copia del fixture con una entrada agregada) e importa el respaldo → `search("")` trae exactamente las tres del fixture.
  - `failed_import_reopens_previous_locked`: con la bóveda desbloqueada, `import_backup(respaldo, "incorrecta")` da `Err(WrongPassword)`; después `is_locked() == Ok(true)` y `unlock(FIXTURE_PASSWORD)` funciona, con las entradas de antes.
  - `import_of_non_vault_reopens_previous` (Review Focus 1): un archivo con bytes `b"no soy una bóveda"` da `Err(NotAVault)`; la anterior queda bloqueada y se desbloquea.
  - `failed_import_without_vault_stays_empty`: sesión nueva sin archivo, importación con la contraseña incorrecta → `Err(WrongPassword)`, `is_locked() == Ok(true)` y `search("")` da `Err(NoVault)`.
  - `export_requires_unlocked_and_new_absolute_dest` (Review Focus 5): bloqueada → `Err(Locked)`; destino existente → `Err(AlreadyExists)`; destino relativo `"respaldo.db"` → `Err(StorageIo)`; en los tres casos la bóveda sigue igual (`is_locked` sin cambios y `search` funciona tras desbloquear).
- [ ] **Step 2: Ejecutar** `cargo test -p rust_lib_boveda backup` → falla al compilar.
- [ ] **Step 3: Implementar** `backup.rs`. `export` delega en `vault.export(Path::new(&dest))`. `import_backup` sigue el "Ciclo de importación" de la spec: tomar el `Mutex` (envenenado → `SessionPoisoned`) y mantenerlo todo el ciclo, hacer `take()`, llamar a `Vault::import(Path::new(&backup), &self.path, &secret)`. Si sale bien, guardar la `Vault` y devolver el reporte convertido. Si falla, guardar `Vault::open(&self.path).ok()` y devolver el error de la importación.
- [ ] **Step 4: Ejecutar** `cargo test -p rust_lib_boveda` → PASS.
- [ ] **Step 5: Regenerar el puente**; `flutter analyze` y `flutter test` → sin problemas y PASS.
- [ ] **Step 6: Commit** `feat: exportar e importar en la fachada (#35)`.

### Task 5: Ruta privada y prueba en el dispositivo

**Files:**
- Modify: `mobile/android/app/src/main/kotlin/com/jesus/boveda/MainActivity.kt`, `mobile/pubspec.yaml`, `mobile/pubspec.lock`
- Create: `mobile/lib/services/vault_location.dart`, `mobile/integration_test/vault_session_test.dart`

**Interfaces:**
- Consumes: la API Dart generada de `VaultSession` (Tasks 2–4) y `VaultError`.
- Produces: `Future<String> vaultPath()` en `lib/services/vault_location.dart`: llama al canal `boveda/storage`, método `noBackupFilesDir`, y devuelve `p.join(dir, 'boveda.db')`. Si el canal devuelve `null`, lanza un `StateError` (sin ruta no hay bóveda).

- [ ] **Step 1: Kotlin.** En `MainActivity`, `configureFlutterEngine` registra `MethodChannel(flutterEngine.dartExecutor.binaryMessenger, "boveda/storage")`. `noBackupFilesDir` responde `result.success(noBackupFilesDir.absolutePath)` y cualquier otro método, `result.notImplemented()`.
- [ ] **Step 2: Dart.** `flutter pub add path:^1.9.1` (comprobar que `pubspec.lock` solo cambia `path` de `transitive` a `direct main`). Escribir `vault_location.dart` según Interfaces.
- [ ] **Step 3: `integration_test/vault_session_test.dart`** con `IntegrationTestWidgetsFlutterBinding.ensureInitialized()` y `RustLib.init()` en `setUpAll`:
  - `'la ruta de la bóveda es absoluta y está en no_backup'`: `final path = await vaultPath();` → `p.isAbsolute(path)`, `p.basename(p.dirname(path)) == 'no_backup'` y `p.basename(path) == 'boveda.db'`. No crea ningún archivo.
  - `'crear, bloquear, desbloquear, agregar, revelar y buscar'`: en `Directory.systemTemp.createTemp()`, `VaultSession(path: ...)`, `create`, `add` de `Gmail` con contraseña `clave-gmail`, `lock`, `unlock` incorrecta lanza `VaultError.wrongPassword`, `unlock` correcta, `search(query: 'gm')` trae `Gmail`, `revealPassword` da `clave-gmail`. Al final, `dispose()` y borrar el directorio temporal.
- [ ] **Step 4: Ejecutar** desde `mobile/`: `flutter analyze` y `flutter test` → sin problemas y PASS. Compilar el APK debug (`flutter build apk --debug --target-platform android-arm64,android-x64`, antes `rustup run stable rustc --version` igual al `channel` fijado).
- [ ] **Step 5: Prueba en el dispositivo (manual, la ejecuta el usuario):** `flutter test integration_test/vault_session_test.dart -d <emulador>` y luego `-d <celular>`. Ambas pasan. Anotar el resultado en el PR.
- [ ] **Step 6: Commit** `feat: ruta de la bóveda en noBackupFilesDir (#35)`.

### Task 6: Documentación y verificación final

**Files:**
- Modify: `docs/especificacion.md`, `ARCHITECTURE.md`, `mobile/CLAUDE.md`

**Interfaces:** ninguna.

- [ ] **Step 1: `docs/especificacion.md`.** En "Respaldos", la decisión del 2026-11-08 (#35, detalle en la spec de diseño): la bóveda vive en `noBackupFilesDir`, fuera de Auto Backup y de la transferencia entre dispositivos; el único camino de restauración es `import`, porque una restauración del sistema se saltaría `import` y, desde la etapa 3, la regeneración de identidad. En "API", junto a la línea del `Mutex`: la fachada lo implementa en `VaultSession` con `Mutex<Option<Vault>>`, y el ciclo de importación (soltar, importar, reabrir bloqueada si falla). Actualizar "Actualizado:".
- [ ] **Step 2: `ARCHITECTURE.md`.** En el mapa, `mobile/rust/src/api/` con `mod.rs`, `error.rs`, `session.rs`, `entries.rs` y `backup.rs`, y `lib/services/vault_location.dart`. En la decisión "Borrado de memoria garantizado solo dentro de `vault-core`", agregar que los búferes de cruce de flutter_rust_bridge y las copias que la fachada entrega al revelar también son de mejor esfuerzo.
- [ ] **Step 3: `mobile/CLAUDE.md`.** Regla: ninguna función de `rust/src/api/` que toque la bóveda lleva `#[frb(sync)]`; solo `VaultSession::new`, que no toca la bóveda ni el disco. Motivo: una función síncrona que espere el `Mutex` mientras otra corre Argon2id congela la interfaz.
- [ ] **Step 4: Verificación final** desde la raíz: `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` y `cargo fmt --all --check`; desde `mobile/`: `flutter analyze`, `flutter test` y `flutter_rust_bridge_codegen generate` seguido de `git status --porcelain mobile/lib/src/rust mobile/rust/src/frb_generated.rs` vacío. Todo pasa.
- [ ] **Step 5: Commit** `docs: fachada del puente y ruta en noBackupFilesDir (#35)`.
