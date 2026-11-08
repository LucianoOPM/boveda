# Diseño: bóvedas de ejemplo y pruebas de integración (issue #10)

Fecha: 2026-11-07 · Etapa 1 · Rama `feat/10-pruebas-integracion`

## Objetivo

Fijar el formato v1 con una bóveda de ejemplo versionada en el repositorio, que debe abrirse en cualquier versión futura del núcleo, y completar las pruebas de integración de `vault-core` con la API pública. Cerrar esta issue cumple el criterio de cierre de la etapa 1.

### Fuera de alcance

- Pruebas del protocolo (etapa 3).
- Cambios en la API pública o en el código de producción: esta issue solo agrega pruebas, una bóveda de ejemplo y documentación.

### Criterios de aceptación (de la issue)

- `vault-v1.db` está en el repositorio y abre con la contraseña de prueba, con todas sus entradas y secretos.
- Flujo completo por la API pública: crear, agregar, bloquear, desbloquear, buscar, revelar, cambiar la contraseña maestra, exportar e importar.
- Están cubiertas todas las pruebas de la estrategia del núcleo: ida y vuelta, contraseña incorrecta, byte alterado, secreto intercambiado, bóvedas de ejemplo y restauración de respaldo.
- La CI está en verde en `dev`.

Del comentario de la #5: `vault-v1.db` incluye al menos una entrada v1 con todos los campos opcionales, y la prueba compara sus datos de búsqueda y su secreto con valores conocidos.

## Punto de partida

- `core/tests/vault.rs` ya existe (de la #9), con cuatro pruebas que usan la calibración real: `create_lock_unlock_with_calibration`, `change_password_with_calibration`, `search_with_public_api` y `export_delete_import_with_calibration`. Se conservan sin cambios. Las pruebas nuevas se agregan al mismo archivo.
- `core/tests/fixtures/` no existe. `.gitignore` ya excluye `core/tests/fixtures/**` de la regla `*.db`, y `.gitattributes` lo marca como `binary`.
- Las pruebas unitarias ya cubren la ida y vuelta (con `proptest`), la contraseña incorrecta, el byte alterado y el secreto movido a otra entrada a nivel de partes cifradas. Falta recorrer esos casos a través del archivo SQLite y de `Vault`, y falta la bóveda de ejemplo.

## Decisiones

### El generador es una prueba unitaria `#[ignore]` con `KdfParams::MIN`

- Vive en `core/src/vault/tests.rs`, junto a las pruebas de `Vault`, porque la regla de `core/CLAUDE.md` pone las pruebas unitarias en un archivo aparte. `create_with_params` y `KdfParams::MIN` siguen siendo `pub(crate)`: la API pública no crece.
- Usa `KdfParams::MIN` (64 MiB, 3 iteraciones, paralelismo 1). Coincide con la especificación ("las pruebas usan exactamente esos mínimos") y pasa la validación de la cabecera. La bóveda de ejemplo abre igual de rápido en cualquier máquina. Con `Vault::create` (pública) quedaría calibrada en la PC que la generó, hasta 128 MiB y muchas iteraciones, y cada apertura en la CI dependería de esa máquina.
- Se descartó exponer `create_with_params` detrás de una feature `test-fixtures`: agrega superficie pública solo para pruebas.
- Las pruebas que **leen** la bóveda de ejemplo sí usan solo la API pública, en `core/tests/vault.rs`.

### Nunca sobrescribir

- El generador no borra ni reemplaza una bóveda de ejemplo existente. No hay ningún `remove_file` previo: `create_with_params` devuelve `AlreadyExists` y la prueba falla con `unwrap()`.
- El número del nombre (`vault-v1.db`) es la `format_version` de la cabecera. Cada versión de formato nueva lleva un nombre nuevo (`vault-v2.db`, ...) y su propio generador. Los archivos existentes no se modifican ni se borran.

### Ruta absoluta

`Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/vault-v1.db")`. `CARGO_MANIFEST_DIR` es el directorio del `Cargo.toml` de `vault-core` (Cargo Book, "Environment variables Cargo sets for crates"). La ruta queda absoluta, como exigen las reglas de rutas por `SQLITE_USE_URI`.

### Las pruebas trabajan sobre una copia

Abrir la bóveda fija pragmas, y `reveal()` escribe `last_used_at`. Las pruebas nunca abren `core/tests/fixtures/vault-v1.db` directamente: lo copian a un directorio temporal de `tempfile` (ruta absoluta) y trabajan sobre la copia. Así tampoco chocan entre sí al correr en paralelo.

### Alterar el archivo desde las pruebas de integración

Las pruebas de `tests/` se enlazan con `[dependencies]` y `[dev-dependencies]` (Cargo Book, "Integration tests"), así que pueden usar `rusqlite` para alterar bytes o intercambiar columnas en la copia, como haría un atacante con acceso al archivo. Es la misma técnica que `test_support::tamper` y que la prueba actual `create_lock_unlock_with_calibration`. No se agregan dependencias.

### Contraseña de prueba

`const FIXTURE_V1_PASSWORD: &str = "prueba-bóveda-v1";`

- Lleva una "ó" para que la bóveda de ejemplo también fije cómo entran a Argon2id los bytes UTF-8 de la contraseña.
- La constante aparece en el generador y en `core/tests/vault.rs`: las pruebas de integración no ven el código `cfg(test)` del crate. En `core/tests/vault.rs` está documentada junto a la prueba que lee la bóveda, y también en el README de `fixtures/`.

## Contenido de `vault-v1.db`

| Servicio | URL | Usuario | Etiquetas | Contraseña | Notas | Estado |
|---|---|---|---|---|---|---|
| `Correo Electrónico` | `https://correo.example.com/entrar` | `ana@example.com` | `Trabajo`, `Médico`, `Año` | `clave-completa-ñ` | `Línea uno\nLínea dos` | viva, nunca usada |
| `Banco` | — | — | — | `clave-minima` | — | viva, nunca usada |
| `Nube` | — | — | — | `clave-nube` | — | viva, con `last_used_at` |
| `Eliminada` | — | — | — | `clave-eliminada` | — | lápida |

- "Correo Electrónico" cubre todos los campos opcionales, con acentos en las etiquetas y un salto de línea en las notas.
- "Banco" es la entrada mínima: solo servicio y contraseña.
- "Nube" tiene `last_used_at`: el generador hace `reveal` sobre ella, que lo escribe en disco.
- "Eliminada" queda como lápida: el generador hace `delete` sobre ella (`deleted = 1`, partes en `NULL`).
- Los identificadores son aleatorios, y las fechas, la hora real. Las pruebas no dependen de ellos: buscan cada entrada por servicio.

## Componentes

| Archivo | Cambio |
| --- | --- |
| `core/src/vault/tests.rs` | Generador `generate_fixture_v1`, con `#[ignore]` |
| `core/tests/fixtures/vault-v1.db` | Bóveda de ejemplo generada una sola vez |
| `core/tests/fixtures/README.md` | Contraseña, contenido, regla de nombres y cómo se generó |
| `core/tests/vault.rs` | Pruebas nuevas sobre la bóveda de ejemplo y el flujo completo |
| `.gitattributes` | `core/tests/fixtures/*.md text eol=lf` después de la regla `binary` |
| `docs/especificacion.md`, `docs/roadmap.md`, `ARCHITECTURE.md` | Ver "Documentación" |

### Generador

```rust
/// Genera `core/tests/fixtures/vault-v1.db`, la bóveda de ejemplo del
/// formato 1. (...)
///
/// Se ejecuta una sola vez, al introducir una versión de formato nueva:
/// `cargo test -p vault-core -- --ignored generate_fixture_v1`
#[test]
#[ignore = "solo al introducir una versión de formato nueva"]
fn generate_fixture_v1() { ... }
```

El comentario completo explica:
- que el número del nombre es la `format_version`;
- que cada versión nueva lleva un nombre y un generador nuevos;
- que los archivos existentes no se modifican ni se borran;
- que, si el archivo ya existe, el generador falla con `AlreadyExists` a propósito.

Pasos:
1. `create_with_params(&path, &password(FIXTURE_V1_PASSWORD), KdfParams::MIN)`.
2. Agregar las cuatro entradas.
3. `reveal` de "Nube".
4. `delete` de "Eliminada".
5. Soltar la bóveda.

### README de `fixtures/`

- Para qué sirven las bóvedas de ejemplo.
- La regla: el número es la `format_version`; nunca se modifican ni se borran; un formato nuevo lleva un archivo nuevo.
- Por cada archivo: contraseña, parámetros de Argon2id (`KdfParams::MIN`), la tabla de contenido y el generador que la creó.
- Las pruebas trabajan sobre una copia, porque abrirla la modificaría.

### `.gitattributes`

La regla `core/tests/fixtures/** binary` también afectaría al README. Una línea posterior `core/tests/fixtures/*.md text eol=lf` lo devuelve a texto: en `.gitattributes`, la línea posterior manda.

## Pruebas nuevas en `core/tests/vault.rs`

Ayudantes en el mismo archivo:
- `fixture_copy() -> (TempDir, PathBuf)`: copia `vault-v1.db` al directorio temporal.
- `unlocked_fixture() -> (TempDir, PathBuf, Vault)`: copia, abre y desbloquea la bóveda, y comprueba que el `UnlockReport` esté vacío.
- `id_of(&Vault, service) -> EntryId`.
- `flip_byte(path, id, column)` y `swap_secrets(path, a, b)` con `rusqlite`.

| Prueba | Qué comprueba |
| --- | --- |
| `fixture_v1_opens_with_known_contents` | `UnlockReport` vacío; exactamente 3 entradas. `search("")` da `[Nube, Banco, Correo Electrónico]`, y solo "Nube" tiene `last_used_at`. Datos de búsqueda exactos de "Correo Electrónico" (URL, usuario y etiquetas en orden). `reveal` de las tres coincide con contraseña y notas exactas; "Banco" y "Nube" sin notas. `search("Eliminada")` vacío y ningún servicio "Eliminada" en `entries()`. |
| `fixture_v1_wrong_password` | `WrongPassword`, la bóveda sigue bloqueada y después abre con la contraseña correcta. |
| `tampered_byte_is_detected` | Un byte alterado en el secreto de "Banco": `unlock` limpio, `reveal(Banco)` da `CorruptEntry` y las otras dos se revelan bien. Un byte alterado en los datos de búsqueda de "Nube": `UnlockReport.corrupt == [id de Nube]` y abren las otras dos. |
| `swapped_secret_between_entries_is_rejected` | Se intercambia la columna `secret` de "Banco" y "Nube": `reveal` da `CorruptEntry` en las dos y "Correo Electrónico" se revela bien. |
| `backup_restore_recovers_everything` | Sobre la copia de la bóveda de ejemplo: `export`, borrar la bóveda e `import` con la contraseña de prueba. Recupera las tres entradas con sus secretos exactos, sin la lápida. |
| `full_flow_through_public_api` | En una sola secuencia: `create`, agregar dos entradas, `lock`, `entries()` da `Locked`, `unlock`, buscar con un error de escritura y `reveal`. Después `change_master_password`, `lock`: la vieja da `WrongPassword` y la nueva abre. Por último `export`, borrar la bóveda e `import` con la contraseña nueva, y verificar los secretos. |

Las pruebas actuales de la #9 cubren por separado partes de ese flujo. `full_flow_through_public_api` las encadena sobre la misma bóveda, como pide el criterio de aceptación.

**Costo en la CI:**
- Cada prueba basada en la bóveda de ejemplo hace un Argon2id de 64 MiB con 3 iteraciones, en décimas de segundo con `opt-level = 3`. `backup_restore_recovers_everything` hace dos (el desbloqueo inicial y la validación del `import`).
- `full_flow_through_public_api` hace dos calibraciones, como las pruebas actuales de la #9.

**Sin TDD clásico:** no hay código de producción nuevo, así que no hay una prueba que falle primero. Para comprobar que cada prueba detecta lo que dice, se rompe a propósito una vez antes del commit (comparar contra un valor equivocado o no alterar el byte) y se confirma que falla.

## Cobertura de la estrategia de pruebas

| Punto de la estrategia | Unitarias existentes | Integración (esta issue) |
| --- | --- | --- |
| Ida y vuelta del cifrado (con `proptest`) | `aead/tests.rs` (`round_trip_prop`), `entry/tests.rs` (`*_round_trip_prop`), `sealed/tests.rs` (`sealed_round_trip_prop`) | `fixture_v1_opens_with_known_contents`, `full_flow_through_public_api` |
| Contraseña maestra incorrecta | `header/tests.rs` (`wrong_password_returns_wrong_password_error`), `vault/tests.rs` (`wrong_password_keeps_vault_locked`) | `fixture_v1_wrong_password`, `full_flow_through_public_api` |
| Un byte alterado se detecta | `aead/tests.rs` (`any_bit_flip_is_detected_prop`), `vault/tests.rs` (`reveal_corrupt_secret`, `unlock_reports_corrupt_search`) | `tampered_byte_is_detected` |
| Secreto intercambiado entre entradas | `sealed/tests.rs` (`secret_moved_to_other_entry_is_rejected`, `swapped_parts_are_rejected`) | `swapped_secret_between_entries_is_rejected` |
| Bóvedas de ejemplo fijas | — | `fixture_v1_opens_with_known_contents` |
| Exportar, borrar e importar | `backup/tests.rs` (`import_round_trip`) | `backup_restore_recovers_everything`, `export_delete_import_with_calibration`, `full_flow_through_public_api` |

## Errores

No hay errores nuevos. Las pruebas comparan contra variantes existentes: `WrongPassword`, `Locked`, `CorruptEntry` y `AlreadyExists`, esta última en el generador.

## Dependencias nuevas

Ninguna. `rusqlite` ya está en `[dependencies]`, y `tempfile` y `secrecy` ya los usan las pruebas de integración.

## Documentación

- `docs/especificacion.md`, "Estrategia de pruebas", línea de las bóvedas de ejemplo: el nombre `vault-vN.db` con N igual a la `format_version`, que nunca se modifican, que se generan con `KdfParams::MIN` y que las pruebas de integración usan solo la API pública. Con enlace a este diseño.
- `docs/roadmap.md`: marcar la casilla "Pruebas" de la etapa 1. La etapa se da por cerrada cuando la CI esté en verde en `dev` después del merge.
- `ARCHITECTURE.md`: en el mapa, `fixtures/` como "bóvedas de ejemplo fijas (`vault-vN.db`, ver su README)".
- `core/CLAUDE.md`: sin cambios; su regla ya cubre que las bóvedas de ejemplo no se modifican.

## Verificación antes del PR

- `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` y `cargo fmt --all -- --check`.
- `cargo test` sin `--ignored` no ejecuta el generador (aparece como `ignored`).
- Ejecutar el generador por segunda vez falla con `AlreadyExists` y no cambia el archivo (mismo hash antes y después).
- `git check-ignore core/tests/fixtures/vault-v1.db` no lo ignora, y `git diff --stat` lo muestra como `Bin`.

## Commits previstos

1. `docs:` diseño de las bóvedas de ejemplo y las pruebas de integración
2. `docs:` plan de implementación
3. `chore:` el README de las bóvedas de ejemplo como texto en `.gitattributes`
4. `test:` generador de la bóveda de ejemplo del formato 1
5. `test:` bóveda de ejemplo `vault-v1.db` y su README
6. `test:` bóveda de ejemplo, contraseña incorrecta, byte alterado y secreto intercambiado
7. `test:` respaldo restaurado y flujo completo por la API pública
8. `docs:` especificación, arquitectura y roadmap
