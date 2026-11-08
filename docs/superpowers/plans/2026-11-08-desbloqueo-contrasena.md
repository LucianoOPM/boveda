# Desbloqueo con contraseña maestra — plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Con la bóveda existente y bloqueada, abrir en una pantalla de desbloqueo que desbloquea con la contraseña maestra, muestra "Contraseña incorrecta" o un mensaje genérico, avisa de las entradas dañadas y pasa al marcador de la pantalla principal.

**Architecture:** `VaultGate` muestra `UnlockScreen` en el estado `locked`; la pantalla llama a `session.unlock` (asíncrona, sin `#[frb(sync)]`) y devuelve el total de dañadas con `onUnlocked(int)`. `VaultGate` guarda ese número y lo pasa a `DamagedEntriesBanner`, que vive dentro del marcador de la pantalla principal. Sin cambios en `rust/src/api/`: no se regenera el puente.

**Tech Stack:** Flutter 3.47 (Material 3), flutter_test, flutter_rust_bridge 2.13.0; Rust (pruebas de la fachada con `rusqlite` 0.41).

**Spec:** `docs/superpowers/specs/2026-11-08-desbloqueo-contrasena-design.md`

## Global Constraints

- Textos exactos: "Bóveda bloqueada", "Contraseña maestra", "Desbloquear", "Desbloqueando…", "Contraseña incorrecta", "No se pudo desbloquear la bóveda. Intenta de nuevo.", "1 entrada no se pudo leer", "N entradas no se pudieron leer" (N es el número), "Entendido", "Pantalla principal: #40".
- El mensaje genérico nunca interpola el error (ni variante, ni `toString`, ni rutas).
- La contraseña solo vive en el controlador y en una variable local de `_unlock`; nunca en un campo del `State` ni fuera de la pantalla.
- Después de cada `await`, comprobar `mounted` antes de `setState` o de llamar a `onUnlocked`.
- Sin `PopScope` en `UnlockScreen`. Sin "¿Olvidaste tu contraseña?" ni acceso a importar.
- `FirstRunScreen` no cambia.
- El banner va en el árbol, nunca con `ScaffoldMessenger.showMaterialBanner`.
- Única dependencia nueva: `rusqlite = { version = "0.41", features = ["bundled"] }` en `[dev-dependencies]` de `mobile/rust` (aprobada). Ningún crate nuevo en `Cargo.lock`.
- Nunca se modifica `core/tests/fixtures/vault-v2.db`: solo la copia de `test_support::fixture_copy()`.
- Identificadores en inglés; comentarios, documentación y commits en español (Conventional Commits, con `(#37)` al final).
- Nunca encadenar `clippy` con `| grep` antes de un commit.

## Review Focus

1. Un intento fallido seguido de uno correcto debe desbloquear: el reintento no puede quedar trabado por `_unlocking` (prueba en la Task 4).
2. La contraseña se envía sin tocar: con "ó" y espacios incluidos (`prueba-bóveda-v2`, y una de solo espacios cuenta como no vacía) (prueba en la Task 4).
3. Una excepción cuyo texto trae una ruta no debe aparecer en la pantalla (prueba en la Task 4).
4. Un reporte con solo filas dañadas sin id (`corrupt` vacío, `unidentifiedCorrupt: 1`) debe mostrar el singular (prueba en la Task 3).
5. Tras "Contraseña incorrecta", el botón queda desactivado (campo vacío) y "Hecho" no lanza otro `unlock` (prueba en la Task 4).

---

### Task 1: Prueba de la fachada con una entrada dañada

**Files:**
- Modify: `mobile/rust/Cargo.toml` (`[dev-dependencies]`)
- Test: `mobile/rust/src/api/session/tests.rs`

**Interfaces:**
- Consumes: `test_support::{fixture_copy, path_string, FIXTURE_PASSWORD}`, `VaultSession::{new, open, unlock, search}`, `UnlockReport { corrupt, unidentified_corrupt }`, `EntrySummary { id, service, .. }`.
- Produces: nada para otras tareas.

- [ ] **Step 1: Agregar `rusqlite = { version = "0.41", features = ["bundled"] }` a `[dev-dependencies]`** con un comentario: misma versión y características que `vault-core`, solo para alterar la copia de la bóveda de ejemplo en las pruebas.

- [ ] **Step 2: Escribir `unlock_reports_corrupt_entry_and_rest_still_opens`**

  1. `fixture_copy()`; con una sesión aparte (abrir, desbloquear, `search("")`) obtener los ids y servicios de las tres vivas; soltar esa sesión.
  2. Confirmar que hay al menos dos vivas (`assert!(len >= 2)`).
  3. Elegir la de servicio `Banco`; convertir su id hexadecimal a bytes y, con `rusqlite::Connection::open(&path)`, leer `search`, `blob[30] ^= 1`, escribirlo (como `corrupt_entry` en `core/tests/vault.rs`). Cerrar la conexión.
  4. Sesión nueva: `open()`, `unlock(FIXTURE_PASSWORD.into())`.

  Aserciones:

  ```rust
  assert_eq!(report, Ok(UnlockReport { corrupt: vec![banco_id.clone()], unidentified_corrupt: 0 }));
  let services: Vec<String> = session.search(String::new()).unwrap().into_iter().map(|e| e.service).collect();
  assert_eq!(services.len(), 2);
  assert!(services.contains(&"Correo Electrónico".to_owned()));
  assert!(services.contains(&"Nube".to_owned()));
  ```

  La conversión hexadecimal → bytes va en la prueba, con un ayudante local de pocas líneas (la de la fachada es privada).

- [ ] **Step 3: Ejecutar**

  Run: `cargo test -p rust_lib_boveda unlock_reports_corrupt_entry` (desde la raíz)
  Expected: PASS. Comprobar que la prueba es capaz de fallar: comentar temporalmente la línea `blob[30] ^= 1` → debe fallar en el `assert_eq!` del reporte; restaurarla.

- [ ] **Step 4: `cargo clippy --workspace --all-targets -- -D warnings` y `cargo fmt --all`**; `git diff Cargo.lock` no muestra paquetes nuevos.

- [ ] **Step 5: Commit** — `test: UnlockReport de la fachada con una entrada dañada (#37)`

### Task 2: Parámetros nuevos de `PasswordField`

**Files:**
- Modify: `mobile/lib/widgets/password_field.dart`
- Test: `mobile/test/widgets_test.dart`

**Interfaces:**
- Produces: `PasswordField({..., TextInputAction? textInputAction, ValueChanged<String>? onSubmitted, bool autofocus = false, FocusNode? focusNode})`, pasados tal cual al `TextField`.

- [ ] **Step 1: Pruebas**
  - `PasswordField pasa textInputAction, onSubmitted, autofocus y focusNode`: con `TextInputAction.done`, un `FocusNode` y `autofocus: true`, `textField(tester)` tiene esos valores; `tester.testTextInput.receiveAction(TextInputAction.done)` tras escribir "abc" llama a `onSubmitted` con "abc".
  - `PasswordField sin los parámetros nuevos conserva los valores por omisión`: `autofocus == false`, `onSubmitted == null`, `focusNode == null`, `textInputAction == null`.
- [ ] **Step 2: `flutter test test/widgets_test.dart`** → la primera falla (parámetros inexistentes).
- [ ] **Step 3: Implementar** los cuatro parámetros y documentarlos en el comentario de la clase.
- [ ] **Step 4: `flutter test` y `flutter analyze`** → todo pasa (incluidas las pruebas de `FirstRunScreen`, que no cambian).
- [ ] **Step 5: Commit** — `feat: acción de envío, foco y autofocus en PasswordField (#37)`

### Task 3: Aviso de entradas dañadas

**Files:**
- Create: `mobile/lib/widgets/damaged_entries_banner.dart`
- Modify: `mobile/lib/screens/placeholder_screen.dart`
- Test: `mobile/test/widgets_test.dart`

**Interfaces:**
- Produces: `DamagedEntriesBanner({Key? key, required int count})` (`StatefulWidget`); `PlaceholderScreen({required String text, Widget? banner})`, con `banner` arriba del texto (en una `Column`, el texto centrado en el resto).

- [ ] **Step 1: Pruebas** (montadas dentro de un `Scaffold`):
  - con `count: 0` no hay `MaterialBanner`;
  - con `count: 1`, "1 entrada no se pudo leer";
  - con `count: 3`, "3 entradas no se pudieron leer";
  - "Entendido" lo oculta (`find.byType(MaterialBanner)` → nada después de `pumpAndSettle`);
  - `PlaceholderScreen` con `banner` muestra el banner y el texto; sin `banner`, solo el texto.
- [ ] **Step 2: `flutter test test/widgets_test.dart`** → fallan.
- [ ] **Step 3: Implementar.** Comentario en la clase: el estado de oculto vive en el widget porque entre dos desbloqueos siempre se pasa por `locked` y el banner sale del árbol; la #40 debe mantenerlo en una posición que salga del árbol al bloquear. No usa `ScaffoldMessenger` (lo mostraría en todos los `Scaffold`).
- [ ] **Step 4: `flutter test` y `flutter analyze`** → pasan.
- [ ] **Step 5: Commit** — `feat: aviso de entradas dañadas (#37)`

### Task 4: Pantalla de desbloqueo

**Files:**
- Create: `mobile/lib/screens/unlock_screen.dart`, `mobile/test/unlock_screen_test.dart`
- Modify: `mobile/test/fakes.dart`

**Interfaces:**
- Consumes: `PasswordField` (Task 2).
- Produces: `UnlockScreen({Key? key, required VaultSession session, required ValueChanged<int> onUnlocked})`. Claves: `Key('unlock-password')` en el `PasswordField`, `Key('unlock-submit')` en el botón. En `FakeVaultSession`: `final List<String> unlockCalls`, `Future<UnlockReport> Function(String password) onUnlock` (por omisión, reporte vacío), y `unlock` registra y delega.
- Ayudante en `fakes.dart`: `const emptyReport = UnlockReport(corrupt: [], unidentifiedCorrupt: 0);`

- [ ] **Step 1: `unlock` en `FakeVaultSession`** (actualizar el comentario de la clase: `create`, `open` y `unlock` hacen algo).

- [ ] **Step 2: Pruebas en `unlock_screen_test.dart`**, con `pumpScreen(tester, session, {ValueChanged<int>? onUnlocked})` que monta `MaterialApp(home: UnlockScreen(...))`:
  - `botón desactivado con el campo vacío y activo al escribir`.
  - `al entrar el campo tiene el foco`: `textField.focusNode!.hasFocus` (o `FocusManager.instance.primaryFocus` dentro del campo).
  - `durante el desbloqueo muestra el progreso y desactiva el formulario`: con un `Completer` pendiente, "Desbloqueando…" y `CircularProgressIndicator` visibles, `TextField.enabled == false`, `onPressed == null`, `unlockCalls == ['prueba-bóveda-v2']`.
  - `éxito llama a onUnlocked con el total de dañadas`: `UnlockReport(corrupt: ['a'*32, 'b'*32], unidentifiedCorrupt: 1)` → `3`.
  - `alreadyUnlocked llama a onUnlocked con 0`.
  - `contraseña incorrecta: mensaje, campo vacío, sigue bloqueada y con foco`: "Contraseña incorrecta"; `controller.text == ''`; `onUnlocked` no llamado; el campo tiene el foco tras `pumpAndSettle`.
  - `otros errores muestran el genérico y vacían el campo`: con `VaultError.storageIo` y con `Exception('/data/ruta/interna')`; "No se pudo desbloquear la bóveda. Intenta de nuevo." visible y `find.textContaining('/data')` sin resultados (Review Focus 3).
  - `el error se borra al volver a escribir`.
  - `reintento tras un fallo desbloquea` (Review Focus 1): primero `wrongPassword`, luego éxito → `onUnlocked` llamado una vez, `unlockCalls.length == 2`.
  - `la contraseña se envía sin modificar` (Review Focus 2): `'   '` (solo espacios) se envía tal cual.
  - `doble envío con Hecho no lanza dos unlock`: `Completer` pendiente; `receiveAction(done)` dos veces → `unlockCalls.length == 1`.
  - `botón y Hecho seguidos no lanzan dos unlock`: tocar el botón y luego `receiveAction(done)` → 1.
  - `Hecho con el campo vacío no llama a unlock`, también justo después de un `wrongPassword` (Review Focus 5).
  - `al salir el campo queda limpio`: guardar `fieldOf(...).controller`, desbloquear con éxito, reemplazar el árbol (`pumpWidget(const SizedBox())`); `controller.text == ''`.
  - `desmontada con unlock pendiente no hace nada al terminar`: `Completer`; tocar; `pumpWidget(const SizedBox())`; completar con éxito (y en otra prueba, con error); `pump()`; `tester.takeException()` es `null` y `onUnlocked` no se llamó.

- [ ] **Step 3: `flutter test test/unlock_screen_test.dart`** → fallan (no existe la pantalla).

- [ ] **Step 4: Implementar `UnlockScreen`** según la sección "Pantalla de desbloqueo" del diseño: guarda `_unlocking` y campo vacío al principio de `_unlock`; variable local para la contraseña (comentario: de mejor esfuerzo, Dart no borra sus cadenas); `mounted` tras el `await`; al fallar, `_password.clear()` dentro del `setState` que reactiva y pone el error, y `WidgetsBinding.instance.addPostFrameCallback((_) { if (mounted) _focus.requestFocus(); })`; `dispose()` limpia y libera el controlador y libera el `FocusNode`. El botón se activa con `!_unlocking && _password.text.isNotEmpty` (reconstruye en `onChanged`).

- [ ] **Step 5: `flutter test` y `flutter analyze`** → pasan. Comprobar que la guarda importa: quitar temporalmente `if (_unlocking) return;` → las dos pruebas de doble envío fallan; restaurarla.

- [ ] **Step 6: Commit** — `feat: pantalla de desbloqueo con contraseña maestra (#37)`

### Task 5: Desbloqueo en `VaultGate`

**Files:**
- Modify: `mobile/lib/app.dart`
- Test: `mobile/test/app_test.dart`

**Interfaces:**
- Consumes: `UnlockScreen` (Task 4), `DamagedEntriesBanner` y `PlaceholderScreen.banner` (Task 3), `FakeVaultSession.onUnlock` (Task 4).

- [ ] **Step 1: Pruebas en `app_test.dart`**:
  - reemplazar `con bóveda abre el marcador de desbloqueo` por `con bóveda abre UnlockScreen` (`find.byType(UnlockScreen)`, `openCalls == 1`);
  - `desbloquear lleva a la pantalla principal sin banner`;
  - `con 3 dañadas muestra el aviso en plural` y `con 1 dañada (sin id) muestra el singular` (`corrupt: []`, `unidentifiedCorrupt: 1`);
  - `alreadyUnlocked lleva a la pantalla principal sin banner`;
  - en la prueba existente del primer uso que llega a la pantalla principal, agregar `find.byType(MaterialBanner)` → nada.
- [ ] **Step 2: `flutter test test/app_test.dart`** → fallan.
- [ ] **Step 3: Implementar** `_damagedEntries`, `UnlockScreen` en `locked` y `PlaceholderScreen(text: 'Pantalla principal: #40', banner: DamagedEntriesBanner(count: _damagedEntries))` en `unlocked`; `onCreated` pone `_damagedEntries = 0`. Actualizar el comentario de `VaultGate` (la #37 ya agregó su transición).
- [ ] **Step 4: `flutter test` y `flutter analyze`** → pasan.
- [ ] **Step 5: Commit** — `feat: desbloqueo en el widget raíz (#37)`

### Task 6: Documentación y verificación completa

**Files:**
- Modify: `ARCHITECTURE.md`, `docs/especificacion.md`

- [ ] **Step 1: `ARCHITECTURE.md`**: en el mapa, `screens/` lista `unlock_screen.dart` (y deja de anunciar el desbloqueo como futuro), `widgets/` lista `damaged_entries_banner.dart`; la línea de `app.dart` menciona el desbloqueo.
- [ ] **Step 2: `docs/especificacion.md`**, en "Experiencia de uso", tras **Desbloqueo.**: "Decidido el 2026-11-08 (#37, detalle en `docs/superpowers/specs/2026-11-08-desbloqueo-contrasena-design.md`)": sin límite ni espera entre intentos (Argon2id ya cuesta unos 0.5 s y el ataque real es sin conexión); el aviso de dañadas es un banner de una vez por desbloqueo con el total de entradas dañadas con y sin id; los errores distintos de la contraseña incorrecta muestran un mensaje genérico sin detalles, como complemento de "La bóveda → Errores".
- [ ] **Step 3: Verificación** (desde la raíz y desde `mobile/`): `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check`, `flutter analyze`, `flutter test`. Todo pasa.
- [ ] **Step 4: Commit** — `docs: desbloqueo con contraseña en la arquitectura y la especificación (#37)`
- [ ] **Step 5: Prueba a mano en el emulador**, según "A mano, en el emulador" del diseño (pasos 1 a 4, con `force-stop`, `/data/local/tmp`, borrar el `-journal` y confirmar dos o más vivas antes de alterar). Los archivos de la prueba van al directorio temporal de la sesión; nada entra al repositorio. Antes de compilar, `rustup run stable rustc --version` coincide con `rust-toolchain.toml`.
