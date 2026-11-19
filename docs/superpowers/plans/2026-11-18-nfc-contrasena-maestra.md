# NFC de la contraseña maestra (#57) — plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Que la contraseña maestra llegue a Argon2id y a `password_strength` en forma NFC, con un solo ayudante en `core/`, sin copias del secreto sin borrar y sin cambiar `format_version`.

**Architecture:** `crypto/normalize.rs` ofrece `normalize_nfc(&SecretString) -> NormalizedPassword` (`Borrowed` si ya está en NFC; `Owned(Zeroizing<String>)` de capacidad exacta si no). `kdf::derive_key` y `password::password_strength` lo usan. Nada cambia en `vault/mod.rs`, la fachada ni Dart.

**Tech Stack:** Rust, `unicode-normalization =0.1.26` (nueva; trae `tinyvec`), pruebas unitarias y de integración de `vault-core`.

**Spec:** `docs/superpowers/specs/2026-11-18-nfc-contrasena-maestra-design.md`

## Global Constraints

- Identificadores en inglés; comentarios, documentación y commits en español, Conventional Commits con `(#57)`.
- Solo `unicode-normalization` con versión exacta `=0.1.26`. Ninguna otra dependencia.
- Prueba que falla primero en cada tarea; ejecutar `cargo test -p vault-core` antes de cada commit.
- `NormalizedPassword` no deriva `Debug` ni `Display`.
- No modificar `core/tests/fixtures/*.db`.
- Nunca encadenar `cargo clippy` ni `cargo test` con `| grep` o `| tail` antes de un commit: usar un script que imprima los códigos de salida.
- Referencia ASCII (ya calculada con el código anterior y verificada con `node:crypto` `argon2Sync`): contraseña `clave-ascii-12345`, sal `00 01 .. 0f`, `KdfParams::MIN` → `7f51bec591d86903e9b95fc8bfcaa86ea4d19111408e28f5a05f4f960d9ffe2a`.

## Review Focus

1. Ninguna ruta de `create`, `unlock`, `change_master_password`, `import` y `password_strength` ve la contraseña sin normalizar (Tareas 3 y 4).
2. La llave de una contraseña ASCII no cambia (Tarea 1, vector fijo; debe pasar antes y después).
3. La copia normalizada tiene `capacity() == len()` (Tarea 2).
4. Cada prueba de normalización falla si se desactiva la normalización (Tarea 3, paso de mutación).

## Estructura de archivos

| Archivo | Responsabilidad |
| --- | --- |
| `core/Cargo.toml` | `unicode-normalization = "=0.1.26"` |
| `core/src/crypto/normalize.rs` (nuevo) y `normalize/tests.rs` | El ayudante y sus pruebas |
| `core/src/crypto/mod.rs` | Declara `normalize` y actualiza la nota de límites |
| `core/src/crypto/kdf.rs` y `kdf/tests.rs` | `derive_key` normaliza; vector ASCII y pruebas de equivalencia |
| `core/src/vault/password.rs` y `password/tests.rs` | `password_strength` cuenta sobre la forma NFC |
| `core/tests/vault.rs` | Compuesta/descompuesta de punta a punta |
| `core/CLAUDE.md`, `ARCHITECTURE.md`, `docs/especificacion.md` | Crate permitida, decisión y excepción de borrado |

### Task 1: Dependencia y vector ASCII

**Files:** `core/Cargo.toml`, `core/src/crypto/kdf/tests.rs`

- [ ] Reemplazar `temp_print_reference_key` por `ascii_password_derives_the_reference_key`: deriva con `password("clave-ascii-12345")`, sal `core::array::from_fn(|i| i as u8)` y `KdfParams::MIN`, y compara el hex con la referencia de Global Constraints. Debe pasar con el código actual.
- [ ] Agregar a `core/Cargo.toml`, en orden alfabético, con un comentario de una línea: `unicode-normalization = "=0.1.26"`.
- [ ] `cargo build -p vault-core` y `cargo deny check` (debe pasar con `tinyvec`; anotar las dos licencias para el PR); `cargo audit`.
- [ ] Commit: `test: vector fijo de la llave derivada de una contraseña ASCII (#57)` y `chore: agregar unicode-normalization =0.1.26 (#57)` (dos commits).

### Task 2: El ayudante `normalize_nfc`

**Files:** `core/src/crypto/normalize.rs`, `core/src/crypto/normalize/tests.rs`, `core/src/crypto/mod.rs`

- [ ] Pruebas primero (fallan por no existir el módulo):
  - ASCII, `"é"` compuesta y `"prueba-bóveda-v1"` dan `Borrowed` (sin copia).
  - `"e\u{0301}"` da `Owned` con el contenido `"\u{00e9}"`, y al comparar bytes es igual a la forma compuesta.
  - Reordenamiento canónico: `"e\u{0302}\u{0323}"` y `"e\u{0323}\u{0302}"` dan ambos `"\u{1ec7}"`.
  - Hangul: `"\u{1100}\u{1161}"` da `"\u{ac00}"`.
  - En todo caso `Owned`, `capacity() == len()`.
  - Cadena vacía: `Borrowed`.
- [ ] Implementar según el diseño: `is_nfc` → `Borrowed`; si no, una pasada con `nfc().map(char::len_utf8).sum()`, `Zeroizing::new(String::with_capacity(total))` y `extend(texto.nfc())`. `impl Deref<Target = str>`. Sin `Debug`.
- [ ] Declarar `pub(crate) mod normalize;` en `crypto/mod.rs` y ajustar la nota "Límite conocido" para mencionar el búfer del iterador.
- [ ] `cargo test -p vault-core --lib normalize`; commit: `feat: normalizar a NFC la contraseña maestra con una copia que se borra (#57)`.

### Task 3: `derive_key` normaliza

**Files:** `core/src/crypto/kdf.rs`, `core/src/crypto/kdf/tests.rs`, `core/tests/vault.rs`

- [ ] Pruebas primero:
  - Unitaria: `derive_key` con `"caf\u{00e9}-clave-larga"` y con `"cafe\u{0301}-clave-larga"` (mismos sal y parámetros) da la misma llave, y es distinta de la de una contraseña realmente distinta.
  - Integración (`core/tests/vault.rs`, con calibración real): `create` con la forma compuesta, `lock`, `unlock` con la descompuesta; y el caso inverso; `change_master_password` con la actual descompuesta y la nueva en la otra forma; `export` + `import` con la contraseña en la otra forma.
- [ ] Ver fallar las nuevas; en `derive_key`, `let normalized = normalize_nfc(password);` y `normalized.as_bytes()` en `hash_password_into`. Mantener `calibrate` como está (usa `"calibrate"`, ASCII).
- [ ] **Mutación:** hacer que `normalize_nfc` devuelva siempre `Borrowed`, comprobar que fallan las pruebas nuevas y la de capacidad, y restaurar desde una copia (nada sin confirmar en el índice).
- [ ] `cargo test -p vault-core` completo (incluye `fixture_v1_*`, `fixture_v2_*` y el vector ASCII de la Tarea 1); commit: `fix: derivar la llave desde la contraseña maestra en NFC (#57)`.

### Task 4: `password_strength` sobre la forma NFC

**Files:** `core/src/vault/password.rs`, `core/src/vault/password/tests.rs`

- [ ] Pruebas primero: 11 `é` compuestas dan `TooShort`; 11 letras `e` + U+0301 también (hoy cuentan 22 y dan `Acceptable`); 12 de cada forma dan `Acceptable`; 16 de cada forma dan `Strong`; `require_master_password` rechaza la forma descompuesta de 11.
- [ ] En `password_strength`, contar `normalize_nfc(password).chars().count()`. Actualizar el comentario del módulo ("cuenta caracteres Unicode de la forma NFC").
- [ ] Mutación como en la Tarea 3; commit: `fix: contar la fortaleza de la contraseña maestra en NFC (#57)`.

### Task 5: Documentación

**Files:** `core/CLAUDE.md`, `ARCHITECTURE.md`, `docs/especificacion.md`, `docs/superpowers/specs/2026-11-18-nfc-contrasena-maestra-design.md` (si algo cambió)

- [ ] `core/CLAUDE.md`: agregar `unicode-normalization` a la lista de crates permitidos y una línea en "Criptografía" (la contraseña maestra se normaliza a NFC en un solo punto).
- [ ] `docs/especificacion.md`: en "Formato de la bóveda", la decisión (NFC, sin cambio de `format_version`, sin segundo intento, bóvedas de ejemplo ya en NFC) y la excepción de borrado con su alcance exacto, junto a la de `nucleo-matcher`; quitar la mención a NFC de la contraseña maestra si la hay entre las limitaciones.
- [ ] `ARCHITECTURE.md`: la excepción en la fila "Borrado de memoria garantizado solo dentro de `vault-core`" y `crypto/normalize.rs` en el mapa y en la tabla de `core/`.
- [ ] Commit: `docs: normalización NFC de la contraseña maestra y su excepción de borrado (#57)`.

### Task 6: Verificación y cierre

- [ ] Desde la raíz, con un script que imprima el código de salida de cada comando: `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check`, `cargo audit`, `cargo deny check`.
- [ ] Verificar que `git diff dev -- core/tests/fixtures` está vacío.
- [ ] Revisión final independiente (opus) y menú de cierre (corregir menores baratos antes del PR).
- [ ] PR contra `dev` con el formato del PR #11, anotando las dos crates nuevas y su licencia (`unicode-normalization` 0.1.26 `(MIT OR Apache-2.0) AND Unicode-3.0`; `tinyvec`, licencia por confirmar con `cargo deny list`), la excepción de borrado y `Closes #57`.
