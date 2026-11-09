# Bloqueo automático: plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** bloquear la bóveda al pasar a segundo plano (`hidden`) y tras 5 minutos de inactividad, sin dejar pantallas de la bóveda en el árbol y sin que una operación asíncrona termine con la bóveda abierta y la app oculta.

**Architecture:** `VaultGate` concentra el bloqueo: un `AppLifecycleListener`, un `Timer` y un contador `_epoch` que remonta las pantallas al bloquear. Toda transición a un estado con Rust desbloqueado pasa por `_enterUnlocked`, que bloquea si la app está oculta. La actividad llega por un `Listener` y un `AutoLockScope` en `MaterialApp.builder`, conectados a `VaultGate` por un `AutoLockController`. Kotlin cancela el diálogo de la huella en `onStop` y la fachada bloquea aunque el `Mutex` esté envenenado.

**Tech Stack:** Flutter/Dart (`flutter_test`, sin paquetes nuevos), Kotlin (API de la plataforma), Rust (fachada `rust_lib_boveda`).

**Spec:** `docs/superpowers/specs/2026-11-08-bloqueo-automatico-design.md`

## Global Constraints

- Sin dependencias nuevas (Dart, Kotlin ni Rust).
- `defaultAutoLockTimeout = Duration(minutes: 5)`, definido solo en `lib/services/auto_lock.dart`.
- Disparador: solo `AppLifecycleState.hidden`. `inactive` nunca bloquea.
- "App oculta" al terminar una operación: `hidden`, `paused` o `detached`. `null` e `inactive` cuentan como presentes.
- Ningún secreto en logs, mensajes ni `toString`. Un error de `lock()` se ignora sin detalles.
- La firma de `VaultSession::lock` no cambia: no se regenera el puente.
- Identificadores en inglés; comentarios, documentación y commits en español, con Conventional Commits y `(#39)`.
- Comandos: desde `mobile/`, `flutter analyze` y `flutter test`; desde la raíz, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` y `cargo fmt --all`.
- En las pruebas que cambian el ciclo de vida: `addTearDown(() => tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.resumed))`, porque el estado del binding sobrevive entre pruebas.

## Review Focus

1. **Ruta con `PopScope(canPop: false)`** (la #41 puede usarla para "descartar cambios"): al bloquear debe salir igual. `popUntil` usa `pop`, que no consulta `PopScope`. Prueba en la Task 4.
2. **Diálogo (`showDialog`) abierto sobre la principal** al bloquear: debe cerrarse, no solo las rutas de página. Prueba en la Task 4.
3. **`hidden` repetido** (ocultar, volver a medias, ocultar) con la app ya bloqueada: `lockCalls` no sube y no hay excepciones. Prueba en la Task 4.
4. **`hidden` en `checking`, `noVault` u `openFailed`**: nada que bloquear, sin `lock()` ni excepciones (el primer uso no pierde lo escrito). Prueba en la Task 4.
5. **Temporizador y teclado después de desmontar `BovedaApp`**: sin temporizadores pendientes (el `testWidgets` falla si quedan) y sin manejador de `HardwareKeyboard` colgado. Prueba en la Task 5.

---

### Task 1: `lock()` de la fachada con el `Mutex` envenenado

**Files:**
- Modify: `mobile/rust/src/api/session.rs:125-130`
- Test: `mobile/rust/src/api/session/tests.rs` (cambia `poisoned_mutex_gives_session_poisoned`)

**Interfaces:**
- Produces: `VaultSession::lock(&self) -> FacadeResult<()>`, misma firma; solo devuelve `Ok(())`.

- [ ] **Step 1: Cambiar la prueba existente.** `poisoned_mutex_gives_session_poisoned` hoy espera `lock() == Err(SessionPoisoned)`. Renombrarla a `poisoned_mutex_still_locks` y reescribirla así: `opened_fixture()`, `session.unlock(FIXTURE_PASSWORD.into()).unwrap()`, envenenar con el mismo `thread::scope`, y después:

```rust
assert_eq!(session.lock(), Ok(()));
let vault = session.vault.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
assert!(vault.as_ref().unwrap().is_locked());
drop(vault);
assert_eq!(session.is_locked(), Err(VaultError::SessionPoisoned));
```

- [ ] **Step 2:** `cargo test -p rust_lib_boveda poisoned` → FAIL (`lock()` da `Err(SessionPoisoned)`).
- [ ] **Step 3:** en `lock()`, `self.vault.lock().unwrap_or_else(PoisonError::into_inner)` en lugar de `self.slot()?`, con el comentario de la spec (bloquear solo suelta la llave; el envenenamiento se conserva). Actualizar el comentario de documentación del método.
- [ ] **Step 4:** `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` y `cargo fmt --all --check` → todo pasa.
- [ ] **Step 5: Commit** `fix: lock bloquea aunque el Mutex de la sesión esté envenenado (#39)`

### Task 2: cancelar el diálogo de la huella en `onStop` (Kotlin)

**Files:**
- Modify: `mobile/android/app/src/main/kotlin/com/jesus/boveda/BiometricVault.kt`
- Modify: `mobile/android/app/src/main/kotlin/com/jesus/boveda/MainActivity.kt`

**Interfaces:**
- Produces: `BiometricVault.cancelPrompt()`. En Dart, el diálogo pendiente responde `canceled` (`BiometricFailure.canceled`), que `unlockWithBiometric` convierte en `BiometricFallback` y `enableBiometric` en `EnableOutcome.canceled`.

Sin pruebas automáticas (como en la #38); se prueba a mano en la Task 8.

- [ ] **Step 1:** reemplazar `private var signal: CancellationSignal?` por `private var active: ActivePrompt?`, con `private class ActivePrompt(val signal: CancellationSignal, val reply: Reply, val onError: () -> Unit)`. `beginEnable` y `unlock` responden `busy` si `active != null`. En `prompt(...)`, crear el `ActivePrompt` y guardarlo; cada camino que hoy hace `signal = null` hace `active = null`.
- [ ] **Step 2:** `fun cancelPrompt()`: si `active` no es `null`, lo toma, pone `active = null`, `signal.cancel()`, `onError()` y `reply.error(CANCELED)`. KDoc: responde de inmediato sin esperar el callback del sistema (si no llegara, la llamada de Dart quedaría colgada); el `Reply` ignora la respuesta duplicada; es una garantía de la app, no de la plataforma.
- [ ] **Step 3:** `onDestroy()` llama a `cancelPrompt()` y después descarta `pendingCipher` como hoy. `MainActivity.onStop()`: `biometric.cancelPrompt()` y `super.onStop()`. Actualizar el KDoc de la clase (diálogo cancelado en `onStop`).
- [ ] **Step 4:** desde `mobile/`, `flutter build apk --debug` → compila sin errores (antes, `rustup run stable rustc --version` igual al `channel` de `rust-toolchain.toml`).
- [ ] **Step 5: Commit** `feat: cancelar el diálogo de la huella al ocultar la app (#39)`

### Task 3: `AutoLockScope`, `AutoLockController` y `ActivityTextField`

**Files:**
- Create: `mobile/lib/services/auto_lock.dart`
- Create: `mobile/lib/widgets/activity_text_field.dart`
- Modify: `mobile/lib/widgets/password_field.dart`
- Test: `mobile/test/widgets_test.dart`

**Interfaces:**
- Produces:
  - `const Duration defaultAutoLockTimeout = Duration(minutes: 5);`
  - `class AutoLockController { VoidCallback? onActivity; void reportActivity(); }`: `reportActivity()` llama a `onActivity?.call()`.
  - `class AutoLockScope extends InheritedWidget { const AutoLockScope({super.key, required AutoLockController controller, required super.child}); final AutoLockController controller; static AutoLockScope? maybeOf(BuildContext context); void reportActivity(); }`. `maybeOf` usa `getInheritedWidgetOfExactType`; `updateShouldNotify` devuelve `false`.
  - `class ActivityTextField extends StatelessWidget`: mismos parámetros que el `TextField` que arma hoy `PasswordField` (`controller`, `enabled`, `onChanged`, `textInputAction`, `onSubmitted`, `focusNode`, `obscureText`, `enableSuggestions`, `autocorrect`, `enableIMEPersonalizedLearning`, `keyboardType`, `maxLines`, `decoration`), con los valores por omisión del `TextField`. Su `onChanged` llama primero a `AutoLockScope.maybeOf(context)?.reportActivity()` y después al recibido.

- [ ] **Step 1: Pruebas** en `widgets_test.dart`, grupo `bloqueo automático`:
  - `defaultAutoLockTimeout es de 5 minutos`: `expect(defaultAutoLockTimeout, const Duration(minutes: 5))`.
  - `AutoLockScope no notifica`: dos `AutoLockScope` con controladores distintos → `updateShouldNotify` da `false`.
  - `ActivityTextField reporta actividad y llama a onChanged`: dentro de `AutoLockScope` con un controlador cuyo `onActivity` cuenta; `enterText('a')` → contador 1 y `onChanged` recibió `'a'`.
  - `ActivityTextField fuera del alcance no lanza`: `enterText` sin `AutoLockScope` → `tester.takeException()` es `null` y `onChanged` se llamó.
  - `PasswordField reporta actividad`: igual que la primera, con `PasswordField`.
- [ ] **Step 2:** `flutter test test/widgets_test.dart` → FAIL (no existen los tipos).
- [ ] **Step 3:** implementar `auto_lock.dart` y `activity_text_field.dart`; `PasswordField` arma `ActivityTextField` en lugar de `TextField`, sin cambiar su API ni su comportamiento.
- [ ] **Step 4:** `flutter test` y `flutter analyze` → pasan (las pruebas de `PasswordField` que ya existen buscan `TextField`: `ActivityTextField` lo contiene, así que siguen pasando).
- [ ] **Step 5: Commit** `feat: campo de texto que reporta actividad (#39)`

### Task 4: bloqueo por ciclo de vida en `VaultGate`

**Files:**
- Modify: `mobile/lib/app.dart`
- Modify: `mobile/lib/screens/unlock_screen.dart` (solo `onVaultOpened`)
- Modify: `mobile/test/fakes.dart`
- Create: `mobile/test/auto_lock_test.dart`

**Interfaces:**
- Consumes: `AutoLockController`, `AutoLockScope`, `defaultAutoLockTimeout` (Task 3).
- Produces:
  - `BovedaApp({..., Duration autoLockTimeout = defaultAutoLockTimeout})`, ahora `StatefulWidget`; crea el `AutoLockController` y lo pasa a `VaultGate` y a `MaterialApp.builder`.
  - `VaultGate({..., required AutoLockController autoLock, required Duration autoLockTimeout})`.
  - `UnlockScreen({..., VoidCallback? onVaultOpened})`: se llama justo después de que `session.unlock` tiene éxito y antes de `enableBiometric`, solo cuando se va a activar la huella con la casilla.
  - `FakeVaultSession`: `int lockCalls`, `Object? lockError`, `bool locked` (inicial `true`; `create`, `unlock` y `unlockWithKey` lo ponen en `false` al terminar con éxito; `lock` lo pone en `true` antes de lanzar `lockError`), `isLocked()` devuelve `locked`, `exportVaultKey()` lanza `VaultError.locked` si `locked`.

Reglas de `VaultGate` (detalle en la spec, "Componentes → VaultGate"):
- `_lock()`: cancela el temporizador (Task 5) y `_openWhileLocked = false`; `session.lock()` sin `await` en la interfaz, con `catchError` que lo ignora; `Navigator.of(context).popUntil((r) => r.isFirst)`; `ScaffoldMessenger.of(context).clearSnackBars()`; `setState` con `_epoch++` y `locked`.
- `_onHidden()`: `_lock()` en `unlocked`, `offerBiometric` o `locked` con `_openWhileLocked`; en cualquier otro caso, nada.
- `_enterUnlocked(VaultGateState next, {int damagedEntries = 0})`: `_openWhileLocked = false`; si `_isAway` (`hidden`, `paused`, `detached`), `_lock()`; si no, cambia de estado. La usan `onUnlocked`, `_afterCreate` (en sus dos salidas) y el `onDone` de la oferta.
- Cada pantalla del `switch` de `build` lleva `key: ValueKey(_epoch)`.

- [ ] **Step 1: Doble.** Ampliar `FakeVaultSession` como dice Interfaces. Correr `flutter test` → todo pasa (las pruebas actuales no usan `lock`, `isLocked` ni `exportVaultKey` bloqueada; si alguna llama a `exportVaultKey` sin desbloquear antes, ajustar su preparación).
- [ ] **Step 2: Pruebas** en `auto_lock_test.dart` con `BovedaApp` y los dobles. Ayudantes: `pumpUnlocked(tester)` (bóveda existente, desbloquea con contraseña), `pumpOffer(tester)` (sin bóveda, huella disponible, crea), `setLifecycle(tester, state)`. Casos, numerados como en la spec:
  - `1 unlocked y hidden bloquea`: `lockCalls == 1`, `find.byType(UnlockScreen)` uno.
  - `2 inactive no bloquea`: `lockCalls == 0`, sigue la principal.
  - `3 offerBiometric y hidden bloquea`.
  - `4 lock que lanza igual muestra el desbloqueo`: `lockError = StateError('x')`.
  - `5 cierra rutas y SnackBar`: desde la principal, `Navigator.of(context).push` de una ruta con texto `'detalle'` y un `SnackBar('copiado')`; `hidden` → ambos `findsNothing`.
  - `5b cierra un diálogo` (`showDialog`) y `5c cierra una ruta con PopScope(canPop: false)` (Review Focus 1 y 2).
  - `6 carrera: unlock pendiente y hidden no llama a lock`; al completar el `Completer` con el ciclo de vida en `hidden`, `paused` y `detached` (un `testWidgets` por estado) → `lockCalls == 1` y `UnlockScreen`. `6 vuelta a resumed antes de completar` → principal, `lockCalls == 0`.
  - `6b huella: unlockWithKey pendiente y hidden` → al completar, `lockCalls == 1` y `UnlockScreen`.
  - `6c alreadyUnlocked con hidden` (`onUnlock` lanza `VaultError.alreadyUnlocked`, y por separado `onUnlockWithKey`) → `lock()` y `UnlockScreen`.
  - `7 create pendiente y hidden` → al completar, `lockCalls == 1`, `UnlockScreen` y `find.byType(BiometricOfferScreen)` nada.
  - `8 activación con la casilla y hidden`: huella disponible y no activa, casilla marcada, `onBeginEnable` con un `Completer` → contraseña correcta → `hidden` → `lockCalls == 1` y `UnlockScreen` nueva. Completar `beginEnable` → `biometric.calls` contiene `abortEnable` y no `completeEnable`; `session.exportedKeys` vacío; sigue `UnlockScreen`.
  - `9 la marca se reinicia`: activación que termina normal → principal → `hidden` (`lockCalls == 1`) → `resumed` → desbloqueo nuevo con `onUnlock` pendiente → `hidden` → `lockCalls` sigue en 1.
  - `10a oferta: beginEnable pendiente y hidden` → al completarlo, `abortEnable`, sin `completeEnable`, sigue `UnlockScreen`. `10b oferta: exportVaultKey pendiente al bloquear` → al completarlo, `tester.takeException()` es `null` y sigue `UnlockScreen`.
  - `hidden repetido no vuelve a bloquear` (Review Focus 3): `hidden`, `resumed`, `hidden` con la app ya bloqueada → `lockCalls == 1`.
  - `hidden sin bóveda abierta no hace nada` (Review Focus 4): en el primer uso con texto escrito, y con `openError` (`openFailed`) → `lockCalls == 0`; en el primer uso el texto sigue en el campo.
- [ ] **Step 3:** `flutter test test/auto_lock_test.dart` → FAIL.
- [ ] **Step 4:** implementar en `app.dart` y `unlock_screen.dart` según las reglas de arriba. `BovedaApp.builder` monta `AutoLockScope(controller: …, child: child!)` (el `Listener` llega en la Task 5).
- [ ] **Step 5:** `flutter test` y `flutter analyze` → pasan, incluidas `app_test.dart` y `unlock_screen_test.dart` sin cambios.
- [ ] **Step 6: Commit** `feat: bloqueo al pasar a segundo plano en VaultGate (#39)`

### Task 5: bloqueo por inactividad

**Files:**
- Modify: `mobile/lib/app.dart`
- Test: `mobile/test/auto_lock_test.dart`

**Interfaces:**
- Consumes: `AutoLockController.onActivity` (Task 3), `_lock()` y `_enterUnlocked` (Task 4).
- Produces: `VaultGate._restartIdle()`: cancela el `Timer` anterior y, en `unlocked` u `offerBiometric`, crea `Timer(widget.autoLockTimeout, _lock)`. `VaultGate` asigna `widget.autoLock.onActivity = _restartIdle` en `initState` y lo pone en `null` en `dispose`. Registra un manejador de `HardwareKeyboard.instance` que llama a `_restartIdle()` y devuelve `false`, y lo quita en `dispose`. `BovedaApp.builder` envuelve el `child` en `Listener(behavior: HitTestBehavior.translucent, onPointerDown: (_) => controller.reportActivity())` dentro del `AutoLockScope`.

- [ ] **Step 1: Pruebas** (reloj simulado con `tester.pump(Duration)`):
  - `11 bloquea a los 5 minutos`: `pump(4 min 59 s)` → `lockCalls == 0`; `pump(1 s)` → 1 y `UnlockScreen`.
  - `12 un toque reinicia la cuenta`: `pump(4 min)`, `tester.tapAt(Offset(10, 10))`, `pump(4 min)` → 0; `pump(1 min)` → 1.
  - `13 una tecla física reinicia la cuenta`: igual con `tester.sendKeyEvent(LogicalKeyboardKey.keyA)`.
  - `14 en locked no corre`: en el desbloqueo, `pump(10 min)` → `lockCalls == 0`. `14 en la oferta sí bloquea`: `pump(5 min)` → 1.
  - `14b sin temporizadores huérfanos`: tras el bloqueo por inactividad, `pump(10 min)` → `lockCalls == 1`. Igual tras `hidden` + `resumed`.
  - `15 respeta el tiempo inyectado`: `BovedaApp(autoLockTimeout: Duration(minutes: 1))` → bloquea a 1 min.
  - `al desmontar no quedan temporizadores ni manejador de teclado` (Review Focus 5): con la principal, `tester.pumpWidget(const SizedBox())`; la prueba termina sin el error de temporizadores pendientes, y `tester.sendKeyEvent` después no lanza.
- [ ] **Step 2:** `flutter test test/auto_lock_test.dart` → FAIL en 11 a 15.
- [ ] **Step 3:** implementar según Interfaces; `_enterUnlocked` llama a `_restartIdle()` después de cambiar de estado.
- [ ] **Step 4:** `flutter test` y `flutter analyze` → pasan.
- [ ] **Step 5: Commit** `feat: bloqueo por inactividad (#39)`

### Task 6: la huella espera a `resumed` y se reintenta tras ocultarse

**Files:**
- Modify: `mobile/lib/screens/unlock_screen.dart`
- Test: `mobile/test/unlock_screen_test.dart` (grupo `huella`)

**Interfaces:**
- Produces: sin cambios de API. `_UnlockScreenState` gana un `AppLifecycleListener` (liberado en `dispose`), `bool _waitingResume`, `bool _hiddenDuringPrompt` y `bool _retryOnResume`.

Reglas:
- `_start()`: si `SchedulerBinding.instance.lifecycleState != AppLifecycleState.resumed`, `_waitingResume = true` y sale; el `onResume` del listener, con `_waitingResume`, lo pone en `false` y corre `_start()`. Corre una sola vez por instancia, como hoy.
- `onHide` con la huella en curso (`_busy` dentro de `_unlockWithBiometric`): `_hiddenDuringPrompt = true`.
- En `_unlockWithBiometric`, caso `BiometricFallback` con `_hiddenDuringPrompt`: la consume (`false`), reactiva el formulario **sin** pedir el foco y deja `_retryOnResume = true`; el siguiente `onResume` con esa marca la consume y llama a `_unlockWithBiometric()` una vez. Sin la marca, el `fallback` sigue como hoy (foco en el campo).

- [ ] **Step 1: Pruebas:**
  - `18 en inactive o hidden no pide la huella hasta resumed` (un caso por estado): `setLifecycle` antes de `pumpScreen` → `unlockPrompts() == 0`; `resumed` → 1; `inactive` y `resumed` otra vez → sigue en 1.
  - `18b cancelada al ocultarse se vuelve a pedir al volver`: `biometric.onUnlock` con un `Completer`; `hidden`; completar con `BiometricException(BiometricFailure.canceled)` → el campo sin foco (`FocusNode` sin `hasFocus`); `resumed` → `unlockPrompts() == 2`; completar el segundo con `canceled`, `hidden` no ocurrió → foco en el campo; otro `resumed` → sigue en 2.
  - `18c cancelada por el usuario no se vuelve a pedir`: `canceled` sin `hidden` → foco en el campo; `inactive` y `resumed` → `unlockPrompts() == 1`.
- [ ] **Step 2:** `flutter test test/unlock_screen_test.dart` → FAIL en 18 a 18c.
- [ ] **Step 3:** implementar según las reglas.
- [ ] **Step 4:** `flutter test` y `flutter analyze` → pasan; las pruebas de la huella que ya existen pasan sin cambios (19).
- [ ] **Step 5: Commit** `feat: la huella espera a resumed y se reintenta tras ocultarse (#39)`

### Task 7: documentación

**Files:**
- Modify: `docs/especificacion.md` ("La bóveda → Bloqueo automático")
- Modify: `ARCHITECTURE.md`
- Modify: `mobile/CLAUDE.md`
- Modify: `mobile/lib/app.dart` (comentario de `VaultGate`: "bloquear llega en la #39" pasa a describir el bloqueo)

- [ ] **Step 1:** copiar a la especificación el bloque de la sección "Documentación" de la spec, tal cual (con la viñeta "Huella al ocultarse" que atribuye la cancelación a la app).
- [ ] **Step 2:** `ARCHITECTURE.md`: `services/` con `auto_lock.dart`; `widgets/` con `activity_text_field.dart`; descripción de `app.dart` (bloqueo por `hidden` e inactividad, revisión al terminar, remontaje por `_epoch`); `BiometricVault.kt` cancela el diálogo en `onStop`.
- [ ] **Step 3:** `mobile/CLAUDE.md`, en "Reglas", las dos reglas de la spec (`ActivityTextField` o `PasswordField`; `_enterUnlocked`).
- [ ] **Step 4:** `flutter analyze` → pasa.
- [ ] **Step 5: Commit** `docs: bloqueo automático en la especificación, la arquitectura y mobile/CLAUDE.md (#39)`

### Task 8: prueba a mano en el emulador y verificación final

**Files:** ninguno se confirma (salvo ajustes de documentación por lo observado).

- [ ] **Step 1:** `rustup run stable rustc --version` igual al `channel` fijado; `flutter run` en el emulador (Android 16, API 37) con una huella registrada.
- [ ] **Step 2:** correr las pruebas 1 a 13 de la tabla "A mano, en el emulador" de la spec, con logcat (`wm_on_paused_called`, `wm_on_stop_called`, `wm_on_resume_called`). La 7 usa una constante temporal de 30 s en `auto_lock.dart`; al terminar, `git diff --exit-code mobile/lib` confirma que no quedó ningún cambio.
- [ ] **Step 3:** si la prueba 4 muestra `onStop` con el selector de recientes, ajustar la viñeta del disparador en `docs/especificacion.md` y el esperado en la spec, y confirmar con `docs: …(#39)`. Si la miniatura de recientes muestra la principal, anotarlo para el comentario en la #43.
- [ ] **Step 4:** cerrar el emulador.
- [ ] **Step 5:** desde la raíz, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`; desde `mobile/`, `flutter analyze` y `flutter test` → todo pasa.
