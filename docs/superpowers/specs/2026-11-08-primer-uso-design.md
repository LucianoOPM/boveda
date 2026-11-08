# Diseño: primer uso (issue #36)

Fecha: 2026-11-08 · Etapa 2 · Rama `feat/36-primer-uso`

## Objetivo

Al abrir la app sin bóveda, el usuario crea su contraseña maestra con un indicador de fortaleza y acepta de forma explícita que no se puede recuperar. La app crea la bóveda con la fachada (#35), queda desbloqueada y pasa a la pantalla principal (un marcador hasta la #40). De paso se fija la estructura de navegación que usarán las siguientes issues: un widget raíz que decide la pantalla según el estado de la sesión.

La regla de la contraseña maestra (longitud mínima y fortaleza) vive en el núcleo, para que la interfaz y el núcleo nunca discrepen y para que la hereden el cambio de contraseña (etapa 4) y la app de escritorio.

### Fuera de alcance

- Ofrecer activar la huella (#38).
- "Restaurar un respaldo" desde el primer uso (#44).
- Invitar a hacer el primer respaldo (etapa 4).
- Desbloqueo con contraseña (#37): aquí solo un marcador.
- `FLAG_SECURE` (#43). Hasta entonces, una contraseña mostrada con el botón de ver sale en capturas.
- Normalización Unicode (NFC) de la contraseña maestra y `-journal` huérfano en `create`: issues aparte (ver "Issues nuevas").

### Criterios de aceptación (de la issue)

- Sin bóveda, la app abre en primer uso; con bóveda, no.
- No se puede crear sin confirmar la contraseña y aceptar la advertencia.
- El indicador de fortaleza cambia mientras se escribe.
- La interfaz no se congela mientras se crea la bóveda.
- La contraseña no queda en estado global; los campos se limpian al salir de la pantalla.
- Pruebas de widgets con el puente simulado.
- Probado a mano en el emulador.
- `flutter analyze` y `flutter test` pasan; CI en verde.

## Lo que confirma la documentación

Verificado con Context7 (flutter_rust_bridge v2.13.0, API de Flutter), crates.io, pub.dev y el código fuente de `zxcvbn` 3.1.1:

- `zxcvbn` (Rust, MIT, 3.1.1 de abril de 2026) copia la contraseña en `String` que no se borran (`chars().take(100).collect()` y el `token` de cada `Match`), y `Entropy` y `Match` derivan `Debug` con esos fragmentos. Se descarta: aceptarlo exigiría una excepción de borrado sobre la contraseña maestra (la de `nucleo-matcher` se aceptó porque son datos de búsqueda, no secretos). Además, sus diccionarios están en inglés y suma varios crates a `Cargo.lock`. Los paquetes de Dart (`zxcvbn` 1.0.0, abandonado desde 2021; `password_strength_checker`) se descartan porque la lógica va en el núcleo.
- `TextField` (Flutter 3.47) tiene `obscureText`, `enableSuggestions`, `autocorrect`, `enableIMEPersonalizedLearning` (que el teclado no guarde historial ni diccionario personal) y `autofillHints`, que por omisión es una lista vacía.
- `PopScope(canPop: false)` desactiva el gesto y el botón de volver; `onPopInvokedWithResult` igual se llama con `didPop: false`.
- `TextEditingController.clear()` vacía el texto; `dispose()` libera el controlador.
- Mocking en frb: `RustLibApi` es el despachador central y se puede reemplazar con `RustLib.initMock`. En Dart, `VaultSession` es una clase abstracta que implementa `RustOpaqueInterface` (`dispose()` e `isDisposed`), así que un fake escrito a mano la puede implementar sin pasar por `RustLibApi`.
- Una función de la fachada con `#[frb(sync)]` corre en el hilo de la interfaz. Es aceptable para una función que no toca la `Vault` ni el `Mutex` y que es instantánea.

## Decisiones

### Regla de la contraseña maestra (núcleo)

Basada solo en la longitud, como recomienda NIST SP 800-63B, y no en tipos de caracteres (que aceptan `P@ssword1`):

| Caracteres | `PasswordStrength` | Efecto |
| --- | --- | --- |
| 0–11 | `TooShort` | No se permite crear ni cambiar a esa contraseña |
| 12–15 | `Acceptable` | Se permite |
| 16 o más | `Strong` | Se permite |

- Cuenta caracteres Unicode (`chars().count()`), no bytes ni grafemas. `ñ` cuenta 1; un emoji simple, 1. La contraseña no se recorta: los espacios cuentan.
- Nuevo módulo `core/src/vault/password.rs`:

  ```rust
  /// Longitud mínima de la contraseña maestra, en caracteres.
  pub const MIN_MASTER_PASSWORD_CHARS: usize = 12;
  /// Desde esta longitud la contraseña cuenta como fuerte.
  pub const STRONG_MASTER_PASSWORD_CHARS: usize = 16;

  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum PasswordStrength { TooShort, Acceptable, Strong }

  pub fn password_strength(password: &SecretString) -> PasswordStrength;
  ```

  Se exporta desde `vault` (`pub use password::{...}`), como pide `core/CLAUDE.md`. `PasswordStrength` puede derivar `Debug`: no contiene el secreto.
- Sin copias del secreto: cuenta sobre `expose_secret().chars()`, sin reservar memoria.
- Error nuevo `Error::PasswordTooShort` ("la contraseña maestra es demasiado corta"), sin datos.
- Se aplica **solo** en `create` y `change_master_password` (y sus variantes `_with_params` de prueba). Nunca en `unlock`, `unlock_with_key` ni `import`: las bóvedas existentes y los respaldos con contraseñas cortas siguen abriendo.
- Se comprueba primero, antes de cualquier trabajo caro: en `create`, antes de `require_absolute` y de `calibrate()`; en `change_master_password`, antes de verificar la contraseña actual (que corre Argon2id) y de calibrar. Sigue el precedente de `require_absolute` en `create`: fallar antes de gastar medio segundo. Consecuencia: con la contraseña actual incorrecta y una nueva corta, el error es `PasswordTooShort`, no `WrongPassword`.
- Para no depender de que cada llamada se acuerde, la comprobación vive en `create_with_params` y en un punto común de los dos `change_master_password*`; `create` la repite al principio solo para fallar antes de calibrar (como ya hace con la ruta).
- Es un cambio de API, no de formato: no sube `format_version` ni requiere una bóveda de ejemplo nueva. Las contraseñas de las bóvedas de ejemplo (`prueba-bóveda-v1`, `prueba-bóveda-v2`) tienen 16 caracteres.
- Las pruebas del núcleo y de la fachada que crean bóvedas o cambian la contraseña con menos de 12 caracteres (`"correcta"`, `"maestra"`, `"vieja"`, `"nueva"`, `"p"`, etc.) se ajustan a contraseñas de 12 o más.

### Fachada (`mobile/rust`)

- Nuevo archivo `rust/src/api/password.rs`:

  ```rust
  pub enum PasswordStrength { TooShort, Acceptable, Strong }   // propio, con From del núcleo

  #[frb(sync)]
  pub fn password_strength(password: String) -> PasswordStrength;
  ```

  Recibe el `String`, lo pasa a `SecretString` con el `secret_from` existente (copia exacta y `zeroize` del original) y delega. Sigue el precedente de la #35: la fachada define sus propios tipos de cruce. Un `enum` sin campos se genera como `enum` de Dart.
- Es `#[frb(sync)]`: no toca la `Vault` ni el `Mutex` y es instantánea. La regla de `mobile/CLAUDE.md` ("ninguna función que toque la bóveda es síncrona") no aplica, y se actualiza para nombrar esta excepción. La contraseña cruza en cada tecla; los búferes de cruce de frb ya están en el borrado de mejor esfuerzo de `ARCHITECTURE.md`.
- `VaultError::PasswordTooShort`, mapeada uno a uno desde `Error::PasswordTooShort`. El mapeo es exhaustivo, así que el compilador obliga a agregarla.

### Navegación: widget raíz por estado

Sin go_router. `MaterialApp(home: VaultGate(...))`, y `VaultGate` elige la pantalla según su estado:

| Estado | Pantalla |
| --- | --- |
| `checking` | Indicador de carga mientras se comprueba el archivo y se abre |
| `noVault` | `FirstRunScreen` |
| `locked` | Marcador de desbloqueo ("Desbloqueo: #37") |
| `unlocked` | Marcador de la pantalla principal ("Pantalla principal: #40") |
| `openFailed` | Mensaje genérico: "No se pudo abrir la bóveda." Nunca el primer uso |

Arranque:

1. ¿Existe el archivo? (`File(path).exists()`, decidido en la #35). Si no → `noVault`.
2. Si existe → `session.open()`. Si abre → `locked`; si falla con cualquier error, o si la comprobación del archivo lanza → `openFailed`.

Al crear con éxito, `FirstRunScreen` llama a `onCreated` y `VaultGate` pasa a `unlocked`. Como la pantalla se reemplaza (no se apila), su `State.dispose()` corre y limpia los campos. Las issues siguientes agregan transiciones (`locked → unlocked` en la #37, `unlocked → locked` en la #39) sobre este mismo estado.

`VaultGate` guarda la `VaultSession`, que vive mientras vive la app. No guarda contraseñas.

### Inyección

`main.dart` arma las dependencias y se las pasa a `BovedaApp`:

```dart
Future<void> main() async {
  await RustLib.init();
  final path = await vaultPath();
  runApp(BovedaApp(
    session: VaultSession(path: path),
    vaultExists: () => File(path).exists(),
    strengthOf: (p) => passwordStrength(password: p),
  ));
}
```

Las pruebas de widgets pasan un `FakeVaultSession` y funciones simuladas, sin cargar la librería nativa ni llamar a `RustLib.initMock`.

### Pantalla de primer uso (`FirstRunScreen`)

Contenido, de arriba abajo:

1. Título "Crea tu contraseña maestra" y la sugerencia: "Usa una frase de varias palabras. Mínimo 12 caracteres."
2. Campo "Contraseña maestra".
3. Indicador de fortaleza: barra de tres pasos y texto ("Demasiado corta", "Aceptable", "Fuerte"), con el valor de `strengthOf` en cada cambio. Con el campo vacío no se muestra.
4. Campo "Confirma la contraseña". Si no está vacío y no coincide, muestra "Las contraseñas no coinciden". La comparación es en Dart: es validación de la interfaz.
5. Casilla: "Entiendo que si olvido la contraseña maestra pierdo el acceso a la bóveda. No se puede recuperar."
6. Botón "Crear bóveda".

Reglas:

- El botón se activa solo si `strengthOf(contraseña) != TooShort`, la confirmación es igual y la casilla está marcada. Por encima del mínimo no hay aceptación extra: solo el indicador.
- Al tocarlo: los campos, la casilla y el botón se desactivan; el botón muestra un indicador indeterminado y "Creando la bóveda…". `PopScope(canPop: false)` mientras dura. No hay cancelar: `create` no se puede interrumpir.
- La contraseña se lee del controlador en una variable local, se pasa a `session.create` y no se guarda en ningún campo del `State` ni fuera de él.
- Con éxito: `onCreated()`; la pantalla sale del árbol y se limpia en `dispose()`.
- Con error: vuelve a activar el formulario y muestra un mensaje en la pantalla, sin detalles internos. `PasswordTooShort` → "La contraseña debe tener al menos 12 caracteres." Cualquier otro → "No se pudo crear la bóveda. Intenta de nuevo." Los campos se conservan para reintentar.
- Se comprueba `mounted` después de cada `await`.

### Campo de contraseña (`PasswordField`)

Widget reutilizable, que también usarán la #37 y la #44:

- `obscureText: true` por omisión, con un botón para mostrar u ocultar (ícono de ojo) en `suffixIcon`.
- `enableSuggestions: false`, `autocorrect: false`, `enableIMEPersonalizedLearning: false`.
- Sin `autofillHints` (lista vacía, el valor por omisión) y sin `AutofillGroup`, para que el gestor de contraseñas del sistema no ofrezca guardar la maestra.
- `keyboardType: TextInputType.visiblePassword`.
- Recibe el `TextEditingController` de quien lo usa; no lo crea ni lo libera.

### Limpieza de los campos

`FirstRunScreen` es dueña de sus dos `TextEditingController`. En `dispose()` llama a `clear()` y después a `dispose()` en cada uno. Es de mejor esfuerzo: Dart no permite borrar sus cadenas (`ARCHITECTURE.md`).

### Archivos

| Archivo | Contenido |
| --- | --- |
| `core/src/vault/password.rs` | `PasswordStrength`, `password_strength`, constantes; pruebas en `password/tests.rs` |
| `core/src/error.rs` | `Error::PasswordTooShort` |
| `core/src/vault/mod.rs` | comprobación en `create` y `change_master_password*`; `pub use` |
| `mobile/rust/src/api/password.rs` | `PasswordStrength` de la fachada y `password_strength` síncrona; pruebas en `password/tests.rs` |
| `mobile/rust/src/api/error.rs` | `VaultError::PasswordTooShort` |
| `mobile/lib/main.dart` | arma las dependencias y llama a `runApp` |
| `mobile/lib/app.dart` | `BovedaApp` (`MaterialApp`) y `VaultGate` |
| `mobile/lib/screens/first_run_screen.dart` | `FirstRunScreen` |
| `mobile/lib/screens/placeholder_screen.dart` | marcador con un texto, para desbloqueo y principal |
| `mobile/lib/widgets/password_field.dart` | `PasswordField` |
| `mobile/lib/widgets/strength_indicator.dart` | barra y texto de la fortaleza |
| `mobile/test/fakes.dart` | `FakeVaultSession` |
| `mobile/test/app_test.dart`, `mobile/test/first_run_screen_test.dart` | pruebas de widgets; reemplazan `widget_test.dart` |

`lib/app.dart` no está en el mapa de `ARCHITECTURE.md`: se agrega (el widget raíz no es una pantalla).

`mocktail` deja de usarse (el fake es a mano) y se quita de `dev_dependencies`.

## Pruebas

### Núcleo (`core/`)

- `password_strength`: 0, 11, 12, 15 y 16 caracteres en los límites; 12 `ñ` (24 bytes) da `Acceptable` y 11 `ñ` da `TooShort` (cuenta caracteres, no bytes); espacios y emoji cuentan.
- `create` con 11 caracteres da `PasswordTooShort` sin crear el archivo y sin calibrar (llamada a `Vault::create` real, que regresa antes de `calibrate()`).
- `create_with_params` con 11 caracteres da `PasswordTooShort`.
- `change_master_password_with_params` con una nueva de 11 caracteres da `PasswordTooShort`, y la contraseña anterior sigue desbloqueando.
- `unlock` e `import` no aplican la regla: una bóveda con una contraseña corta, creada con la API interna (`VaultHeader::create` y `Storage::create`, sin pasar por `create`), desbloquea, exporta e importa.
- Las bóvedas de ejemplo siguen abriendo (pruebas existentes).
- Se ajustan las contraseñas cortas de las pruebas existentes.

### Fachada (`mobile/rust`)

- `password_strength` devuelve el valor del núcleo en los tres casos.
- `create` con una contraseña corta da `VaultError::PasswordTooShort` y la sesión sigue sin bóveda.
- El mapeo de errores incluye `PasswordTooShort`.

### Widgets (`mobile/test/`)

Con `FakeVaultSession` (implementa `VaultSession` a mano; `create` y `open` devuelven un `Future` controlable con `Completer` o un error configurable) y funciones simuladas para `vaultExists` y `strengthOf`:

- Sin archivo → primer uso. Con archivo y `open` correcto → marcador de desbloqueo. Con `open` fallido → mensaje genérico, sin primer uso. Con `vaultExists` que lanza → mensaje genérico.
- El botón está desactivado hasta cumplir las tres condiciones, y cada condición por separado lo mantiene desactivado (corta, confirmación distinta, casilla sin marcar).
- El indicador cambia al escribir (`TooShort` → `Acceptable` → `Strong`) y no aparece con el campo vacío.
- La confirmación distinta muestra "Las contraseñas no coinciden".
- Durante la creación: indicador visible, botón y campos desactivados, y volver no saca de la pantalla. Al completar el `Future`, aparece el marcador de la pantalla principal y `create` recibió la contraseña escrita.
- Un error de `create` muestra el mensaje genérico y vuelve a activar el formulario; `PasswordTooShort` muestra el de la longitud.
- Configuración de los campos: `obscureText`, `enableSuggestions`, `autocorrect` y `enableIMEPersonalizedLearning` como se decidió, y `autofillHints` vacío. El botón de ver alterna `obscureText`.

### A mano, en el emulador

- Instalación limpia → primer uso → crear con una frase de 16 o más → marcador de la pantalla principal; la interfaz responde durante la calibración.
- Cerrar y volver a abrir la app → marcador de desbloqueo, no primer uso.
- El teclado no sugiere ni recuerda lo escrito en los campos.

## Documentación

- `docs/especificacion.md`, sección "API": decisión del 2026-11-08 (#36), la regla de la contraseña maestra (mínimo de 12 caracteres Unicode, fuerte desde 16, solo en `create` y `change_master_password`, nunca en `unlock` ni en `import`; `PasswordTooShort`; cambio de API, no de formato).
- `ARCHITECTURE.md`: `vault/password.rs` en la tabla del núcleo, `api/password.rs` en el mapa de la fachada, y `lib/app.dart`, `screens/` y `widgets/` con sus archivos.
- `mobile/CLAUDE.md`: la excepción `password_strength` a la regla de `#[frb(sync)]`.

## Issues nuevas (después de tu confirmación)

- **Normalización NFC de la contraseña maestra** (prioridad: antes de cerrar la etapa 2, cuando aún no hay bóvedas reales que migrar). La misma contraseña escrita con acentos compuestos o descompuestos produce otros bytes para Argon2id, y también cuenta distinto para el mínimo de 12. Requiere un crate nuevo (`unicode-normalization`) y decidir cómo quedan las bóvedas ya creadas.
- **`-journal` huérfano en `create`**: `create` usa `create_new` solo sobre la base; si queda un `boveda.db-journal` sin la base, SQLite podría aplicarlo sobre la bóveda nueva. `import` ya trata ese caso. Hay que verificarlo con un diario real antes de corregirlo.
