# Diseño: desbloqueo con huella y copia de la llave en el Keystore (issue #38)

Fecha: 2026-11-08 · Etapa 2 · Rama `feat/38-huella-keystore`

## Objetivo

Guardar una copia cifrada de la llave de bóveda con una llave del Keystore que solo se usa con huella, y desbloquear con ella. La app ofrece activar la huella al terminar el primer uso y desde la pantalla de desbloqueo con contraseña. Si la llave del Keystore queda invalidada (por ejemplo, al registrar una huella nueva) o el núcleo rechaza la llave, la app desactiva la huella, avisa y pide la contraseña.

Usa la API del núcleo de la #34 (`export_vault_key`, `unlock_with_key`, `WrongVaultKey`) y amplía la pantalla de desbloqueo de la #37.

### Fuera de alcance

- **Fecha del último desbloqueo con contraseña.** La issue la incluía para preparar la regla de 7 días, pero esa regla es de la etapa 4 y `CLAUDE.md` pide no salir de la etapa actual. Queda anotado para la etapa 4: si no hay fecha, se pide la contraseña. Eso también resuelve el caso de quien activa la huella al crear la bóveda, sin escribir nada ahora. Se publica como comentario en la #38.
- Pedir la contraseña cada 7 días (etapa 4).
- Huella para enviar a la PC (etapa 3).
- Ajustes de huella y la invitación al primer respaldo (etapa 4).
- Bloqueo automático (#39): esta issue solo hace que la pantalla de desbloqueo, al abrirse, intente la huella.

### Criterios de aceptación (de la issue, con la fecha movida)

- Activar la huella y desbloquear con ella en el celular y en el emulador (huella simulada).
- Cancelar la huella permite usar la contraseña.
- Registrar una huella nueva invalida la copia: la app pide la contraseña y desactiva la huella.
- La copia vive fuera del archivo de la bóveda. (La fecha pasó a la etapa 4.)
- Ninguna llave ni material aparece en logs ni en errores.
- `flutter analyze`, `flutter test` y `cargo test --workspace` pasan; CI en verde.

## Lo que confirma la documentación

Verificado con Context7 (documentación de Android y AndroidX, flutter_rust_bridge 2.13.0), Maven de Google y pub.dev:

- Una llave del Keystore con `setUserAuthenticationRequired(true)` y sin duración de validez positiva exige autenticación **en cada uso**, y solo con biometría: se inicializa un `Cipher` con ella, se envuelve en un `BiometricPrompt.CryptoObject` y se usa solo si la autenticación tiene éxito.
- `setInvalidatedByBiometricEnrollment(true)` (el valor por omisión, API 24+) invalida la llave de forma irreversible al registrar una huella nueva o al borrar todas. También queda invalidada si se quita el bloqueo seguro de la pantalla. Se manifiesta como `KeyPermanentlyInvalidatedException` al inicializar el `Cipher`.
- Una llave simétrica que exige autenticación la exige también para cifrar: activar la huella muestra el diálogo.
- `BiometricPrompt` de la plataforma (`android.hardware.biometrics`): con `CryptoObject`, el autenticador por omisión es `BIOMETRIC_STRONG`. `setAllowedAuthenticators`, `BiometricManager.canAuthenticate(int)` y `KeyGenParameterSpec.Builder.setUserAuthenticationParameters` existen desde la API 30. `setNegativeButton` es obligatorio sin credencial del dispositivo. `canAuthenticate` requiere el permiso `USE_BIOMETRIC` (normal, no se pide al usuario).
- Códigos de error de `BiometricPrompt`: `ERROR_HW_UNAVAILABLE` (1), `ERROR_UNABLE_TO_PROCESS` (2), `ERROR_TIMEOUT` (3), `ERROR_NO_SPACE` (4), `ERROR_CANCELED` (5), `ERROR_LOCKOUT` (7), `ERROR_VENDOR` (8), `ERROR_LOCKOUT_PERMANENT` (9), `ERROR_USER_CANCELED` (10), `ERROR_NO_BIOMETRICS` (11), `ERROR_HW_NOT_PRESENT` (12), `ERROR_NEGATIVE_BUTTON` (13). `onAuthenticationFailed` (dedo no reconocido) no termina el diálogo.
- `androidx.biometric`: la última estable es la 1.1.0 (la 1.4.0 sigue en beta); exige `FlutterFragmentActivity` y agrega `appcompat` 1.2.0. Descartada (decisión 1).
- `biometric_storage` (pub.dev, 5.0.1, sin estable nueva en dos años) guarda `String` y exige `FlutterFragmentActivity`: descartado, porque la llave no puede cruzar como `String`. `local_auth` no expone `CryptoObject`.
- flutter_rust_bridge: un `Vec<u8>` devuelto por Rust llega a Dart como `Uint8List`, sin copia en modo asíncrono (la memoria pasa a ser de Dart). Un `Vec<u8>` como argumento aparece en Dart como `List<int>`.
- Flutter 3.47: `minSdk` por omisión 24 y `targetSdk` 36.

## Decisiones

1. **Canal propio en Kotlin con la API de la plataforma, sin dependencias nuevas.** La huella se ofrece solo desde la API 30 (Android 11); en versiones anteriores queda solo la contraseña, sin subir `minSdk`. Se descarta `androidx.biometric`: no compensa una dependencia nueva ni el cambio a `FlutterFragmentActivity` solo para cubrir Android 6 a 10.
2. **Solo `BIOMETRIC_STRONG`**, sin PIN ni credencial del dispositivo. La alternativa a la huella es la contraseña maestra.
3. **La llave cruza por Dart, sin JNI**, como dice la #34. JNI exigiría el crate `jni` y un registro global de sesiones compartido entre frb y JNI, contra la decisión de la #35 de no usar estado `static`, para ahorrar un solo salto. Todo cruza como bytes (`Vec<u8>` ↔ `Uint8List` ↔ `ByteArray`), nunca como `String` ni en base64. Dart y Kotlin ponen en ceros cada arreglo apenas lo entregan, como mejor esfuerzo. Los búferes de `StandardMessageCodec` quedan en la misma excepción documentada que los de flutter_rust_bridge.
4. **Fachada:** `export_vault_key() -> Vec<u8>` y `unlock_with_key(key: Vec<u8>) -> UnlockReport`, ambos asíncronos (sección "Fachada").
5. **Copia en `noBackupFilesDir/biometric/vault_key.bin`**, administrada solo por Kotlin; el texto cifrado nunca cruza a Dart (sección "Kotlin").
6. **Activar en dos pasos:** primero la huella, después la llave sale del núcleo. Si el usuario cancela, la llave nunca sale de Rust.
7. **Qué desactiva la huella:** `KeyPermanentlyInvalidatedException`, `WrongVaultKey`, archivo ilegible o llave del Keystore inexistente. No la desactivan la cancelación, el bloqueo temporal por intentos ni ningún otro error.
8. **Dónde se ofrece:** (a) al terminar el primer uso, una pantalla propia; (b) en la pantalla de desbloqueo con contraseña, una casilla. Ninguna aparece si el dispositivo tiene menos de la API 30, si `canAuthenticate(BIOMETRIC_STRONG)` no da `SUCCESS` o si la huella ya está activa.
9. **Sin fecha del último desbloqueo con contraseña** (ver "Fuera de alcance").
10. **`disable()`** borra el archivo y la llave del Keystore, sin error si no existen. El primer uso lo llama antes de crear la bóveda; la #44 lo usará al importar.
11. **Pruebas:** Dart con un `BiometricService` abstracto y un doble escrito a mano; Rust en la fachada; Kotlin a mano en el emulador y en el celular (sección "Pruebas").

## Kotlin: Keystore, diálogo y canal

### Archivos

| Archivo | Cambio |
| --- | --- |
| `android/app/src/main/kotlin/com/jesus/boveda/BiometricVault.kt` | Nuevo: llave del Keystore, archivo de la copia y `BiometricPrompt` |
| `android/app/src/main/kotlin/com/jesus/boveda/MainActivity.kt` | Registra el canal `boveda/biometric` junto a `boveda/storage`; avisa a `BiometricVault` cuando la actividad se destruye |
| `android/app/src/main/AndroidManifest.xml` | `<uses-permission android:name="android.permission.USE_BIOMETRIC"/>` |

### Llave del Keystore

- Alias `boveda_biometric_wrap`, en `AndroidKeyStore`.
- `KeyGenParameterSpec`: `PURPOSE_ENCRYPT | PURPOSE_DECRYPT`, `BLOCK_MODE_GCM`, `ENCRYPTION_PADDING_NONE`, `setKeySize(256)`, `setUserAuthenticationRequired(true)`, `setUserAuthenticationParameters(0, KeyProperties.AUTH_BIOMETRIC_STRONG)` y `setInvalidatedByBiometricEnrollment(true)`. Las dos últimas se escriben aunque coincidan con el valor por omisión, para que el código documente la intención.
- Cifrador `AES/GCM/NoPadding`, etiqueta de 128 bits. No se desactiva `setRandomizedEncryptionRequired`: el IV lo genera el Keystore al cifrar y se lee de `cipher.iv` (12 bytes).
- Dato autenticado de GCM (`updateAAD`): el byte de versión del archivo. Cambiar la versión hace fallar `doFinal`, que cuenta como archivo ilegible.
- Todo el código del Keystore y del diálogo lleva `@RequiresApi(30)`. Con una API menor, el canal responde que la huella no está disponible sin tocar el Keystore.

### Archivo de la copia

`noBackupFilesDir/biometric/vault_key.bin`, 61 bytes:

| Offset | Campo | Tamaño |
| --- | --- | --- |
| 0 | Versión (`1`) | 1 byte |
| 1–12 | IV de GCM | 12 bytes |
| 13–60 | Texto cifrado: llave de bóveda (32) y etiqueta (16) | 48 bytes |

- Fuera de Auto Backup y de la transferencia entre dispositivos, como la bóveda (#35): restaurado en otro celular no serviría, porque la llave del Keystore no viaja.
- Escritura atómica: `vault_key.bin.tmp` → `FileOutputStream` con `fd.sync()` → `renameTo(vault_key.bin)` en el mismo directorio. Si `renameTo` devuelve `false`, es un fallo (`failed`): se borran el `.tmp` y la llave del Keystore, y no queda la huella a medias.
- Cuenta como **ilegible** si no existe, si no mide exactamente 61 bytes, si la versión no es `1` o si `doFinal` falla (`AEADBadTagException`).

### Métodos del canal `boveda/biometric`

Los errores salen con `result.error(código, null, null)`: sin mensaje ni detalles, para que ninguna excepción cruce texto.

| Método | Comportamiento |
| --- | --- |
| `isAvailable` | `true` solo si la API es ≥ 30 y `canAuthenticate(BIOMETRIC_STRONG) == BIOMETRIC_SUCCESS` |
| `isEnabled` | `true` si existe `vault_key.bin`. No prueba el Keystore: una llave invalidada se descubre al desbloquear |
| `beginEnable` | Borra el archivo, el `.tmp` y la llave anteriores; crea la llave; `cipher.init(ENCRYPT_MODE)`; muestra el diálogo "Activar huella" (botón negativo "Cancelar"). Con éxito guarda el `Cipher` autenticado en un campo y responde `null`. Si falla, borra la llave y responde con el código |
| `completeEnable(key: ByteArray)` | `doFinal(key)` con el `Cipher` pendiente y escritura atómica del archivo. `key.fill(0)` en un `finally`, también si falla. Si falla cualquier paso, o si no hay `Cipher` pendiente: borra la llave, el archivo y el `.tmp`, y responde `failed` |
| `abortEnable` | Descarta el `Cipher` pendiente y borra la llave recién creada (cuando `export_vault_key` falla en Dart) |
| `unlock` | Lee el archivo; obtiene la llave del Keystore; `cipher.init(DECRYPT_MODE, GCMParameterSpec(128, iv))`; muestra el diálogo "Desbloquear Bóveda" (botón negativo "Usar contraseña"). Con éxito: `doFinal`, `result.success(bytes)` y `bytes.fill(0)` |
| `disable` | Borra el archivo, el `.tmp` y la llave del Keystore. Sin error si no existen |

Reglas:

- **`Cipher` pendiente:** solo vive entre `beginEnable` y `completeEnable` o `abortEnable`. Si la actividad se destruye antes, se descarta y se borra la llave recién creada. Si un cierre forzado deja una llave sin archivo, no pasa nada: `isEnabled` da `false` y el siguiente `beginEnable` la borra.
- **Borrado del arreglo descifrado:** `result.success(bytes)` y `bytes.fill(0)` se llaman en el hilo principal (el `mainExecutor` del diálogo), porque la copia síncrona del codec dentro de `success` solo está garantizada ahí. Un comentario lo documenta como mejor esfuerzo, igual que el resto del borrado fuera del núcleo.
- **Un diálogo a la vez:** si ya hay uno abierto, el método responde `busy`.
- El diálogo usa `mainExecutor` y un `CancellationSignal` que se cancela si la actividad se destruye.
- Kotlin nunca borra la copia por su cuenta al desbloquear: Dart decide y llama a `disable` (una sola regla, para `invalidated` y para `WrongVaultKey`).
- Nada se escribe en logs.

### Códigos de error

| Código | Origen | ¿Desactiva la huella? |
| --- | --- | --- |
| `invalidated` | `KeyPermanentlyInvalidatedException`, llave del Keystore inexistente o archivo ilegible | Sí (Dart llama a `disable`) |
| `canceled` | `ERROR_USER_CANCELED`, `ERROR_NEGATIVE_BUTTON`, `ERROR_CANCELED` | No |
| `lockout` | `ERROR_LOCKOUT`, `ERROR_LOCKOUT_PERMANENT` | No |
| `failed` | Cualquier otro error del diálogo (incluidos `ERROR_TIMEOUT`, `ERROR_HW_UNAVAILABLE` y `ERROR_NO_BIOMETRICS`) o del Keystore | No |
| `busy` | Ya hay un diálogo abierto | No |

Solo los casos de `invalidated` desactivan la huella.

## Fachada (`mobile/rust/src/api/session.rs`)

Dos métodos nuevos de `VaultSession`, sin `#[frb(sync)]`:

```rust
/// Entrega la llave de bóveda para la copia del Keystore. Bloqueada: `Locked`.
pub fn export_vault_key(&self) -> FacadeResult<Vec<u8>>;

/// Desbloquea con la llave que liberó el Keystore. Largo distinto de 32: `WrongVaultKey`.
pub fn unlock_with_key(&self, key: Vec<u8>) -> FacadeResult<UnlockReport>;
```

**`export_vault_key`:** `with_vault(Vault::export_vault_key)` y `expose_secret().to_vec()`. El `SecretBox` del núcleo se borra al soltarse. El `Vec` devuelto es la copia que sale del núcleo, en la misma excepción que `reveal_password`; al llegar a Dart es memoria de Dart, y ponerlo en ceros es responsabilidad de `enableBiometric` (sección "Dart"). Sin bóveda: `NoVault`; bloqueada: `Locked`; si no se puede reparar `key_check`, el error de almacenamiento que ya existe.

**`unlock_with_key`:**

1. Lo primero es envolver el argumento en `Zeroizing<Vec<u8>>`: se pone en ceros al soltarse en todos los caminos, incluidos los errores y el `?` del `Mutex`.
2. Si el largo no es 32: `WrongVaultKey`, sin tocar la `Vault` ni agregar una variante.
3. Si es 32: la llave se arma como lo hace el núcleo en `export_vault_key` (`core/src/vault/mod.rs`), para que ambos lados se lean igual: un `SecretBox<[u8; 32]>` con `default()` y `expose_secret_mut().copy_from_slice(&key)`, sin pasar por la pila. Después, `with_vault(|v| v.unlock_with_key(key))` y la conversión a `UnlockReport`.
4. Caso límite documentado: con un largo incorrecto y la bóveda ya desbloqueada, da `WrongVaultKey` y no `AlreadyUnlocked`, porque la fachada no consulta el estado. En la práctica no ocurre: la pantalla de desbloqueo solo existe con la bóveda bloqueada.

Sin variantes nuevas en `VaultError` ni dependencias nuevas (`zeroize` y `secrecy` ya están). Se regenera el puente con `flutter_rust_bridge_codegen generate`, sin editar a mano el código generado, y antes de `cargo test`. El codegen 2.13.0 y `cargo-expand` 1.0.127 ya están instalados: no se instala nada.

## Dart

### Servicio (`lib/services/biometric_service.dart`)

```dart
enum BiometricFailure { invalidated, canceled, lockout, failed, busy }

class BiometricException implements Exception {
  const BiometricException(this.failure);
  final BiometricFailure failure;
}

abstract class BiometricService {
  Future<bool> isAvailable();
  Future<bool> isEnabled();
  Future<void> beginEnable();
  Future<void> completeEnable(Uint8List key);
  Future<void> abortEnable();
  Future<Uint8List> unlock();
  Future<void> disable();
}
```

- `BiometricException` no agrega detalles en `toString`: el código es todo lo que cruza.
- `PlatformBiometricService` implementa el canal y convierte el `code` de cada `PlatformException` en un `BiometricFailure`. Un código desconocido o una `MissingPluginException` se convierten en `failed`.
- `isAvailable` e `isEnabled` dan `false` si el canal falla.
- `completeEnable` envía la llave por el canal como `Uint8List`.

### Flujos (`lib/services/biometric_unlock.dart`)

Funciones sin estado que reciben la sesión y el servicio.

**`enableBiometric(session, biometric) → EnableOutcome { enabled, canceled, failed }`**

1. `beginEnable()`. `canceled` → `canceled`. Cualquier otro error → `failed` (Kotlin ya borró la llave).
2. `final key = await session.exportVaultKey()`. Si falla → `abortEnable()` y `failed`.
3. `completeEnable(key)` en un `try`, con `key.fillRange(0, key.length, 0)` en el `finally`, también si falla. Error → `failed`. `invokeMethod` codifica el mensaje antes de su primer `await`, así que los ceros no alcanzan la copia en tránsito.

**`unlockWithBiometric(session, biometric) → BiometricUnlockOutcome`**

| Paso | Resultado |
| --- | --- |
| `biometric.unlock()` da `invalidated` | `disable()` → `disabled` |
| `canceled`, `lockout`, `failed` o `busy` | `fallback`, sin mensaje |
| `session.unlockWithKey(key: key)` con éxito | `unlocked(corrupt.length + unidentifiedCorrupt)` |
| `VaultError.alreadyUnlocked` | `unlocked(0)` |
| `VaultError.wrongVaultKey` | `disable()` → `disabled` |
| Cualquier otro error, `VaultError` o no | `coreError` |

- `key.fillRange(0, …)` en un `finally` después de `unlockWithKey`, en todos los caminos.
- Si `disable()` falla, igual devuelve `disabled`: la copia que quede dará `invalidated` o `WrongVaultKey` en el siguiente intento y se volverá a desactivar.

### `VaultGate` (`lib/app.dart`) y `main.dart`

- `BovedaApp` y `VaultGate` reciben un `BiometricService`; `main.dart` pasa `PlatformBiometricService()`.
- Estado nuevo `offerBiometric` → `BiometricOfferScreen`.
- Al terminar `create`: si `isAvailable() && !isEnabled()`, pasa a `offerBiometric`; si no, a `unlocked`, como hoy. Si la comprobación lanza, cuenta como `false`.
- `UnlockScreen` recibe el servicio.

### Oferta del primer uso (`lib/screens/biometric_offer_screen.dart`)

- Título "¿Activar huella?". Texto: "Desbloquea la bóveda con tu huella. La contraseña maestra seguirá funcionando y la necesitarás si registras una huella nueva."
- Botones "Activar" y "Ahora no"; ambos desactivados mientras corre `enableBiometric`.
- "Activar": `enabled` → `unlocked`; `canceled` → se queda, sin mensaje; `failed` → se queda con "No se pudo activar la huella." y se puede reintentar o tocar "Ahora no".
- "Ahora no" → `unlocked`.
- Cumple "ofrece activar la huella" del primer uso en la especificación.

### Primer uso (`FirstRunScreen`)

Antes de `session.create`, llama a `biometric.disable()` como mejor esfuerzo: si falla, crea la bóveda igual. Una copia de una bóveda anterior no abre la nueva: `key_check` la rechaza con `WrongVaultKey` y eso la desactiva. Bloquear la creación por un Keystore averiado dejaría al usuario sin bóveda.

### Pantalla de desbloqueo (`UnlockScreen`)

**Intento automático:**

- Se lanza con `WidgetsBinding.instance.addPostFrameCallback` desde `initState`, para que el diálogo salga después del primer cuadro y no choque con la construcción de la pantalla.
- Corre una sola vez por instancia de `UnlockScreen`: si la pantalla se reconstruye tras un `fallback` o un `disabled`, no vuelve a abrir el diálogo. Para reintentar está el botón "Usar huella".
- Solo si `isEnabled()`. Mientras corre, el campo y el botón se desactivan y el botón muestra "Desbloqueando…".

| Resultado | Acción |
| --- | --- |
| `unlocked(n)` | `onUnlocked(n)` |
| `fallback` | Campo activo, con el foco y sin mensaje (nota de la #37: el foco se pide cuando la animación de arranque ya terminó) |
| `disabled` | Aviso "La huella se desactivó. Desbloquea con tu contraseña maestra.", foco en el campo; vuelve a evaluar `isAvailable() && !isEnabled()`: se oculta "Usar huella" y aparece la casilla, para reactivarla en este mismo desbloqueo |
| `coreError` | "No se pudo desbloquear la bóveda. Intenta de nuevo." (#37), foco en el campo |

**Botón "Usar huella":** debajo de "Desbloquear", visible mientras la huella está activa. Vuelve a correr `unlockWithBiometric` con la misma tabla. Cubre la cancelación por error y el caso de que el diálogo no aparezca en el arranque en frío.

**Casilla "Usar huella para desbloquear":**

- Sin marcar; visible solo si `isAvailable() && !isEnabled()`.
- Con la casilla marcada y la contraseña correcta, corre `enableBiometric` antes de `onUnlocked`; el botón muestra "Activando huella…".
- `canceled` → `onUnlocked` sin mensaje. `failed` → `onUnlocked` y un `SnackBar` "No se pudo activar la huella.".
- El `ScaffoldMessenger` se obtiene con `ScaffoldMessenger.of(context)` antes del primer `await` del flujo (y antes de `onUnlocked`), y se usa después sin volver a leer el `context`: no depende de `mounted` y sale en el `ScaffoldMessenger` raíz de `MaterialApp`, que sobrevive al cambio de pantalla.
- Con la contraseña incorrecta no se intenta activar nada.

**Otras reglas:**

- Una sola guarda `_busy` para la huella y la contraseña: nunca corren dos desbloqueos a la vez.
- Se comprueba `mounted` después de cada `await` (salvo el uso del `ScaffoldMessenger` ya obtenido).
- El resto del comportamiento de la #37 no cambia.

### Archivos de Dart

| Archivo | Contenido |
| --- | --- |
| `lib/services/biometric_service.dart` | `BiometricFailure`, `BiometricException`, `BiometricService`, `PlatformBiometricService` |
| `lib/services/biometric_unlock.dart` | `enableBiometric`, `unlockWithBiometric` y sus resultados |
| `lib/screens/biometric_offer_screen.dart` | `BiometricOfferScreen` |
| `lib/screens/unlock_screen.dart` | Intento automático, "Usar huella", casilla y aviso |
| `lib/screens/first_run_screen.dart` | `disable()` antes de `create` |
| `lib/app.dart`, `lib/main.dart` | Servicio inyectado y estado `offerBiometric` |
| `test/fakes.dart` | `FakeBiometricService`; `exportVaultKey` y `unlockWithKey` en `FakeVaultSession` |

## Pruebas

### Fachada (`mobile/rust/src/api/session/tests.rs`)

Sobre la copia de `vault-v2.db` (`test_support::fixture_copy()`), nunca sobre el original:

- Ida y vuelta: `open` → `unlock` → `export_vault_key` (32 bytes) → `lock` → `unlock_with_key`: `UnlockReport` vacío y `search("")` con las tres entradas.
- Largo 31 y 33: `WrongVaultKey`; sigue bloqueada.
- 32 bytes en ceros (llave alterada, del largo correcto, que no viene de ninguna bóveda): `WrongVaultKey`; sigue bloqueada.
- Llave de otra bóveda (exportada de una creada en un directorio temporal): `WrongVaultKey`.
- `export_vault_key` bloqueada: `Locked`.
- `unlock_with_key` desbloqueada: `AlreadyUnlocked`.
- Sin bóveda, ambos: `NoVault`.

Que el `Vec` quede en ceros lo garantiza `Zeroizing`; comprobarlo desde afuera exigiría leer memoria liberada.

### Widgets (`mobile/test/`)

Los dobles guardan la referencia de cada `Uint8List` que reciben, para comprobar que queda en ceros.

`biometric_unlock_test.dart`:

- `enableBiometric`: orden `beginEnable` → `exportVaultKey` → `completeEnable` y la llave en ceros; `beginEnable` con `canceled` → `canceled` sin llamar a `exportVaultKey`; con `lockout` o `failed` → `failed`; `exportVaultKey` falla → `abortEnable` y `failed`; `completeEnable` falla → `failed` y la llave en ceros.
- `unlockWithBiometric`: éxito → `unlocked(3)` (2 ids y 1 sin id) y la llave en ceros; `invalidated` → `disable` y `disabled`; `wrongVaultKey` → `disable`, `disabled` y la llave en ceros; `canceled`, `lockout`, `failed` y `busy` → `fallback` sin `disable`; `alreadyUnlocked` → `unlocked(0)` y la llave en ceros; `storageIo` y una excepción que no es `VaultError` → `coreError` y la llave en ceros; `disable` que falla → igual `disabled`.

`biometric_service_test.dart` (`PlatformBiometricService` con un manejador simulado del canal en `TestDefaultBinaryMessengerBinding`):

- Cada `code` produce su `BiometricFailure`; uno desconocido produce `failed`.
- `unlock` devuelve el `Uint8List`.
- `completeEnable` envía la llave como `Uint8List` (no `String` ni una `List<int>` genérica): deja fijada la regla de que la llave cruza solo como bytes.
- `isAvailable` e `isEnabled` dan `false` si el canal lanza.

`unlock_screen_test.dart` (ampliado):

- Con huella activa, el diálogo se pide una sola vez después del primer cuadro y no se repite al reconstruir.
- Durante el intento: campo y botón desactivados, "Desbloqueando…".
- `fallback`: foco en el campo, sin mensaje. `disabled`: aviso, sin "Usar huella", con la casilla. `coreError`: mensaje genérico.
- "Usar huella" vuelve a abrir el diálogo; no aparece sin huella activa.
- La casilla no aparece sin `isAvailable` ni con la huella activa.
- Casilla marcada y contraseña correcta: `enableBiometric` antes de `onUnlocked`; con `canceled`, sin `SnackBar`; con `failed`, el `SnackBar` visible después del cambio de pantalla (montando `BovedaApp`).
- Casilla marcada y contraseña incorrecta: no llama a `beginEnable` ni a `exportVaultKey`.
- Casilla sin marcar: no llama a `beginEnable`.
- Tocar "Desbloquear" mientras corre la huella no llama a `unlock`.
- Desmontar la pantalla con la huella pendiente (`Completer`) no produce excepciones.

`biometric_offer_screen_test.dart`:

- "Activar" con `enabled` → pantalla principal; `canceled` → se queda sin mensaje; `failed` → mensaje y se puede reintentar; "Ahora no" → pantalla principal; botones desactivados mientras corre.

`app_test.dart` y `first_run_screen_test.dart`:

- Después de crear, con huella disponible y sin activar, aparece la oferta; sin huella disponible, la pantalla principal; `isAvailable` que lanza cuenta como `false`.
- `FirstRunScreen` llama a `disable` antes de `create`; si `disable` falla, igual crea la bóveda.

### A mano, en el emulador (API ≥ 30, con una huella registrada en Ajustes)

1. Instalación limpia → primer uso → "¿Activar huella?" → "Activar" → `adb -e emu finger touch 1` → pantalla principal.
2. Cerrar la app y abrirla: aparece el diálogo sin tocar nada → `finger touch 1` → pantalla principal. Si el diálogo no aparece en el arranque en frío, se documenta con logcat y se usa "Usar huella".
3. Cancelar con "Usar contraseña": foco en el campo, la huella sigue activa. "Usar huella" vuelve a abrir el diálogo.
4. Dedo no reconocido (`finger touch 2`, sin registrar) hasta el bloqueo: contraseña disponible, la huella sigue activa.
5. Registrar una huella nueva en Ajustes y abrir la app: aviso de huella desactivada y la casilla. Desbloquear con la contraseña y la casilla marcada la reactiva.
6. Cerrar y abrir otra vez: la huella reactivada abre la bóveda (la llave nueva reemplazó a la invalidada).
7. Con `run-as`: `no_backup/biometric/vault_key.bin` existe y mide 61 bytes, y no hay `.tmp`.
8. En otra instalación limpia, en la oferta del primer uso: "Activar" y cancelar el diálogo → se queda en la oferta; "Ahora no" → con `run-as`, no existe `vault_key.bin`.
9. logcat filtrado por el paquete, sin llaves ni bytes.
10. En el celular, los pasos 1 a 6. Antes, confirmar que tiene Android 11 o posterior.

### Antes de dar la issue por terminada

`cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all`, `flutter analyze`, `flutter test` y el puente regenerado sin diferencias, como comprueba `mobile.yml`.

## Hallazgos de la prueba a mano

En el emulador (Android 16, API 37) aparecieron dos fallos que las pruebas automáticas no podían ver. Se corrigieron en commits `fix:` propios:

1. **`updateAAD` antes de la huella.** keystore2 rechaza `updateAad` sobre una llave que exige autenticación en cada uso si todavía no hay huella (`KEY_USER_NOT_AUTHENTICATED`, "No operation auth token received") y aborta la operación; el `doFinal` posterior falla con `IllegalBlockSizeException`. El dato autenticado se pasa ahora en el callback de éxito, justo antes de `doFinal`, al cifrar y al descifrar. Regla: con una llave así, ninguna operación del `Cipher` salvo `init` va antes de la huella.
2. **El canal entrega una vista no modificable.** El `Uint8List` que llega de Kotlin por `StandardMessageCodec` es un `_UnmodifiableUint8ArrayView`: `fillRange` lanza `UnsupportedError`. En el `finally` de `unlockWithBiometric` ese error reemplazaba al resultado, la pantalla se quedaba en "Desbloqueando…" y la bóveda ya estaba desbloqueada en Rust. El canal simulado de las pruebas entrega una lista modificable, así que no lo mostraba. Ahora el borrado pasa por `_wipe`, que nunca lanza; esa copia queda en la excepción de mejor esfuerzo de `StandardMessageCodec`. Hay pruebas con `asUnmodifiableView()` en los dos flujos.

Otro hallazgo, sin cambio de código: con Android 16, tras varios dedos no reconocidos el propio diálogo pide el PIN del dispositivo para recuperar la biometría. Cancelarlo vuelve a la contraseña sin desactivar la huella, como manda la tabla de códigos.

## Documentación

- `docs/especificacion.md`:
  - "Viaje de la llave de bóveda hacia el Keystore": decisión del 2026-11-08 (#38), por Dart y sin JNI; los búferes de `StandardMessageCodec` dentro del mejor esfuerzo.
  - "Experiencia de uso → Desbloqueo": huella solo desde la API 30 y solo `BIOMETRIC_STRONG`; qué la desactiva y qué no; dónde se ofrece activarla.
  - "Respaldos": la copia del Keystore vive en `noBackupFilesDir/biometric/`, fuera de Auto Backup.
- `ARCHITECTURE.md`: `biometric_service.dart`, `biometric_unlock.dart`, `biometric_offer_screen.dart` y `BiometricVault.kt` en el mapa; el estado `offerBiometric` en la descripción de `app.dart`; los búferes de `StandardMessageCodec` en la fila del borrado de mejor esfuerzo de "Decisiones clave".
- `docs/roadmap.md`, etapa 4, junto a "Pedir la contraseña maestra cada 7 días": guardar la fecha del último desbloqueo con contraseña (movido desde la #38); sin fecha, se pide la contraseña.
- Comentario en la #38: la fecha del último desbloqueo con contraseña pasa a la etapa 4 (autorizado).
- En el PR, el criterio "la copia y la fecha viven fuera del archivo de la bóveda" queda como "la copia vive fuera…", con la nota.

## Commits previstos

1. `docs: diseño del desbloqueo con huella (#38)`
2. `feat: exportar la llave y desbloquear con ella en la fachada (#38)`
3. `feat: llave del Keystore y diálogo de huella en Kotlin (#38)`
4. `feat: servicio de huella y flujos de activar y desbloquear (#38)`
5. `feat: ofrecer la huella al terminar el primer uso (#38)`
6. `feat: huella en la pantalla de desbloqueo (#38)`
7. `docs: huella en la especificación, la arquitectura y el roadmap (#38)`
