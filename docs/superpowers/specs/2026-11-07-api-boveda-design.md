# Diseño: API de la bóveda (issue #7)

Fecha: 2026-11-07 · Etapa 1 · Rama `feat/7-api-boveda`

## Objetivo

Exponer la primera API pública de `vault-core`: una estructura `Vault` que junta lo que ya existe (cabecera y llaves de la #3, modelo de entrada de la #4, cifrado por entrada de la #5 y almacenamiento SQLite de la #6) en las operaciones que usarán las apps a través del crate puente: `create`, `open`, `unlock`, `lock`, `entries`, `add`, `edit`, `delete`, `reveal` y `change_master_password`.

### Fuera de alcance

- Búsqueda (issue propia). Trabajará sobre la caché de `entries()`.
- Exportar e importar respaldos.
- El `Mutex`: `rusqlite::Connection` no es `Sync`, así que va en la fachada de `mobile/rust`.
- Bóvedas de ejemplo en `core/tests/fixtures/` (#10).
- Editar o eliminar filas dañadas sin un id de 16 bytes: solo se cuentan en `unidentified_corrupt`.

### Criterios de aceptación (de la issue)

- Crear, bloquear y desbloquear con la contraseña correcta devuelve las mismas entradas.
- Una contraseña incorrecta devuelve el error de contraseña incorrecta.
- Toda operación con la bóveda bloqueada devuelve `Error::Locked`.
- Después de `lock()`, la llave y los datos de búsqueda descifrados ya no están en memoria.
- Cambiar la contraseña maestra: la anterior falla, la nueva abre y las partes cifradas de las entradas no cambian.
- Alterar un byte de una sola entrada: `unlock()` abre el resto y reporta su identificador.
- `reveal()` de una entrada con el secreto dañado devuelve el error de entrada dañada.
- `edit` actualiza `modified_at`; `delete` marca la entrada como eliminada.
- La API pública no hace panic y ningún error contiene secretos.
- `cargo test -p vault-core` y `clippy` pasan, y la CI está en verde.

Más las notas de la #6 ([comentario en la #7](https://github.com/LucianoOPM/boveda/issues/7)): prueba de lápidas a través de `unlock()`, reloj leído en un solo punto con error tipado y conteo de filas dañadas sin id válido.

## Decisiones

### Estado: llave y caché juntas

- La issue y la especificación piden "la llave en `Option<SecretBox<[u8; 32]>>`". La llave sigue siendo ese `SecretBox` (`VaultKey`), pero vive junto con la caché de entradas en un solo `Option<Unlocked>`. Así no puede existir la llave sin la caché ni al revés.
- `lock()` pone `unlocked = None`: `SecretBox` borra la llave y cada `SearchData` de la caché se borra al soltarse (`ZeroizeOnDrop`). No falla y, sobre una bóveda ya bloqueada, no hace nada.
- La conexión a `Storage` sigue abierta mientras la bóveda está bloqueada. `unlock` y `lock` solo cambian el estado en memoria.
- `unlock()` sobre una bóveda ya desbloqueada devuelve `Error::AlreadyUnlocked` y no cambia nada. Bloquear y reabrir a escondidas reconstruiría la caché y gastaría otro Argon2 sin que la app lo pida; si la fachada quiere reabrir, llama `lock()` primero.
- La caché es un `Vec<EntrySummary>`. Al crecer mueve las estructuras, no el texto (que vive en el heap de cada `String`), así que no deja copias del texto plano. Al editar, el elemento se reemplaza en su lugar y el viejo se borra al soltarse. La búsqueda por id es lineal: suficiente para cientos de entradas.

### Lo que devuelve `unlock()`

- `UnlockReport` es propio y no lleva texto plano: `corrupt: Vec<EntryId>` y `unidentified_corrupt: usize`. Las entradas válidas se leen después con `entries() -> Result<&[EntrySummary]>`, sin préstamos que cuelguen de `unlock()`.
- `unidentified_corrupt` es un conteo, para que la app muestre "N entradas no se pudieron leer" sin exponer un tipo de id inválido.
- Una lápida no aparece ni en `entries()` ni en `corrupt` (ya lo garantiza `list_entries`).

### Tipos públicos

Se reutilizan los tipos de la #4 en lugar de crear otros: ya se validan al construirse, se borran al soltarse y tienen un `Debug` que oculta los valores.

- `SearchData` y `Secret` pasan a `pub`, con `new()` y sus getters públicos. `encode` y `decode` siguen `pub(crate)`. `Vault` nunca recibe datos sin validar: si la validación falla, falla el constructor y no se escribe nada.
- Sin impls públicas de `serde`: hoy solo `RawSearchDataV1` y `RawSecretV1`, en el módulo privado `entry/raw.rs`, derivan `Serialize` y `Deserialize`, y `decode` pasa por `build_*_v1`. Se mantiene así.
- Sin `Clone`: `Secret` ya no lo deriva; a `SearchData` se le quita, porque nadie lo usa. Si la búsqueda lo necesita, se decide en su issue. `PartialEq` se conserva en `SearchData` (lo usan las pruebas de la #4).
- `reveal()` devuelve el mismo `Secret`, sin un tipo `RevealedSecret` aparte.
- `EntryId` pasa a `pub` con `pub fn as_bytes(&self) -> &[u8; 16]` y `pub fn from_bytes([u8; 16]) -> Self`, para que la fachada lo convierta al cruzar a Dart. Un id arbitrario no es un riesgo: si no existe, da `EntryNotFound`. `generate` sigue `pub(crate)`.

### `edit` y el secreto

- La especificación dice que "una contraseña solo cruza a Flutter al mostrarla o copiarla localmente". Por eso `edit` recibe los datos de búsqueda completos y un `SecretChange`: `Keep` conserva el secreto sin que salga del núcleo; `Replace(Secret)` lo reemplaza. Contraseña y notas se reemplazan juntas.
- Con `Keep` y con `Replace`, `edit` primero lee la fila con `get_entry` (un id inexistente o una lápida → `EntryNotFound`) para obtener `last_used_at`. Los metadatos vienen de la fila, no de la caché, así que funciona igual para una entrada que estaba en `corrupt`.
- Solo `Keep` descifra el secreto viejo, en un búfer `Zeroizing`. Si está dañado → `CorruptEntry`, sin tocar el archivo.
- `edit` siempre vuelve a cifrar ambas partes con nonces nuevos y la versión de estructura actual (`SealedEntry::seal`); la versión de la fila no se usa. Actualiza `modified_at`, no `last_used_at`.
- **Reparación:** con `Replace`, una entrada dañada (secreto dañado, o datos de búsqueda dañados que aparecían en `corrupt`) se reescribe completa y vuelve a ser válida. Se agrega a la caché si no estaba.

### `reveal` y `last_used_at` de mejor esfuerzo

- Orden: descifrar el secreto. Si falla → `CorruptEntry` y no se escribe nada.
- Si sale bien: se actualiza `last_used_at` en la caché y luego se intenta `touch_entry(now)`. Si la escritura falla, o si el reloj da `Clock` en ese punto, `reveal()` devuelve el secreto igual y el error se descarta.
- Los errores de `reveal()` solo vienen de la bóveda bloqueada, el id inexistente o el secreto dañado.
- El comentario de `reveal()` documenta que `last_used_at` es de mejor esfuerzo: dentro de la sesión, el orden en memoria es correcto; si el disco falló, ese uso se pierde al cerrar. No afecta la integridad de la bóveda.

### `change_master_password`

- Pide la contraseña actual y la verifica antes de nada. Orden: exigir que esté desbloqueada → verificar `current` (`read_header` + `header.unlock`, con los parámetros guardados, así que tarda lo mismo que un desbloqueo) → `calibrate()` → `VaultHeader::rekey` → `replace_header`.
- Con la contraseña incorrecta → `WrongPassword`, sin calibrar y sin cambiar nada.
- `rekey` vuelve a cifrar la **misma** `VaultKey` de memoria con sal y nonce nuevos. No toca la llave de bóveda ni las entradas. `replace_header` escribe la cabecera en una sola transacción.

### Calibración y pruebas

- `create()` y `change_master_password()` públicos llaman `calibrate()` y delegan en `create_with_params()` y en `rekey_with_params()`, que son `pub(crate)` o privadas. Así el código de producción recorre las mismas funciones que las pruebas y no queda código muerto.
- Para no verificar dos veces, `change_master_password` se divide en dos funciones privadas: `verify_current` y `rekey_with_params`. La pública hace verificar → calibrar → `rekey_with_params`; `change_master_password_with_params` (`pub(crate)`) hace verificar → `rekey_with_params`.
- Las pruebas unitarias usan `KdfParams::MIN`. `core/tests/vault.rs` tiene unas pocas pruebas de punta a punta con la calibración real.

### `create`

- `Vault::create` hace todo lo que puede fallar en memoria (calibrar, generar sal y llave, cifrar la llave) **antes** de crear el archivo.
- `Storage::create` (#6) ya escribe `application_id`, el esquema y la cabecera en una sola transacción, y si algo falla después de crear el archivo lo borra antes de devolver el error. Como usa `create_new`, solo borra un archivo que creó en esa llamada.
- Devuelve la bóveda desbloqueada y con la caché vacía.

### Reloj

- Una sola función privada, `now_millis() -> Result<i64>`, lee `SystemTime::now()`. Delega en `millis_since_epoch(SystemTime) -> Result<i64>`, pura, para poder probarla.
- Si `duration_since(UNIX_EPOCH)` falla (reloj anterior a 1970) → `Error::Clock`, sin `unwrap()`. Si los milisegundos no caben en `i64` → también `Clock`.
- Cada operación que escribe lee la hora una sola vez. La excepción es `reveal`, que descarta el error (ver arriba).

### Errores

Variantes nuevas de `Error`, sin datos:

```rust
/// La bóveda está bloqueada: hay que desbloquearla antes.
#[error("la bóveda está bloqueada")]
Locked,

/// `unlock` sobre una bóveda ya desbloqueada.
#[error("la bóveda ya está desbloqueada")]
AlreadyUnlocked,

/// El reloj del sistema marca una fecha anterior a 1970 o fuera de rango.
#[error("la hora del sistema no es válida")]
Clock,

/// `create` sobre una ruta que ya existe. No incluye la ruta.
#[error("el archivo ya existe")]
AlreadyExists,
```

- `AlreadyExists` deja de ser `StorageErrorKind::AlreadyExists` y pasa a ser variante propia de `Error`: es una condición que la app muestra al usuario, no un fallo del almacenamiento. Se quita de `StorageErrorKind` (aún no publicada) y se ajustan `storage.rs`, su prueba y `error/tests.rs`. El diseño de la #6 no se edita: es un registro histórico.
- Con la bóveda bloqueada, todas las operaciones salvo `unlock` y `lock` devuelven `Locked` antes de tocar el disco.

### Exportaciones

- `lib.rs` queda con `pub mod vault;`. `crypto` y `error` siguen privados. Se quita `#![cfg_attr(not(test), allow(dead_code))]`, porque `Vault` ya consume el código de llaves y cabecera. Si algo queda sin usar, se quita o se justifica en el plan.
- `vault` reexporta `Error`, `Result`, `InvalidEntryField` y `StorageErrorKind` (se mueven desde `lib.rs`), y además `EntryId`, `SearchData`, `Secret`, `EntrySummary`, `UnlockReport` y `SecretChange`.
- Los submódulos `entry`, `header` y `storage` siguen `pub(crate)`.

## Componentes

```
core/src/lib.rs                # pub mod vault; sin reexportaciones en la raíz
core/src/error.rs              # Locked, AlreadyUnlocked, Clock, AlreadyExists; quita StorageErrorKind::AlreadyExists
core/src/vault/mod.rs          # Vault, Unlocked, EntrySummary, UnlockReport, SecretChange, reloj, reexportaciones
core/src/vault/tests.rs        # pruebas unitarias de Vault
core/src/vault/entry.rs        # EntryId, SearchData y Secret pasan a pub; SearchData sin Clone
core/src/vault/header.rs       # VaultHeader::rekey
core/src/vault/storage.rs      # create devuelve Error::AlreadyExists
core/tests/vault.rs            # pruebas de punta a punta con calibración real
```

### Tipos y firmas

```rust
/// Bóveda abierta sobre un archivo. No deriva `Debug`.
pub struct Vault {
    storage: Storage,
    unlocked: Option<Unlocked>,
}

/// Estado desbloqueado: la llave y la caché, que se borran juntas.
struct Unlocked {
    key: VaultKey,                // SecretBox<[u8; 32]>
    entries: Vec<EntrySummary>,
}

/// Datos de búsqueda descifrados de una entrada válida, con sus fechas.
/// Se borra al soltarse gracias a `SearchData`; su `Debug` no muestra valores.
#[derive(Debug)]
pub struct EntrySummary {
    id: EntryId,
    search: SearchData,
    modified_at: i64,
    last_used_at: Option<i64>,
}

impl ZeroizeOnDrop for EntrySummary {} // la garantía la da `SearchData`

impl EntrySummary {
    pub fn id(&self) -> EntryId;
    pub fn search(&self) -> &SearchData;
    pub fn modified_at(&self) -> i64;
    pub fn last_used_at(&self) -> Option<i64>;
}

/// Resultado de desbloquear, sin texto plano.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlockReport {
    /// Entradas que no se pudieron descifrar.
    pub corrupt: Vec<EntryId>,
    /// Filas dañadas sin un id de 16 bytes.
    pub unidentified_corrupt: usize,
}

/// Qué hacer con el secreto al editar.
#[derive(Debug)]             // `Secret` ya tiene un `Debug` que oculta su valor
pub enum SecretChange {
    Keep,
    Replace(Secret),
}

impl Vault {
    pub fn create(path: &Path, password: &SecretString) -> Result<Self>;
    pub fn open(path: &Path) -> Result<Self>;
    pub fn unlock(&mut self, password: &SecretString) -> Result<UnlockReport>;
    pub fn lock(&mut self);
    pub fn is_locked(&self) -> bool;
    pub fn entries(&self) -> Result<&[EntrySummary]>;
    pub fn add(&mut self, search: SearchData, secret: Secret) -> Result<EntryId>;
    pub fn edit(&mut self, id: EntryId, search: SearchData, change: SecretChange) -> Result<()>;
    pub fn delete(&mut self, id: EntryId) -> Result<()>;
    pub fn reveal(&mut self, id: EntryId) -> Result<Secret>;
    pub fn change_master_password(&mut self, current: &SecretString, new: &SecretString) -> Result<()>;

    pub(crate) fn create_with_params(path: &Path, password: &SecretString, params: KdfParams) -> Result<Self>;
    pub(crate) fn change_master_password_with_params(
        &mut self, current: &SecretString, new: &SecretString, params: KdfParams,
    ) -> Result<()>;
    fn verify_current(&self, current: &SecretString) -> Result<()>;
    fn rekey_with_params(&mut self, new: &SecretString, params: KdfParams) -> Result<()>;
}

impl VaultHeader {
    /// Cabecera nueva para la misma llave de bóveda: sal y nonce nuevos.
    pub(crate) fn rekey(vault_key: &VaultKey, password: &SecretString, params: KdfParams) -> Result<Self>;
}

fn now_millis() -> Result<i64>;
fn millis_since_epoch(time: SystemTime) -> Result<i64>;
```

`add` y `edit` reciben `SearchData` y `Secret` por valor: la caché se queda con el `SearchData` sin copiarlo, y el `Secret` se borra al terminar.

`VaultHeader::rekey` reutiliza `kdf::generate_salt` y `keys::seal_vault_key`; `VaultHeader::create` puede delegar en ella después de generar la llave, para tener un solo camino.

## Flujo de cada operación

- **`create(path, password)`:** `calibrate()` → `create_with_params`: `VaultHeader::create` → `Storage::create` (ruta existente → `AlreadyExists`; fallo posterior → borra el archivo) → bóveda desbloqueada con caché vacía.
- **`open(path)`:** `Storage::open` → bóveda bloqueada. Aquí aparecen `NotAVault`, `UnsupportedFormatVersion` y los errores de almacenamiento.
- **`unlock(password)`:**
  1. Desbloqueada → `AlreadyUnlocked`.
  2. `read_header` → `header.unlock`. Incorrecta → `WrongPassword` y sigue bloqueada.
  3. `list_entries` y `open_search` de cada una. Las que fallan van a `corrupt`; `unidentified_corrupt` se copia tal cual.
  4. Solo al final se instala `Unlocked`. Si `list_entries` falla (por ejemplo, `Storage(Corrupt)`), la llave se suelta y la bóveda sigue bloqueada.
- **`lock()`:** `unlocked = None`.
- **`entries()`:** la caché, o `Locked`.
- **`add(search, secret)`:** `now_millis` → `EntryId::generate` → `SealedEntry::seal` → `insert_entry(now)` → agrega a la caché.
- **`edit(id, search, change)`:** `now_millis` → `get_entry` → (`Keep`: `open_secret`) → `seal` → `update_entry(now)` → reemplaza en la caché, o agrega si venía de `corrupt`, con `last_used_at` de la fila.
- **`delete(id)`:** `now_millis` → `delete_entry(now)` → lo quita de la caché si estaba. También funciona sobre una entrada dañada: es solo SQL.
- **`reveal(id)`:** `get_entry` → `open_secret` (falla → `CorruptEntry`) → `last_used_at` en la caché → intenta `now_millis` y `touch_entry`, descartando el error.
- **`change_master_password(current, new)`:** desbloqueada → `verify_current` → `calibrate()` → `rekey_with_params`: `VaultHeader::rekey` con la llave de memoria → `replace_header`.

En todas las escrituras, primero se escribe en `Storage` y solo después se cambia la caché: si la escritura falla, la caché queda igual. La única excepción es `reveal`, que actualiza la caché antes del intento de escritura de mejor esfuerzo.

## Pruebas

### Unitarias (`core/src/vault/tests.rs`)

Con `KdfParams::MIN`, archivos reales en un directorio de `tempfile` y SQL directo con una segunda conexión de `rusqlite` cuando hace falta alterar filas.

| Criterio | Prueba |
| --- | --- |
| Ida y vuelta | Crear, agregar 3 entradas, `lock`, `unlock`: `entries()` tiene los mismos ids y campos. También cerrando y reabriendo el archivo con `open` |
| Contraseña incorrecta | `unlock` da `WrongPassword` y la bóveda sigue bloqueada (`entries()` da `Locked`) |
| Bloqueada | `entries`, `add`, `edit`, `delete`, `reveal` y `change_master_password` dan `Locked`. `lock()` dos veces no falla |
| `AlreadyUnlocked` | `unlock` sobre una desbloqueada da el error y la caché sigue intacta |
| Después de `lock()` | `entries()` da `Locked`. `EntrySummary` y `SearchData` implementan `ZeroizeOnDrop` (comprobación al compilar con `assert_zod`, como en la #4) |
| Cambio de contraseña | La vieja falla, la nueva abre, y los bytes de `search` y `secret` de cada fila son idénticos antes y después (SQL directo). Con `current` incorrecta: `WrongPassword` y la cabecera queda igual byte por byte |
| Entrada dañada | Se altera un byte de `search` de una entrada: `unlock` abre las demás y reporta su id en `corrupt`. Una fila con un id de 15 bytes cuenta en `unidentified_corrupt` |
| Lápidas | Eliminar una entrada, `lock`, `unlock`: no aparece ni en `entries()` ni en `corrupt` |
| `reveal` dañado | Se altera `secret`: `CorruptEntry` y `last_used_at` de la fila no cambia |
| `reveal` correcto | Devuelve la contraseña y actualiza `last_used_at` en la caché y en el disco |
| `reveal` con fallo de escritura | Una función `pub(crate)` de prueba deja la conexión sin poder escribir (`PRAGMA query_only = ON`; si no produce `SQLITE_READONLY`, se reabre con `Storage::open_read_only`): `reveal` devuelve la contraseña correcta y `add` da `Storage(ReadOnly)` |
| `edit` con `Keep` | Cambiar solo una etiqueta: `reveal` da la contraseña original, `modified_at` cambia y `last_used_at` no |
| `edit` con `Keep` y secreto dañado | `CorruptEntry` y la fila queda igual byte por byte |
| `edit` con `Replace` | Repara una entrada con el secreto alterado y otra que aparecía en `corrupt`: después de `lock` y `unlock` son válidas |
| `edit` sin entrada | Un id inexistente y una lápida dan `EntryNotFound` |
| `delete` | La fila queda como lápida (`deleted = 1`, partes en `NULL`). Una entrada dañada también se puede eliminar |
| `create` sobre una ruta existente | `AlreadyExists` y el archivo queda igual byte por byte |
| `create` con fallo al escribir | Se crea antes un **directorio** `<ruta>-journal`: SQLite no puede crear su diario, `create` devuelve un error y la ruta queda libre. Si en la práctica no falla ahí, se usa un gancho `#[cfg(test)]` en `Storage::initialize` |
| Reloj | `millis_since_epoch(UNIX_EPOCH - 1 s)` da `Clock`; `UNIX_EPOCH` da `0` |
| Sin secretos en errores | Ningún `Display` ni `Debug` de los errores devueltos en estas pruebas contiene la contraseña maestra, la contraseña de la entrada ni el servicio |

`error/tests.rs` agrega las cuatro variantes nuevas a la prueba que comprueba que ningún mensaje lleva valores.

### De punta a punta (`core/tests/vault.rs`)

Con la calibración real; aceptan tardar algunos segundos.

1. Crear → agregar → `lock` → `unlock`: mismas entradas. Además, los parámetros de la cabecera, leídos con SQL directo (`KdfParams` es privado), están dentro de los límites: memoria de 64 a 128 MiB, al menos 3 iteraciones y paralelismo 1, sin fijar valores exactos.
2. Cambiar la contraseña: abre con la nueva y falla con la vieja con `WrongPassword`.

Ambas usan solo la API pública (`vault_core::vault::...`), lo que comprueba también las exportaciones.

## Dependencias nuevas

Ninguna. `rusqlite` y `tempfile` ya están en `core/Cargo.toml` y sirven también a `core/tests/`.

## Documentación

- `docs/especificacion.md`, "Formato de la bóveda → API":
  - La llave vive junto con la caché en `Option<Unlocked>`, y `lock()` borra ambas al poner `None`.
  - `unlock()` devuelve `UnlockReport` (ids dañados y conteo sin id) y las entradas se leen con `entries()`.
  - `edit` con `Keep` o `Replace`; `reveal` actualiza `last_used_at` como mejor esfuerzo.
  - `change_master_password` verifica la contraseña actual antes de calibrar.
- `ARCHITECTURE.md`: en la fila de `vault/`, describir `mod.rs` como la API `Vault`.
- `docs/roadmap.md`: marcar "API de la bóveda" como terminado.

## Commits previstos

1. `docs:` diseño de la API de la bóveda
2. `refactor:` `AlreadyExists` como variante propia de `Error`
3. `feat:` errores `Locked`, `AlreadyUnlocked` y `Clock`, y lectura del reloj
4. `feat:` `Vault` con `create`, `open` y `unlock`, y contraseña incorrecta
5. `feat:` `lock`, `entries` y `Error::Locked`
6. `feat:` `add`, `edit`, `delete` y `reveal`
7. `test:` entradas dañadas, lápidas y reparación
8. `feat:` `change_master_password` con `VaultHeader::rekey`
9. `refactor:` exponer `vault` como API pública y mover las reexportaciones
10. `test:` pruebas de punta a punta en `core/tests/vault.rs`
11. `docs:` especificación, arquitectura y roadmap
