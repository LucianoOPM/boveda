# Diseño: fachada del puente (issue #35)

Fecha: 2026-11-08 · Etapa 2 · Rama `feat/35-fachada-puente`

## Objetivo

Convertir el crate puente `mobile/rust` (`rust_lib_boveda`), que hoy solo tiene el ejemplo `greet`, en la fachada delgada que describe `ARCHITECTURE.md`: expone de una vez las operaciones del núcleo que necesita la etapa 2, para que las issues de pantallas (#36 a #44) queden solo en Dart. La fachada convierte tipos y delega en `vault-core`; no tiene lógica propia.

### Fuera de alcance

- Desbloquear con la llave del Keystore (#38, sobre la API de #34).
- Cambiar la contraseña maestra: la #35 lo condiciona a que "salga gratis", no lo pide. El roadmap lo pone en la etapa 4 (Ajustes) y la regla es trabajar dentro de la etapa actual.
- Pantallas. `main.dart` queda como un marcador hasta la #36.
- Saber si existe la bóveda (#36): Dart usa `File(ruta).exists()`, sin pasar por la fachada.

### Criterios de aceptación (de la issue)

- Las operaciones con Argon2id (crear, desbloquear e importar) no bloquean el hilo de la interfaz. Comprobado contra la documentación de flutter_rust_bridge y en el dispositivo.
- La ruta de la bóveda es absoluta y está dentro del almacenamiento privado de la app.
- Importar: si falla, la bóveda anterior vuelve a quedar abierta y bloqueada; si sale bien, queda la importada, desbloqueada, con su `UnlockReport`. Probado en Rust.
- La fachada no tiene lógica propia: solo convierte tipos y delega.
- Ningún error ni `Debug` cruza un secreto.
- `lib/src/rust/` y `frb_generated.rs` regenerados; la CI de `mobile/` lo comprueba.
- Pruebas en Rust de la fachada, y `integration_test/` con crear, bloquear, desbloquear, agregar, revelar y buscar, pasando a mano en el emulador y en el celular.
- `cargo test --workspace`, `clippy`, `flutter analyze` y `flutter test` pasan; CI en verde.

## Lo que confirma la documentación

Verificado con Context7 (flutter_rust_bridge v2.13.0) y la documentación de Android:

- Una función de Rust sin `#[frb(sync)]` genera un `Future` en Dart y se ejecuta en un thread pool ("Async Dart + Sync Rust: Dart is non-blocking, Rust uses thread pool"). Con `#[frb(sync)]` se ejecuta en el hilo principal y bloquea la interfaz.
- Un `#[frb(sync)] fn new()` se vuelve el constructor por omisión en Dart; los métodos `async` (o sin `sync`) se vuelven métodos `Future`.
- Un `enum` de Rust con campos se genera como `@freezed sealed class` y exige `freezed`, `freezed_annotation` y `build_runner`. Un `enum` sin campos se genera como `enum` de Dart. Un `Option<T>` como argumento cruza como `T?`.
- `RustAutoOpaque<T>` es, aproximadamente, un `Arc<RwLock<T>>`: frb toma un candado de lectura para métodos `&self` y de escritura para `&mut self`. Un tipo opaco debe ser `Send + Sync`; para uno que no es `Sync`, la documentación propone envolverlo en un `Mutex`.
- `Vec<u8>` cruza de Rust a Dart sin copia en modo asíncrono: la memoria pasa a ser de Dart.
- `libsqlite3-sys` 0.39.1 compila SQLite con `SQLITE_TEMP_STORE=3` para Android: los temporales de `VACUUM INTO` e `integrity_check` van a memoria, sin depender de un directorio temporal.
- Auto Backup de Android respalda por omisión `getFilesDir()`, `getDir()` y `databases/` en Google Drive y en la transferencia entre dispositivos; solo excluye `cache`, `code_cache` y `getNoBackupFilesDir()`. Con target 31 o mayor, `allowBackup="false"` no siempre desactiva la transferencia entre dispositivos.
- `path_provider` (2.1.6) obtiene `getApplicationSupportDirectory()` de `Context.getFilesDir()` y, desde `path_provider_android` 2.3.0, lo hace por JNI (`jni`, `jni_flutter`). No expone `getNoBackupFilesDir()`.

Dos puntos no los muestra la documentación y se comprueban en el código generado en la primera tarea del plan: que un `enum` plano funcione como tipo de error en `Result`, y que los métodos de un mismo tipo opaco puedan repartirse en bloques `impl` de varios archivos.

## Decisiones

### Estado: objeto opaco `VaultSession`

```rust
#[frb(opaque)]
pub struct VaultSession {
    path: PathBuf,
    vault: Mutex<Option<Vault>>,
}
```

- La especificación pone el `Mutex` en la fachada porque `rusqlite::Connection` no es `Sync`. `Vault` es `Send`, así que `Mutex<Option<Vault>>` es `Sync` y la sesión puede ser opaca.
- Se prefirió un objeto opaco a un `static Mutex` porque las pruebas de Rust corren en paralelo: con un estado global compartirían la misma bóveda.
- Todos los métodos toman `&self`: el `Mutex` interno hace la exclusión, y frb solo toma su candado de lectura, que no compite consigo mismo.
- El `Option` representa "sin bóveda" y permite el ciclo de importación (`take()`, importar y reabrir).
- Dart guarda el objeto y llama a `dispose()` al terminar. Al soltarse la `Vault`, el núcleo borra la llave y la caché.

### Ciclo de vida

- `VaultSession::new(path: String) -> VaultSession`: el único método `#[frb(sync)]`. No toca el disco ni la bóveda y no valida la ruta: el núcleo rechaza una relativa con `Storage(Io)` en `create`, `open` e `import`.
- `create(password)` y `open()` solo valen sin bóveda; con una ya cargada devuelven `VaultAlreadyOpen`. `create` la deja desbloqueada; `open`, bloqueada.
- Sin bóveda, `is_locked()` devuelve `true` y las demás operaciones (salvo `create`, `open` e `import_backup`) devuelven `NoVault`. `lock()` sin bóveda no hace nada, igual que `Vault::lock` sobre una bloqueada.
- Un `PoisonError` del `Mutex` se convierte en `SessionPoisoned`, nunca en `unwrap`. El núcleo no hace panic por contrato, así que no debería ocurrir.

### Todas las funciones que tocan la bóveda son asíncronas

Ninguna lleva `#[frb(sync)]`, ni siquiera las rápidas como `is_locked`: si una función síncrona esperara el `Mutex` mientras otra corre Argon2id, congelaría la interfaz alrededor de medio segundo. La regla queda en `mobile/CLAUDE.md`.

Consecuencia anotada para la #39: si la app pasa a segundo plano durante un desbloqueo, `lock()` espera el `Mutex` y bloquea justo después de que termine el desbloqueo. El resultado es correcto.

### Funciones expuestas

| Método | Delegación | Notas |
| --- | --- | --- |
| `new(path)` | — | `#[frb(sync)]`, sin E/S |
| `create(password) -> ()` | `Vault::create` | calibra Argon2id |
| `open() -> ()` | `Vault::open` | queda bloqueada |
| `unlock(password) -> UnlockReport` | `Vault::unlock` | |
| `lock() -> ()` | `Vault::lock` | |
| `is_locked() -> bool` | `Vault::is_locked` | `true` sin bóveda |
| `search(query) -> Vec<EntrySummary>` | `Vault::search` | texto vacío = todas, recientes primero |
| `add(input, secret) -> String` | `Vault::add` | devuelve el id en hexadecimal |
| `edit(id, input, new_secret: Option<NewSecret>) -> ()` | `Vault::edit` | `None` = `Keep`, `Some` = `Replace` |
| `delete(id) -> ()` | `Vault::delete` | |
| `reveal_password(id) -> String` | `Vault::reveal` | descarta las notas |
| `reveal_notes(id) -> Option<String>` | `Vault::reveal` | descarta la contraseña |
| `export_backup(dest) -> ()` | `Vault::export` | `export` es palabra reservada en Dart (frb generaba `export_`) |
| `import_backup(backup, password) -> UnlockReport` | `Vault::import` | ver "Ciclo de importación" |

Todos los métodos, salvo `new`, devuelven `Result<_, VaultError>`: también `lock` e `is_locked`, que pueden encontrar el `Mutex` envenenado (`SessionPoisoned`). La tabla muestra solo el tipo del `Ok`.

`SecretChange` tiene datos (`Replace(Secret)`): exponerlo como enum exigiría freezed. `Option<NewSecret>` expresa lo mismo.

### Revelar: contraseña y notas por separado, como `String`

La especificación dice que una contraseña solo cruza a Flutter al mostrarla o copiarla localmente. Separarlas hace que las notas crucen solo cuando se muestran. La fachada llama a `Vault::reveal`, toma la parte pedida y suelta el `Secret`; la otra parte se borra al soltarse su `SecretString`.

Cada llamada actualiza `last_used_at` (lo hace `Vault::reveal`), así que revelar ambas lo escribe dos veces. No cambia el orden de la búsqueda.

Se descartaron los bytes: mostrar en un `Text` o copiar con `ClipData` exige un `String` de todos modos. El borrado fuera del núcleo es de mejor esfuerzo (`ARCHITECTURE.md`); los búferes de cruce de frb entran en esa misma regla, sin una excepción nueva.

### Tipos de cruce

```rust
pub struct EntrySummary {
    pub id: String,                 // 32 caracteres hexadecimales en minúsculas
    pub service: String,
    pub url: Option<String>,
    pub username: Option<String>,
    pub tags: Vec<String>,
    pub modified_at: i64,           // ms Unix
    pub last_used_at: Option<i64>,  // ms Unix; None = nunca usada
}

pub struct EntryInput {
    pub service: String,
    pub url: Option<String>,
    pub username: Option<String>,
    pub tags: Vec<String>,
}

pub struct NewSecret {
    pub password: String,
    pub notes: Option<String>,
}

pub struct UnlockReport {
    pub corrupt: Vec<String>,       // ids en hexadecimal
    pub unidentified_corrupt: u32,   // usize llegaría a Dart como BigInt; satura en u32::MAX
}
```

- `EntrySummary`, `EntryInput` y `UnlockReport` no contienen secretos. Ningún tipo de cruce deriva `Debug` con secretos; `NewSecret` no deriva `Debug`.
- Las contraseñas y notas que entran se copian con `SecretString::from(s.as_str())` (reserva exactamente el largo, como en `Secret::decode` del núcleo) y el `String` recibido se borra con `zeroize`. No se usa `SecretString::from(String)`: si la capacidad supera el largo, `into_boxed_str` reasigna y deja una copia sin borrar. Para esto `mobile/rust` depende de `secrecy` 0.10 y `zeroize` 1.9, las mismas versiones que ya usa el núcleo (no entran crates nuevos a `Cargo.lock`). Las reglas de la entrada (recortes, límites, etiquetas) las aplican `SearchData::new` y `Secret::new` en el núcleo.
- Las contraseñas maestras (`create`, `unlock`, `import_backup`) entran como `String` y se tratan igual.
- Al revelar, la fachada copia el valor con `expose_secret().to_owned()` y entrega ese `String` a frb. Esa copia y los búferes de frb son de mejor esfuerzo.

### Identificador de entrada en hexadecimal

- Se eligió hexadecimal y no `[u8; 16]` (que frb genera como `U8Array16`): en Dart, un `Uint8List` se compara por referencia, así que `==`, `Set`, `Map` y las `Key` de los widgets no funcionarían. El id no es secreto.
- Sale en minúsculas (32 caracteres). Entra solo si tiene exactamente 32 caracteres en `0-9a-f`; cualquier otra cosa da `InvalidEntryId`. Se convierte a mano, sin crate nuevo.

### Errores: `VaultError`, un enum plano sin campos

Se genera como un `enum` de Dart, sin freezed ni build_runner. Ninguna variante lleva datos, así que no puede cruzar un secreto.

| Origen | Variantes |
| --- | --- |
| `vault_core::vault::Error`, uno a uno | `WrongPassword`, `InvalidKdfParams`, `UnsupportedFormatVersion`, `UnsupportedAlgorithm`, `MalformedEncryptedKey`, `Random`, `CorruptEntry`, `NotAVault`, `EntryNotFound`, `Locked`, `AlreadyUnlocked`, `Clock`, `WrongVaultKey`, `AlreadyExists` |
| `Error::Storage(kind)` | `StorageIo`, `StorageBusy`, `StorageCorrupt`, `StorageReadOnly`, `StorageHotJournal`, `StorageJournalMode`, `StorageOther` |
| `Error::InvalidEntry(field)` | `InvalidEntryServiceEmpty`, `InvalidEntryServiceTooLong`, `InvalidEntryUrlTooLong`, `InvalidEntryUsernameTooLong`, `InvalidEntryPasswordEmpty`, `InvalidEntryPasswordTooLong`, `InvalidEntryNotesTooLong`, `InvalidEntryTooManyTags`, `InvalidEntryTagTooLong`, `InvalidEntryOther` |
| Fachada | `NoVault`, `VaultAlreadyOpen`, `InvalidEntryId`, `SessionPoisoned` |

- `StorageErrorKind` e `InvalidEntryField` son `#[non_exhaustive]`: el mapeo lleva un brazo `_` hacia `StorageOther` e `InvalidEntryOther`.
- `Error` del núcleo no es `#[non_exhaustive]`: su mapeo es exhaustivo, así que una variante nueva en el núcleo rompe la compilación de la fachada a propósito.
- `UnsupportedFormatVersion` y `UnsupportedAlgorithm` pierden el número; la app solo muestra un mensaje genérico (#37).

### Ciclo de importación

`import_backup(backup, password)` toma el `Mutex` durante todo el ciclo, para que ninguna otra operación se intercale:

1. `take()` sobre el `Option`: suelta la `Vault` actual, si hay una (la especificación exige soltarla antes de importar sobre `dest`).
2. `Vault::import(backup, &self.path, password)`.
3. Si sale bien: guarda la importada (desbloqueada) y devuelve su `UnlockReport`.
4. Si falla: intenta `Vault::open(&self.path)`. Si abre, queda bloqueada; si no, la sesión queda sin bóveda. En ambos casos devuelve el error de la importación, no el de la reapertura.

Casos:

- Celular nuevo (sin bóveda): si la importación falla, la reapertura también falla (no hay archivo) y la sesión sigue vacía.
- Caso "sin vuelta atrás" del núcleo (falla la apertura final después de instalar el respaldo): la reapertura abre el respaldo ya instalado, bloqueado.

Con esto, la #44 queda solo en Dart.

### Ruta de la bóveda: `noBackupFilesDir`

- `MainActivity.kt` registra un `MethodChannel` `boveda/storage` con el método `noBackupFilesDir`, que devuelve `noBackupFilesDir.absolutePath`. Sin dependencias nuevas.
- `lib/services/vault_location.dart` llama al canal y arma la ruta con `p.join(dir, 'boveda.db')` (paquete `path`, ya presente como transitiva en 1.9.1; se declara en `pubspec.yaml`).
- Se descartó `path_provider`: no expone `getNoBackupFilesDir()` y suma dos plugins nativos (`jni`, `jni_flutter`).
- Motivo: si Auto Backup restaurara la bóveda por su cuenta, se saltaría `import` y, desde la etapa 3, la regeneración de identidad que exige "Respaldos". Se registra en la especificación como decisión del 2026-11-08 (#35): la bóveda vive en `noBackupFilesDir` y el único camino de restauración es `import`.
- Nota para la #38: la copia cifrada de la llave, si va en `SharedPreferences`, entraría en Auto Backup. Tiene que ir también fuera del respaldo, porque restaurada sin la llave del Keystore no sirve.

### Archivos de `mobile/rust/src/api/`

| Archivo | Contenido |
| --- | --- |
| `mod.rs` | módulos e `init_app` (`#[frb(init)]`, se mueve desde `simple.rs`) |
| `session.rs` | `VaultSession`: `new`, `create`, `open`, `unlock`, `lock`, `is_locked` |
| `entries.rs` | `EntrySummary`, `EntryInput`, `NewSecret`, conversión del id; `search`, `add`, `edit`, `delete`, `reveal_password`, `reveal_notes` |
| `backup.rs` | `export_backup`, `import_backup` |
| `error.rs` | `VaultError` y `From<vault_core::vault::Error>` |

`UnlockReport` de la fachada va en `session.rs`. Se borra `simple.rs`. Si el codegen no acepta bloques `impl` repartidos, todos los métodos van en `session.rs` y los tipos se quedan en sus archivos.

### Dart

- `main.dart`: `RustLib.init()` y un marcador mínimo ("Bóveda") hasta la #36, sin llamadas a Rust.
- `test/widget_test.dart`: prueba el marcador. `mocktail` se queda para las pantallas.
- `integration_test/simple_test.dart` se reemplaza por `integration_test/vault_session_test.dart`.

## Pruebas

### Rust

El crate es solo `cdylib` y `staticlib`, así que `tests/` no puede enlazarlo: las pruebas son unitarias, con el patrón del núcleo (`area.rs` termina con `#[cfg(test)] mod tests;` y las pruebas viven en `area/tests.rs`). `tempfile` entra como dependencia de desarrollo de `mobile/rust`; ya está en `Cargo.lock` por el núcleo y no es criptográfica ni de red.

Las pruebas usan una copia de `core/tests/fixtures/vault-v2.db` (contraseña `prueba-bóveda-v2`, `KdfParams::MIN`) en un directorio temporal, con la ruta resuelta desde `env!("CARGO_MANIFEST_DIR")`. Nunca abren el original. Solo la prueba de `create` calibra.

Casos:

- Sin bóveda: `is_locked` es `true`; `unlock`, `search`, `add`, `edit`, `delete`, `reveal_*` y `export_backup` dan `NoVault`; `lock` no falla.
- `open` sobre la copia: queda bloqueada; `search` da `Locked`.
- `unlock` con la contraseña incorrecta da `WrongPassword` y sigue bloqueada; con la correcta, `UnlockReport` vacío.
- `open` o `create` con la bóveda cargada dan `VaultAlreadyOpen`.
- `search("")` devuelve las tres entradas vivas del fixture, con `Nube` (la usada) primero; `search("banco")` encuentra `Banco`. Los campos de `EntrySummary` coinciden con la tabla del README del fixture.
- `add` devuelve un id de 32 caracteres hexadecimales que aparece en `search`.
- `edit` con `None` conserva la contraseña; con `Some` la reemplaza.
- `delete` la quita de `search`; `reveal_password` sobre ella da `EntryNotFound`.
- `reveal_password` y `reveal_notes` devuelven los valores del fixture; `reveal_notes` de `Banco` devuelve `None`.
- Un id inválido (largo distinto, mayúsculas, caracteres fuera de `0-9a-f`) da `InvalidEntryId`.
- Un `EntryInput` con el servicio vacío da `InvalidEntryServiceEmpty`.
- `export_backup` y después `import_backup` sobre otra sesión recuperan todas las entradas.
- `import_backup` con la contraseña incorrecta: devuelve `WrongPassword` y la bóveda anterior queda abierta y bloqueada (desbloquearla con su contraseña funciona).
- `import_backup` sin bóveda previa: con éxito queda desbloqueada; con fallo, la sesión sigue sin bóveda.
- `create` en una ruta nueva: queda desbloqueada y vacía (calibra).
- El mapeo de errores: cada variante de `Error` del núcleo cae en la de `VaultError` esperada.

### Dispositivo (`integration_test/`)

A mano, en el emulador y en el celular:

- Crear, bloquear, desbloquear, agregar, revelar y buscar con `VaultSession`, en un directorio temporal absoluto (no en la ruta real).
- El canal `boveda/storage` devuelve una ruta absoluta que termina en `no_backup`.

La CI de `mobile/` no ejecuta `integration_test/`; sí comprueba que el código generado esté al día.

## Documentación

- `docs/especificacion.md`, "Almacenamiento" o "Respaldos": decisión del 2026-11-08 (#35), la bóveda vive en `noBackupFilesDir` y el único camino de restauración es `import`.
- `ARCHITECTURE.md`: los archivos de `mobile/rust/src/api/`, `lib/services/vault_location.dart`, y los búferes de cruce de frb dentro del borrado de mejor esfuerzo fuera del núcleo.
- `mobile/CLAUDE.md`: ninguna función que toque la bóveda lleva `#[frb(sync)]`.
- Comentarios en la #38 (la copia del Keystore fuera de Auto Backup) y en la #39 (bloqueo durante un desbloqueo), solo después de la confirmación del usuario.
