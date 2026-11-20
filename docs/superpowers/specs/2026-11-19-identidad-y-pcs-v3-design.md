# Diseño: identidad del celular y PCs emparejadas dentro de la bóveda, formato v3 (issue #93)

Fecha: 2026-11-19 · Etapa 3 (épica #92) · Rama `feat/93-formato-v3`

## Objetivo

La bóveda guarda la llave privada X25519 del celular y la lista de PCs emparejadas, cifradas con la llave de bóveda. Sube a formato v3, con migración desde la v1 y la v2 al desbloquear y al importar. Es solo almacenamiento: el protocolo (#95, #96), la fachada (#102) y el aviso al usuario tras importar (#106) quedan fuera.

### Cambio de alcance respecto a la épica

La épica y la #106 asignan a la #106 el borrado de la identidad y de las PCs al importar. Se mueve a la #93: una vez que existe la v3, cualquier respaldo v3 traería la identidad, y el núcleo no debe poder importarlo sin descartarla. La #106 conserva el aviso y las pruebas de la interfaz. Su texto debe ajustarse (ver "Cambios en otras issues").

## Hallazgo que condiciona el diseño

El dato autenticado de `encrypted_key` incluye la versión (`header.rs`, `associated_data`). Pasar de v2 a v3 exigiría volver a cifrar `encrypted_key` con la llave derivada de la contraseña, y `unlock_with_key` no la tiene. Como la tabla `header` **no cambia** en la v3, no hay motivo para volver a cifrarla.

### Decisión 1: versión de la cabecera, independiente de `format_version`

- Nueva constante `HEADER_VERSION: u16 = 2` y función `header_aad_version(format_version: u16) -> u16` (`1` para la v1, `2` para la v2 y `2` para la v3 y las siguientes). Es la versión de la **estructura de la tabla `header`** y solo cambia cuando esa tabla cambia. No es `format_version`.
- El dato autenticado de `encrypted_key` sigue siendo `versión (u16) | algoritmo (u8) | memoria (u32) | iteraciones (u32) | paralelismo (u32) | sal (16)`, pero el campo `versión` lleva la versión de la cabecera. Para las bóvedas v1 y v2 existentes los bytes no cambian.
- `key_check` **sí** se autentica con `format_version` (v3 para una v3). Es lo que ata la bóveda a su versión.
- **Consecuencia para el futuro.** Si una versión posterior cambia la tabla `header`, esa migración tendrá que volver a cifrar `encrypted_key` con la contraseña. Entonces `unlock_with_key` no podrá migrarla, igual que hoy una v1 da `WrongVaultKey`; se documenta en la especificación cuando ocurra.
- **Qué se pierde.** `format_version` deja de estar autenticado por `encrypted_key`. Un archivo v3 al que se le baje `format_version` a 2:
  - con la contraseña **abre**, porque `unlock` no lee `key_check` y el dato autenticado de `encrypted_key` no cambió; la migración v2→v3 encuentra que las tablas ya existen y falla con un error de almacenamiento limpio (ver "Migración");
  - con la llave, `unlock_with_key` lo rechaza con `WrongVaultKey`: `key_check` se creó con la versión 3 y no se autentica con la 2. Ese caso lo cubre `key_check`, no `encrypted_key`.

## Esquema de la versión de formato 3

`header` y `entries` no cambian. Se agregan dos tablas, con el mismo texto SQL en `SCHEMA` y en la migración (la prueba de igualdad de `sqlite_schema` entre una bóveda migrada y una creada lo exige):

```sql
CREATE TABLE identity (
    id                      INTEGER PRIMARY KEY CHECK (id = 1),
    identity_schema_version INTEGER NOT NULL,
    data                    BLOB NOT NULL CHECK (length(data) = 72)  -- nonce 24 | llave 32 | etiqueta 16
) STRICT;

CREATE TABLE paired_pcs (
    id                BLOB PRIMARY KEY NOT NULL CHECK (length(id) = 16),
    pc_schema_version INTEGER NOT NULL,
    data              BLOB NOT NULL CHECK (length(data) >= 40)       -- nonce 24 | texto | etiqueta 16
) STRICT, WITHOUT ROWID;
```

- Sin lápidas ni fechas en claro: quitar una PC es un `DELETE` (con `secure_delete = ON` el contenido se sobrescribe) y la sincronización queda fuera del MVP. Todo lo demás va cifrado.
- Un `CHECK` solo se aplica al escribir. Una fila de un archivo alterado que no lo cumpla (id que no mide 16 bytes, versión fuera de `u16`, `data` ilegible) cuenta como **PC dañada** y falla sola, como una entrada dañada.
- Verificado con SQLite 3.53 (2026-11-19): el DDL dentro de una transacción se revierte, `CREATE TABLE` sobre una tabla existente falla con `table ... already exists`, y los `CHECK` de longitud rechazan ids de 15 bytes.

### Cifrado y dato autenticado

Mismo cifrado que las partes de una entrada: XChaCha20-Poly1305 con la llave de bóveda, nonce aleatorio de 24 bytes por operación, almacenado como `nonce | texto cifrado | etiqueta`. Cada pieza lleva su propio dato autenticado, con etiquetas distintas entre sí y de `boveda-v1` y `boveda-key-check`:

| Pieza | Dato autenticado |
| --- | --- |
| Identidad | `boveda-identity` (15 bytes) \| versión de estructura (`u16` LE) = 17 bytes |
| PC | `boveda-pc` (9 bytes) \| id de la PC (16) \| versión de estructura (`u16` LE) = 27 bytes |

Así no se puede mover una PC a otro id, intercambiar identidad y PC, ni leer algo con otra versión de estructura. Rollback sigue fuera del modelo de amenazas, como en las entradas.

### Identidad (versión de estructura 1)

- El texto en claro son los **32 bytes de la llave privada, sin serializar**. Cualquier secuencia de 32 bytes aleatorios sirve como llave privada X25519, porque se recorta al usarla: **la #94 no debe validar ni transformar esos bytes al leerlos.**
- Se genera con `getrandom` de forma perezosa, la primera vez que se pide (al emparejar). La llave pública no se guarda: la deriva la #94. La #93 no agrega `x25519-dalek`.

### PC emparejada (versión de estructura 1)

Texto en claro, serializado con `serde` + `postcard` en un `Zeroizing<Vec<u8>>`:

| Campo | Tipo | Regla |
| --- | --- | --- |
| `public_key` | `[u8; 32]` | Llave pública X25519 de la PC. Debe ser única en la lista. |
| `name` | `String` | Recortado; de 1 a 128 bytes UTF-8. |
| `last_ip` | `Option<IpAddr>` | En disco, 4 o 16 bytes; otra longitud es PC dañada. |
| `paired_at` | `i64` | ms Unix; lo fija `Vault` al agregar, con el reloj (`Error::Clock` si no es válido). |
| `last_seen_at` | `Option<i64>` | ms Unix; se fija al actualizar `last_ip`. |

## API (`vault`)

Todas exigen la bóveda desbloqueada (`Locked` si no). Nada se carga al desbloquear: `UnlockReport` no cambia, y una identidad o una PC dañada nunca impide abrir la bóveda ni las entradas.

```rust
impl Vault {
    pub fn identity(&self) -> Result<Option<Identity>>;      // ausente = Ok(None); dañada = Err(CorruptIdentity)
    pub fn ensure_identity(&mut self) -> Result<Identity>;   // ausente: la crea; presente y válida: la devuelve; dañada: CorruptIdentity, nunca sobrescribe
    pub fn paired_pcs(&self) -> Result<PairedPcs>;           // válidas y ids de las dañadas
    pub fn add_paired_pc(&mut self, new: NewPairedPc) -> Result<PairedPcId>;
    pub fn update_paired_pc(&mut self, id: PairedPcId, update: PairedPcUpdate) -> Result<()>;
    pub fn remove_paired_pc(&mut self, id: PairedPcId) -> Result<()>;   // también una fila dañada
}
```

- `Identity`: `private_key(&self) -> &SecretBox<[u8; 32]>`; sin `Debug` con el valor, se borra al soltarse.
- `PairedPcs { valid: Vec<PairedPc>, corrupt: Vec<PairedPcId> }`; una fila sin id de 16 bytes no tiene id que reportar y se cuenta en `unidentified_corrupt: usize`, como `UnlockReport`.
- `PairedPc`: `id`, `public_key`, `name`, `last_ip`, `paired_at`, `last_seen_at`; su `Debug` solo muestra el id (nombre e IP revelan con qué equipos se usa la bóveda).
- `NewPairedPc { public_key, name, last_ip }`; `PairedPcUpdate { name: Option<String>, last_ip: Option<IpAddr> }` (`None` conserva; un `last_ip` nuevo también fija `last_seen_at`).
- Errores nuevos (sin datos ni secretos): `CorruptIdentity`, `CorruptPairedPc`, `PairedPcNotFound`, `PairedPcExists` (llave pública ya emparejada), `InvalidPairedPc(InvalidPairedPcField)` con `NameEmpty` y `NameTooLong`.
- Los métodos son `pub` porque la pareja `pub(crate)` sin uso daría `dead_code` bajo `clippy -D warnings`. La fachada no los expone hasta la #102.

## Migración

Abrir (`open` y la apertura en solo lectura) acepta las versiones 1, 2 y 3 y nunca modifica el archivo.

**`unlock` con contraseña:**

- v1: una derivación de Argon2id, migración v1→v2 con el procedimiento actual (transacción propia) y, a continuación, v2→v3.
- v2: solo v2→v3.
- v3: no escribe.

**`unlock_with_key`:** v1 da `WrongVaultKey`, como hoy. En v2 se verifica la llave con `key_check` (versión 2) **antes** de escribir, y después se migra a v3 (`key_check` regenerado con la versión 3; la copia del Keystore sigue valiendo porque la llave de bóveda no cambia). Una llave equivocada no escribe nada.

**Migración v2→v3** (`Storage::migrate_v2_to_v3`), una transacción `IMMEDIATE`:

1. Relee `format_version`. Si ya es 3 (otra conexión migró): no escribe y devuelve `Migration::NotNeeded`; con contraseña, `unlock` relee la cabecera y vuelve a verificarla (como en la v1→v2: una contraseña cambiada da `WrongPassword`), y con llave vuelve a verificar `key_check`. Si no es 2 ni 3: `Storage(Corrupt)`.
2. Si `identity` o `paired_pcs` ya existen en `sqlite_schema`: `Storage(Corrupt)` y se revierte. Es el caso de una v3 degradada a mano.
3. Crea las dos tablas y ejecuta `UPDATE header SET format_version = 3, key_check = ?` (con `WHERE format_version = 2`; si no cambia una fila, `Corrupt`). Nunca toca `encrypted_key`, la sal ni los parámetros: la carrera con un cambio de contraseña no puede revertirlo.
4. Verifica `format_version`, la longitud de `key_check` y que las tablas existan; confirma.

**Fallos.** Si la v1→v2 se confirma y la v2→v3 falla, el archivo queda en una **v2 válida**, la bóveda sigue bloqueada y el error es el de almacenamiento (la especificación hoy dice "el archivo sigue siendo v1" y se corrige). Una v3 degradada a v2: con contraseña, la bóveda queda bloqueada, el archivo igual byte por byte y el error es `Storage(Corrupt)`, sin pánico.

## Importar

`Vault::import` migra y descarta la identidad **en la copia temporal, antes del renombrado**:

1. `validate` (solo lectura) abre la copia, comprueba integridad, desbloquea y descifra los datos de búsqueda, y devuelve también la versión de origen.
2. Si el origen es v1, `migrate_header` v1→v2 (como hoy).
3. `Storage::prepare_import_copy`, **una sola transacción** sobre la copia: si el origen era v1 o v2, la migración v2→v3 con el `key_check` de la versión 3 (calculado con la llave ya validada, sin otro Argon2id); si era v3, nada que migrar. En los dos casos termina con `DELETE FROM identity` y `DELETE FROM paired_pcs`. Si la versión que lee dentro de la transacción no es la esperada, la copia cambió después de validarla: `Storage(Corrupt)`.
4. Solo entonces `settle_dest` y el renombrado.

Cualquier fallo en la cadena (migración o borrado) deja `dest` intacta y el guardián borra la copia. Importar **no genera** identidad. Lo que se garantiza es lógico: la bóveda instalada no tiene filas en `identity` ni en `paired_pcs`. No se afirma que los bytes cifrados de la identidad desaparezcan físicamente del archivo: aunque `secure_delete` está activo, SQLite no ofrece esa garantía sobre páginas liberadas, el diario ni el medio de almacenamiento.

## `export_vault_key` y exportar

Sin cambios. La llave de bóveda es la misma, y `export_vault_key` regenera un `key_check` dañado con `header.format_version()`. El respaldo (`VACUUM INTO`) copia las tablas nuevas, así que lleva la llave privada del celular: ya está en el modelo de amenazas de la especificación ("Emparejamiento y envío").

## Fachada

`vault_core::vault::Error` crece, y el `match` de `mobile/rust/src/api/error.rs` es exhaustivo a propósito. La #93 mapea las variantes nuevas a variantes existentes de `VaultError` (`CorruptIdentity` y `CorruptPairedPc` a `StorageCorrupt`; `PairedPcNotFound` a `EntryNotFound`; `PairedPcExists` a `AlreadyExists`; `InvalidPairedPc` a `StorageOther`) con comentarios `//` (no `///`: el generador copia los `///` al Dart). Es provisional y la #102 define el mapeo definitivo. No cambia ningún tipo expuesto, así que el puente no se regenera; se comprueba con el generador.

## Pruebas

Cada una se escribe antes del código y se comprueba que falla.

- **Cabecera:** `header_aad_version` (1, 2, 2, 2); los bytes del dato autenticado de una v2 no cambian; una cabecera nueva es v3 con `key_check` de la versión 3; `from_parts` acepta 1, 2 y 3.
- **Migración:**
  - una v2 con contraseña y con `unlock_with_key` queda en v3 sin identidad, con `key_check` de la versión 3 y la copia del Keystore valiendo;
  - una v1 con contraseña termina en v3 con una sola derivación;
  - una v1 sigue dando `WrongVaultKey` con `unlock_with_key`;
  - `sqlite_schema` de una bóveda migrada igual al de una creada, desde v1 y desde v2;
  - la carrera con otra conexión que migra (con y sin cambio de contraseña) no revierte nada;
  - fallo de la v2→v3 tras la v1→v2: archivo en v2 válida y bóveda bloqueada;
  - una llave equivocada no escribe.
- **Degradación:** una v3 con `format_version = 2` puesto a mano. Con contraseña: error `Storage(Corrupt)`, bóveda bloqueada, archivo igual byte por byte, sin pánico. Con llave: `WrongVaultKey`.
- **Apertura:** abrir (`Vault::open` y `Storage::open_read_only`) una v1, una v2 y una v3 no modifica el archivo. Reescribir `fixture_v2_unlock_does_not_write` y `v2_unlock_writes_nothing`: una v3 no escribe al desbloquear y una v2 sí (migra).
- **Identidad:** `ensure_identity` crea una vez y devuelve la misma; sobrevive a bloquear y reabrir; un byte alterado en `data`, en `identity_schema_version` o intercambiado con una PC da `CorruptIdentity`; una identidad dañada no impide `unlock` ni leer entradas; `ensure_identity` sobre una dañada no la sobrescribe.
- **PCs:** agregar, listar, actualizar y quitar; llave pública duplicada; nombre vacío o largo; una fila alterada, con id de 15 bytes o movida a otro id aparece en `corrupt` y no impide listar las demás; `remove_paired_pc` la elimina; los bytes de nombre e IP no aparecen en claro en el archivo.
- **Secretos:** la llave privada no aparece en `Debug`, `Display`, errores ni logs; `Identity` se borra al soltarse (como `SecretBox`).
- **Importar:** un respaldo v3 con identidad y PCs, y uno v2, quedan con `identity` y `paired_pcs` **vacías** (consultas directas y `Vault::identity` / `paired_pcs`); uno v1 migra a v3 vacío; un fallo del borrado (un `TRIGGER ... RAISE(ABORT)` en la copia de prueba) deja `dest` intacta y sin copia sobrante; una copia que cambia de versión tras validarla falla con `Storage(Corrupt)`.
- **Fixtures:** `vault-v3.db` abre y trae 4 entradas, una identidad fija y 2 PCs; `vault-v1.db` y `vault-v2.db` siguen abriendo y migrando sin tocarse.

## Documentación que cambia

- `docs/especificacion.md`: "Formato de la bóveda" (nueva subsección de la v3 con `HEADER_VERSION`, esquema, datos autenticados y la consecuencia para el futuro), la cabecera ("versión" del dato autenticado), la migración (y "el archivo sigue siendo v1"), "Respaldos" (el borrado ya está en `Vault::import`), la API y "Emparejamiento y envío" (identidad perezosa ya en v3).
- `ARCHITECTURE.md`: la fila de `vault/` (formato 3, `identity.rs`, `paired_pc.rs`).
- `core/tests/fixtures/README.md`: sección de `vault-v3.db` y de su generador.

## Cambios en otras issues (pendientes de tu visto bueno)

- **#106:** quitar de su alcance el borrado en el núcleo y su criterio "si falla el borrado, la bóveda queda intacta" (ya en la #93); queda el aviso, las pruebas de la interfaz y comprobar desde Dart que tras importar no hay identidad ni PCs.
- **Roadmap (se actualiza en la #93):** la etapa 3 refleja el formato v3 con sus migraciones y que la limpieza de identidad y PCs al restaurar es de la #93; el punto de regenerar la identidad y avisar queda abierto hasta la #106, que solo pone el aviso y las pruebas de interfaz.

## Fuera de alcance

El protocolo (#95, #96), la fachada (#102), el aviso tras importar (#106), la revocación (#107), derivar la llave pública (#94) y `reset_identity` (ver "Identidad ausente frente a dañada").

## Identidad ausente frente a dañada (decidido el 2026-11-19)

Son dos estados distintos y nunca se confunden: una fila inexistente es `Ok(None)` y `ensure_identity` la crea; una fila que existe pero no se autentica, tiene una versión de estructura desconocida o no mide 72 bytes es `CorruptIdentity`, y `ensure_identity` la deja intacta (las PCs ya emparejadas conocen esa llave). La consecuencia es que el emparejamiento queda bloqueado hasta restaurar un respaldo.

`reset_identity` **no** se implementa en la #93. Si la #95 lo necesita, se trata en una issue independiente.

## Criterios de aceptación

- [ ] `vault-v3.db` abre, y `vault-v1.db` y `vault-v2.db` siguen abriendo y migrando.
- [ ] Una v2 desbloqueada con contraseña o con `unlock_with_key` queda en v3 sin identidad, con `key_check` de la versión 3 y la copia del Keystore valiendo; una v1 sigue dando `WrongVaultKey` en `unlock_with_key`.
- [ ] Importar un respaldo v1 o v2 migra la copia temporal a v3 antes del renombrado, y cualquier respaldo deja la bóveda instalada sin identidad ni PCs; si algo falla, no se instala nada.
- [ ] Alterar un byte de la identidad o de una PC se detecta; una identidad dañada no impide abrir la bóveda ni las entradas.
- [ ] La llave privada no aparece en `Debug`, `Display`, errores ni logs, y se borra de memoria al soltarla.
- [ ] La bóveda migrada y la recién creada tienen el mismo `sqlite_schema`.
- [ ] Una v3 degradada a v2 termina en error de almacenamiento limpio con contraseña y en `WrongVaultKey` con llave.
- [ ] `cargo test --workspace`, `clippy`, `fmt` y `deny` pasan.
