# Diseño: desbloquear con la llave que libera el Keystore (issue #34)

Fecha: 2026-11-08 · Etapa 2 · Rama `feat/34-desbloqueo-con-llave`

## Objetivo

Que el núcleo entregue la llave de bóveda para que la app guarde una copia en el Keystore (#38), y que pueda desbloquear con esa llave con las mismas reglas que con la contraseña maestra. Para rechazar la llave de otra bóveda o una llave alterada, también con la bóveda vacía, la cabecera guarda un verificador de la llave. Eso es la versión de formato 2, con migración desde la 1.

### Fuera de alcance

- El Keystore, `BiometricPrompt`, dónde guarda la app la copia y la fachada de `mobile/rust` (#38).
- Pedir la contraseña maestra cada 7 días (etapa 4).

### Criterios de aceptación (de la issue)

- Desbloquear con el material correcto da las mismas entradas y el mismo `UnlockReport` que con la contraseña.
- Material de otra bóveda o alterado da un error tipado y la bóveda sigue bloqueada, también con la bóveda vacía.
- Sobre una bóveda desbloqueada da `AlreadyUnlocked`.
- El material viaja en tipos de `secrecy` o `Zeroizing`; ningún tipo nuevo deriva `Debug` ni `Display` con su valor.
- Si cambia el formato: `format_version` sube, la migración tiene prueba, existe `vault-v2.db` y `vault-v1.db` sigue abriendo.
- La especificación registra las decisiones.
- `cargo test -p vault-core` y `clippy` pasan, y la CI está en verde.

## Decisiones

### El material es la llave de bóveda tal cual

Se descartó que el núcleo generara un secreto del dispositivo y le entregara a la app la llave de bóveda envuelta con él. El secreto también tendría que cruzar a Kotlin para que el Keystore lo cifre, y la llave envuelta quedaría en el almacenamiento privado de la app: quien obtenga el secreto tiene el mismo poder que con la llave de bóveda. Además, agregaría un segundo formato con versión. La opción elegida encaja con la especificación: "El Keystore de Android guarda otra copia cifrada" de la llave de bóveda.

La llave sigue valiendo después de `change_master_password`, que no cambia la llave de bóveda. Es el comportamiento esperado y queda documentado.

### API

- `Vault::export_vault_key(&mut self) -> Result<SecretBox<[u8; 32]>>`: solo con la bóveda desbloqueada (si no, `Error::Locked`). Es `&mut` porque puede reparar el verificador (ver "Verificador dañado").
- `Vault::unlock_with_key(&mut self, key: SecretBox<[u8; 32]>) -> Result<UnlockReport>`: se comporta como `unlock`. Sobre una bóveda desbloqueada da `AlreadyUnlocked`; si falla, la bóveda sigue bloqueada; si todo va bien, devuelve el mismo `UnlockReport` y llena la misma caché con `load_entries`.
- `unlock_with_key` recibe la llave **por valor**: se mueve el `Box` sin copiar el secreto y el núcleo se queda con ella como la llave de la sesión. Si falla, la llave recibida se descarta y se borra de memoria al soltarse. Quien llama no la recupera: para reintentar, debe pedirla de nuevo al Keystore.
- Los nombres no mencionan el Keystore: el núcleo no conoce la plataforma.

### Error nuevo: `Error::WrongVaultKey`

Mensaje: "la llave no corresponde a esta bóveda" (español, en minúsculas y sin punto, como el resto de `error.rs`). Es distinto de `WrongPassword` para que la app sepa que debe desactivar la huella y pedir la contraseña. No distingue entre una llave equivocada y un verificador dañado.

### El verificador `key_check`

En el formato v1 no hay nada con qué comprobar la llave en una bóveda vacía: `encrypted_key` está cifrada con la llave derivada de la contraseña y `application_id` es igual en todas las bóvedas. La cabecera de la v2 agrega `key_check`, de 40 bytes: `nonce (24) | etiqueta (16)`, resultado de cifrar un texto vacío con XChaCha20-Poly1305 y la llave de bóveda.

Dato autenticado, binario de ancho fijo sin separadores (18 bytes):

| Offset | Campo | Tamaño | Valor |
| --- | --- | --- | --- |
| 0–15 | Etiqueta | 16 bytes | ASCII `boveda-key-check` |
| 16–17 | Versión de formato | 2 bytes | `u16` little-endian |

- La etiqueta no puede confundirse con `boveda-v1`, la de las entradas.
- Como incluye la versión de formato, una migración futura tendrá que regenerarlo; toda migración ya tiene la llave en memoria.
- No forma parte del dato autenticado de `encrypted_key`: alterarlo solo inutiliza la huella, y eso ya puede lograrlo quien escriba el archivo.
- XChaCha20-Poly1305 no compromete la llave: se puede construir un texto que se autentique con dos llaves, pero eso exige conocer ambas. Quien altera el archivo no conoce la llave real, así que no aplica.
- La comparación de la etiqueta de Poly1305 ya es de tiempo constante.

Alternativas descartadas: un hash de la llave (BLAKE2b) daría la misma garantía, pero agrega una dependencia criptográfica directa. Una entrada "testigo" en `entries` obligaría a ignorarla en la lista, la búsqueda, los respaldos y la futura sincronización. Un identificador aleatorio de bóveda detecta la llave de otra bóveda, pero no una llave alterada.

### Quién crea, comprueba y repara el verificador

- `VaultHeader::rekey` siempre calcula `key_check`. Como `create` usa `rekey`, hay un solo camino de creación. `change_master_password` lo vuelve a generar con la misma llave, así que la copia del Keystore sigue valiendo, y de paso repara un verificador dañado.
- `unlock` con contraseña **no** lee `key_check`: la contraseña nunca depende de él.
- `unlock_with_key` lo comprueba con la llave recibida. Si no se autentica, da `WrongVaultKey`.

### Verificador dañado

`export_vault_key` comprueba `key_check` con la llave en memoria. Si no se autentica, lo regenera con un nonce nuevo en una transacción y después entrega la llave. Si esa escritura falla, devuelve el error y no entrega la llave, para no activar una huella que no va a funcionar.

Regenerar es seguro: la llave en memoria ya se autenticó al descifrar `encrypted_key` con la contraseña (o con `key_check`, si se desbloqueó con la llave). Por eso un `key_check` que no se autentica solo puede estar dañado; nunca significa que la llave en memoria sea la equivocada.

Recuperación: con el verificador dañado, la huella falla una vez, la app la desactiva y pide la contraseña, y al reactivarla `export_vault_key` lo repara. Se descartó informar de un verificador dañado en el `UnlockReport`: la app lo descubre en el siguiente intento con huella, que ya tiene su camino de recuperación, y el campo también habría que explicarlo en `import`.

### Formato v2

`entries` no cambia. Para `header` se agrega una columna:

```sql
-- "header" va entre comillas para coincidir con lo que deja ALTER TABLE
-- RENAME en la migración desde la v1. La prueba de igualdad de
-- sqlite_schema entre una bóveda migrada y una creada lo comprueba.
CREATE TABLE "header" (
    id              INTEGER PRIMARY KEY CHECK (id = 1),
    format_version  INTEGER NOT NULL,
    kdf_algorithm   INTEGER NOT NULL,
    kdf_memory_kib  INTEGER NOT NULL,
    kdf_iterations  INTEGER NOT NULL,
    kdf_parallelism INTEGER NOT NULL,
    salt            BLOB NOT NULL CHECK (length(salt) = 16),
    key_nonce       BLOB NOT NULL CHECK (length(key_nonce) = 24),
    encrypted_key   BLOB NOT NULL CHECK (length(encrypted_key) = 48),
    key_check       BLOB NOT NULL CHECK (length(key_check) = 40)
) STRICT;
```

El DDL de la v1 no cambia: la especificación lo conserva como "Esquema de la versión de formato 1", y `vault-v1.db` lo contiene. En el código, la constante del esquema pasa a ser la de la v2, porque `create` siempre crea una v2. El comentario de arriba va junto a esa constante.

Por qué las comillas: lo comprobé con SQLite 3.53.4. Después de `ALTER TABLE header_v2 RENAME TO header`, `sqlite_schema` guarda `CREATE TABLE "header" (...)`, mientras que una tabla creada directamente guarda `CREATE TABLE header (...)`. Con el nombre entre comillas en el DDL, ambos textos son idénticos.

### Lectura de las versiones 1 y 2

- `open` y la apertura en solo lectura aceptan las versiones 1 y 2. Una versión mayor sigue dando `UnsupportedFormatVersion`.
- `read_header` ya lee `format_version` primero y sola. Después usa una consulta por versión: la v1 no tiene la columna `key_check`. En memoria, la cabecera de una v1 no tiene verificador.
- `open` sigue sin modificar el archivo, incluida una v1.

### Migración dentro de `unlock`

`unlock` con contraseña sobre una v1:

1. Argon2id corre una sola vez: con la llave derivada se descifra `encrypted_key` con el dato autenticado de la v1.
2. Con esa misma llave derivada se arma la cabecera v2: misma sal y mismos parámetros, nonce nuevo, dato autenticado de la v2 (versión 2) y `key_check`.
3. En una sola transacción `IMMEDIATE`, se sigue el procedimiento que documenta SQLite para cambios de esquema ("Making Other Kinds Of Table Schema Changes", `lang_altertable.html` §8):
   1. `CREATE TABLE header_v2 (...)` con el DDL de la v2;
   2. se inserta la cabecera nueva;
   3. `DROP TABLE header`;
   4. `ALTER TABLE header_v2 RENAME TO header`;
   5. verificación, todavía dentro de la transacción: `format_version = 2` y la fila tiene un `key_check` de 40 bytes.

   Se renombra la tabla nueva, no la vieja: la documentación advierte que renombrar primero la vieja puede dañar referencias. `header` no tiene índices, triggers, vistas ni llaves foráneas que reconstruir.
4. Recién entonces se cargan las entradas con `load_entries`.

Si algo falla, la transacción se revierte: la bóveda sigue bloqueada, el archivo sigue siendo v1 y el error es el de almacenamiento que ya existe (`Storage(...)`), sin un error propio de migración. Se puede reintentar.

Sobre una v2, `unlock` no lee `key_check` ni escribe nada.

Consecuencia que se agrega a la especificación: abrir sigue sin modificar el archivo, pero `unlock` con contraseña sobre una v1 sí escribe (la migra), como `reveal` escribe `last_used_at`. En un medio de solo lectura, una v1 no se puede desbloquear. Hoy no hay bóvedas v1 reales, así que es aceptable.

### Importar un respaldo v1

La validación de hoy deriva la llave sobre la copia temporal, abierta en solo lectura. Con una v1:

1. La validación conserva la llave derivada y arma con ella la cabecera v2, sin un segundo Argon2id.
2. Se cierra la conexión de solo lectura (como hoy).
3. Se abre la copia temporal en lectura y escritura con `Storage::open` y se migra con la misma transacción de `unlock`.
4. Se cierra y se renombra sobre `dest`, como hoy.

Así `dest` nunca queda en v1 y el único punto sin vuelta atrás sigue siendo el mismo de hoy.

### `unlock_with_key` sobre una v1

Da `WrongVaultKey`. Una v1 nunca pudo activar la huella: entregar la llave exige desbloquear, y desbloquear con contraseña la migra. Solo llega aquí si alguien reemplazó el archivo por fuera, y en ese caso lo correcto es desactivar la huella.

### Ciclo de vida

- `change_master_password`: la llave de bóveda no cambia, así que la copia del Keystore sigue valiendo (`key_check` se regenera con la misma llave).
- Importar un respaldo cambia la llave. La app desactiva la huella al importar (#38 y #44). Si no lo hiciera, `unlock_with_key` lo detecta con `key_check` y da `WrongVaultKey`.

### Viaje de la llave fuera del núcleo

Lo implementa la #38. Queda escrito aquí y en la especificación:

- La llave cruza como bytes (`Vec<u8>` en Rust, `Uint8List` en Dart, `ByteArray` en Kotlin), nunca como `String` ni en base64.
- La fachada pasa los bytes recibidos a `SecretBox` y borra el `Vec` de origen con `zeroize`.
- En Dart y en Kotlin, el arreglo se sobrescribe con ceros apenas se entrega. Es de mejor esfuerzo, como dice `ARCHITECTURE.md`.
- El camino Keystore → Rust pasa por Dart lo menos posible. La opción por omisión es pasar por Dart (flutter_rust_bridge y un canal de plataforma), como mejor esfuerzo. Que Kotlin le entregue los bytes directamente a la fachada por JNI se evalúa en la #38: el crate `jni` sería una dependencia nueva y requiere aprobación.

## Componentes

| Archivo | Cambio |
| --- | --- |
| `core/src/error.rs` | Variante `WrongVaultKey` |
| `core/src/crypto/keys.rs` | `seal_key_check(key, format_version)` y `verify_key_check(key, format_version, key_check)`, con el dato autenticado de 18 bytes; reutilizan el cifrador de `chacha20poly1305`. Una variante de abrir la llave de bóveda que también devuelve la llave derivada, para la migración |
| `core/src/crypto/kdf.rs` | Contador `thread_local` de llamadas a `derive_key`, solo con `#[cfg(test)]` |
| `core/src/vault/header.rs` | `FORMAT_VERSION = 2`; `key_check` en `VaultHeader` (ausente en una v1); `from_parts` acepta las versiones 1 y 2; `rekey` calcula `key_check`; desbloqueo de una v1 que devuelve la llave y la cabecera v2 |
| `core/src/vault/storage.rs` | Esquema v2 con el comentario de las comillas; `read_header` por versión; `replace_header` escribe `key_check`; `migrate_header` (la transacción de la migración); `replace_key_check` (reparación) |
| `core/src/vault/mod.rs` | `export_vault_key`, `unlock_with_key` y la migración en `unlock` |
| `core/src/vault/backup.rs` | Migrar la copia temporal antes del renombrado |
| `core/tests/fixtures/vault-v2.db` | Bóveda de ejemplo nueva |

`VaultKey` sigue siendo `pub(crate)`; las firmas públicas usan `SecretBox<[u8; 32]>`. Ningún tipo nuevo deriva `Debug` ni `Display` con su valor.

## Flujos

### `vault.export_vault_key()`

1. Bloqueada → `Locked`.
2. Lee la cabecera y comprueba `key_check` con la llave en memoria.
3. Si no se autentica, regenera `key_check` con un nonce nuevo y lo escribe en una transacción. Si falla la escritura, devuelve el error sin entregar la llave.
4. Copia la llave directamente dentro de un `SecretBox` nuevo (como `generate_vault_key`, sin pasar por la pila) y la devuelve.

### `vault.unlock_with_key(key)`

1. Desbloqueada → `AlreadyUnlocked` (la llave recibida se suelta y se borra).
2. Lee la cabecera. Una v1 → `WrongVaultKey`.
3. `key_check` no se autentica con `key` → `WrongVaultKey`.
4. `load_entries` con `key` y devuelve el `UnlockReport`.

### `vault.unlock(password)`

1. Desbloqueada → `AlreadyUnlocked`.
2. Lee la cabecera. Una v2: deriva y descifra como hoy. Una v1: deriva, descifra y migra (ver "Migración dentro de `unlock`").
3. `load_entries`.

## Errores

| Caso | Error | Estado |
| --- | --- | --- |
| `export_vault_key` bloqueada | `Locked` | Sin cambios |
| `export_vault_key` sin poder reparar `key_check` | `Storage(...)` | Sin entregar la llave; sigue desbloqueada |
| `unlock_with_key` desbloqueada | `AlreadyUnlocked` | Sin cambios |
| `unlock_with_key` con llave de otra bóveda, alterada, verificador dañado o una v1 | `WrongVaultKey` | Sigue bloqueada |
| `unlock` sobre una v1 con la migración fallida | `Storage(...)` | Sigue bloqueada; el archivo sigue siendo v1 |
| `unlock` con contraseña incorrecta sobre una v1 | `WrongPassword` | Sigue bloqueada; nada escrito |

## Pruebas

Todas con TDD. Las que usan las bóvedas de ejemplo trabajan siempre sobre una copia.

### Unitarias

`crypto/keys/tests.rs`:

- Tabla de offsets del dato autenticado de `key_check` (etiqueta, versión little-endian, 18 bytes).
- `seal_key_check` y `verify_key_check`: ida y vuelta; otra llave, otra versión o un byte alterado fallan; mide 40 bytes; dos llamadas dan nonces distintos.

`vault/header/tests.rs` y `vault/storage/tests.rs`:

- `from_parts` acepta las versiones 1 y 2 y rechaza la 3.
- `rekey` crea un `key_check` válido para la llave.
- El `sqlite_schema` de una bóveda migrada es igual (tipo, nombre, `tbl_name` y `sql`) al de una creada en v2.
- La migración deja `integrity_check` en `ok`.

`vault/tests.rs`:

- Ida y vuelta: crear, agregar entradas, `export_vault_key`, `lock`, `unlock_with_key`: mismas entradas, mismo `UnlockReport` y los secretos se leen.
- Llave equivocada → `WrongVaultKey` y sigue bloqueada.
- Bóveda vacía con llave equivocada → `WrongVaultKey` y sigue bloqueada.
- Llave de otra bóveda → `WrongVaultKey`.
- `export_vault_key` con la bóveda bloqueada → `Locked`.
- `unlock_with_key` sobre una desbloqueada → `AlreadyUnlocked`.
- Después de `change_master_password`, la llave vieja sigue sirviendo (comportamiento esperado) y `key_check` cambió de bytes.
- `key_check` dañado: `unlock` con contraseña funciona; `unlock_with_key` da `WrongVaultKey`; `export_vault_key` lo repara y después `unlock_with_key` funciona.
- Migración fallida: sobre una copia de `vault-v1.db`, con `deny_writes_for_test()`, `unlock` falla con `Storage(...)`, la bóveda sigue bloqueada y el archivo es idéntico byte por byte al de antes.
- `unlock` con contraseña incorrecta sobre una v1 no escribe nada (byte por byte).

`vault/backup/tests.rs`:

- Importar una copia de `vault-v1.db`: `dest` queda en v2 y el contador de `derive_key` vale 1. La prueba lo pone en cero al empezar: `thread_local` aísla las pruebas entre sí, pero no protege de lo que haya corrido antes en el mismo hilo.

### De punta a punta (`core/tests/vault.rs`)

- `vault-v1.db`: `open` la acepta; `unlock` la migra; después es v2 (se comprueba reabriendo: `unlock_with_key` funciona); las entradas y los secretos se leen igual; `export_vault_key` y `unlock_with_key` funcionan; la misma contraseña sigue abriendo.
- `unlock_with_key` sobre una copia de `vault-v1.db` sin migrar → `WrongVaultKey`.
- `vault-v2.db`: abre con su contraseña y su contenido es el esperado.

### Bóveda de ejemplo `vault-v2.db`

Mismo contenido que `vault-v1.db` (ver su README), con la contraseña `prueba-bóveda-v2`, generada una sola vez con `KdfParams::MIN` por `generate_fixture_v2` en `core/src/vault/tests.rs` (`#[ignore]`, falla con `AlreadyExists` si el archivo existe). `vault-v1.db` no se toca.

## Dependencias nuevas

Ninguna.

## Documentación

En el mismo PR:

- **Especificación**, "Formato de la bóveda":
  - esquema de la versión 2 (el de la 1 se conserva);
  - la tabla de offsets del dato autenticado de `key_check`;
  - la migración y sus consecuencias (`unlock` sobre una v1 escribe; en solo lectura no se desbloquea);
  - la nota sobre XChaCha20-Poly1305;
  - en "API", `export_vault_key`, `unlock_with_key`, `WrongVaultKey` y que la llave recibida por valor se descarta si falla;
  - el viaje de la llave fuera del núcleo;
  - la decisión de las bóvedas de ejemplo, si nombra solo la v1.
- **`core/tests/fixtures/README.md`**: sección de `vault-v2.db`.
- **`ARCHITECTURE.md`**: el verificador en la fila de `crypto/` y la cabecera v2 en la de `vault/`.

Al cerrar, como comentario en la #38: la opción de JNI (dependencia nueva `jni`, que se decide allí con aprobación; por omisión, pasar por Dart como mejor esfuerzo).

## Commits previstos

1. `docs: diseño del desbloqueo con la llave de bóveda (#34)`
2. `docs: especificación del formato v2 y el desbloqueo con llave`
3. `feat: verificador de la llave de bóveda en crypto`
4. `feat: formato v2 de la cabecera con key_check y migración desde la v1`
5. `feat: export_vault_key y unlock_with_key`
6. `feat: migrar los respaldos v1 al importar`
7. `test: bóveda de ejemplo vault-v2.db`
8. `docs: README de bóvedas de ejemplo y ARCHITECTURE.md`
