# Diseño: respaldos, exportar e importar (issue #9)

Fecha: 2026-11-07 · Etapa 1 · Rama `feat/9-respaldos`

## Objetivo

Exportar la bóveda a un archivo de respaldo cifrado e importarlo, en el mismo celular o en uno nuevo. Un respaldo es una bóveda completa: abre con la misma contraseña maestra con la que se exportó. La lógica de archivos vive en `core/src/vault/backup.rs` y se expone mediante `Vault`.

### Fuera de alcance

- Regenerar la identidad del dispositivo al restaurar: requisito anotado para la etapa 3.
- El selector de archivos de Android (etapa 2). El núcleo recibe rutas; la fachada copia desde y hacia el selector.
- Volver a activar la huella tras importar: la copia de la llave en el Keystore no viaja y le toca a la app (etapa 2).
- Rutas URI en `Storage::open` y `Storage::create` (ver "Pendiente para otras issues").

### Criterios de aceptación (de la issue)

- Exportar, borrar la bóveda e importar recupera todas las entradas y sus secretos.
- Importar con una contraseña incorrecta falla y deja la bóveda actual intacta.
- Importar un archivo con una versión de formato no soportada falla y deja la bóveda actual intacta.
- Importar un archivo que no es una bóveda falla sin panic.
- El archivo exportado no contiene ninguna cadena de texto plano conocida.
- `cargo test -p vault-core` y `clippy` pasan, y la CI está en verde.

## Decisiones

### Exportar exige la bóveda desbloqueada

Es la decisión pendiente de la issue. El archivo ya está cifrado, así que no es por confidencialidad: exigirla desbloqueada impide exportaciones sin que el usuario esté presente. Además, mantiene la regla de la #7: con la bóveda bloqueada, todo salvo `unlock` y `lock` devuelve `Error::Locked`.

### Importar es una función asociada

`Vault::import(backup, dest, password)`, como `create`: el mismo camino sirve para restaurar sobre una bóveda existente y para un celular nuevo sin bóveda.

- Quien llama suelta antes cualquier `Vault` abierta sobre `dest`. Si la importación falla, la vuelve a abrir con `Vault::open` (queda bloqueada).
- Se descartó un método sobre `&mut self`: obligaría a `Vault` a guardar su ruta, cerrar y reabrir su propia conexión, y aun así haría falta otra función para el celular nuevo.
- Por qué quien llama debe soltar la bóveda: en Windows no se puede reemplazar un archivo que SQLite tiene abierto (falla y `dest` queda intacta). En Android sí se puede, pero la conexión vieja seguiría escribiendo sobre el inodo ya reemplazado y esas escrituras se perderían en silencio.
- La contraseña es la del respaldo, que puede ser distinta de la actual si se cambió después de exportar.
- Devuelve la bóveda importada desbloqueada, con su `UnlockReport`, para no gastar otro Argon2.

### Validación antes de reemplazar

Además de la contraseña y la versión de formato que pide la issue:

- `PRAGMA integrity_check` sobre la copia. Revisa páginas, registros, `CHECK`, `NOT NULL` y la unicidad de la llave primaria. Su costo es O(N log N), irrelevante con cientos de filas, y verifica más que `quick_check`, que omite índices y unicidad. Cualquier resultado distinto de una sola fila `ok` da `Storage(Corrupt)`. Los mensajes se descartan sin leerlos.
- Desbloqueo real: se listan las entradas y se descifran sus datos de búsqueda.
- Las entradas dañadas sueltas no impiden importar: se reportan en el `UnlockReport`, igual que en `unlock`. Rechazar la importación por una sola entrada impediría recuperar un respaldo que es casi todo bueno.

### `VACUUM INTO` y durabilidad

Verificado con la documentación de SQLite (`lang_vacuum.html`):

- El destino "must not previously exist, or else it must be an empty file". Por eso `export` crea antes un archivo vacío con `create_new`: la comprobación de `AlreadyExists` es atómica.
- "if the PRAGMA synchronous setting of the original database is NORMAL or FULL, then SQLite invokes fsync() or FileFlushBuffers() to sync the output database". Cada apertura fija `synchronous = FULL`, así que el núcleo no hace un `sync_all()` propio del respaldo. Una prueba fija ese supuesto.
- "all deleted content is purged from the backup, leaving behind no forensic traces". El respaldo sale compacto, sin páginas libres: las partes cifradas de las entradas eliminadas no viajan. Las lápidas sí, porque son filas.
- Se prefiere a la API de respaldo en línea (feature `backup` de `rusqlite`): esa copia también las páginas libres, y además agregaría una feature.

### Rutas absolutas

El SQLite empaquetado se compila con `-DSQLITE_USE_URI` (`build.rs` de `libsqlite3-sys` 0.39.1). Una ruta que empiece con `file:` se interpretaría como URI, con parámetros como `?mode=` o `?vfs=`.

- `export` e `import` rechazan `backup` y `dest` si no son absolutas, con `Storage(Io)`. Una ruta absoluta no puede empezar con `file:`.
- `VACUUM INTO ?1` recibe la ruta como parámetro, nunca formateada dentro del SQL. Una ruta que no es UTF-8 da `Storage(Io)`.

### El `-journal` de la bóveda actual

"How To Corrupt Your Database Files" (§1.4) lista como causa de corrupción: "Overwriting a database file with another without also deleting any hot journal associated with the original database". Si `dest` quedó con un `-journal` caliente (un cierre forzado a mitad de una transacción), SQLite lo aplicaría sobre el respaldo recién instalado.

- Antes de reemplazar, si existe `<dest>-journal` o `<dest>-wal`, se abre `dest` con `Storage::open` y se cierra: eso revierte el diario (o integra el WAL y lo pasa a `delete`).
- Si esa apertura falla (por ejemplo, `dest` no es una bóveda, así que SQLite nunca la toca), se devuelve el error sin reemplazar nada.
- Se descartaron dos alternativas. Borrar el diario antes de renombrar dañaría la bóveda actual si después falla el renombrado. Borrarlo después de renombrar deja un intervalo en el que, si hay un corte de luz, SQLite aplicaría el diario viejo sobre el respaldo.
- (Agregado en la #20.) Si `dest` no existe, su `-journal`, `-wal` y `-shm` son huérfanos (la bóveda se borró a mano, o un celular con restos de otra instalación): no hay base que revertir ni que dañar. Se borran antes del `rename` y, en Unix, se sincroniza el directorio (mejor esfuerzo), para que un corte de luz no deje el `rename` hecho y el borrado perdido. Si un borrado falla → `Storage(Io)`, sin reemplazar nada. Dejarlos no es opción: SQLite aplicaría el diario viejo sobre el respaldo. Para saber si `dest` existe se usa `fs::symlink_metadata`, no `Path::exists`, que da `false` ante cualquier error (por ejemplo, sin permisos) y haría borrar el diario de una bóveda que sí existe. Solo `NotFound` cuenta como ausente; otro error → `Storage(Io)`.

### Reemplazo con `rename`

- La copia temporal vive junto a `dest` (`<dest>.import`), para que `fs::rename` quede en el mismo volumen: `std` lo exige.
- En Unix, `rename` es atómico. En Windows, `std` usa `MoveFileExW`, o `FileRenameInfoEx` con semántica POSIX desde Windows 10 1607. La documentación no promete atomicidad; con `dest` abierta por otra conexión, el reemplazo falla y `dest` queda intacta.
- En Unix, después de renombrar se hace `fsync` del directorio como mejor esfuerzo; en Windows no aplica.

## Componentes

```
core/src/vault/backup.rs        # export_to, import_into, rutas, -journal de dest
core/src/vault/backup/tests.rs  # pruebas unitarias
core/src/vault/mod.rs           # Vault::export, Vault::import, carga de entradas compartida
core/src/vault/storage.rs       # integrity_check; quita los expect(dead_code)
core/src/vault/test_support.rs  # #[cfg(test)] ayudantes compartidos (diario caliente, SQL directo)
core/tests/vault.rs             # prueba de punta a punta
```

`make_hot_journal_copy` y sus auxiliares (`raw`, `sibling`) pasan de `storage/tests.rs` a `test_support.rs`, para que las usen las pruebas de `storage` y las de `backup`.

### Tipos y firmas

```rust
impl Vault {
    /// Copia consistente de la bóveda en `dest` con `VACUUM INTO`.
    /// Exige la bóveda desbloqueada. `dest` debe ser absoluta y no existir.
    pub fn export(&self, dest: &Path) -> Result<()>;

    /// Valida `backup` con `password` en una copia temporal y, solo si todo
    /// pasa, reemplaza `dest` (exista o no). Devuelve la bóveda importada,
    /// desbloqueada. Quien llama no debe tener abierta una `Vault` sobre `dest`.
    pub fn import(backup: &Path, dest: &Path, password: &SecretString)
        -> Result<(Vault, UnlockReport)>;

    /// Lista las entradas y descifra sus datos de búsqueda. La usan
    /// `unlock` e `import`.
    fn load_entries(storage: &Storage, key: VaultKey) -> Result<(Unlocked, UnlockReport)>;
}

impl Storage {
    /// `VACUUM INTO` sobre `dest`, que ya existe y está vacío.
    pub(crate) fn vacuum_into(&self, dest: &Path) -> Result<()>;
    /// `PRAGMA integrity_check`: `Storage(Corrupt)` si no da una sola fila `ok`.
    pub(crate) fn check_integrity(&self) -> Result<()>;
}
```

Los nombres internos de `backup.rs` se fijan en el plan.

## Flujo

### Exportar: `vault.export(dest)`

1. Con la bóveda bloqueada → `Locked`, antes de tocar el disco.
2. `dest` no absoluta → `Storage(Io)`.
3. Crea `dest` vacío con `create_new`. Si ya existe → `AlreadyExists`, sin tocarlo.
4. `VACUUM INTO ?1`. SQLite hace el `fsync` porque `synchronous = FULL`.
5. Si algo falla después del paso 3, borra `dest`.

### Importar: `Vault::import(backup, dest, password)`

1. `backup` o `dest` no absolutas → `Storage(Io)`.
2. Copia `backup` a `<dest>.import`. Si existe un `.import` de un intento interrumpido, se borra antes. El respaldo original nunca se abre con SQLite.
3. `sync_all()` de la copia.
4. `Storage::open_read_only(copia)`: rechaza un archivo ajeno (`NotAVault`), en WAL (`Storage(JournalMode)`) o de una versión desconocida (`UnsupportedFormatVersion`).
5. `check_integrity()` → `Storage(Corrupt)` si falla.
6. `read_header()?.unlock(password)` → `WrongPassword` si no corresponde.
7. `load_entries`: caché y `UnlockReport`.
8. `close()` de la conexión de solo lectura (en Windows hace falta para renombrar).
9. Si existe `<dest>-journal` o `<dest>-wal`: `Storage::open(dest)` y `close()`. Si falla → error, sin reemplazar. Si `dest` no existe, en cambio, se borran `<dest>-journal`, `<dest>-wal` y `<dest>-shm` huérfanos y se sincroniza el directorio (#20).
10. `fs::rename(copia, dest)`. En Unix, `fsync` del directorio como mejor esfuerzo.
11. `Storage::open(dest)`, que fija los pragmas, y se devuelve `Vault` con el `Unlocked` del paso 7 y el `UnlockReport`.

**Garantía:** si falla cualquiera de los pasos 1 a 10, se borra la copia temporal y `dest` queda igual byte por byte, o sigue sin existir si no existía. La única excepción es la reversión del paso 9: deja `dest` en su último estado confirmado, que es lo mismo que haría cualquier apertura normal.

**Único punto sin vuelta atrás:** si falla el paso 11, el archivo ya reemplazado es el respaldo validado. Se devuelve el error y la app puede reintentar con `Vault::open`. Queda documentado en el comentario de `import`.

**Limitación conocida:** si se elige como respaldo una bóveda viva con su propio `-journal` caliente, la copia viaja sin ese diario. Un respaldo hecho con `VACUUM INTO` nunca tiene `-journal`, y un daño así lo detecta el paso 5.

## Errores

No hay variantes nuevas. Se reutilizan:

| Situación | Error |
| --- | --- |
| Exportar con la bóveda bloqueada | `Locked` |
| `dest` de la exportación ya existe | `AlreadyExists` |
| Ruta no absoluta o no UTF-8 | `Storage(Io)` |
| El respaldo no es una bóveda | `NotAVault` |
| Versión de formato desconocida | `UnsupportedFormatVersion(v)` |
| Respaldo en WAL | `Storage(JournalMode)` |
| `integrity_check` falla | `Storage(Corrupt)` |
| Contraseña incorrecta | `WrongPassword` |
| `dest` abierta por otra conexión (Windows) | `Storage(Io)` o `Storage(Busy)` |

## Pruebas

### Unitarias (`core/src/vault/backup/tests.rs`)

Con `KdfParams::MIN`, archivos reales en un directorio de `tempfile` y SQL directo con una segunda conexión de `rusqlite` cuando hace falta alterar el archivo.

| Criterio | Prueba |
| --- | --- |
| Ida y vuelta | Crear, agregar 3 entradas (una con notas), eliminar una, exportar, borrar el archivo e importar. `entries()` tiene las mismas 2, `reveal()` devuelve sus secretos y la lápida no aparece |
| Contraseña incorrecta | `WrongPassword`; `dest` queda igual byte por byte y no queda `.import` |
| Versión no soportada | Un respaldo con `format_version = 2` (SQL directo) da `UnsupportedFormatVersion(2)` y `dest` queda intacta |
| No es una bóveda | Bytes aleatorios, un archivo vacío, texto y un SQLite ajeno dan `NotAVault`, sin panic, y `dest` queda intacta |
| Sin texto plano | Los bytes exportados no contienen ninguna cadena conocida: servicio, URL, usuario, contraseña, notas, etiquetas y contraseña maestra |
| Sin restos de lo eliminado | Los bytes cifrados de la entrada eliminada, tomados antes de eliminarla, no aparecen en el respaldo |
| Exportar bloqueada | `Locked` y `dest` no se crea |
| Exportar sobre un archivo existente | `AlreadyExists` y el archivo no cambia |
| Rutas no absolutas | `export` e `import` dan `Storage(Io)` y no crean nada |
| Sincronización | Antes de exportar, `synchronous = 2` en la conexión: fija el supuesto de que SQLite hace el `fsync` |
| Archivo dañado | Páginas de `entries` alteradas: pasa `open_read_only`, pero da `Storage(Corrupt)` y `dest` queda intacta |
| Entrada dañada | Un byte alterado en la parte cifrada de una entrada: la importación sale bien y el `UnlockReport` trae su id |
| Celular nuevo | `dest` no existe: la importación la crea. Si falla, `dest` sigue sin existir |
| Otra contraseña | El respaldo tiene una contraseña distinta de la bóveda actual: se importa con la del respaldo, y la actual ya no sirve |
| `-journal` caliente en `dest` | Con `make_hot_journal_copy`, `dest` tiene un diario caliente: se revierte y la importación sale bien, sin el diario. Si `dest` no es una bóveda y tiene un `-journal`, da error y ninguno de los dos archivos cambia |
| `.import` sobrante | Un `.import` de un intento anterior se reemplaza; tras un fallo no queda ninguno |
| `dest` abierta (`cfg(windows)`) | Con otra conexión abierta sobre `dest`: error y `dest` queda intacta |
| Sin secretos en errores | Ningún `Display` ni `Debug` de los errores devueltos contiene la contraseña maestra ni datos de las entradas |

### De punta a punta (`core/tests/vault.rs`)

Con la calibración real y solo la API pública: crear, agregar entradas, exportar, borrar la bóveda e importar recupera todas las entradas y sus secretos.

## Dependencias nuevas

Ninguna. `VACUUM INTO` ya está en el SQLite empaquetado de `rusqlite`, y `tempfile` ya está en las dependencias de desarrollo.

## Documentación

- `docs/especificacion.md`, "Formato de la bóveda → Respaldos":
  - Exportar exige la bóveda desbloqueada (decidido el 2026-11-07).
  - `Vault::import(backup, dest, password)` como función asociada; quien llama suelta antes la bóveda abierta.
  - Validación en una copia temporal: `integrity_check` y desbloqueo real; las entradas dañadas se reportan y no impiden importar.
  - El `-journal` de `dest` se revierte antes de reemplazar.
  - Rutas absolutas por `SQLITE_USE_URI`, y el `fsync` que hace SQLite con `synchronous = FULL`.
- `ARCHITECTURE.md`: en la fila de `vault/`, describir `backup.rs`.
- `docs/roadmap.md`: marcar "Exportar e importar el respaldo cifrado".
- `core/src/vault/mod.rs`: quitar "Los respaldos llegan más adelante".

## Pendiente para otras issues

- `Storage::open` y `Storage::create` también reciben rutas que SQLite interpretaría como URI si empiezan con `file:`. Hoy las rutas de la app son absolutas, pero conviene la misma comprobación ahí.

## Commits previstos

1. `docs:` diseño de los respaldos
2. `refactor:` ayudantes de prueba compartidos en `test_support.rs`
3. `feat:` exportar con `VACUUM INTO`
4. `refactor:` extraer la carga de entradas de `unlock`
5. `feat:` importar con validación en una copia temporal
6. `feat:` revertir el `-journal` de la bóveda actual antes de reemplazarla
7. `test:` casos de error, texto plano y punta a punta
8. `docs:` especificación, arquitectura y roadmap
