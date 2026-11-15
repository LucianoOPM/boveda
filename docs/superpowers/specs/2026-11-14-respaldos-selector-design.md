# Diseño: respaldos con el selector de archivos del sistema (issue #44)

Fecha: 2026-11-14 · Etapa 2 · Rama `feat/44-respaldos`

## Objetivo

Exportar e importar el respaldo cifrado desde la app con el selector de archivos de Android, y restaurar un respaldo desde el primer uso (celular nuevo o emulador limpio). Lo que viaja es el archivo de la bóveda: la lógica de archivos y de cifrado ya vive en el núcleo (#9) y la fachada ya la expone (#35). Esta issue agrega la pantalla de ajustes mínima, el puente con el Storage Access Framework (SAF) y la convivencia con el bloqueo automático (#39).

### Fuera de alcance

- Invitar al primer respaldo (etapa 4).
- Regenerar la identidad del dispositivo al restaurar (etapa 3).
- Tiempo de bloqueo y cambio de contraseña maestra en Ajustes (etapa 4).
- Validar un respaldo sin soltar la bóveda actual: es la #69 (ver "Errores al importar").

### Criterios de aceptación (de la issue)

- Exportar desde el emulador a una carpeta elegida e importar ese archivo de vuelta recupera todas las entradas.
- En un emulador limpio, "Restaurar un respaldo" desde el primer uso recupera todas las entradas y sus contraseñas.
- Importar con la contraseña incorrecta, un archivo que no es una bóveda o una versión no soportada deja la bóveda actual intacta y muestra el error.
- Tras importar, la huella queda desactivada hasta volver a activarla.
- No quedan temporales después de exportar o importar, con éxito o con error.
- Pruebas de widgets con el puente simulado.
- `flutter analyze` y `flutter test` pasan; CI en verde.

## Lo que confirma la documentación

Verificado con la guía oficial del SAF (`developer.android.com/training/data-storage/shared/documents-files`) y Context7 (`file_picker`):

- `ACTION_CREATE_DOCUMENT` no sobrescribe: si el nombre existe, el sistema agrega "(1)". La comprobación de "el destino ya existe" del núcleo no aplica al destino del selector.
- `ACTION_OPEN_DOCUMENT` con `CATEGORY_OPENABLE` excluye los archivos virtuales (documentos de Drive): un respaldo debe ser un archivo real.
- La URI del selector da acceso hasta reiniciar el dispositivo; no hace falta `takePersistableUriPermission` porque cada operación usa la URI una sola vez.
- El resultado llega a `onActivityResult`; el flujo de bytes (`openInputStream`, `openFileDescriptor(uri, "w")`) va fuera del hilo principal. Si la copia falla a medias en el destino, `DocumentsContract.deleteDocument` lo borra como mejor esfuerzo.
- `file_picker`: `saveFile` en Android escribe él mismo los bytes recibidos (el respaldo pasaría entero por memoria de Dart) y `pickFiles` copia el archivo al caché de la app, fuera de `noBackupFilesDir`.
- Del código: `importBackup` suelta la bóveda y, si falla, la reabre **bloqueada** (o deja la sesión sin bóveda). `exportBackup` exige la bóveda desbloqueada y que el destino no exista. `BiometricVault.disable()` ya borra la copia y la llave del Keystore (`deleteAll`).

## Enfoques

1. **`file_picker`.** Rápido, pero exige aprobar un paquete nuevo, pasa los bytes por memoria de Dart y deja temporales en el caché. Descartado.
2. **Canal propio en Kotlin con SAF** (elegido). Sin dependencias nuevas; la copia entre la URI y un temporal en `noBackupFilesDir` es por flujo y nunca cruza a Dart. Costo: Kotlin no tiene pruebas automáticas (como `BiometricVault`) y se prueba a mano.
3. **Canal propio que entrega bytes a Dart.** Descartado: sin ventaja sobre el 2 y con el respaldo en memoria de Dart.

## Decisiones

1. **Canal propio, sin paquete nuevo.** Decidido el 2026-11-14.
2. **Nombre sugerido:** `boveda-AAAA-MM-DD.db`, con la fecha local que arma Dart. Tipo MIME `application/octet-stream`.
3. **Limpieza de temporales:** los crea solo Kotlin, en `noBackupFilesDir/backup-tmp/`; `MainActivity.onCreate` la vacía antes de que Flutter arranque (ninguna operación puede estar en curso). Cada flujo borra el suyo en un `finally`, y Kotlin también lo borra antes de responder si la copia falla a medias.
4. **Convivencia con el bloqueo:** `runWithoutAutoLock` (sección propia).
5. **Errores al importar desde Ajustes (opción a):** la bóveda actual está desbloqueada y la importación fallida la deja bloqueada en Rust; tras cualquier error, la interfaz se bloquea y la pantalla de desbloqueo muestra el motivo. En el primer uso no hay bóveda abierta, así que el error se muestra en el mismo diálogo. La alternativa (b), validar el respaldo sin soltar la bóveda, es la #69 y no entra aquí.
6. **Tras importar con éxito:** se llama a `biometric.disable()` (borra la copia y la llave del Keystore, que protegían la bóveda anterior) y solo después se ofrece activar la huella.
7. **Entrada a Ajustes:** un engranaje a la derecha de la búsqueda, que apila `SettingsScreen` en el `Navigator` raíz.

## Componentes

### Kotlin: `BackupFiles.kt` y el canal `boveda/backup`

`MainActivity` lo registra y reenvía `onActivityResult`; lo que no es suyo pasa a `super`.

| Método | Qué hace | Resultado |
| --- | --- | --- |
| `newTempPath()` | Crea `backup-tmp/` y devuelve una ruta nueva (`<uuid>.bak`) dentro de ella, sin crear el archivo | `String` |
| `saveTo(temp, name)` | Valida `temp`, abre `ACTION_CREATE_DOCUMENT` (`octet-stream`, `CATEGORY_OPENABLE`, `EXTRA_TITLE`), copia el temporal a la URI en un hilo de fondo y borra el temporal | `true`; `false` si el usuario cancela |
| `pickToTemp()` | Abre `ACTION_OPEN_DOCUMENT` (`*/*`, `CATEGORY_OPENABLE`), copia la URI a un temporal nuevo con tope de tamaño | ruta; `null` si cancela |
| `deleteTemp(path)` | Borra un temporal; si no existe, no hace nada | `null` |

- **Ruta del temporal.** `saveTo` y `deleteTemp` normalizan con `File(path).canonicalFile` y exigen que su padre sea el `canonicalFile` de `backup-tmp/`. No comparan texto. Una ruta fuera (`../`, un enlace simbólico, otro directorio) devuelve `invalidPath` y no borra ni lee nada.
- **Tope de tamaño:** 20 MiB al copiar de la URI a un temporal. Si lo supera, aborta, borra el parcial y devuelve `tooLarge`. Un archivo enorme no llena el almacenamiento interno.
- **Copia a medias:** Kotlin borra el temporal (y el documento parcial del destino, como mejor esfuerzo) antes de responder `failed`, sin depender del `finally` de la interfaz.
- **Una petición a la vez:** una segunda con otra en curso responde `busy` de inmediato.
- **Errores:** `result.error(código, null, null)`, sin mensaje. Códigos: `failed`, `busy`, `tooLarge`, `invalidPath`. Cancelar no es error.
- Nada se escribe en logs. El selector es una actividad del sistema: `FLAG_SECURE` no aplica a su ventana, y no muestra contenido de la bóveda.
- Si el proceso muere con el selector abierto, el resultado se pierde; el temporal se limpia en el siguiente arranque.

### Dart: `lib/services/backup_files.dart`

`abstract class BackupFiles` (`newTempPath`, `saveTo`, `pickToTemp`, `deleteTemp`) y `PlatformBackupFiles` sobre el canal. Los errores se traducen a `BackupFilesException(BackupFailure)` sin texto (`failed`, `busy`, `tooLarge`, `invalidPath`), como `BiometricException`. `main.dart` lo inyecta en `BovedaApp`, que lo pasa a `VaultGate`.

### `runWithoutAutoLock<T>` (`lib/services/auto_lock.dart` y `VaultGate`)

Lo expone `AutoLockController` (y `AutoLockScope` lo reenvía). Es genérico: la etapa 3 puede usarlo para el escáner del QR.

- Mientras corre, ocultarse no bloquea y el temporizador de inactividad no corre. Al terminar (también con excepción), la marca se baja y la cuenta de inactividad se reinicia.
- **No se anida:** con una operación en curso, una segunda llamada lanza `StateError` de inmediato, sin esperar. Las pantallas desactivan sus botones mientras `busy`, así que no ocurre en el uso normal.
- **Tope de 2 minutos fuera de la app.** Al pasar a `hidden` con la operación en curso, `VaultGate` guarda la hora y arma un `Timer` de 2 minutos que bloquea. Android puede congelar el temporizador en segundo plano, así que al volver a `resumed` también compara la hora guardada: si pasaron más de 2 minutos, bloquea. Bloquear descarta el resultado del selector: las pantallas lo detectan con `mounted` y `ModalRoute.isActive` (el bloqueo quita las rutas con `removeRoute` y el `State` sigue montado hasta el siguiente cuadro; lección de la #41).
- Al volver a `resumed` dentro del tope, el temporizador del tope se cancela.

### Pantalla de ajustes y flujos

`SettingsScreen` (`lib/screens/settings_screen.dart`): dos `ListTile`, "Exportar respaldo" e "Importar respaldo". Mientras un flujo corre, ambos se desactivan. Recibe `session`, `files`, `biometric`, el controlador y tres acciones de `VaultGate`: `onImported(damaged)`, `onImportFailed(mensaje)` y `onNotice(texto, isError)`. Esas acciones viven en `VaultGate`, que sobrevive al bloqueo.

**Exportar**

1. `tmp = files.newTempPath()`; `session.exportBackup(dest: tmp)`. El núcleo exige el destino inexistente: `newTempPath` no crea el archivo.
2. `autoLock.runWithoutAutoLock(() => files.saveTo(tmp, nombre))`.
3. `finally`: `files.deleteTemp(tmp)`, sin propagar errores.
4. Resultado: `true` → "Respaldo guardado"; `false` → nada; error → "No se pudo guardar el respaldo".
5. **Bloqueo a mitad del flujo:** si al volver la bóveda ya está bloqueada, el archivo se escribe igual (es texto cifrado que el usuario eligió guardar) y el aviso no se pierde: `onNotice` lo muestra en la pantalla de desbloqueo. Si no hay bloqueo, va en un `SnackBar`.

**Importar** (comparte el flujo con el primer uso)

1. `tmp = await runWithoutAutoLock(files.pickToTemp)`. `null` → fin. Si al volver la pantalla ya no está activa (bloqueo por el tope), se borra `tmp` y no se toca la bóveda.
2. Se abre `RestoreDialog`: advierte que reemplaza la bóveda actual (en el primer uso no hay nada que reemplazar y no lo dice), pide la contraseña **del respaldo** con `PasswordField` y llama a `session.importBackup(backup: tmp, password)`. Al terminar hace `pop` con el resultado: `ImportOk(damaged)` o `ImportFailed(VaultError)`.
3. `finally`: `files.deleteTemp(tmp)`. **Siempre antes** de lo que sigue.
4. `ImportOk`: `biometric.disable()` (mejor esfuerzo) y `onImported(damaged)`.
5. `ImportFailed` desde Ajustes: `onImportFailed(mensaje)` (decisión 5).
6. `null` (diálogo cerrado, o quitado por un bloqueo): se trata como cancelado.

En el primer uso, el diálogo no hace `pop` con el error: lo muestra en su propio campo y deja reintentar la contraseña o cerrarlo. Un `ImportOk` sigue el mismo camino que Ajustes.

**Mensajes de error** (sin detalles del núcleo): `wrongPassword` → "Contraseña del respaldo incorrecta."; `notAVault` → "El archivo no es una bóveda."; `unsupportedFormatVersion` → "Este respaldo es de una versión que la app no admite."; `storageCorrupt` → "El archivo del respaldo está dañado."; `tooLarge` → "El archivo es demasiado grande para ser un respaldo."; cualquier otro → "No se pudo importar el respaldo."

### `VaultGate` y `UnlockScreen`

- `_lock({notice})` y `_showNotice(texto, isError)`: guardan el aviso y lo pasan a `UnlockScreen` (parámetro nuevo `notice`). El aviso se borra al entrar en un estado desbloqueado. En `locked`, `_showNotice` actualiza el aviso; en otro estado usa un `SnackBar`.
- `onImportFailed(mensaje)`: `_lock(notice: mensaje)`. La fachada ya dejó la bóveda bloqueada en Rust; el `lock()` es inofensivo.
- `onImported(damaged)`: quita lo apilado sobre la raíz (`_removeStackedRoutes`, incluida `SettingsScreen`), sube `_epoch` y sigue el camino de `_afterCreate` (oferta de la huella si el dispositivo la admite y no está activa; si no, la pantalla principal), pasando por `_enterUnlocked`. El total de dañadas se conserva para el aviso de la principal: el `onDone` de la oferta lo pasa de nuevo.
- Borde: si la app se oculta mientras corre `importBackup`, `_onHidden` bloquea y quita el diálogo; el resultado se descarta, el temporal se borra y la huella no se desactiva en ese camino. La copia vieja del Keystore no abre la bóveda importada (`key_check` da `WrongVaultKey` y la app la desactiva sola).

### `FirstRunScreen`

Botón "Restaurar un respaldo" bajo "Crear bóveda", desactivado mientras se crea. Recibe `files` y `onRestored(damaged)`. Usa el flujo de importar. Como en el estado `noVault` ocultarse no bloquea, el selector no necesita suspender nada, pero usa el mismo `runWithoutAutoLock` por uniformidad.

## Seguridad

- Ningún secreto cruza: la contraseña del respaldo vive en el controlador del diálogo y en una variable local (`PasswordField`); el respaldo llega ya cifrado.
- El temporal contiene una bóveda cifrada: vive solo en `noBackupFilesDir/backup-tmp/` (fuera de Auto Backup), con nombre aleatorio, y se borra en todos los caminos.
- Los errores de Kotlin no llevan texto; los de Dart, solo el tipo.
- No se agregan paquetes, permisos de almacenamiento ni cambios en el manifiesto: el SAF no los necesita.

## Pruebas

### Widgets (doble `FakeBackupFiles`; `FakeVaultSession` gana `onExportBackup` y `onImportBackup`)

- **Ajustes:** el engranaje abre `SettingsScreen`. Exportar feliz (`exportBackup` al temporal, `saveTo`, `deleteTemp`, "Respaldo guardado"). Cancelar el selector no muestra aviso y borra el temporal. Un `saveTo` que lanza muestra el error y borra el temporal. Importar feliz: la contraseña llega a `importBackup`, se borra el temporal, se llama a `disable` y se llega a la oferta de la huella o a la principal, con el aviso de dañadas.
- **Importación fallida desde Ajustes** (a pedido): `wrongPassword`, `notAVault` y `unsupportedFormatVersion` terminan en la pantalla de desbloqueo, con el motivo visible y sin temporales (`deleteTemp` llamado antes del bloqueo).
- **Primer uso:** "Restaurar un respaldo" feliz y con error. El error queda en el diálogo, no hay bloqueo y se puede reintentar.
- **Bloqueo a mitad del flujo:** exportar con la bóveda bloqueada al volver del selector muestra el aviso en el desbloqueo. Importar con el tope vencido no llama a `importBackup` y borra el temporal.
- **`runWithoutAutoLock`** (reloj simulado): no bloquea al ocultarse ni por inactividad mientras corre; reinicia la cuenta al terminar, también si lanza; una segunda llamada lanza `StateError`; **tope de 2 minutos** (a pedido): al pasar 2 minutos oculta, la app bloquea (por el temporizador y, con el reloj saltado, al volver a `resumed`) y el temporal se borra; antes del tope, no bloquea.
- **Servicio de plataforma** (`backup_files_test.dart`, canal simulado): traduce los códigos a `BackupFailure`; `null` y `false` de cancelar no son error.

**`deleteTemp` con una ruta fuera de `backup-tmp/`** (a pedido; `../`, enlace simbólico, otro directorio, el propio `backup-tmp/`): la comprobación vive en una función pura `isInsideTempDir(dir: File, path: String): Boolean` en `BackupFiles.kt`, sin tipos de Android, para probarla como prueba JVM (`mobile/android/app/src/test/`) con una carpeta temporal real. Eso exige `testImplementation("junit:junit:…")` en `mobile/android/app/build.gradle*`: una dependencia de pruebas nueva, que se aprueba antes de agregarla (pendiente de confirmar). Sin esa aprobación, la prueba queda a mano (prueba 10 de abajo) y Kotlin sigue sin pruebas automáticas, como `BiometricVault`. Además, la prueba de Dart comprueba que el servicio traduce `invalidPath`.

### A mano, en el emulador

Avisar antes de usarlo y cerrarlo al terminar (emulador, `adb` y Gradle).

| # | Prueba | Esperado |
| --- | --- | --- |
| 1 | Exportar a Descargas y a una carpeta propia | El archivo existe con el nombre propuesto; abre como SQLite (`node:sqlite` sobre una copia) |
| 2 | Exportar con un nombre que ya existe | El sistema agrega "(1)" |
| 3 | Importar ese archivo con la contraseña correcta | Todas las entradas, con sus contraseñas; la oferta de la huella |
| 4 | Importar con la contraseña incorrecta | Desbloqueo con el motivo; la bóveda sigue abriendo con la contraseña actual |
| 5 | Importar un archivo que no es una bóveda (un `.txt`) | Desbloqueo con "El archivo no es una bóveda." |
| 6 | Importar un archivo de más de 20 MiB | "El archivo es demasiado grande…"; sin temporales |
| 7 | Emulador limpio (datos borrados): "Restaurar un respaldo" | Entradas y contraseñas recuperadas |
| 8 | Abrir el selector, volver a inicio y esperar más de 2 minutos | Al volver, el desbloqueo; sin temporales |
| 9 | Tras cada caso: `adb shell run-as com.jesus.boveda ls files/../no_backup/backup-tmp` (ruta real de `noBackupFilesDir`) | Vacío |
| 10 | `deleteTemp` con una ruta fuera de `backup-tmp/` (con un canal de prueba temporal) | `invalidPath`, nada borrado |
| 11 | Matar la app con el selector abierto y volver a abrirla | `backup-tmp/` vacía |
| 12 | `logcat` filtrado por el paquete | Sin contraseñas, rutas de usuario ni bytes |

## Archivos

| Archivo | Cambio |
| --- | --- |
| `mobile/android/.../BackupFiles.kt` | Nuevo: canal `boveda/backup` |
| `mobile/android/.../MainActivity.kt` | Registra el canal, reenvía `onActivityResult`, vacía `backup-tmp/` en `onCreate` |
| `mobile/lib/services/backup_files.dart` | Nuevo: `BackupFiles`, `PlatformBackupFiles`, `BackupFilesException` |
| `mobile/lib/services/auto_lock.dart` | `runWithoutAutoLock` en `AutoLockController` y `AutoLockScope` |
| `mobile/lib/app.dart` | Inyecta `files`; tope de 2 minutos; `_lock({notice})`, `_showNotice`, `onImported`, `onImportFailed`; el `onDone` de la oferta conserva las dañadas |
| `mobile/lib/screens/settings_screen.dart` | Nuevo: exportar e importar |
| `mobile/lib/screens/restore_dialog.dart` | Nuevo: contraseña del respaldo e importación |
| `mobile/lib/screens/search_screen.dart` | Engranaje que abre Ajustes |
| `mobile/lib/screens/first_run_screen.dart` | "Restaurar un respaldo" |
| `mobile/lib/screens/unlock_screen.dart` | Parámetro `notice` |
| `mobile/lib/main.dart` | Inyecta `PlatformBackupFiles` |
| `mobile/test/fakes.dart` | `FakeBackupFiles`; `onExportBackup` y `onImportBackup` en `FakeVaultSession` |
| `mobile/test/…` | Pruebas de arriba |

No cambian `core/`, `mobile/rust/` ni `lib/src/rust/`: no hay que regenerar el puente.

## Documentación

- `docs/especificacion.md`: en "Bloqueo automático", reemplazar la viñeta "Selector de archivos y otras actividades" por lo decidido aquí; en "Respaldos", una entrada "Decidido el 2026-11-14 (#44)" con el canal, el tope de 20 MiB, el nombre, el temporal y la opción (a) con su referencia a la #69.
- `ARCHITECTURE.md`: `services/backup_files.dart`, `screens/settings_screen.dart`, `screens/restore_dialog.dart` y `BackupFiles.kt` en el mapa; quitar "futuro: ajustes" y "futuro: selector".
- `mobile/CLAUDE.md`: reglas nuevas: los temporales de respaldo viven solo en `noBackupFilesDir/backup-tmp/`, los crea y borra Kotlin y la interfaz los borra en un `finally`; toda actividad externa se envuelve en `runWithoutAutoLock`; el selector no se reemplaza por un paquete sin aprobarlo.
- `docs/roadmap.md`: sin cambios; la casilla se marca con el merge.

## Comentarios en issues

Tras aprobar el diseño: en la #45 (cierre de la etapa), que la prueba de "restaurar en un emulador limpio" ya tiene el camino de la #44 y qué casos cubre la prueba a mano.

## Commits previstos

1. `docs: diseño de los respaldos con el selector del sistema (#44)`
2. `feat: runWithoutAutoLock y tope de 2 minutos fuera de la app (#44)`
3. `feat: canal de Kotlin para exportar e importar con el selector (#44)`
4. `feat: servicio BackupFiles y su doble de pruebas (#44)`
5. `feat: ajustes con exportar el respaldo (#44)`
6. `feat: importar el respaldo desde ajustes y bloquear si falla (#44)`
7. `feat: restaurar un respaldo desde el primer uso (#44)`
8. `docs: respaldos en la especificación, la arquitectura y mobile/CLAUDE.md (#44)`
