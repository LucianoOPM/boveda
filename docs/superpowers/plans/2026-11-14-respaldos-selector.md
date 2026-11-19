# Respaldos con el selector del sistema (#44) — plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Exportar e importar el respaldo desde Ajustes y restaurarlo desde el primer uso, con el selector de archivos del sistema y sin cambiar `core/` ni la fachada.

**Architecture:** Un canal de Kotlin (`boveda/backup`) con el Storage Access Framework copia por flujo entre la URI `content://` y un temporal en `noBackupFilesDir/backup-tmp/`; Dart solo maneja rutas. `AutoLockController.runWithoutAutoLock` suspende el bloqueo mientras el selector está abierto, con un tope de 2 minutos. Tras un error al importar desde Ajustes, `VaultGate` bloquea y la pantalla de desbloqueo muestra un aviso de un solo uso.

**Tech Stack:** Flutter/Dart, Kotlin (SAF), JUnit 4.13.2 (solo prueba de `isInsideTempDir`). Sin paquetes de Dart nuevos.

**Spec:** `docs/superpowers/specs/2026-11-14-respaldos-selector-design.md`

## Global Constraints

- Sin dependencias nuevas salvo `testImplementation("junit:junit:4.13.2")` en `mobile/android/app/build.gradle.kts`; sin Robolectric ni mocks.
- Identificadores en inglés; comentarios, documentación y commits en español, Conventional Commits con `(#44)`.
- No se cambian `core/`, `mobile/rust/` ni `lib/src/rust/`; no se regenera el puente.
- Tope de tamaño al copiar de la URI: `20 * 1024 * 1024` bytes. Tope fuera de la app: 2 minutos. Nombre: `boveda-AAAA-MM-DD.db`, MIME `application/octet-stream`.
- Temporales solo en `noBackupFilesDir/backup-tmp/`, con nombre `<uuid>.bak`.
- Códigos de error de Kotlin: `failed`, `busy`, `tooLarge`, `invalidPath`; sin texto. El `notice` es de un solo uso, solo en memoria y con texto fijo por resultado: nunca datos de la bóveda ni rutas.
- Todo campo de texto de las pantallas de la bóveda es `ActivityTextField` o `PasswordField`.
- Nunca encadenar `flutter analyze`, `cargo clippy` ni `flutter test` con `| grep` o `| tail` antes de un commit: usar un script que imprima los códigos de salida.
- Antes de usar el emulador, avisar al usuario; no compilar a la vez; al terminar, cerrar emulador, `adb` y Gradle (`./gradlew --stop`).
- Comandos de Flutter desde `mobile/`; `adb` está en `$LOCALAPPDATA/Android/Sdk/platform-tools/adb.exe` y en Git Bash necesita `MSYS_NO_PATHCONV=1`.

## Review Focus

Cada línea tiene su prueba en la tarea que se indica.

1. Un archivo que no es una bóveda (vacío, texto) o de más de 20 MiB: el primero bloquea con el motivo; el segundo avisa sin bloquear (Tareas 4 y 6).
2. Doble toque en Exportar o Importar: un solo flujo (Tarea 5).
3. Cerrar el diálogo (atrás, tocar fuera) con la importación en curso: no se puede (Tarea 6).
4. Respaldo con entradas dañadas: se importa y la principal muestra el aviso con el total (Tarea 6).
5. `exportBackup` falla (por ejemplo `locked`): mensaje y el temporal se borra igual (Tarea 5).

## Estructura de archivos

| Archivo | Responsabilidad |
| --- | --- |
| `mobile/android/app/src/main/kotlin/com/jesus/boveda/BackupPaths.kt` (nuevo) | `isInsideTempDir`: función pura, sin Android |
| `mobile/android/app/src/test/kotlin/com/jesus/boveda/BackupPathsTest.kt` (nuevo) | Prueba JVM |
| `mobile/android/app/src/main/kotlin/com/jesus/boveda/BackupFiles.kt` (nuevo) | Canal `boveda/backup`: selector, copia por flujo, temporales |
| `mobile/android/app/src/main/kotlin/com/jesus/boveda/MainActivity.kt` | Registra el canal, `onActivityResult`, limpia `backup-tmp/` |
| `mobile/lib/services/backup_files.dart` (nuevo) | `BackupFiles`, `PlatformBackupFiles`, `backupFileName` |
| `mobile/lib/services/auto_lock.dart` | `runWithoutAutoLock`, `AutoLockScope.run` |
| `mobile/lib/screens/restore_dialog.dart` (nuevo) | Flujo de importar compartido: `runRestore`, `RestoreDialog`, resultados, mensajes |
| `mobile/lib/screens/settings_screen.dart` (nuevo) | Exportar e importar |
| `mobile/lib/app.dart`, `search_screen.dart`, `first_run_screen.dart`, `unlock_screen.dart`, `main.dart`, `services/biometric_unlock.dart` | Ver tareas |
| `mobile/test/fakes.dart`, `backup_files_test.dart`, `settings_screen_test.dart`, `backup_gate_test.dart`, `auto_lock_test.dart` | Pruebas |

### Task 1: `isInsideTempDir` con JUnit

**Files:**
- Modify: `mobile/android/app/build.gradle.kts` (agregar `dependencies { testImplementation("junit:junit:4.13.2") }`)
- Create: `mobile/android/app/src/main/kotlin/com/jesus/boveda/BackupPaths.kt`
- Test: `mobile/android/app/src/test/kotlin/com/jesus/boveda/BackupPathsTest.kt`

**Interfaces:**
- Produces: `fun isInsideTempDir(dir: File, path: File): Boolean` — `true` solo si `path.canonicalFile.parentFile == dir.canonicalFile`.

- [ ] **Step 1: Write the failing test.** `BackupPathsTest` con `@get:Rule val folder = TemporaryFolder()`. `dir = folder.newFolder("backup-tmp")`. Casos (un `@Test` cada uno): `validPathInsideDirIsAccepted` (`File(dir, "a.bak")` → `true`); `nonexistentPathInsideDirIsAccepted` (no creado → `true`, deleteTemp no hará nada); `nonexistentPathOutsideDirIsRejected`; `dotDotPathIsRejected` (`File(dir, "../secret.bak")` con el archivo real en `folder.root` → `false`); `symlinkPointingOutsideIsRejected` (`Files.createSymbolicLink(File(dir, "link.bak").toPath(), outside.toPath())` → `false`; si el sistema no permite crearlo, `Assume.assumeNoException`); `dirItselfIsRejected` (`dir` → `false`); `siblingWithSimilarPrefixIsRejected` (`folder.newFolder("backup-tmp2")` y un archivo dentro → `false`).
- [ ] **Step 2: Run to verify it fails.** `cd mobile/android && ./gradlew :app:testDebugUnitTest --tests "com.jesus.boveda.BackupPathsTest"`. Esperado: no compila (`isInsideTempDir` sin definir).
- [ ] **Step 3: Implement** `isInsideTempDir` en `BackupPaths.kt`, con un comentario que explique por qué compara `canonicalFile` y no texto (`..`, enlaces, prefijos parecidos). Agregar la dependencia al Gradle.
- [ ] **Step 4: Run to verify it passes.** El mismo comando. Esperado: 7 pruebas en verde.
- [ ] **Step 5: Commit.** `git add mobile/android/app && git commit -m "feat: comprobar que un temporal de respaldo está dentro de backup-tmp (#44)"`

### Task 2: `runWithoutAutoLock` y tope de 2 minutos

**Files:**
- Modify: `mobile/lib/services/auto_lock.dart`, `mobile/lib/app.dart`
- Test: `mobile/test/auto_lock_test.dart` (grupo `runWithoutAutoLock`)

**Interfaces:**
- Produces:
  - `AutoLockController({DateTime Function() now = DateTime.now})`, `bool get isSuspended`, `static const maxAwayDuringOperation = Duration(minutes: 2)`, `Future<T> runWithoutAutoLock<T>(Future<T> Function() operation)`. Con una operación en curso lanza `StateError` de inmediato. En `finally` baja la marca y llama a `onActivity` (reinicia la cuenta).
  - `AutoLockScope.runWithoutAutoLock<T>(op)` y `static Future<T> run<T>(BuildContext context, Future<T> Function() operation)`: usa el alcance más cercano o, sin alcance, ejecuta `operation` tal cual.
  - `BovedaApp({…, DateTime Function() now = DateTime.now})`, que se pasa al controlador.
- Consumes: `VaultGate._onHidden`, `_restartIdle`, `_lock`, `_lifecycle` (`app.dart`).

- [ ] **Step 1: Write the failing tests** en `auto_lock_test.dart` (obtener el alcance con `AutoLockScope.maybeOf(tester.element(find.byType(SearchScreen)))!`; extender `pumpApp` con `now`). Nombres y aserciones:
  - `ocultarse con la operación en curso no bloquea`: `hidden` con `Completer` pendiente → `lockCalls == 0`.
  - `sin inactividad mientras corre y la cuenta reinicia al terminar`: 6 min con la operación pendiente → `lockCalls == 0`; se completa; a los 4:59 sigue en 0 y a los 5:00 es 1.
  - `una operación que lanza baja la marca`: tras el error, `isSuspended == false` y el temporizador corre.
  - `una segunda llamada lanza StateError y no anida`.
  - `tope por temporizador`: `hidden` con la operación pendiente; `pump(2 min)` → `lockCalls == 1`.
  - `tope por hora al volver`: `hidden`; la hora inyectada avanza 3 min sin disparar el `Timer`; `resumed` → `lockCalls == 1`.
  - `volver antes del tope no bloquea`: `hidden`, 1 min, `resumed`, `pump(3 min)` → `lockCalls == 0`.
  - `sin bóveda el tope no cambia de estado`: en el primer uso, `hidden` + 3 min con la operación pendiente → sigue `FirstRunScreen` y `lockCalls == 0`.
- [ ] **Step 2: Run** `flutter test test/auto_lock_test.dart`. Esperado: FAIL (símbolos sin definir).
- [ ] **Step 3: Implement.** Controlador y alcance como en Interfaces. En `VaultGate`: con `isSuspended`, `_onHidden` no bloquea y, solo en `unlocked`/`offerBiometric`, guarda `_hiddenAt = widget.autoLock.now()` y arma `_awayCap = Timer(maxAwayDuringOperation, _lockIfShowingVault)`; `AppLifecycleListener(onResume:)` cancela el `Timer` y, si `now - _hiddenAt` supera el tope, llama a `_lockIfShowingVault()`; `_restartIdle` no arma temporizador mientras `isSuspended`; `dispose` cancela `_awayCap`. `_lockIfShowingVault` llama a `_lock()` solo en `unlocked`/`offerBiometric`.
- [ ] **Step 4: Run** `flutter test test/auto_lock_test.dart`. Esperado: PASS, incluidas las de la #39.
- [ ] **Step 5: Commit.** `git commit -m "feat: runWithoutAutoLock y tope de 2 minutos fuera de la app (#44)"`

### Task 3: servicio `BackupFiles` y dobles de prueba

**Files:**
- Create: `mobile/lib/services/backup_files.dart`
- Modify: `mobile/lib/app.dart` (`BovedaApp.files`, `VaultGate.files`), `mobile/lib/main.dart`, `mobile/test/fakes.dart`, `mobile/test/app_test.dart`, `mobile/test/auto_lock_test.dart` (pasar `files: FakeBackupFiles()`)
- Test: `mobile/test/backup_files_test.dart`

**Interfaces:**
- Produces:
  - `enum BackupFailure { failed, busy, tooLarge, invalidPath }`; `class BackupFilesException implements Exception { const BackupFilesException(this.failure); final BackupFailure failure; }`
  - `abstract class BackupFiles { Future<String> newTempPath(); Future<bool> saveTo(String tempPath, String name); Future<String?> pickToTemp(); Future<void> deleteTemp(String tempPath); }`
  - `class PlatformBackupFiles implements BackupFiles` (`const`, canal `boveda/backup`; `saveTo` envía `{'temp': tempPath, 'name': name}`; `deleteTemp` envía la ruta).
  - `String backupFileName(DateTime date)` → `boveda-AAAA-MM-DD.db`.
  - `FakeBackupFiles` en `fakes.dart`: `List<String> calls` (`newTempPath`, `saveTo`, `pickToTemp`, `deleteTemp`), `List<String> deleted`, `String tempPath = '/tmp/backup-1.bak'`, `String? savedName`, y `onSaveTo`, `onPickToTemp`, `onDeleteTemp` sobrescribibles (por omisión: `true`, `'/tmp/backup-1.bak'`, nada).
  - `FakeVaultSession.onExportBackup(String dest)` y `onImportBackup(String backup, String password)` (por omisión devuelven `emptyReport`); las llamadas quedan en `exportCalls` e `importCalls`. Reemplazan los `UnimplementedError`.

- [ ] **Step 1: Write the failing tests** (`backup_files_test.dart`, canal simulado): `newTempPath devuelve la ruta`; `saveTo envía temp y name y devuelve true o false`; `pickToTemp devuelve null al cancelar`; `un PlatformException con code tooLarge o invalidPath o busy se traduce a su BackupFailure`; `un código desconocido y MissingPluginException dan failed`; `backupFileName(DateTime(2026, 11, 4)) == 'boveda-2026-11-04.db'`.
- [ ] **Step 2: Run** `flutter test test/backup_files_test.dart`. Esperado: FAIL.
- [ ] **Step 3: Implement** el servicio (traducción de errores como `PlatformBiometricService._invoke`, sin texto), los dobles y el cableado: `BovedaApp` y `VaultGate` reciben `required BackupFiles files`; `main.dart` pasa `const PlatformBackupFiles()`; las pruebas existentes pasan `FakeBackupFiles()`.
- [ ] **Step 4: Run** `flutter test`. Esperado: todo PASS.
- [ ] **Step 5: Commit.** `git commit -m "feat: servicio BackupFiles y sus dobles de prueba (#44)"`

### Task 4: canal de Kotlin `BackupFiles.kt`

**Files:**
- Create: `mobile/android/app/src/main/kotlin/com/jesus/boveda/BackupFiles.kt`
- Modify: `mobile/android/app/src/main/kotlin/com/jesus/boveda/MainActivity.kt`

**Interfaces:**
- Produces: `class BackupFiles(private val activity: Activity)` con `fun handle(call: MethodCall, result: MethodChannel.Result)`, `fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?): Boolean` (`true` si era suyo) y `fun cleanTempDir()`. Canal `boveda/backup` con `newTempPath`, `saveTo`, `pickToTemp`, `deleteTemp` como en el diseño.
- Consumes: `isInsideTempDir` (Tarea 1); contrato de la Tarea 3 (argumentos y códigos).

- [ ] **Step 1: Implement** según la sección "Kotlin" del diseño. Decisiones fijas: una sola petición pendiente (otra responde `busy`); `saveTo` y `deleteTemp` validan con `isInsideTempDir(tempDir, File(path))` y responden `invalidPath` sin tocar nada; `saveTo` usa `ACTION_CREATE_DOCUMENT` (`CATEGORY_OPENABLE`, `octet-stream`, `EXTRA_TITLE`) y `pickToTemp` usa `ACTION_OPEN_DOCUMENT` (`CATEGORY_OPENABLE`, `*/*`); la copia corre en un `Executor` de un hilo y responde en el hilo principal; `saveTo` borra el temporal en todos los resultados salvo `busy` e `invalidPath` (éxito, cancelar, fallo) y, si falla a medias, intenta `DocumentsContract.deleteDocument` del destino; `pickToTemp` cuenta bytes con un búfer de 64 KiB y, al pasar de 20 MiB o fallar, borra el parcial y responde `tooLarge` o `failed`; cancelar responde `false` o `null`, no error; un `Reply` de una sola respuesta como el de `BiometricVault`; sin logs. `MainActivity`: `private val backup = BackupFiles(this)`, canal `boveda/backup` → `backup::handle`, `onCreate` llama a `backup.cleanTempDir()` antes de `super.onCreate`, y `override fun onActivityResult(...)` envía a `super` lo que `backup.onActivityResult` no tome.
- [ ] **Step 2: Compile.** `cd mobile/android && ./gradlew :app:compileDebugKotlin`. Esperado: BUILD SUCCESSFUL. Si falla por artefactos de Flutter ausentes, `flutter build apk --debug` desde `mobile/` (avisar antes: compila Rust y usa recursos).
- [ ] **Step 3: Run** `./gradlew :app:testDebugUnitTest` (la prueba de la Tarea 1 sigue en verde) y `./gradlew --stop`.
- [ ] **Step 4: Commit.** `git commit -m "feat: canal de Kotlin para exportar e importar con el selector (#44)"`

### Task 5: Ajustes con exportar

**Files:**
- Create: `mobile/lib/screens/settings_screen.dart`
- Modify: `mobile/lib/screens/search_screen.dart`, `mobile/lib/screens/unlock_screen.dart`, `mobile/lib/app.dart`
- Test: `mobile/test/settings_screen_test.dart`, `mobile/test/backup_gate_test.dart` (nuevo)

**Interfaces:**
- Produces:
  - `typedef NoticeCallback = void Function(String text, {bool isError});`
  - `SettingsScreen({super.key, required VaultSession session, required BackupFiles files, required BiometricService biometric, required ValueChanged<int> onImported, required ValueChanged<String> onImportFailed, required NoticeCallback onNotice})`. Dos `ListTile` con `Key('settings-export')` y `Key('settings-import')`, desactivados mientras corre un flujo (`_busy`).
  - `SearchScreen.onOpenSettings` (`VoidCallback?`; si no es `null`, `IconButton` con `Key('open-settings')`, `tooltip: 'Ajustes'`, a la derecha del campo de búsqueda).
  - `class UnlockNotice { const UnlockNotice(this.text, {this.isError = true}); }` y `UnlockScreen({…, UnlockNotice? notice, VoidCallback? onNoticeShown})`: lo muestra encima del campo (rojo si `isError`, neutro si no) y llama a `onNoticeShown` una vez mostrado (un solo uso).
  - En `VaultGate`: `_notice`, `_showNotice(String text, {bool isError})` (en `locked` lo guarda; en otro estado, `SnackBar`), `_openSettings()` (apila `SettingsScreen` en el `Navigator` raíz), `_lock({UnlockNotice? notice})`. `_enterUnlocked` borra `_notice`.
- Consumes: `FakeBackupFiles`, `backupFileName`, `AutoLockScope.run` (Tareas 2 y 3).

- [ ] **Step 1: Write the failing tests.** `settings_screen_test.dart` (pantalla suelta con dobles): `exportar crea el temporal, exporta a él, guarda con el nombre del día y lo borra` (`files.calls == ['newTempPath','saveTo','deleteTemp']`, `session.exportCalls.single == files.tempPath`, `files.savedName` coincide con `^boveda-\d{4}-\d{2}-\d{2}\.db$`, aviso "Respaldo guardado"); `cancelar el selector no avisa y borra el temporal`; `exportBackup que falla (locked) avisa "No se pudo guardar el respaldo", no llama a saveTo y borra el temporal`; `saveTo que lanza BackupFilesException avisa y borra el temporal`; `doble toque: un solo flujo` (con `saveTo` pendiente, segundo toque no repite `newTempPath` y ambos tiles quedan desactivados). `backup_gate_test.dart` (con `BovedaApp`): `el engranaje abre Ajustes`; `exportar con la bóveda ya bloqueada al volver: el archivo se guarda y el aviso sale una sola vez en el desbloqueo` (`hidden` + 2 min con `saveTo` pendiente; completar con `true`; "Respaldo guardado" visible en el desbloqueo y, tras desbloquear y volver a bloquear, ausente).
- [ ] **Step 2: Run** `flutter test test/settings_screen_test.dart test/backup_gate_test.dart`. Esperado: FAIL.
- [ ] **Step 3: Implement.** Flujo de exportar (`_export`) como en el diseño: `finally` con `deleteTemp` sin propagar errores; el selector va con `AutoLockScope.run(context, …)`; el resultado se anuncia con `onNotice` si la pantalla ya no está activa (`!context.mounted || ModalRoute.of(context)?.isActive == false`) y con `SnackBar` si sigue activa.
- [ ] **Step 4: Run** `flutter test`. Esperado: PASS.
- [ ] **Step 5: Commit.** `git commit -m "feat: ajustes con exportar el respaldo (#44)"`

### Task 6: importar desde Ajustes

**Files:**
- Create: `mobile/lib/screens/restore_dialog.dart`
- Modify: `mobile/lib/screens/settings_screen.dart`, `mobile/lib/app.dart`
- Test: `mobile/test/backup_gate_test.dart`

**Interfaces:**
- Produces (`restore_dialog.dart`):
  - `sealed class RestoreResult`; `RestoreImported(int damagedEntries)`, `RestoreImportFailed(String message)` (la importación falló: la bóveda quedó bloqueada en Rust), `RestorePickFailed(String message)` (no se llegó a importar: la bóveda sigue como estaba).
  - `String restoreErrorMessage(VaultError error)`: `wrongPassword` → "Contraseña del respaldo incorrecta."; `notAVault` → "El archivo no es una bóveda."; `unsupportedFormatVersion` → "Este respaldo es de una versión que la app no admite."; `storageCorrupt` → "El archivo del respaldo está dañado."; otro → "No se pudo importar el respaldo."
  - `Future<RestoreResult?> runRestore(BuildContext context, {required VaultSession session, required BackupFiles files, required BiometricService biometric, required bool replacesVault, required bool closeOnError})`: (1) `pickToTemp` con `AutoLockScope.run`; `null` → `null`; `BackupFilesException` → `RestorePickFailed` (`tooLarge` → "El archivo es demasiado grande para ser un respaldo.", otro → "No se pudo leer el archivo."); (2) si la pantalla ya no está activa, borra el temporal y devuelve `null` sin importar; (3) `showDialog<RestoreResult>` con `RestoreDialog`; (4) `finally` `deleteTemp` sin propagar errores, siempre antes de devolver; (5) `RestoreImported` → `biometric.disable()` de mejor esfuerzo.
  - `RestoreDialog({session, backupPath, replacesVault, closeOnError})`: `AlertDialog` con título "Importar respaldo", la advertencia "Reemplaza la bóveda actual." solo con `replacesVault`, `PasswordField` "Contraseña del respaldo", botones "Cancelar" e "Importar" (`Key('restore-submit')`). Con la importación en curso no se puede cerrar (`barrierDismissible: false` y `PopScope(canPop: !_busy)`). Con éxito hace `pop(RestoreImported)`; con error, `pop(RestoreImportFailed)` si `closeOnError` y, si no, muestra `restoreErrorMessage` en el diálogo y deja reintentar.
  - `VaultGate`: `_onImported(int damaged)` (quita lo apilado con `_removeStackedRoutes`, sube `_epoch` y sigue el camino de `_afterCreate`, ahora `_afterCreate({int damaged = 0})`; el `onDone` de la oferta pasa `_damagedEntries` para no perder el total) y `_onImportFailed(String message)` → `_lock(notice: UnlockNotice(message))`.
- Consumes: `SettingsScreen` y `UnlockNotice` (Tarea 5).

- [ ] **Step 1: Write the failing tests** (`backup_gate_test.dart`, con Ajustes abierto): `importar feliz` (`importCalls.single` = ruta del temporal y la contraseña escrita; `deleted` contiene el temporal; `biometric.calls` contiene `disable` y, con huella disponible y no activa, termina en `BiometricOfferScreen`, y sin huella en `SearchScreen`); `el aviso de dañadas conserva el total tras importar` (reporte con 2 dañadas → "2 entradas no se pudieron leer" en la principal, también tras la oferta de la huella); parametrizada `importación fallida desde Ajustes` con `wrongPassword`, `notAVault` y `unsupportedFormatVersion`: termina en `UnlockScreen` con el texto de `restoreErrorMessage` visible y sin ruta en pantalla, y `lockCalls` valía 0 en el momento de `deleteTemp` (el borrado va antes del bloqueo); `cerrar el diálogo con la importación en curso no es posible` (atrás y tocar fuera con `importBackup` pendiente: el diálogo sigue; al completar, sale); `si el tope vence mientras se elige el archivo no se importa` (`hidden` + 2 min con `pickToTemp` pendiente; al completar: `importCalls` vacío y temporal borrado); `un archivo demasiado grande avisa sin bloquear` (`pickToTemp` lanza `tooLarge` → "El archivo es demasiado grande para ser un respaldo.", `lockCalls == 0`, sigue en Ajustes); `un respaldo vacío o de texto da notAVault` (cubierto por el caso parametrizado con `onImportBackup` lanzando `notAVault`).
- [ ] **Step 2: Run** `flutter test test/backup_gate_test.dart`. Esperado: FAIL.
- [ ] **Step 3: Implement** lo de Interfaces; "Importar respaldo" de `SettingsScreen` llama a `runRestore(replacesVault: true, closeOnError: true)` y enruta el resultado: `RestoreImported` → `onImported`, `RestoreImportFailed` → `onImportFailed`, `RestorePickFailed` → `SnackBar`, `null` → nada.
- [ ] **Step 4: Run** `flutter test`. Esperado: PASS.
- [ ] **Step 5: Commit.** `git commit -m "feat: importar el respaldo desde ajustes y bloquear si falla (#44)"`

### Task 7: restaurar desde el primer uso

**Files:**
- Modify: `mobile/lib/screens/first_run_screen.dart`, `mobile/lib/app.dart`
- Test: `mobile/test/first_run_screen_test.dart`, `mobile/test/backup_gate_test.dart`

**Interfaces:**
- Produces: `FirstRunScreen({…, required BackupFiles files, required ValueChanged<int> onRestored})`; botón `Key('first-run-restore')` "Restaurar un respaldo" bajo "Crear bóveda", desactivado mientras se crea o se restaura. Usa `runRestore(replacesVault: false, closeOnError: false)`. `VaultGate` pasa `onRestored: _onImported`.
- Consumes: `runRestore` (Tarea 6).

- [ ] **Step 1: Write the failing tests:** `restaurar con éxito llama a disable y llega a la oferta o a la principal con el total de dañadas`; `un error queda en el diálogo, sin bloqueo (lockCalls == 0), y reintentar con otra contraseña funciona`; `cancelar el selector no cambia nada y no llama a importBackup`; `el botón se desactiva mientras se crea la bóveda`; `no aparece la advertencia de reemplazo`.
- [ ] **Step 2: Run** `flutter test test/first_run_screen_test.dart test/backup_gate_test.dart`. Esperado: FAIL.
- [ ] **Step 3: Implement** el botón y el cableado; las pantallas que construyen `FirstRunScreen` en pruebas pasan `files: FakeBackupFiles()`.
- [ ] **Step 4: Run** `flutter test`. Esperado: PASS.
- [ ] **Step 5: Commit.** `git commit -m "feat: restaurar un respaldo desde el primer uso (#44)"`

### Task 8: huella desactivada por `WrongVaultKey` sin alarma

**Files:**
- Modify: `mobile/lib/services/biometric_unlock.dart`, `mobile/lib/screens/unlock_screen.dart`
- Test: `mobile/test/unlock_screen_test.dart`, `mobile/test/biometric_unlock_test.dart`

**Interfaces:**
- Produces: `BiometricDisabled({this.vaultChanged = false})`; `unlockWithBiometric` devuelve `BiometricDisabled(vaultChanged: true)` solo con `VaultError.wrongVaultKey` (la llave invalidada y la copia ilegible siguen con `false`). `UnlockScreen`, con `vaultChanged`, no muestra el aviso en rojo; tras desbloquear con la contraseña muestra un `SnackBar` neutro: "La huella se desactivó porque la bóveda cambió. Puedes activarla de nuevo al desbloquear."

- [ ] **Step 1: Write the failing tests:** en `biometric_unlock_test.dart`, `wrongVaultKey devuelve BiometricDisabled(vaultChanged: true) y llama a disable`, y `invalidated sigue con vaultChanged false`; en `unlock_screen_test.dart`, `con wrongVaultKey de la huella no hay texto de error en la pantalla, pide la contraseña y, tras desbloquear, aparece el SnackBar neutro` (usar `FakeVaultSession.onUnlockWithKey` que lanza `VaultError.wrongVaultKey`). Actualizar las aserciones de la #38 que esperaban el aviso en rojo en el camino `wrongVaultKey`.
- [ ] **Step 2: Run** `flutter test test/unlock_screen_test.dart test/biometric_unlock_test.dart`. Esperado: FAIL.
- [ ] **Step 3: Implement.** El `SnackBar` se muestra con el `ScaffoldMessenger` capturado antes del primer `await` en `_unlock`, como el aviso de la activación fallida.
- [ ] **Step 4: Run** `flutter test`. Esperado: PASS.
- [ ] **Step 5: Commit.** `git commit -m "feat: aviso neutro cuando la huella se desactiva porque la bóveda cambió (#44)"`

### Task 9: documentación

**Files:**
- Modify: `docs/especificacion.md`, `ARCHITECTURE.md`, `mobile/CLAUDE.md`

- [ ] **Step 1: Escribir** según la sección "Documentación" del diseño: en `especificacion.md`, reemplazar la viñeta "Selector de archivos y otras actividades" por el mecanismo decidido y agregar "Decidido el 2026-11-14 (#44)" en "Respaldos" (canal, tope de 20 MiB, nombre, temporal, opción (a) con referencia a #69, huella tras `WrongVaultKey`); en `ARCHITECTURE.md`, los archivos nuevos en el mapa y fuera "futuro: ajustes" y "futuro: selector"; en `mobile/CLAUDE.md`, las tres reglas del diseño.
- [ ] **Step 2: Verify.** `git diff --stat` solo muestra esos tres archivos.
- [ ] **Step 3: Commit.** `git commit -m "docs: respaldos en la especificación, la arquitectura y mobile/CLAUDE.md (#44)"`

### Task 10: verificación final y prueba a mano

- [ ] **Step 1: Pruebas y lint**, con un script que imprima los códigos de salida de cada comando: `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`, `flutter analyze`, `flutter test` (desde `mobile/`) y `./gradlew :app:testDebugUnitTest`. Esperado: todos `0`.
- [ ] **Step 2: Avisar al usuario** y, con su visto bueno, correr la tabla "A mano, en el emulador" del diseño (pruebas 1 a 12). Anotar lo observado en el diseño ("Hallazgos de la implementación y de la prueba a mano") y commit `docs: anotar la prueba a mano de la #44`.
- [ ] **Step 3: Cerrar** emulador, `adb` y Gradle (`./gradlew --stop`).
- [ ] **Step 4: Handoff.** La revisión final independiente y el PR contra `dev` (formato del PR #11, `Closes #44`, enlace a #69) siguen el flujo acordado y no forman parte de este plan.
