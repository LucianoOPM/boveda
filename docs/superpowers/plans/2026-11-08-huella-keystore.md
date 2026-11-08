# Plan de implementación: desbloqueo con huella y copia de la llave en el Keystore (#38)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Activar la huella (copia de la llave de bóveda cifrada con una llave del Keystore que solo se usa con `BIOMETRIC_STRONG`) y desbloquear con ella, con la desactivación y el aviso cuando la copia deja de servir.

**Architecture:** La fachada de `mobile/rust` expone `export_vault_key` y `unlock_with_key` sobre la API de la #34. Kotlin (`BiometricVault.kt`) administra la llave del Keystore, el archivo `noBackupFilesDir/biometric/vault_key.bin` y `BiometricPrompt`, detrás del canal `boveda/biometric`. Dart lo envuelve en `BiometricService`, orquesta los flujos en `biometric_unlock.dart` y los usa en `VaultGate`, la oferta del primer uso y la pantalla de desbloqueo.

**Tech Stack:** Rust (flutter_rust_bridge 2.13.0, `secrecy` 0.10, `zeroize` 1.9), Kotlin (API de la plataforma, `android.hardware.biometrics`, `AndroidKeyStore`), Flutter 3.47 (`MethodChannel`, `flutter_test`).

**Spec:** `docs/superpowers/specs/2026-11-08-huella-keystore-design.md`

## Global Constraints

- Sin dependencias nuevas (ni crates, ni paquetes de pub, ni artefactos de Gradle).
- Huella solo con API ≥ 30 y `canAuthenticate(BIOMETRIC_STRONG) == BIOMETRIC_SUCCESS`; sin PIN ni credencial del dispositivo. `minSdk` no cambia.
- La llave cruza solo como bytes (`Vec<u8>` ↔ `Uint8List` ↔ `ByteArray`), nunca `String` ni base64. Cada arreglo se pone en ceros apenas se entrega, en todos los caminos.
- Ningún log, mensaje de error, `toString` ni `result.error` lleva llaves, bytes ni texto de excepciones. Kotlin responde `result.error(código, null, null)`.
- Ninguna función nueva de `mobile/rust/src/api/` lleva `#[frb(sync)]`. No se edita a mano `lib/src/rust/` ni `frb_generated.rs`; se regenera con `flutter_rust_bridge_codegen generate` (desde `mobile/`) antes de `cargo test`.
- Códigos del canal: `invalidated`, `canceled`, `lockout`, `failed`, `busy`. Solo `invalidated` y `WrongVaultKey` desactivan la huella.
- Textos exactos: "¿Activar huella?", "Desbloquea la bóveda con tu huella. La contraseña maestra seguirá funcionando y la necesitarás si registras una huella nueva.", "Activar", "Ahora no", "No se pudo activar la huella.", "La huella se desactivó. Desbloquea con tu contraseña maestra.", "Usar huella", "Usar huella para desbloquear", "Activando huella…", "Desbloqueando…", "No se pudo desbloquear la bóveda. Intenta de nuevo.". Diálogo: "Desbloquear Bóveda" / "Usar contraseña" y "Activar huella" / "Cancelar".
- Identificadores en inglés; comentarios, documentación y commits en español, con Conventional Commits y `(#38)`.

## Review Focus

1. **`Uint8List` del canal no modificable:** si `StandardMessageCodec` entregara una vista de solo lectura, `fillRange` lanzaría y rompería el desbloqueo. La prueba de la Task 3 decodifica un sobre real del codec y pone en ceros el resultado.
2. **Huella activa sin huellas registradas** (el usuario borró todas): la llave queda invalidada, `unlock` da `invalidated` → aviso y `disable`; como `isAvailable` da `false`, no aparecen ni "Usar huella" ni la casilla. Prueba en la Task 6.
3. **La app pasa a segundo plano con el diálogo abierto:** el sistema cancela el diálogo con `ERROR_CANCELED` → `canceled` → `fallback`, sin desactivar. Cubierto por el mapeo de la Task 2 y por el paso manual de la Task 8.
4. **`isEnabled` lanza en el arranque:** cuenta como `false`; la pantalla de desbloqueo queda con la contraseña y sin intento automático. Prueba en la Task 6.
5. **`exportVaultKey` con la bóveda bloqueada entre los dos pasos de activar:** `abortEnable` y `failed`, nunca una llave del Keystore huérfana con archivo. Prueba en la Task 4.

---

### Task 1: Fachada — `export_vault_key` y `unlock_with_key`

**Files:**
- Modify: `mobile/rust/src/api/session.rs`
- Test: `mobile/rust/src/api/session/tests.rs`
- Regenerados: `mobile/lib/src/rust/**`, `mobile/rust/src/frb_generated.rs`

**Interfaces:**
- Produces (Rust): `VaultSession::export_vault_key(&self) -> FacadeResult<Vec<u8>>`, `VaultSession::unlock_with_key(&self, key: Vec<u8>) -> FacadeResult<UnlockReport>`.
- Produces (Dart, generado): `Future<Uint8List> exportVaultKey()` y `Future<UnlockReport> unlockWithKey({required List<int> key})` en `VaultSession`.

- [ ] **Step 1: Escribir las pruebas que fallan** en `session/tests.rs`, con `opened_fixture()` y `FIXTURE_PASSWORD`:

```rust
#[test]
fn vault_key_round_trip() {
    let (_dir, session) = opened_fixture();
    session.unlock(FIXTURE_PASSWORD.into()).unwrap();
    let key = session.export_vault_key().unwrap();
    assert_eq!(key.len(), 32);
    session.lock().unwrap();
    assert_eq!(
        session.unlock_with_key(key),
        Ok(UnlockReport { corrupt: vec![], unidentified_corrupt: 0 })
    );
    assert_eq!(session.search(String::new()).unwrap().len(), 3);
}

#[test]
fn unlock_with_key_of_wrong_length_stays_locked() {
    for len in [31, 33] {
        let (_dir, session) = opened_fixture();
        assert_eq!(session.unlock_with_key(vec![7; len]), Err(VaultError::WrongVaultKey));
        assert_eq!(session.is_locked(), Ok(true));
    }
}

#[test]
fn unlock_with_zeroed_key_stays_locked() // vec![0; 32] → WrongVaultKey, is_locked == Ok(true)

#[test]
fn unlock_with_key_of_other_vault_fails()
// otra sesión con temp_path() + create("otra-contraseña-larga".into()) + export_vault_key;
// sobre opened_fixture(): unlock_with_key(esa) → Err(WrongVaultKey)

#[test]
fn export_vault_key_locked_is_locked() // opened_fixture → Err(Locked)

#[test]
fn unlock_with_key_when_unlocked_is_already_unlocked()
// unlock con contraseña, export, unlock_with_key(key) → Err(AlreadyUnlocked)

#[test]
fn vault_key_without_vault_is_no_vault()
// VaultSession::new(temp_path) sin abrir: export_vault_key() y unlock_with_key(vec![0; 32]) → Err(NoVault)
```

- [ ] **Step 2: Comprobar que no compila** — `cargo test -p rust_lib_boveda vault_key` → error: no existe el método `export_vault_key`.

- [ ] **Step 3: Implementar los dos métodos en `session.rs`**, después de `unlock`, con su comentario de documentación en español:
  - `export_vault_key`: `self.with_vault(Vault::export_vault_key).map(|key| key.expose_secret().to_vec())`. Comentario: el `Vec` es la copia que sale del núcleo, como `reveal_password`; en Dart la pone en ceros `enableBiometric`.
  - `unlock_with_key`: primera línea `let key = Zeroizing::new(key);`. Largo ≠ 32 → `Err(VaultError::WrongVaultKey)`. Si no, `let mut vault_key = SecretBox::<[u8; 32]>::default(); vault_key.expose_secret_mut().copy_from_slice(&key);` (igual que `export_vault_key` del núcleo), y `self.with_vault(|v| v.unlock_with_key(vault_key)).map(UnlockReport::from)`. Comentario con el caso límite (largo incorrecto con la bóveda desbloqueada da `WrongVaultKey`).
  - Imports: `secrecy::{ExposeSecret, ExposeSecretMut, SecretBox}`, `zeroize::Zeroizing`.

- [ ] **Step 4: Regenerar el puente y correr las pruebas**

Run (desde `mobile/`): `flutter_rust_bridge_codegen generate`
Run (desde la raíz): `cargo test -p rust_lib_boveda`
Expected: PASS, con las 7 pruebas nuevas.

- [ ] **Step 5: Lint y formato**

Run: `cargo clippy --workspace --all-targets -- -D warnings` y `cargo fmt --all` (sin `| grep`). Expected: sin avisos.

- [ ] **Step 6: Commit**

```bash
git add mobile/rust/src/api/session.rs mobile/rust/src/api/session/tests.rs mobile/rust/src/frb_generated.rs mobile/lib/src/rust
git commit -m "feat: exportar la llave y desbloquear con ella en la fachada (#38)"
```

---

### Task 2: Kotlin — Keystore, archivo, diálogo y canal

**Files:**
- Create: `mobile/android/app/src/main/kotlin/com/jesus/boveda/BiometricVault.kt`
- Modify: `mobile/android/app/src/main/kotlin/com/jesus/boveda/MainActivity.kt`
- Modify: `mobile/android/app/src/main/AndroidManifest.xml`

**Interfaces:**
- Produces: canal `boveda/biometric` con los métodos `isAvailable` → `Boolean`, `isEnabled` → `Boolean`, `beginEnable` → `null`, `completeEnable(ByteArray)` → `null`, `abortEnable` → `null`, `unlock` → `ByteArray`, `disable` → `null`. Errores: `result.error(code, null, null)` con `code` en `invalidated | canceled | lockout | failed | busy`.

Sin pruebas automáticas (el Keystore exige un dispositivo): la verificación es que compile y la prueba a mano de la Task 8.

- [ ] **Step 1: Permiso.** En `AndroidManifest.xml`, antes de `<application>`: `<uses-permission android:name="android.permission.USE_BIOMETRIC"/>`.

- [ ] **Step 2: `BiometricVault(private val activity: Activity)`** en `BiometricVault.kt`, con un comentario de cabecera que resuma las reglas del spec ("Kotlin: Keystore, diálogo y canal"). Contenido:
  - Constantes: `ALIAS = "boveda_biometric_wrap"`, `FILE_VERSION: Byte = 1`, `IV_LEN = 12`, `CIPHERTEXT_LEN = 48`, `FILE_LEN = 61`, `TAG_BITS = 128`, `TRANSFORMATION = "AES/GCM/NoPadding"`; directorio `File(activity.noBackupFilesDir, "biometric")`, archivos `vault_key.bin` y `vault_key.bin.tmp`.
  - `fun handle(call: MethodCall, result: MethodChannel.Result)`: despacha por `call.method`. Con `Build.VERSION.SDK_INT < 30`: `isAvailable`/`isEnabled` → `false`, `disable` → borra el archivo y el `.tmp` (sin Keystore) y `null`, el resto → `failed`.
  - `fun onDestroy()`: cancela el `CancellationSignal` activo y, si hay un `Cipher` pendiente, lo descarta y borra la llave.
  - Lo que sigue lleva `@RequiresApi(30)`:
    - `createKey()`: `KeyGenerator.getInstance(KEY_ALGORITHM_AES, "AndroidKeyStore")` con `KeyGenParameterSpec.Builder(ALIAS, PURPOSE_ENCRYPT or PURPOSE_DECRYPT)`, `.setBlockModes(BLOCK_MODE_GCM)`, `.setEncryptionPaddings(ENCRYPTION_PADDING_NONE)`, `.setKeySize(256)`, `.setUserAuthenticationRequired(true)`, `.setUserAuthenticationParameters(0, KeyProperties.AUTH_BIOMETRIC_STRONG)`, `.setInvalidatedByBiometricEnrollment(true)`, con un comentario: explícitos aunque sean el valor por omisión.
    - `deleteAll()`: borra `vault_key.bin`, `.tmp` y `keyStore.deleteEntry(ALIAS)` si existe; nunca lanza.
    - `isAvailable`: `getSystemService(BiometricManager::class.java).canAuthenticate(BIOMETRIC_STRONG) == BIOMETRIC_SUCCESS`.
    - `isEnabled`: `vault_key.bin` existe.
    - `beginEnable`: si hay un diálogo activo → `busy`. `deleteAll()`, `createKey()`, `cipher.init(ENCRYPT_MODE, key)`, `cipher.updateAAD(byteArrayOf(FILE_VERSION))`, diálogo "Activar huella"/"Cancelar". En `onAuthenticationSucceeded`: guarda `result.cryptoObject.cipher` en `pendingCipher` y responde `null`. En error: `deleteAll()` y el código.
    - `completeEnable(key)`: `try { pendingCipher ?: failed; val ct = cipher.doFinal(key); check(ct.size == CIPHERTEXT_LEN); escribir [FILE_VERSION] + cipher.iv + ct en .tmp con FileOutputStream + fd.sync(); if (!tmp.renameTo(file)) → fallo } catch → deleteAll(); failed; finally { key.fill(0); pendingCipher = null }`.
    - `abortEnable`: `pendingCipher = null`, borra la llave (y el `.tmp`), `null`.
    - `unlock`: si hay un diálogo activo → `busy`. Lee el archivo: si no existe, no mide `FILE_LEN` o `bytes[0] != FILE_VERSION` → `invalidated`. Llave del Keystore `null` → `invalidated`. `cipher.init(DECRYPT_MODE, key, GCMParameterSpec(TAG_BITS, iv))` + `updateAAD`; `KeyPermanentlyInvalidatedException` → `invalidated`; otra excepción → `failed`. Diálogo "Desbloquear Bóveda"/"Usar contraseña". En `onAuthenticationSucceeded`: `doFinal(ciphertext)` (`AEADBadTagException` → `invalidated`, otra → `failed`), `result.success(plain)` y `plain.fill(0)`, con el comentario: ambos en el hilo principal (`mainExecutor`), donde la copia síncrona del codec está garantizada; mejor esfuerzo.
    - `disable`: `deleteAll()`, `null`.
    - Diálogo: `BiometricPrompt.Builder(activity).setTitle(...).setAllowedAuthenticators(BIOMETRIC_STRONG).setNegativeButton(texto, mainExecutor) { _, _ -> }`, `authenticate(CryptoObject(cipher), CancellationSignal, mainExecutor, callback)`. Un solo `result` por diálogo: el callback del botón negativo no responde (lo hace `onAuthenticationError` con `ERROR_NEGATIVE_BUTTON`). `onAuthenticationFailed` no hace nada.
    - Mapeo de `onAuthenticationError`: `ERROR_USER_CANCELED`, `ERROR_NEGATIVE_BUTTON`, `ERROR_CANCELED` → `canceled`; `ERROR_LOCKOUT`, `ERROR_LOCKOUT_PERMANENT` → `lockout`; cualquier otro (incluidos `ERROR_TIMEOUT`, `ERROR_HW_UNAVAILABLE`, `ERROR_NO_BIOMETRICS`) → `failed`.
  - Sin `Log` en ningún camino.

- [ ] **Step 3: `MainActivity`.** Campo `private val biometric = BiometricVault(this)` (o `lateinit` creado en `configureFlutterEngine`); registra `MethodChannel(..., "boveda/biometric").setMethodCallHandler(biometric::handle)`; `override fun onDestroy() { biometric.onDestroy(); super.onDestroy() }`.

- [ ] **Step 4: Compilar**

Run (desde `mobile/`): `rustup run stable rustc --version` (debe coincidir con `rust-toolchain.toml`) y `flutter build apk --debug --target-platform android-x64`
Expected: `Built build/app/outputs/flutter-apk/app-debug.apk`, sin errores de Kotlin.

- [ ] **Step 5: Commit**

```bash
git add mobile/android/app/src/main
git commit -m "feat: llave del Keystore y diálogo de huella en Kotlin (#38)"
```

---

### Task 3: Dart — `BiometricService` y su implementación con el canal

**Files:**
- Create: `mobile/lib/services/biometric_service.dart`
- Test: `mobile/test/biometric_service_test.dart`

**Interfaces:**
- Consumes: canal `boveda/biometric` (Task 2).
- Produces: `enum BiometricFailure { invalidated, canceled, lockout, failed, busy }`; `class BiometricException implements Exception { const BiometricException(this.failure); final BiometricFailure failure; }`; `abstract class BiometricService` con `Future<bool> isAvailable()`, `Future<bool> isEnabled()`, `Future<void> beginEnable()`, `Future<void> completeEnable(Uint8List key)`, `Future<void> abortEnable()`, `Future<Uint8List> unlock()`, `Future<void> disable()`; `class PlatformBiometricService implements BiometricService` con `const PlatformBiometricService()`.

- [ ] **Step 1: Pruebas que fallan** con `TestWidgetsFlutterBinding.ensureInitialized()` y `TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger.setMockMethodCallHandler(const MethodChannel('boveda/biometric'), handler)`:
  - `'cada código produce su BiometricFailure'`: para cada nombre de `BiometricFailure`, el manejador lanza `PlatformException(code: nombre)` en `unlock` → `throwsA(isA<BiometricException>().having((e) => e.failure, 'failure', valor))`.
  - `'un código desconocido produce failed'` (`code: 'raro'`).
  - `'unlock devuelve los bytes y se pueden poner en ceros'`: el manejador devuelve `Uint8List.fromList(List.filled(32, 7))`; el resultado mide 32 y, tras `fillRange(0, 32, 0)`, todos son 0. Pasa por el codec real del canal (cubre el punto 1 de Review Focus).
  - `'completeEnable envía la llave como Uint8List'`: el manejador guarda `call.arguments`; `expect(args, isA<Uint8List>())` y los mismos bytes.
  - `'isAvailable e isEnabled dan false si el canal lanza'`: el manejador lanza `PlatformException(code: 'failed')`; ambos `false`. También con el manejador `null` (`MissingPluginException`).
  - `'beginEnable, abortEnable y disable llaman al método del mismo nombre'`.

- [ ] **Step 2:** `flutter test test/biometric_service_test.dart` (desde `mobile/`) → falla: no existe el archivo importado.

- [ ] **Step 3: Implementar `biometric_service.dart`.** Un método privado `Future<T> _invoke<T>(String method, [Object? args])` que traduce `PlatformException` (por `code`) y `MissingPluginException` a `BiometricException`; un código fuera del enum → `failed`. `unlock` lanza `BiometricException(failed)` si el canal devuelve `null`. `isAvailable` e `isEnabled` capturan cualquier error y devuelven `false`. Comentario: `completeEnable` envía el `Uint8List` tal cual; ponerlo en ceros es de quien llama (`enableBiometric`). `BiometricException` sin `toString` con datos.

- [ ] **Step 4:** `flutter test test/biometric_service_test.dart` → PASS. `flutter analyze` → `No issues found!`.

- [ ] **Step 5: Commit** — `git add mobile/lib/services/biometric_service.dart mobile/test/biometric_service_test.dart` y `git commit -m "feat: servicio de huella sobre el canal de Kotlin (#38)"`.

---

### Task 4: Dart — flujos `enableBiometric` y `unlockWithBiometric`

**Files:**
- Create: `mobile/lib/services/biometric_unlock.dart`
- Modify: `mobile/test/fakes.dart`
- Test: `mobile/test/biometric_unlock_test.dart`

**Interfaces:**
- Consumes: `BiometricService`, `BiometricException`, `BiometricFailure` (Task 3); `VaultSession.exportVaultKey()`, `VaultSession.unlockWithKey({required List<int> key})` (Task 1).
- Produces:
  - `enum EnableOutcome { enabled, canceled, failed }` y `Future<EnableOutcome> enableBiometric(VaultSession session, BiometricService biometric)`.
  - `sealed class BiometricUnlockOutcome` con `BiometricUnlocked(int damagedEntries)`, `BiometricFallback()`, `BiometricDisabled()`, `BiometricCoreError()`; `Future<BiometricUnlockOutcome> unlockWithBiometric(VaultSession session, BiometricService biometric)`.
  - En `fakes.dart`: `class FakeBiometricService implements BiometricService` con `bool available = true`, `bool enabled = false`, `final List<String> calls = []` (nombre de cada método en orden), `Uint8List? receivedKey`, y una función configurable por método (`Future<void> Function() onBeginEnable`, `onCompleteEnable`, `onAbortEnable`, `onDisable`; `Future<Uint8List> Function() onUnlock`; `isAvailable`/`isEnabled` devuelven los campos o lanzan si `availableError`/`enabledError` no es `null`). Por omisión todo tiene éxito; `onUnlock` devuelve 32 bytes de valor 7; `disable` pone `enabled = false` y `completeEnable` lo pone en `true`. `FakeVaultSession` gana `Future<Uint8List> Function() onExportVaultKey` (por omisión 32 bytes de valor 9), `List<List<int>> unlockWithKeyCalls` (guarda la referencia) y `Future<UnlockReport> Function(List<int>) onUnlockWithKey` (por omisión `emptyReport`); `exportVaultKey` registra en `exportedKeys` la referencia que devuelve.

- [ ] **Step 1: Pruebas que fallan** (`biometric_unlock_test.dart`). Ayudante `bool zeroed(List<int> k) => k.every((b) => b == 0);`.
  - `enableBiometric`:
    - `'activa en orden y pone la llave en ceros'`: resultado `enabled`; `biometric.calls == ['beginEnable', 'completeEnable']`; `identical(biometric.receivedKey, session.exportedKeys.single)` y `zeroed(...)`.
    - `'beginEnable cancelado no exporta la llave'`: `onBeginEnable` lanza `canceled` → `canceled`, `session.exportedKeys` vacío.
    - `'beginEnable con lockout o failed da failed'`.
    - `'exportVaultKey que falla llama a abortEnable'`: `onExportVaultKey` lanza `VaultError.locked` → `failed`, `calls == ['beginEnable', 'abortEnable']` (punto 5 de Review Focus).
    - `'completeEnable que falla da failed y pone la llave en ceros'`.
  - `unlockWithBiometric`:
    - `'con éxito suma las dañadas y pone la llave en ceros'`: `onUnlockWithKey` devuelve `UnlockReport(corrupt: ['a'*32, 'b'*32], unidentifiedCorrupt: 1)` → `BiometricUnlocked` con `damagedEntries == 3`; la llave recibida por `unlockWithKey` está en ceros.
    - `'invalidated desactiva'`: `onUnlock` lanza `invalidated` → `BiometricDisabled`, `calls` contiene `'disable'`.
    - `'wrongVaultKey desactiva y pone la llave en ceros'`.
    - `'canceled, lockout, failed y busy vuelven a la contraseña sin desactivar'` (un caso por valor): `BiometricFallback`, sin `'disable'`.
    - `'alreadyUnlocked cuenta como éxito y pone la llave en ceros'`: `BiometricUnlocked(0)`.
    - `'storageIo y una excepción ajena dan coreError y ponen la llave en ceros'`.
    - `'disable que falla igual da disabled'`.

- [ ] **Step 2:** `flutter test test/biometric_unlock_test.dart` → falla al compilar.

- [ ] **Step 3: Implementar `biometric_unlock.dart`** según la tabla del spec ("Flujos"). La llave se pone en ceros con `key.fillRange(0, key.length, 0)` en un `finally` que rodea `completeEnable` y `unlockWithKey`. Ningún `catch` interpola el error. Agregar los métodos a los dobles de `fakes.dart`.

- [ ] **Step 4:** `flutter test` → PASS (todas). `flutter analyze` → sin problemas.

- [ ] **Step 5: Commit** — `git commit -m "feat: flujos de activar y desbloquear con huella (#38)"` con los tres archivos.

---

### Task 5: Oferta del primer uso, `VaultGate` y arranque

**Files:**
- Create: `mobile/lib/screens/biometric_offer_screen.dart`
- Modify: `mobile/lib/app.dart`, `mobile/lib/main.dart`, `mobile/lib/screens/first_run_screen.dart`
- Test: `mobile/test/biometric_offer_screen_test.dart`, `mobile/test/app_test.dart`, `mobile/test/first_run_screen_test.dart`, y los demás que construyen `BovedaApp`, `FirstRunScreen` o `UnlockScreen` (`bootstrap_test.dart`, `unlock_screen_test.dart`)

**Interfaces:**
- Consumes: `enableBiometric`, `EnableOutcome` (Task 4); `BiometricService`, `PlatformBiometricService` (Task 3).
- Produces:
  - `BiometricOfferScreen({required VaultSession session, required BiometricService biometric, required VoidCallback onDone})`.
  - `BovedaApp` y `VaultGate` con `required BiometricService biometric`; `VaultGateState.offerBiometric`.
  - `FirstRunScreen` con `required BiometricService biometric`.
  - `UnlockScreen` con `required BiometricService biometric` (el parámetro se agrega aquí, sin usarlo hasta la Task 6, para que `app.dart` compile).

- [ ] **Step 1: Pruebas que fallan.**
  - `biometric_offer_screen_test.dart` (montada en `MaterialApp(home: ...)`): muestra el título, el texto y los dos botones con los textos exactos; `'Activar con éxito llama a onDone'`; `'Activar cancelado se queda sin mensaje'` (`onBeginEnable` lanza `canceled`: `onDone` no se llamó, sin "No se pudo activar la huella."); `'Activar fallido muestra el mensaje y permite reintentar'` (segunda vez con éxito → `onDone`); `'Ahora no llama a onDone sin activar'` (`biometric.calls` vacío); `'los botones se desactivan mientras activa'` (`onBeginEnable` con un `Completer`).
  - `app_test.dart`: `pumpApp` recibe un `FakeBiometricService`. Nuevas: `'después de crear, con huella disponible, ofrece activarla'`; `'sin huella disponible pasa a la pantalla principal'` (`available = false`); `'si isAvailable lanza pasa a la pantalla principal'`; `'Ahora no lleva a la pantalla principal'`.
  - `first_run_screen_test.dart`: `'llama a disable antes de create'` (orden con un registro compartido: `onDisable` agrega `'disable'` y `onCreate` agrega `'create'` a la misma lista); `'si disable falla igual crea la bóveda'`.

- [ ] **Step 2:** `flutter test` → fallan al compilar.

- [ ] **Step 3: Implementar.**
  - `BiometricOfferScreen`: `StatefulWidget` con `_busy` y `String? _error`; "Activar" → `enableBiometric`; con `mounted`: `enabled` → `onDone()`, `canceled` → solo `_busy = false`, `failed` → `_error = 'No se pudo activar la huella.'`. Mismo estilo de `Scaffold`/`SafeArea`/`SingleChildScrollView`/`padding: 24` que `UnlockScreen`. Keys: `Key('offer-enable')`, `Key('offer-skip')`.
  - `VaultGate`: al crear (`onCreated`), `_afterCreate()`: `try { offer = await biometric.isAvailable() && !await biometric.isEnabled(); } catch (_) { offer = false; }`; con `mounted`, `offerBiometric` o `_unlocked(0)`. `offerBiometric` → `BiometricOfferScreen(onDone: () => _unlocked(0))`. Actualizar el comentario de la clase con la transición nueva.
  - `FirstRunScreen._create`: antes de `session.create`, `try { await widget.biometric.disable(); } catch (_) {}` con el comentario del spec (mejor esfuerzo; una copia vieja la rechaza `key_check`).
  - `main.dart`: `biometric: const PlatformBiometricService()`.
  - Ajustar las pruebas existentes que construyen estos widgets para pasar `FakeBiometricService()`.

- [ ] **Step 4:** `flutter test` → PASS. `flutter analyze` → sin problemas.

- [ ] **Step 5: Commit** — `git commit -m "feat: ofrecer la huella al terminar el primer uso (#38)"`.

---

### Task 6: Huella en la pantalla de desbloqueo

**Files:**
- Modify: `mobile/lib/screens/unlock_screen.dart`
- Test: `mobile/test/unlock_screen_test.dart`, `mobile/test/app_test.dart`

**Interfaces:**
- Consumes: `unlockWithBiometric`, `BiometricUnlockOutcome` y sus subclases, `enableBiometric`, `EnableOutcome` (Task 4); `UnlockScreen.biometric` (Task 5).
- Produces: keys `Key('unlock-biometric')` (botón "Usar huella") y `Key('unlock-enable-biometric')` (casilla).

- [ ] **Step 1: Pruebas que fallan** en `unlock_screen_test.dart` (`pumpScreen` recibe un `FakeBiometricService`; sin huella activa por omisión, así que las pruebas de la #37 no cambian de comportamiento):
  - `'con huella activa pide la huella una vez después del primer cuadro'`: `enabled = true`; tras `pumpScreen`, `biometric.calls.where((c) => c == 'unlock').length == 1`; tras `onUnlock` que lanza `canceled` y otro `pump`, sigue en 1.
  - `'mientras corre la huella el campo y el botón están desactivados'`: `onUnlock` con `Completer`; "Desbloqueando…" visible; tocar `submitKey` con texto no llama a `session.unlock`.
  - `'huella con éxito llama a onUnlocked con las dañadas'`.
  - `'fallback deja el foco en el campo sin mensaje'`: `field(tester).focusNode!.hasFocus`, sin `wrong` ni `generic`.
  - `'disabled muestra el aviso, oculta Usar huella y muestra la casilla'`.
  - `'disabled sin huella disponible no muestra ni el botón ni la casilla'`: `available = false` (punto 2 de Review Focus).
  - `'coreError muestra el mensaje genérico'`.
  - `'Usar huella vuelve a pedir la huella'` y `'sin huella activa no hay botón Usar huella'`.
  - `'si isEnabled lanza no hay intento automático'`: `enabledError` no nulo → sin `'unlock'` en `calls`, campo activo (punto 4 de Review Focus).
  - `'la casilla no aparece sin huella disponible ni con la huella activa'`.
  - `'casilla marcada y contraseña correcta activan antes de onUnlocked'`: orden con un registro compartido (`completeEnable` antes de la llamada a `onUnlocked`).
  - `'casilla marcada y contraseña incorrecta no activan'`: `onUnlock` de la sesión lanza `wrongPassword` → `biometric.calls` sin `'beginEnable'` y `session.exportedKeys` vacío.
  - `'casilla sin marcar no activa'`.
  - `'activación cancelada desde el desbloqueo sigue sin mensaje'`.
  - `'desmontar con la huella pendiente no lanza'`: `onUnlock` con `Completer`, `pumpWidget(SizedBox())`, completar con éxito y con error; `tester.takeException()` es `null` y `onUnlocked` no se llamó.
  - En `app_test.dart`: `'activación fallida desde el desbloqueo muestra el SnackBar en la pantalla principal'` (montando `BovedaApp`, casilla marcada, `onBeginEnable` lanza `failed`): tras `pumpAndSettle`, "Pantalla principal: #40" y "No se pudo activar la huella." visibles.

- [ ] **Step 2:** `flutter test test/unlock_screen_test.dart` → fallan.

- [ ] **Step 3: Implementar en `UnlockScreen`** (spec, "Pantalla de desbloqueo"):
  - Estado nuevo: `bool _biometricEnabled = false`, `bool _canOfferBiometric = false`, `bool _enableBiometric = false`, `String? _notice` (aviso de huella desactivada), `bool _activating = false`; `_unlocking` pasa a llamarse `_busy` y cubre ambos desbloqueos.
  - `initState`: `addPostFrameCallback((_) => _start())`. `_start()` lee `isEnabled()` (error → `false`) e `isAvailable()`, fija `_biometricEnabled` y `_canOfferBiometric = available && !enabled`, y si está activa llama a `_unlockWithBiometric()`. Solo corre una vez por instancia.
  - `_unlockWithBiometric()`: guarda `_busy`; aplica la tabla del spec. Tras `BiometricDisabled`, vuelve a evaluar `isAvailable() && !isEnabled()`. Foco con `addPostFrameCallback` como `_fail`.
  - `_unlock()` (contraseña): al principio, `final messenger = ScaffoldMessenger.of(context);`. Tras el éxito, si `_enableBiometric`: `_activating = true` (el botón muestra "Activando huella…"), `enableBiometric`; con `failed`, `messenger.showSnackBar(const SnackBar(content: Text('No se pudo activar la huella.')))`; después, `onUnlocked`.
  - Widgets: el aviso `_notice` con el mismo estilo que `_error`; "Usar huella" (`OutlinedButton`, `Key('unlock-biometric')`) si `_biometricEnabled`, desactivado con `_busy`; `CheckboxListTile` (`Key('unlock-enable-biometric')`) si `_canOfferBiometric`, desactivado con `_busy`.
  - Actualizar el comentario de la clase.

- [ ] **Step 4:** `flutter test` → PASS (todas, también las de la #37). `flutter analyze` → sin problemas.

- [ ] **Step 5: Commit** — `git commit -m "feat: huella en la pantalla de desbloqueo (#38)"`.

---

### Task 7: Documentación

**Files:**
- Modify: `docs/especificacion.md`, `ARCHITECTURE.md`, `docs/roadmap.md`

- [ ] **Step 1: `docs/especificacion.md`.**
  - "Viaje de la llave de bóveda hacia el Keystore": agregar "Decidido el 2026-11-08 (#38, detalle en `docs/superpowers/specs/2026-11-08-huella-keystore-design.md`): la llave pasa por Dart, sin JNI…" y los búferes de `StandardMessageCodec` dentro del mejor esfuerzo; reemplazar la frase "se evalúa en la #38".
  - "Experiencia de uso → Desbloqueo": decisión del 2026-11-08 (#38): huella solo con API ≥ 30 y `BIOMETRIC_STRONG`, sin PIN; qué la desactiva (`KeyPermanentlyInvalidatedException`, `WrongVaultKey`, copia ilegible o llave inexistente) y qué no (cancelación, bloqueo temporal, otros errores); se ofrece al terminar el primer uso y con una casilla en el desbloqueo con contraseña.
  - "Respaldos", junto a la decisión de `noBackupFilesDir`: la copia de la llave vive en `noBackupFilesDir/biometric/vault_key.bin` (versión | IV | texto cifrado, 61 bytes), también fuera de Auto Backup.
- [ ] **Step 2: `ARCHITECTURE.md`.** En el mapa: `biometric_offer_screen.dart` en `screens/`, `biometric_service.dart` y `biometric_unlock.dart` en `services/` (quitar "futuro: huella, Keystore"), `android/` con `MainActivity.kt` y `BiometricVault.kt`. En `app.dart`: el estado `offerBiometric`. En la fila "Borrado de memoria garantizado solo dentro de `vault-core`": los búferes de `StandardMessageCodec` y la copia de la llave al exportarla para el Keystore. En "Flujos principales → Desbloqueo": se ofrece activar y se desactiva sola si la copia deja de servir.
- [ ] **Step 3: `docs/roadmap.md`,** etapa 4, en la tarea de los 7 días: "Guardar la fecha del último desbloqueo con contraseña fuera de la bóveda (movido desde la #38); sin fecha, se pide la contraseña."
- [ ] **Step 4: Commit** — `git commit -m "docs: huella en la especificación, la arquitectura y el roadmap (#38)"`.

---

### Task 8: Verificación completa y prueba a mano

- [ ] **Step 1: Todo lo de la CI**, desde la raíz y desde `mobile/`:

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cd mobile && flutter analyze && flutter test
flutter_rust_bridge_codegen generate && git status --porcelain -- lib/src/rust rust/src/frb_generated.rs
```

Expected: todo pasa y el último comando no imprime nada.

- [ ] **Step 2: Prueba a mano en el emulador** (API ≥ 30, huella registrada en Ajustes; `MSYS_NO_PATHCONV=1` para `adb` en Git Bash), los pasos 1 a 9 de la sección "A mano" del spec, incluido mandar la app a segundo plano con el diálogo abierto (punto 3 de Review Focus: vuelve a la contraseña sin desactivar). Si el diálogo no aparece en el arranque en frío, detenerse con la evidencia de logcat antes de intentar arreglos.
- [ ] **Step 3: En el celular del usuario**, los pasos 1 a 6 (confirmar antes que tiene Android 11 o posterior). Lo hace el usuario; se registra el resultado en el PR.
- [ ] **Step 4:** Si algo de la prueba a mano exige cambios, cada corrección va en un `fix:` propio con su prueba, y se repite el Step 1.
