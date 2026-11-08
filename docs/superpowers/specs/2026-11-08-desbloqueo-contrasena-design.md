# Diseño: desbloqueo con contraseña maestra (issue #37)

Fecha: 2026-11-08 · Etapa 2 · Rama `feat/37-desbloqueo-contrasena`

## Objetivo

Con una bóveda existente y bloqueada, la app abre en la pantalla de desbloqueo. El usuario escribe la contraseña maestra, la app desbloquea con la fachada (#35) mostrando el progreso y pasa a la pantalla principal (un marcador hasta la #40). Una contraseña incorrecta solo muestra "Contraseña incorrecta"; si hay entradas dañadas, un aviso lo dice sin impedir el uso.

Reemplaza el marcador "Desbloqueo: #37" que dejó la #36 en el estado `locked` de `VaultGate`.

### Fuera de alcance

- Huella (#38).
- Bloqueo automático y botón de bloquear (#39). La prueba a mano cierra la app en lugar de bloquear.
- Pantalla principal real y marca de entrada dañada en la lista (#40).
- `FLAG_SECURE` (#43).
- "¿Olvidaste tu contraseña?" (la contraseña no se puede recuperar) y el acceso a importar un respaldo desde esta pantalla (#44).

### Criterios de aceptación (de la issue)

- Flujo completo a mano: primer uso → cerrar la app → desbloquear con la contraseña.
- Contraseña incorrecta: "Contraseña incorrecta" y sigue bloqueada.
- Una bóveda con una entrada dañada abre y muestra el aviso.
- La interfaz no se congela durante Argon2id.
- El campo de contraseña se limpia tras cada intento y al salir.
- Pruebas de widgets con el puente simulado.
- `flutter analyze` y `flutter test` pasan; CI en verde.

## Lo que confirma la documentación

Verificado con Context7 (API de Flutter) y en el código de la #35 y la #36:

- `VaultSession.unlock` no lleva `#[frb(sync)]`: devuelve un `Future` y Argon2id corre en el pool de hilos de flutter_rust_bridge, fuera del hilo de la interfaz.
- `MaterialBanner` es un aviso persistente y no modal, con acciones para atenderlo o descartarlo. Se puede poner directo en el árbol o mostrar con `ScaffoldMessengerState.showMaterialBanner`, que lo muestra **en todos los `Scaffold` registrados** del mismo `ScaffoldMessenger`.
- `TextField.onSubmitted` se llama cuando el usuario indica que terminó (por ejemplo, la tecla "Hecho" del teclado).
- Un `TextField` desactivado no acepta el foco: pedirlo en el mismo `setState` que lo reactiva no surte efecto. Se pide después del cuadro siguiente.
- `UnlockReport` cruza como `corrupt: List<String>` (ids en hexadecimal) y `unidentifiedCorrupt: int` (`u32` en Rust).

## Decisiones

### `VaultGate` (`lib/app.dart`)

- Nuevo campo `int _damagedEntries = 0`.
- Estado `locked` → `UnlockScreen(session: ..., onUnlocked: (damaged) => setState(() { _damagedEntries = damaged; _state = VaultGateState.unlocked; }))`.
- Al crear la bóveda en el primer uso, `_damagedEntries` queda en 0.
- Estado `unlocked` → el marcador de la pantalla principal ("Pantalla principal: #40") con `DamagedEntriesBanner(count: _damagedEntries)` arriba. `PlaceholderScreen` gana un parámetro opcional `banner` (un widget que va antes del texto).

### Pantalla de desbloqueo (`UnlockScreen`, `lib/screens/unlock_screen.dart`)

Contenido, de arriba abajo:

1. Título "Bóveda bloqueada".
2. `PasswordField` con la etiqueta "Contraseña maestra", `textInputAction: TextInputAction.done`, `onSubmitted` conectado a `_unlock` y un `FocusNode` propio de la pantalla.
3. Mensaje de error, si hay.
4. Botón "Desbloquear". Mientras desbloquea, muestra un indicador indeterminado y "Desbloqueando…".

El campo **no** se enfoca al abrir la pantalla (ver "Sin foco inicial", abajo). Sin "¿Olvidaste tu contraseña?" ni acceso a importar un respaldo. Sin `PopScope`: interrumpir `unlock` no deja nada a medias, a diferencia de `create`.

Estado: `TextEditingController _password`, `FocusNode _focus`, `bool _unlocking`, `String? _error`. Nunca guarda la contraseña en un campo.

Flujo de `_unlock()`:

1. Si `_unlocking` es `true` o el campo está vacío, regresa sin hacer nada. La misma guarda cubre el botón y `onSubmitted`: nunca corren dos `unlock` a la vez.
2. `setState`: `_unlocking = true`, `_error = null`. El campo y el botón se desactivan.
3. Lee la contraseña del controlador en una variable local y llama a `await widget.session.unlock(password: password)`. La variable local no se puede borrar de memoria: es de mejor esfuerzo, como dice "Decisiones clave" en `ARCHITECTURE.md`.
4. Después del `await`, antes de cualquier `setState` o de llamar a `onUnlocked`, comprueba `mounted`; si la pantalla ya salió del árbol, no hace nada más.
5. Resultado:

   | Resultado | Acción |
   | --- | --- |
   | Éxito | `onUnlocked(report.corrupt.length + report.unidentifiedCorrupt)`. La pantalla sale del árbol |
   | `VaultError.alreadyUnlocked` | `onUnlocked(0)`: la bóveda sí está desbloqueada; tratarlo como error haría fallar cualquier reintento para siempre (mismo razonamiento que `AlreadyExists` en la #36) |
   | `VaultError.wrongPassword` | Limpia el campo; error "Contraseña incorrecta" |
   | Cualquier otro error, `VaultError` o no | Limpia el campo; error "No se pudo desbloquear la bóveda. Intenta de nuevo." |

   El mensaje genérico nunca lleva detalles del error: ni la variante, ni el texto de una excepción, ni rutas. `VaultError` ya no trae datos, pero la pantalla tampoco interpola nada.
6. Al fallar: `setState` con `_unlocking = false` y el error; después, en `addPostFrameCallback`, si sigue `mounted`, `_focus.requestFocus()` para que el teclado vuelva sin tocar el campo.

Otras reglas:

- Al volver a escribir, el error se borra (`onChanged`).
- Con el campo vacío, el botón queda desactivado.
- `dispose()`: `_password.clear()` y `dispose()`, y `_focus.dispose()`.

### Campo de contraseña (`PasswordField`)

Tres parámetros opcionales nuevos, que se pasan tal cual al `TextField`:

| Parámetro | Por omisión |
| --- | --- |
| `textInputAction` | `null` (el valor por omisión del `TextField`) |
| `onSubmitted` | `null` |
| `focusNode` | `null` |

`FirstRunScreen` no cambia: no pasa ninguno.

### Aviso de entradas dañadas (`DamagedEntriesBanner`, `lib/widgets/damaged_entries_banner.dart`)

- `StatefulWidget` con `count`. Con `count == 0`, o después de descartarlo, no muestra nada (`SizedBox.shrink()`).
- Si no, un `MaterialBanner` con "1 entrada no se pudo leer" o "N entradas no se pudieron leer" y el botón "Entendido", que lo oculta.
- El total es `corrupt.length + unidentifiedCorrupt`: cuenta las entradas con la fila ilegible o los datos de búsqueda dañados, con o sin id. Un secreto dañado no aparece aquí (lo reporta `reveal` con `CorruptEntry`, ver la especificación).
- Va **en el árbol**, no con `ScaffoldMessenger.showMaterialBanner`: ese método lo muestra en todos los `Scaffold` registrados, así que podría seguir visible en la pantalla de desbloqueo cuando la #39 bloquee.
- El estado de oculto vive en el propio widget. Es correcto porque entre dos desbloqueos siempre se pasa por `locked` y el banner sale del árbol, así que cada desbloqueo lo muestra una vez. **La #40 debe mantenerlo dentro de la pantalla principal**, en una posición que salga del árbol al bloquear; si lo moviera a un ancestro que sobrevive al bloqueo, el estado de oculto se arrastraría al siguiente desbloqueo.
- Cumple "una entrada corrupta se marca como dañada sin bloquear el resto" sin adelantar la #40, que marcará cada entrada en la lista.

### Sin foco inicial

El desbloqueo no enfoca el campo al abrir, porque Android no permite abrir el teclado durante la animación de arranque en frío. Tras un intento fallido sí se reenfoca: el teclado ya está abierto y funciona. Se probaron en el emulador (Android 16, API 37), tres arranques en frío cada uno, sin éxito:

1. `autofocus`, y después pedir el foco al llegar a `AppLifecycleState.resumed`: el `showSoftInput` de Flutter llega antes de que Android sirva la vista (`Ignoring showSoftInput() ... is not served`).
2. Canal de Kotlin con `WindowInsetsController.show(ime())`: la petición llega antes de que la ventana tenga el foco y el sistema la cancela al dárselo (`HIDE_UNSPECIFIED_WINDOW`).
3. Canal de Kotlin que espera el foco de la ventana y `InputMethodManager.isActive(view)` antes de `showSoftInput`: llega al servidor, que la aborta durante la animación de apertura (`PHASE_SERVER_ABORT_SHOW_IME_POST_LAYOUT`).

Con la huella (#38), el arranque muestra primero su diálogo; el foco se pide solo si se cancela o falla, cuando la animación ya terminó.

### Intentos fallidos

Sin límite ni espera entre intentos. Cada intento ya cuesta unos 0.5 s de Argon2id, y quien tiene el archivo ataca sin conexión, sin pasar por la interfaz: un bloqueo en la app solo estorbaría al dueño. La especificación no lo pide.

### Nota para la #39

Si la app pasa a segundo plano mientras corre Argon2id, el evento de bloqueo llega antes de que `unlock` termine, y la bóveda quedaría desbloqueada en segundo plano. La #39 debe bloquear al terminar un `unlock` si la app ya no está en primer plano. No se resuelve en esta issue; se deja como comentario en la #39.

### Archivos

| Archivo | Contenido |
| --- | --- |
| `mobile/lib/app.dart` | `_damagedEntries`, `UnlockScreen` en `locked`, banner en `unlocked` |
| `mobile/lib/screens/unlock_screen.dart` | `UnlockScreen` |
| `mobile/lib/screens/placeholder_screen.dart` | parámetro opcional `banner` |
| `mobile/lib/widgets/password_field.dart` | `textInputAction`, `onSubmitted`, `focusNode` |
| `mobile/lib/widgets/damaged_entries_banner.dart` | `DamagedEntriesBanner` |
| `mobile/test/fakes.dart` | `unlock` en `FakeVaultSession` |
| `mobile/test/unlock_screen_test.dart` | pruebas de la pantalla (nuevo) |
| `mobile/test/app_test.dart`, `mobile/test/widgets_test.dart` | pruebas nuevas y ajustadas |
| `mobile/rust/src/api/session/tests.rs` | prueba de `UnlockReport` con una entrada dañada |
| `mobile/rust/Cargo.toml` | `rusqlite` como dependencia de desarrollo (ver abajo) |

`mobile/rust/src/api/` no cambia: no hace falta regenerar el puente.

## Pruebas

### Widgets (`mobile/test/`)

`FakeVaultSession.unlock` guarda la contraseña en `unlockCalls` y delega en `onUnlock`, configurable, que por omisión devuelve un `UnlockReport` vacío. Las pruebas usan un `Completer` para dejarlo pendiente o lanzan un error.

`unlock_screen_test.dart`:

- El botón está desactivado con el campo vacío y se activa al escribir.
- Durante el desbloqueo: indicador y "Desbloqueando…" visibles, campo y botón desactivados, y `unlock` recibió la contraseña escrita.
- Éxito: `onUnlocked` recibe `corrupt.length + unidentifiedCorrupt` (2 ids y 1 sin id → 3).
- `alreadyUnlocked`: `onUnlocked(0)`.
- `wrongPassword`: "Contraseña incorrecta", campo vacío, `onUnlocked` no se llamó y el campo tiene el foco.
- `VaultError.storageIo` y una excepción que no es `VaultError`: el mensaje genérico, sin otro texto, y el campo vacío.
- El error se borra al volver a escribir.
- Doble envío: "Hecho" dos veces seguidas, y botón más "Hecho", con el `Future` pendiente → `unlockCalls.length == 1`.
- "Hecho" con el campo vacío no llama a `unlock`.
- Limpieza al salir: se guarda la referencia al controlador del campo, se escribe, se desbloquea con éxito y la pantalla sale del árbol; el texto del controlador queda vacío (patrón `fieldOf(...).controller` de `first_run_screen_test.dart`).
- `mounted`: se desmonta la pantalla con `unlock` pendiente y se completa el `Future`, con éxito y con error; no hay excepciones ni llamada a `onUnlocked`.

`app_test.dart`:

- Con bóveda, abre `UnlockScreen` (reemplaza la prueba del marcador).
- Al desbloquear, aparece el marcador de la pantalla principal, sin banner con el reporte vacío.
- Con 3 dañadas, "3 entradas no se pudieron leer"; con 1, "1 entrada no se pudo leer".
- `alreadyUnlocked` lleva a la pantalla principal sin banner.
- Después del primer uso, sin banner.

`widgets_test.dart`:

- `PasswordField` pasa `textInputAction`, `onSubmitted` y `focusNode` al `TextField`; sin ellos, los valores por omisión no cambian.
- `DamagedEntriesBanner`: con 0 no muestra nada; singular y plural; "Entendido" lo oculta.

### Fachada (`mobile/rust`)

Una prueba nueva en `api/session/tests.rs`:

- Sobre la copia de `vault-v2.db` que da `test_support::fixture_copy()`, **nunca sobre el original**, se altera un byte de la columna `search` de una entrada viva, como hace `corrupt_entry` en `core/tests/vault.rs` (abrir con `rusqlite`, leer el blob, invertir un bit, escribirlo).
- `open` y `unlock` con `FIXTURE_PASSWORD`: el `UnlockReport` trae 1 id de 32 caracteres hexadecimales igual al de la entrada alterada, y `unidentified_corrupt == 0`.
- `search("")` sigue devolviendo las otras dos entradas vivas (`Correo Electrónico` y `Nube` o `Banco`, según la alterada): la entrada dañada falla sola.

**Dependencia de desarrollo nueva en `mobile/rust`: `rusqlite = { version = "0.41", features = ["bundled"] }`**, la misma versión y características que `vault-core`. No entra ningún crate nuevo a `Cargo.lock`, y no es criptográfica ni de red. Pendiente de tu confirmación al revisar este diseño.

### A mano, en el emulador

1. Instalación limpia → primer uso → cerrar la app (quitarla de recientes) → abrir: aparece el desbloqueo; un toque en el campo abre el teclado.
2. Contraseña incorrecta: "Contraseña incorrecta", campo vacío, teclado abierto.
3. Contraseña correcta: el indicador se anima durante Argon2id y se llega a la pantalla principal, sin banner.
4. Entrada dañada (nada entra al repositorio):
   1. Copiar `core/tests/fixtures/vault-v2.db` al directorio temporal de la sesión. Con `node:sqlite` (Node 24, sin instalar nada) confirmar que la copia tiene al menos dos entradas vivas y alterar un byte de la columna `search` de una.
   2. `adb shell am force-stop <paquete>`, para que no quede una conexión abierta sobre la bóveda anterior.
   3. `adb push` de la copia a `/data/local/tmp/`.
   4. Con `run-as <paquete>`: borrar `no_backup/boveda.db-journal` si existe (SQLite lo aplicaría sobre la copia nueva) y copiar el archivo a `no_backup/boveda.db`.
   5. Abrir la app y desbloquear con `prueba-bóveda-v2`: "1 entrada no se pudo leer"; "Entendido" lo oculta.

## Documentación

- `ARCHITECTURE.md`: `unlock_screen.dart` en `screens/`, `damaged_entries_banner.dart` en `widgets/` y la transición de desbloqueo en la descripción de `app.dart`.
- `docs/especificacion.md`, "Experiencia de uso → Desbloqueo": decisión del 2026-11-08 (#37): sin límite ni espera entre intentos (Argon2id ya cuesta unos 0.5 s por intento y el ataque real es sin conexión); el aviso de entradas dañadas es un banner de una vez por desbloqueo con el total de dañadas con y sin id; y los errores distintos de la contraseña incorrecta muestran un mensaje genérico sin detalles, como complemento de "La bóveda → Errores".
- Comentario en la #39 con la nota de Argon2id en segundo plano (autorizado).
