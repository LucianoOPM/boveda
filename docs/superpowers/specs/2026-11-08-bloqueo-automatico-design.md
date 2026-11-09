# Diseño: bloqueo automático por inactividad y en segundo plano (issue #39)

Fecha: 2026-11-08 · Etapa 2 · Rama `feat/39-bloqueo-automatico`

## Objetivo

La llave de bóveda se borra de memoria cuando la app pasa a segundo plano y tras 5 minutos de inactividad. Al bloquear, ninguna pantalla con datos de la bóveda queda en el árbol ni en la pila de navegación y se vuelve al desbloqueo. Ninguna operación asíncrona que termine con la app oculta deja la bóveda desbloqueada.

Agrega la transición `unlocked → locked` que la #36 y la #37 dejaron pendiente en `VaultGate`.

### Fuera de alcance

- Ajuste del tiempo de bloqueo (etapa 4). El valor vive en una sola constante para que ese ajuste solo la reemplace.
- Suspender el bloqueo mientras el selector de archivos está abierto: se decide aquí el mecanismo (sección "Selector de archivos") y se implementa en la #44, que es la primera que lo usa.
- Botón "Bloquear" manual: la issue no lo pide y la pantalla principal es de la #40.
- Ocultar la miniatura de recientes: es de la protección de pantalla (`FLAG_SECURE`, #43).

### Criterios de aceptación (de la issue)

- Ir a otra app y volver muestra el desbloqueo.
- Tras el tiempo de inactividad, la app se bloquea sola.
- Tras bloquear, ninguna pantalla con datos de la bóveda queda en la pila de navegación.
- Pruebas de widgets del bloqueo por ciclo de vida y por tiempo (con reloj simulado).
- Probado a mano en el emulador.
- `flutter analyze` y `flutter test` pasan; CI en verde.

## Lo que confirma la documentación

Verificado con Context7 (API de Flutter) y en el código de la #35 a la #38:

- `AppLifecycleState` en Android: `inactive` llega con `Activity.onPause` **o** con la actividad en `onResume` pero sin el foco de la ventana. Ejemplos: un diálogo del sistema, la cortina de notificaciones, el selector de apps recientes, la pantalla dividida. `paused` llega con `onStop`. `hidden` se sintetiza justo antes de `paused` (y antes de `inactive` al volver de `paused`).
- El `BiometricPrompt` de la plataforma (`android.hardware.biometrics`, el que usa `BiometricVault.kt`) es una ventana de SystemUI, no otra actividad: quita el foco (`inactive`) sin `onStop`. Se confirma en el emulador (prueba a mano 1).
- El estado actual se lee con `SchedulerBinding.instance.lifecycleState`. Los cambios se observan con `AppLifecycleListener`.
- En las pruebas de widgets, `tester.pump(duración)` avanza el reloj simulado y dispara los `Timer`. El ciclo de vida se simula con `tester.binding.handleAppLifecycleStateChanged`.
- `VaultSession::lock` (`mobile/rust/src/api/session.rs`) solo falla si el `Mutex` está envenenado (`SessionPoisoned`). `Vault::lock` del núcleo es `self.unlocked = None` y no falla. Del lado de Dart, la llamada solo fallaría si el objeto opaco se liberó con `dispose`, y en ese caso Rust ya soltó la `Vault` y la llave. La app nunca libera la sesión.
- Todas las operaciones de `VaultSession` pasan por el `Mutex` (#35): un `lock()` llamado durante Argon2id espera y bloquea justo después del desbloqueo. Por eso, con un desbloqueo en curso, no se llama a `lock()` al ocultarse (decisión 3).
- `BiometricVault.kt` responde `busy` a una petición con un diálogo abierto. Cada llamada tiene su propio `Reply`, que responde una sola vez: un resultado nunca llega a la llamada equivocada. Hoy solo `onDestroy` cancela el diálogo; `onStop` no hace nada.

## Decisiones

1. **Disparador: solo `hidden`**, con un `AppLifecycleListener` en `VaultGate`. `inactive` no bloquea: llega con la huella, la cortina y el selector de recientes, que no sacan al usuario de la app. Apagar la pantalla llega como `onStop` y bloquea.
2. **Qué hace el bloqueo:** `session.lock()`, `popUntil(isFirst)` en el `Navigator` raíz, `clearSnackBars()`, una pantalla de desbloqueo nueva. Si `lock()` lanza, igual se muestra el desbloqueo. La fachada bloquea aunque el `Mutex` esté envenenado, así que en la práctica `lock()` no falla.
3. **Regla general al terminar:** toda operación asíncrona que termina con Rust desbloqueado revisa el ciclo de vida. Si la app está en `hidden`, `paused` o `detached`, bloquea en lugar de mostrar la bóveda. Con un desbloqueo en curso no se llama a `lock()` al ocultarse. Durante la activación de la huella con la casilla, Rust ya está desbloqueado y sin ninguna operación larga en curso, así que sí se bloquea al ocultarse.
4. **Inactividad: 5 minutos**, valor por omisión hasta la etapa 4, en la constante `defaultAutoLockTimeout`. El temporizador corre solo con la bóveda en pantalla (`unlocked` y `offerBiometric`).
5. **Actividad:** cualquier toque en la app, las teclas físicas y escribir en un campo de texto de las pantallas de la bóveda. El teclado en pantalla no genera eventos globales en Flutter, así que lo reporta un componente compartido, `ActivityTextField`, sobre el que se arma `PasswordField`. `mobile/CLAUDE.md` exige usarlo.
6. **Selector de archivos (#44):** mecanismo genérico `runWithoutAutoLock`, implementado en la #44 (sección "Selector de archivos").
7. **Sin botón "Bloquear"** en esta issue.
8. **La huella automática espera a `resumed`**, siempre, sin depender de lo que haga cada fabricante.
9. **Pruebas:** de widgets con el ciclo de vida simulado y el reloj simulado, una prueba de la fachada para el `Mutex` envenenado, y prueba a mano en el emulador. Sin dependencias nuevas.

## Componentes

### `lib/services/auto_lock.dart` (nuevo)

```dart
/// Tiempo de inactividad por omisión. La etapa 4 lo reemplaza por el ajuste;
/// es el único lugar del valor.
const defaultAutoLockTimeout = Duration(minutes: 5);

/// Da acceso a `reportActivity()` a cualquier widget bajo `MaterialApp`.
class AutoLockScope extends InheritedWidget {
  const AutoLockScope({super.key, required this.reportActivity, required super.child});

  final VoidCallback reportActivity;

  static AutoLockScope? maybeOf(BuildContext context) =>
      context.getInheritedWidgetOfExactType<AutoLockScope>();

  /// Nunca notifica: `reportActivity()` no reconstruye nada.
  @override
  bool updateShouldNotify(AutoLockScope oldWidget) => false;
}
```

`maybeOf` usa `getInheritedWidgetOfExactType`, sin registrar dependencia: el alcance no cambia de forma observable. Fuera del alcance (pruebas de pantallas sueltas, la pantalla de error de arranque), devuelve `null` y el campo no hace nada.

### `VaultGate` (`lib/app.dart`)

Campos nuevos:

| Campo | Para qué |
| --- | --- |
| `AppLifecycleListener _lifecycle` | `onHide` → `_onHidden()` |
| `Timer? _idle` | Temporizador de inactividad |
| `int _epoch` | `key` de la pantalla de cada estado (`ValueKey(_epoch)`); sube con cada bloqueo |
| `bool _openWhileLocked` | Rust ya está desbloqueado mientras la interfaz sigue en `locked` (fase de activar la huella con la casilla) |

`VaultGate` recibe `autoLockTimeout` (de `BovedaApp`).

**`_lock()`**

1. `_idle?.cancel()`; `_idle = null`; `_openWhileLocked = false`.
2. `session.lock()` sin esperar el resultado en la interfaz: si lanza, se ignora (sin detalles). Se lanza antes de cambiar de pantalla, para que la llave se borre lo antes posible.
3. `Navigator.of(context).popUntil((route) => route.isFirst)`: cierra rutas, diálogos y hojas que apilen la #40 y la #41.
4. `ScaffoldMessenger.of(context).clearSnackBars()`.
5. `setState`: `_epoch++` y `_state = locked`.

El cambio de `_epoch` saca del árbol **toda** pantalla que estaba montada, también una `UnlockScreen` en `locked`: sus `dispose` limpian los campos y sus `await` pendientes encuentran `mounted == false`, así que no llaman a ningún callback. La pantalla que entra es nueva.

**Al ocultarse (`_onHidden`)**

| Estado | Acción |
| --- | --- |
| `unlocked`, `offerBiometric` | `_lock()` |
| `locked` con `_openWhileLocked` | `_lock()`: no hay Argon2id en curso (como mucho, un `exportVaultKey`, que es instantáneo). La `UnlockScreen` se remonta y la activación vieja no llama a `onUnlocked` |
| `locked` sin la marca (desbloqueo en curso o en espera), `noVault`, `checking`, `openFailed` | Nada. Lo resuelve la revisión al terminar |

**Revisión al terminar (`_enterUnlocked`)**

Toda transición a un estado con Rust desbloqueado pasa por una sola función:

```dart
void _enterUnlocked(VaultGateState next, {int damagedEntries = 0}) {
  _openWhileLocked = false;
  if (_isAway) {
    _lock();
    return;
  }
  setState(() { ... _state = next; });
  _restartIdle();
}

bool get _isAway => switch (SchedulerBinding.instance.lifecycleState) {
  AppLifecycleState.hidden ||
  AppLifecycleState.paused ||
  AppLifecycleState.detached => true,
  _ => false,
};
```

Pasan por ella: el `onUnlocked` de `UnlockScreen` (contraseña, huella y `alreadyUnlocked` de ambas), `_afterCreate` (a `offerBiometric` o a `unlocked`) y el `onDone` de la oferta. `null` (sin estado todavía) y `inactive` cuentan como presentes. Si la app vuelve a `resumed` antes de que Argon2id termine, la bóveda queda desbloqueada: la contraseña la escribió el usuario medio segundo antes.

**Fase de activación:** `UnlockScreen` recibe un callback nuevo, `onVaultOpened`. Lo llama apenas `unlock` tiene éxito y antes de `enableBiometric`, y `VaultGate` pone `_openWhileLocked = true`. Antes de ese punto, ocultarse no llama a `lock()`. La marca se reinicia en `_lock()` y en `_enterUnlocked()`: si no, una activación que termina normal la dejaría puesta y un ocultamiento posterior, con un desbloqueo nuevo en curso, llamaría a `lock()` en plena carrera con Argon2id.

**Temporizador**

- `_restartIdle()`: cancela el anterior y, si el estado es `unlocked` u `offerBiometric`, crea `Timer(autoLockTimeout, _lock)`.
- `reportActivity()` (el del `AutoLockScope`) llama a `_restartIdle()`. En otros estados no hace nada.
- `dispose()`: cancela el temporizador, libera el `AppLifecycleListener` y quita el manejador de `HardwareKeyboard`.

**Acceso a `reportActivity` desde `BovedaApp`:** `VaultGate` vive dentro del `Navigator`, y el `Listener` y el `AutoLockScope` van en `MaterialApp.builder`, por encima. `BovedaApp` pasa a ser `StatefulWidget` y crea un `AutoLockController`, una clase simple (no un `ChangeNotifier`: nadie escucha cambios) con `reportActivity()`, que reenvía a un `VoidCallback?` que registra `VaultGate` en `initState` y borra en `dispose`. El `builder` y `VaultGate` comparten el mismo controlador. No se usa `GlobalKey` para llamar al estado de `VaultGate`.

### Actividad (`BovedaApp`)

- `MaterialApp.builder: (context, child) => AutoLockScope(reportActivity: controller.reportActivity, child: Listener(behavior: HitTestBehavior.translucent, onPointerDown: (_) => controller.reportActivity(), child: child!))`. Cubre rutas, diálogos y hojas, porque todos viven bajo el `Navigator` que es `child`.
- Teclas físicas: un manejador de `HardwareKeyboard.instance.addHandler` que llama a `reportActivity()` y devuelve `false` (no consume la tecla). Lo registra `VaultGate` en `initState` y lo quita en `dispose`.
- `BovedaApp` recibe `autoLockTimeout`, por omisión `defaultAutoLockTimeout`. `main.dart` no lo pasa.

### `ActivityTextField` (`lib/widgets/activity_text_field.dart`, nuevo)

Un `TextField` con los parámetros que usan las pantallas (los de `PasswordField` más los que pidan la #40 y la #41). Su `onChanged` llama a `AutoLockScope.maybeOf(context)?.reportActivity()` y después al `onChanged` recibido. `PasswordField` se arma sobre él, sin cambiar su API. `PasswordField` no basta solo: la búsqueda y las notas no son contraseñas.

### `UnlockScreen` (`lib/screens/unlock_screen.dart`)

- Callback nuevo `onVaultOpened` (ver "Fase de activación").
- **Esperar a `resumed`:** `_start()` (el intento automático de la huella) comprueba `SchedulerBinding.instance.lifecycleState`. Si no es `resumed`, espera al siguiente `onResume` de un `AppLifecycleListener` propio y solo entonces pide la huella. Corre una sola vez por instancia, como hoy. El listener se libera en `dispose`.
- **Huella cancelada al ocultarse:** si la app pasa a `hidden` mientras el diálogo de la huella está abierto (`_busy` con la huella en curso), la pantalla pone `_hiddenDuringPrompt = true`. Al llegar un `BiometricFallback` con la marca puesta, no enfoca el campo, consume la marca y vuelve a pedir la huella **una sola vez** en el siguiente `resumed`. Si el usuario canceló antes de ocultar la app, la marca no está y la pantalla cae a la contraseña, como hoy. Un `lockout` reintentado vuelve a dar `fallback` sin repetirse. No hace falta distinguir las dos cancelaciones en el canal.

### `BiometricOfferScreen`

No cambia su lógica: ya comprueba `mounted` después de `enableBiometric`. Si el bloqueo la saca del árbol a mitad de `beginEnable` o de `exportVaultKey`, no llama a `onDone`. Si la huella del diálogo viejo ya pasó, `exportVaultKey` da `Locked`, `enableBiometric` llama a `abortEnable` y la llave no sale de Rust.

### Kotlin (`BiometricVault.kt`, `MainActivity.kt`)

- `BiometricVault` guarda el diálogo en curso como un objeto con la `CancellationSignal`, el `Reply` y `onError`, en lugar de solo la señal.
- `cancelPrompt()`: si hay un diálogo, cancela la señal, corre `onError` (en la activación borra la llave recién creada) y responde **de inmediato** `canceled` a la llamada pendiente, sin esperar el callback del sistema: si ese callback no llegara, la llamada de Dart quedaría colgada. El `Reply` ignora la respuesta duplicada del sistema.
- `MainActivity.onStop()` llama a `biometric.cancelPrompt()`. `onDestroy` usa la misma función, y además descarta una activación a medias (`pendingCipher`) como hoy.
- Una petición con otra en curso sigue respondiendo `busy`.
- Es una garantía de la app, no un comportamiento de la plataforma.

### Fachada (`mobile/rust/src/api/session.rs`)

```rust
pub fn lock(&self) -> FacadeResult<()> {
    // Un pánico anterior envenena el `Mutex`, pero bloquear solo suelta la
    // llave: es seguro sobre cualquier estado. El envenenamiento se
    // conserva para el resto de las operaciones (`SessionPoisoned`).
    let mut slot = self.vault.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(vault) = slot.as_mut() {
        vault.lock();
    }
    Ok(())
}
```

La firma no cambia: no hace falta regenerar el puente. `is_locked` y el resto siguen dando `SessionPoisoned`.

### Selector de archivos (#44)

Se implementa en la #44. Queda decidido:

- `runWithoutAutoLock<T>(Future<T> Function() operation)`, expuesto por `AutoLockScope`. Es genérico: la etapa 3 puede necesitarlo para el escáner del QR si abre otra actividad.
- Mientras la operación corre, ocultarse no bloquea y el temporizador no corre. Al terminar, el temporizador se reinicia.
- Tope de 2 minutos fuera de la app: si la app estuvo oculta más tiempo, bloquea al volver, aunque la operación siga a medias.
- Con el tope superado a mitad de camino: si el selector devuelve el destino con la bóveda ya bloqueada, `exportBackup` da `Locked`, y la pantalla debe manejarlo (aviso, vuelta al desbloqueo y ningún temporal huérfano). `importBackup` no exige la bóveda desbloqueada: la #44 decide si se permite importar tras un bloqueo a mitad del flujo.

Se publica como comentario en la #44.

## Pruebas

### Doble de sesión (`test/fakes.dart`)

`FakeVaultSession` gana estado:

- `lockCalls` (contador) y `lockError` (si no es `null`, `lock()` lo lanza después de contar).
- `locked`: `unlock`, `unlockWithKey` y `create` lo ponen en `false` al terminar con éxito; `lock` en `true`. `isLocked` lo devuelve.
- `exportVaultKey` lanza `VaultError.locked` si `locked` es `true`, como la fachada.

### Fachada (`mobile/rust/src/api/session/tests.rs`)

- Mutex envenenado: abrir y desbloquear la copia de `vault-v2.db` (`test_support::fixture_copy()`), envenenar el `Mutex` con un hilo que entra en pánico con el candado tomado y llamar a `lock()`. Da `Ok`, la `Vault` queda bloqueada (leída con `into_inner`) y `is_locked()` sigue dando `SessionPoisoned`.

### Widgets: ciclo de vida (`test/auto_lock_test.dart`, nuevo)

Con `tester.binding.handleAppLifecycleStateChanged` y `BovedaApp` con los dobles:

1. `unlocked` y `hidden` → `lockCalls == 1`, se ve el desbloqueo.
2. `unlocked` e `inactive` → `lockCalls == 0`.
3. `offerBiometric` y `hidden` → `lock()`, se ve el desbloqueo.
4. `lock()` que lanza → igual se ve el desbloqueo.
5. Pila: con una ruta apilada sobre la principal (con el `Navigator` raíz) y un `SnackBar` visible, al ocultarse no queda ninguna de las dos (`findsNothing`).
6. Carrera con la contraseña: `unlock` pendiente (`Completer`) → `hidden` → `lockCalls == 0`. Al completarlo con `hidden`, `paused` o `detached`, `lock()` y desbloqueo. Si la app vuelve a `resumed` antes de completarlo, llega a la principal.
   - 6b. Con la huella: `unlockWithKey` pendiente → `hidden` → al completarlo, `lock()` y desbloqueo.
   - 6c. `alreadyUnlocked` (con contraseña y con huella) con el ciclo de vida en `hidden` → `lock()` y desbloqueo.
7. `create` pendiente → `hidden` → al completarlo, `lock()` y desbloqueo, sin la oferta de la huella.
8. Activación con la casilla: `unlock` con éxito y `beginEnable` pendiente → `hidden` → `lock()`, `UnlockScreen` nueva. Al completar `beginEnable`, `exportVaultKey` da `Locked` → `abortEnable`, `exportedKeys` vacío, sin `completeEnable`, y la app sigue en el desbloqueo.
9. La marca se reinicia: la activación termina normal → principal → `hidden` (`lockCalls == 1`) → un desbloqueo nuevo pendiente → `hidden`: `lockCalls` sigue en 1.
10. Oferta del primer uso: (a) `beginEnable` pendiente → `hidden` → al completarlo, `exportVaultKey` da `Locked`, `abortEnable` y no hay cambio de estado; (b) `exportVaultKey` pendiente al bloquear → al completarlo, sin excepciones y sin cambio de estado.

### Widgets: inactividad (reloj simulado)

11. Con `defaultAutoLockTimeout`: a los 4:59 sin bloquear y a los 5:00 bloqueada.
12. Un toque a los 4 min reinicia la cuenta: 4 min más sin bloquear y 1 min más, bloqueada.
13. Una tecla física (`tester.sendKeyEvent`) reinicia la cuenta.
14. En `locked`, 10 min sin `lock()`. En `offerBiometric`, sí bloquea.
   - 14b. Tras un bloqueo por inactividad, 10 min más: `lockCalls` sigue en 1. Lo mismo tras un bloqueo por `hidden` seguido de `resumed`: no queda ningún temporizador huérfano al remontar.
15. `BovedaApp(autoLockTimeout: 1 min)` respeta el valor inyectado, y `defaultAutoLockTimeout == Duration(minutes: 5)`.

### Widgets: componentes

16. `ActivityTextField` dentro de un `AutoLockScope` de prueba: escribir llama a `reportActivity()` y al `onChanged` recibido. Fuera del alcance no lanza. `PasswordField` también llama a `reportActivity()`.
17. `AutoLockScope.updateShouldNotify` devuelve `false`.

### Widgets: `UnlockScreen`

18. Montada con el ciclo de vida en `inactive` o `hidden`: no pide la huella. En `resumed` la pide una vez, y un segundo `resumed` no la repite.
   - 18b. Diálogo abierto (`unlock` del doble de huella pendiente) → `hidden` → el doble responde `canceled` → sin foco en el campo. En `resumed`, la huella se pide otra vez, y un segundo `resumed` no la repite.
   - 18c. El usuario cancela (`canceled` sin pasar por `hidden`) → foco en el campo. En `resumed` no se vuelve a pedir.
19. Las pruebas que ya existen de la huella pasan sin cambios (arrancan en `resumed`).

`biometric_service_test.dart` ya cubre la conversión de `canceled`. Kotlin no tiene pruebas automáticas, como en la #38.

### A mano, en el emulador

Android 16 (API 37) con una huella registrada. Antes de compilar, `rustup run stable rustc --version`. El ciclo de vida se observa con `adb logcat` (eventos `wm_on_paused_called`, `wm_on_stop_called`, `wm_on_resume_called`), sin logs en Dart. Si no alcanzan, un log temporal con solo el nombre del estado, que se quita antes del commit.

| # | Prueba | Esperado |
| --- | --- | --- |
| 1 | Diálogo de la huella (arranque en frío y "Usar huella") | Sin `onStop` ni `onPause` en logcat; no bloquea. `finger touch 1` desbloquea |
| 2 | Bóveda abierta → inicio → volver | Desbloqueo; con la huella, el diálogo aparece después de volver |
| 3 | Bóveda abierta → apagar la pantalla (`input keyevent 26`) → encender y quitar el bloqueo del sistema | Desbloqueo |
| 4 | Bóveda abierta → recientes (`keyevent 187`) → volver sin elegir otra app | No bloquea (solo `inactive`). Si Android 16 da `onStop` y bloquea, no se fuerza: bloquear de más es el lado seguro. Se registra lo observado y se ajustan la especificación y este esperado, no el código |
| 5 | Recientes → otra app → volver | Desbloqueo |
| 6 | Bóveda abierta → bajar la cortina (`cmd statusbar expand-notifications`) → subirla | No bloquea |
| 7 | Inactividad con una constante temporal de 30 s | Bloquea a los 30 s; un toque a los 20 s reinicia la cuenta. Antes del commit, `git diff` confirma que el archivo tocado quedó igual |
| 8 | Diálogo de la huella abierto en el desbloqueo → inicio → volver | El diálogo viejo desapareció; se pide la huella una vez al volver (18b) |
| 9 | Diálogo abierto → "Usar contraseña" → inicio → volver | Contraseña, sin pedir la huella (18c) |
| 10 | Oferta: "Activar" → con el diálogo abierto, inicio → volver | Desbloqueo; con `run-as`, sin `vault_key.bin` |
| 11 | Casilla marcada, desbloquear con contraseña → con el diálogo de activar abierto, inicio → volver | Desbloqueo; la huella sin activar, sin `vault_key.bin` |
| 12 | "Desbloquear" e inicio de inmediato (durante Argon2id) → volver | Al volver, el desbloqueo. Si no se logra ocultar a tiempo con `adb`, se anota como no reproducible a mano (lo cubren 6 a 6c) |
| 13 | logcat filtrado por el paquete | Sin llaves, contraseñas ni bytes |

La miniatura de recientes (pruebas 2, 5 y 12) no es parte del esperado: Android la toma al salir, posiblemente antes de que se pinte el desbloqueo, y ocultarla es de la #43. Si muestra la principal, se anota como observación para la #43. Al terminar, se cierra el emulador.

### Antes de dar la issue por terminada

`cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all`, `flutter analyze` y `flutter test`.

## Archivos

| Archivo | Cambio |
| --- | --- |
| `mobile/lib/services/auto_lock.dart` | Nuevo: `defaultAutoLockTimeout`, `AutoLockScope`, `AutoLockController` |
| `mobile/lib/widgets/activity_text_field.dart` | Nuevo: `ActivityTextField` |
| `mobile/lib/widgets/password_field.dart` | Se arma sobre `ActivityTextField` |
| `mobile/lib/app.dart` | `BovedaApp` con estado, `builder` y `autoLockTimeout`; `VaultGate` con ciclo de vida, temporizador, `_epoch`, `_lock`, `_enterUnlocked` |
| `mobile/lib/screens/unlock_screen.dart` | `onVaultOpened`, espera a `resumed`, reintento tras ocultarse |
| `mobile/android/.../BiometricVault.kt` | Diálogo en curso con su `Reply`; `cancelPrompt()` |
| `mobile/android/.../MainActivity.kt` | `onStop` → `cancelPrompt()` |
| `mobile/rust/src/api/session.rs` | `lock()` con `PoisonError::into_inner` |
| `mobile/rust/src/api/session/tests.rs` | Prueba del `Mutex` envenenado |
| `mobile/test/fakes.dart` | Estado de bloqueo en `FakeVaultSession` |
| `mobile/test/auto_lock_test.dart` | Nuevo: pruebas 1 a 15 |
| `mobile/test/widgets_test.dart`, `mobile/test/unlock_screen_test.dart` | Pruebas 16 a 19 |

## Documentación

- `docs/especificacion.md`, "La bóveda → Bloqueo automático", debajo del párrafo actual:

  > Decidido el 2026-11-08 (#39, detalle en `docs/superpowers/specs/2026-11-08-bloqueo-automatico-design.md`):
  > - **Disparador:** `AppLifecycleState.hidden`, que en Android precede a `onStop` (botón de inicio, otra app, pantalla apagada). `inactive` no bloquea: llega con los diálogos del sistema (incluida la huella), la cortina de notificaciones y el selector de recientes, que no sacan al usuario de la app.
  > - **Inactividad:** 5 minutos fijos hasta la etapa 4, que agrega el ajuste. El valor vive en una sola constante (`defaultAutoLockTimeout`). Cuenta solo con la bóveda desbloqueada en pantalla.
  > - **Actividad:** cualquier toque en la app, las teclas físicas y escribir en un campo de las pantallas de la bóveda (`ActivityTextField` o `PasswordField`).
  > - **Al bloquear:** se borra la llave en Rust, se cierran todas las rutas sobre la raíz y los `SnackBar`, y se monta un desbloqueo nuevo. La fachada bloquea aunque el `Mutex` esté envenenado (`PoisonError::into_inner`), así que `lock()` no falla en la práctica; el envenenamiento se conserva para el resto de las operaciones.
  > - **Operaciones en curso:** toda operación asíncrona que termina con Rust desbloqueado (desbloqueo con contraseña o con huella, crear la bóveda, `AlreadyUnlocked`) revisa el ciclo de vida al terminar y, si la app está en `hidden`, `paused` o `detached`, bloquea en lugar de mostrar la bóveda. Con un desbloqueo en curso no se llama a `lock()` al ocultarse, para no desfasar Rust y la interfaz durante Argon2id. Durante la activación de la huella con la casilla, Rust ya está desbloqueado y sí se bloquea al ocultarse.
  > - **Huella al ocultarse:** la app cancela el diálogo en `onStop` (`BiometricVault.cancelPrompt` desde `MainActivity.onStop`) y responde `canceled` de inmediato, sin esperar el callback del sistema. Si la cancelación vino de ocultar la app, se vuelve a pedir la huella una vez al volver; si la canceló el usuario, se queda en la contraseña. El diálogo automático espera a que la app esté en `resumed`.
  > - **Miniatura de recientes:** Android la toma al salir, posiblemente antes de que se pinte el desbloqueo. Ocultarla le corresponde a la protección de pantalla (`FLAG_SECURE`, #43), no al bloqueo.
  > - **Selector de archivos y otras actividades:** mecanismo genérico que se implementa en la #44 (ver el diseño).

  Si la prueba a mano 4 muestra que el selector de recientes da `onStop` en Android 16, se ajusta la viñeta del disparador con lo observado.
- `ARCHITECTURE.md`: `services/auto_lock.dart` y `widgets/activity_text_field.dart` en el mapa; en la descripción de `app.dart`, el bloqueo por `hidden` e inactividad, la revisión al terminar y el remontaje por `_epoch`; `BiometricVault.kt` cancela el diálogo en `onStop`.
- `mobile/CLAUDE.md`, en "Reglas":
  - "En las pantallas de la bóveda, todo campo de texto es `ActivityTextField` o `PasswordField`: escribir cuenta como actividad para el bloqueo por inactividad."
  - "Toda operación asíncrona que termina con Rust desbloqueado pasa por `_enterUnlocked` de `VaultGate`, que bloquea si la app está oculta."
- `docs/roadmap.md`: sin cambios; la casilla de la etapa 2 se marca con el merge.

## Hallazgos de la implementación y de la prueba a mano

En el emulador (Android 16, API 37) pasaron las pruebas 1 a 13. Lo que no estaba previsto:

1. **Android 16 cierra el diálogo de la huella por su cuenta al tocar inicio.** Responde `onAuthenticationError` con `ERROR_USER_CANCELED` (10), el mismo código que una cancelación del usuario, unos 150 ms **antes** de `onPause`. Cuando llega `onStop`, `cancelPrompt` ya no encuentra diálogo (comprobado con un log temporal que solo registraba el código y el momento; se quitó). Ni el código ni el orden de los mensajes distinguen las dos cancelaciones. Lo que las distingue es lo que sigue: tras una del sistema, `hidden` llega unos 0.5 s después; tras una del usuario, la app vuelve a `resumed`. Mientras el diálogo está abierto, la app está en `inactive` (sin el foco de la ventana). Arreglo, en un `fix:` propio: un `fallback` que llega con la app sin estar en `resumed` espera hasta 1 s (`_systemCancelWindow`). Si llega `hidden`, se reintenta al volver; si no, se trata como del usuario y se enfoca el campo. Si algo falla, el costo es pequeño en cualquier dirección: en un celular lento, `hidden` puede llegar tarde y la pantalla cae a la contraseña (el comportamiento aceptado en la sección "UnlockScreen"); quien cancela y sale en menos de 1 s recibe la huella al volver. Pruebas 18d y 18e. `cancelPrompt` en `onStop` se mantiene para los fabricantes que no cierran el diálogo, y `MainActivity.onStop` llama primero a `super.onStop()`, que envía a Dart el aviso de ciclo de vida antes de la respuesta `canceled`.
2. **Sin cuadros con la app en `hidden`.** El binding desactiva los cuadros fuera de `resumed` e `inactive`, así que la pantalla nueva se construye al volver, también en el celular. Las pruebas de widgets comprueban `lockCalls` con la app oculta y la pantalla después de volver a `resumed`.
3. **`AppLifecycleListener` rechaza transiciones inválidas** y el binding de pruebas no sintetiza los estados intermedios. `test/lifecycle.dart` recorre `resumed ↔ inactive ↔ hidden ↔ paused ↔ detached` paso a paso.
4. **El selector de recientes no llega ni a `onPause`** (prueba 4): la app sigue al frente y no bloquea. La cortina de notificaciones tampoco genera eventos (prueba 6). El diálogo de la huella no genera `onPause` ni `onStop` (prueba 1, al activar y al desbloquear).
5. **Observación para la #43:** la miniatura de recientes muestra la pantalla principal tal cual, porque abrir los recientes no saca la app del frente. Ocultarla es de `FLAG_SECURE`.
6. **Callbacks tardíos del diálogo** (Kotlin, al implementar): cada callback del `BiometricPrompt` actúa solo si su diálogo sigue siendo el activo. Si `cancelPrompt` ya lo resolvió, el aviso tardío del sistema se ignora: si no, su `onError` borraría la llave de una activación nueva.
7. **El aviso "No se pudo activar la huella."** solo se muestra si la pantalla sigue montada: tras un bloqueo, la activación vieja termina en `failed` por `Locked` y el aviso aparecía sobre el desbloqueo nuevo.
8. **Prueba 12:** desde `adb` no se puede asegurar que la salida coincida con Argon2id (`onStop` llegó 0.5 s después del toque). Al volver se ve el desbloqueo; la carrera la cubren las pruebas 6 a 6c.

## Comentarios en issues

Se publican después de aprobar este diseño:

- **#44:** el mecanismo `runWithoutAutoLock` de la sección "Selector de archivos": genérico (la etapa 3 puede necesitarlo para el escáner del QR), el tope de 2 minutos y qué pasa si se supera a mitad de camino (`exportBackup` da `Locked`; decidir si se permite importar tras un bloqueo).
- **#40 y #41:** usar `ActivityTextField` en los campos y apilar las pantallas con el `Navigator` raíz, que `popUntil(isFirst)` limpia al bloquear; mantener el aviso de dañadas dentro de la principal (ya anotado en la #37).
- **#43:** la observación de la miniatura de recientes, si la prueba a mano la muestra (después de la prueba a mano).

## Commits previstos

1. `docs: diseño del bloqueo automático (#39)`
2. `fix: lock bloquea aunque el Mutex de la sesión esté envenenado (#39)`
3. `feat: cancelar el diálogo de la huella al ocultar la app (#39)`
4. `feat: campo de texto que reporta actividad (#39)`
5. `feat: bloqueo por ciclo de vida e inactividad en VaultGate (#39)`
6. `feat: la huella espera a resumed y se reintenta tras ocultarse (#39)`
7. `docs: bloqueo automático en la especificación, la arquitectura y mobile/CLAUDE.md (#39)`
