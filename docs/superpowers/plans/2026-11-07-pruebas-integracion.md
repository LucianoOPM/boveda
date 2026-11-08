# Bóvedas de ejemplo y pruebas de integración: plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Versionar la bóveda de ejemplo `core/tests/fixtures/vault-v1.db` y completar las pruebas de integración de `vault-core` para cerrar la etapa 1 (issue #10).

**Architecture:** Un generador `#[ignore]` en las pruebas unitarias de `vault` crea la bóveda de ejemplo una sola vez con `KdfParams::MIN`. Las pruebas de `core/tests/vault.rs` copian esa bóveda a un directorio temporal y la leen solo con la API pública; usan `rusqlite` sobre la copia para simular a un atacante.

**Tech Stack:** Rust 2024, `rusqlite` 0.41 (SQLite empaquetado), `tempfile`, `secrecy`.

**Spec:** `docs/superpowers/specs/2026-11-07-pruebas-integracion-design.md`

## Global Constraints

- Sin código de producción nuevo ni cambios en la API pública; `create_with_params` y `KdfParams::MIN` siguen `pub(crate)`.
- Sin dependencias nuevas.
- Contraseña de prueba: `"prueba-bóveda-v1"`, en la constante `FIXTURE_V1_PASSWORD`.
- Parámetros de la bóveda de ejemplo: `KdfParams::MIN` (64 MiB, 3 iteraciones, paralelismo 1).
- El generador nunca borra ni reemplaza `vault-v1.db`: se apoya en `AlreadyExists`; sin `remove_file`.
- El número de `vault-vN.db` es la `format_version` de la cabecera; cada formato nuevo, archivo nuevo; los existentes no se modifican ni se borran.
- Rutas absolutas: `Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/vault-v1.db")`.
- Ninguna prueba abre el archivo del repositorio: siempre una copia en `tempfile::tempdir()`.
- Identificadores en inglés; comentarios, documentación y commits en español (Conventional Commits).
- Nunca escribir secretos en mensajes de `assert!`/`panic!`; comparar con `assert_eq!` sobre `expose_secret()` es aceptable en pruebas (es la contraseña de prueba conocida), como ya hacen las pruebas actuales.
- Las cuatro pruebas existentes de `core/tests/vault.rs` se conservan sin cambios.

## Review Focus

- La prueba abre `vault-v1.db` del repositorio sin copiarlo: `reveal` escribiría `last_used_at` y el archivo versionado cambiaría. Todas las pruebas usan `fixture_copy()` (Task 3).
- En Windows, borrar la bóveda o importar sobre ella con una `Vault` todavía abierta falla: soltar (`drop`) la bóveda antes de `remove_file` e `import` (Task 4).
- Ejecutar el generador por segunda vez debe fallar con `AlreadyExists` y dejar el archivo idéntico (Task 2, paso de verificación con hash).
- Git podría convertir finales de línea o ignorar el `.db`: `git check-ignore` y `git diff --stat` (`Bin`) lo comprueban (Task 2).
- Una prueba que pasa aunque no detecte nada: cada prueba nueva se rompe a propósito una vez y se confirma que falla (Tasks 3 y 4).

---

### Task 1: Generador de la bóveda de ejemplo

**Files:**
- Modify: `core/src/vault/tests.rs` (al final, en una sección `// --- Bóveda de ejemplo del formato 1 ---`)

**Interfaces:**
- Consumes: `Vault::create_with_params(&Path, &SecretString, KdfParams)`, `KdfParams::MIN`, `test_support::password`, `SearchData::new`, `Secret::new`, `Vault::{add, reveal, delete}`.
- Produces: `const FIXTURE_V1_PASSWORD: &str = "prueba-bóveda-v1";` y `fn generate_fixture_v1()` (`#[test] #[ignore = "solo al introducir una versión de formato nueva"]`).

- [ ] **Step 1: Escribir el generador**

Comentario de documentación del generador (texto fijo, en español):

```rust
/// Genera `core/tests/fixtures/vault-v1.db`, la bóveda de ejemplo del
/// formato 1, que las pruebas de `core/tests/vault.rs` deben abrir siempre.
///
/// El número del nombre es la `format_version` de la cabecera. Cada versión
/// de formato nueva lleva un archivo nuevo (`vault-v2.db`, ...) y su propio
/// generador; los archivos existentes no se modifican ni se borran. Si el
/// archivo ya existe, falla a propósito con `AlreadyExists`.
///
/// Se ejecuta una sola vez, al introducir una versión de formato nueva:
/// `cargo test -p vault-core -- --ignored generate_fixture_v1`
///
/// Contenido y contraseña: `core/tests/fixtures/README.md`.
```

Cuerpo, en este orden:
1. `path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/vault-v1.db")`; `fs::create_dir_all(path.parent().unwrap())`.
2. `Vault::create_with_params(&path, &password(FIXTURE_V1_PASSWORD), KdfParams::MIN).unwrap()`.
3. `add` de las cuatro entradas con los valores exactos:
   - `SearchData::new("Correo Electrónico", Some("https://correo.example.com/entrar"), Some("ana@example.com"), &["Trabajo", "Médico", "Año"])`, `Secret::new(password("clave-completa-ñ"), Some(password("Línea uno\nLínea dos")))`
   - `SearchData::new("Banco", None, None, &[])`, `Secret::new(password("clave-minima"), None)`
   - `SearchData::new("Nube", None, None, &[])`, `Secret::new(password("clave-nube"), None)`
   - `SearchData::new("Eliminada", None, None, &[])`, `Secret::new(password("clave-eliminada"), None)`
4. `vault.reveal(id_nube).unwrap()`; `vault.delete(id_eliminada).unwrap()`; `drop(vault)`.

Agregar `use std::path::Path;` a las importaciones si falta.

- [ ] **Step 2: Comprobar que no se ejecuta por defecto**

Run: `cargo test -p vault-core --lib generate_fixture_v1`
Expected: `test vault::tests::generate_fixture_v1 ... ignored`, 0 passed, 0 failed. `core/tests/fixtures/` no existe todavía.

- [ ] **Step 3: Lint y formato**

Run: `cargo fmt --all && cargo clippy -p vault-core --all-targets -- -D warnings`
Expected: sin advertencias.

- [ ] **Step 4: Commit**

```bash
git add core/src/vault/tests.rs
git commit -m "test: generador de la bóveda de ejemplo del formato 1"
```

---

### Task 2: Generar `vault-v1.db`, su README y la regla de `.gitattributes`

**Files:**
- Modify: `.gitattributes`
- Create: `core/tests/fixtures/vault-v1.db` (generado)
- Create: `core/tests/fixtures/README.md`

**Interfaces:**
- Consumes: `generate_fixture_v1` (Task 1).
- Produces: el archivo `core/tests/fixtures/vault-v1.db` con el contenido de la tabla del diseño.

- [ ] **Step 1: Regla de `.gitattributes` y commit**

Agregar justo después de `core/tests/fixtures/** binary`:

```
# Excepto su documentación, que es texto.
core/tests/fixtures/*.md text eol=lf
```

```bash
git add .gitattributes
git commit -m "chore: el README de las bóvedas de ejemplo como texto en .gitattributes"
```

- [ ] **Step 2: Ejecutar el generador**

Run: `cargo test -p vault-core --lib -- --ignored generate_fixture_v1`
Expected: `1 passed`; existe `core/tests/fixtures/vault-v1.db` y no quedan `vault-v1.db-journal` ni otros archivos.

- [ ] **Step 3: Verificar que no sobrescribe**

Run: `sha256sum core/tests/fixtures/vault-v1.db && cargo test -p vault-core --lib -- --ignored generate_fixture_v1; sha256sum core/tests/fixtures/vault-v1.db`
Expected: la prueba falla con `AlreadyExists` en el `unwrap()` y los dos hashes son iguales.

- [ ] **Step 4: Escribir `core/tests/fixtures/README.md`**

Secciones (en español):
- Para qué sirven: garantizar que una bóveda creada hoy siga abriendo en cualquier versión futura del núcleo.
- Regla: el número de `vault-vN.db` es la `format_version` de la cabecera; nunca se modifican ni se borran; un formato nuevo lleva archivo y generador nuevos.
- Las pruebas trabajan sobre una copia, porque abrir la bóveda y `reveal` la modifican.
- `vault-v1.db`: contraseña `prueba-bóveda-v1`; Argon2id con `KdfParams::MIN` (64 MiB, 3 iteraciones, paralelismo 1); generador `generate_fixture_v1` en `core/src/vault/tests.rs` y comando `cargo test -p vault-core -- --ignored generate_fixture_v1`; la tabla de contenido del diseño (cuatro filas, notas escritas como `Línea uno` + salto de línea + `Línea dos`).

- [ ] **Step 5: Verificar Git**

Run: `git check-ignore -v core/tests/fixtures/vault-v1.db; git add core/tests/fixtures && git diff --cached --stat && git check-attr text binary -- core/tests/fixtures/README.md core/tests/fixtures/vault-v1.db`
Expected: `check-ignore` muestra la regla de negación `!core/tests/fixtures/**` (no se ignora); `--stat` muestra `vault-v1.db | Bin 0 -> N bytes`; el README tiene `text: set` y el `.db`, `binary: set`.

- [ ] **Step 6: Commit**

```bash
git commit -m "test: bóveda de ejemplo vault-v1.db y su README"
```

---

### Task 3: Pruebas sobre la bóveda de ejemplo

**Files:**
- Modify: `core/tests/vault.rs` (agregar al final; no tocar las cuatro pruebas existentes)

**Interfaces:**
- Consumes: `core/tests/fixtures/vault-v1.db` (Task 2); API pública `vault_core::vault::{Vault, Error, EntryId, UnlockReport}`.
- Produces (ayudantes en `core/tests/vault.rs`, usados en Task 4):
  - `const FIXTURE_V1_PASSWORD: &str = "prueba-bóveda-v1";` con comentario que remite a `fixtures/README.md`
  - `fn fixture_copy() -> (tempfile::TempDir, PathBuf)`: `fs::copy` de `Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/vault-v1.db")` a `dir.path().join("vault-v1.db")`
  - `fn unlocked_fixture() -> (tempfile::TempDir, PathBuf, Vault)`: copia, `Vault::open`, `unlock` con la contraseña de prueba y `assert_eq!(report, UnlockReport { corrupt: vec![], unidentified_corrupt: 0 })`
  - `fn id_of(vault: &Vault, service: &str) -> EntryId`
  - `fn flip_byte(path: &Path, id: EntryId, column: &str)`: invierte un bit en el índice 30 del BLOB (`search` o `secret`), como `test_support::tamper`
  - `fn swap_secrets(path: &Path, a: EntryId, b: EntryId)`: lee los dos BLOB `secret` y los escribe cruzados

- [ ] **Step 1: Escribir las cuatro pruebas**

```rust
#[test]
fn fixture_v1_opens_with_known_contents() {
    let (_dir, _path, mut vault) = unlocked_fixture();

    let order: Vec<&str> = vault.search("").unwrap().iter().map(|e| e.search().service()).collect();
    assert_eq!(order, ["Nube", "Banco", "Correo Electrónico"]);
    // solo "Nube" tiene last_used_at; las otras dos, None

    // "Correo Electrónico": url Some("https://correo.example.com/entrar"),
    // username Some("ana@example.com"), tags == ["Trabajo", "Médico", "Año"]
    // "Banco" y "Nube": url, username None; tags vacías

    // reveal: Correo Electrónico → "clave-completa-ñ", notas Some("Línea uno\nLínea dos")
    //         Banco → "clave-minima", notas None; Nube → "clave-nube", notas None

    assert!(vault.search("Eliminada").unwrap().is_empty());
    assert!(vault.entries().unwrap().iter().all(|e| e.search().service() != "Eliminada"));
}

#[test]
fn fixture_v1_wrong_password() {
    // open → unlock("prueba-boveda-v1") (sin acento) == Err(WrongPassword);
    // is_locked(); después unlock(FIXTURE_V1_PASSWORD) es Ok
}

#[test]
fn tampered_byte_is_detected() {
    // Secreto: copia, id de "Banco" desde unlocked_fixture, drop de la bóveda,
    // flip_byte(path, banco, "secret"), reabrir y desbloquear:
    //   report vacío; reveal(banco) == Err(CorruptEntry);
    //   reveal de "Nube" y "Correo Electrónico" con sus claves exactas.
    // Datos de búsqueda: otra copia, flip_byte(path, nube, "search"):
    //   report.corrupt == vec![nube]; entries() tiene exactamente
    //   "Banco" y "Correo Electrónico".
}

#[test]
fn swapped_secret_between_entries_is_rejected() {
    // swap_secrets(path, banco, nube) sobre la copia cerrada; reabrir:
    //   report vacío (los datos de búsqueda no cambiaron);
    //   reveal(banco) y reveal(nube) == Err(CorruptEntry);
    //   reveal de "Correo Electrónico" == "clave-completa-ñ".
}
```

Para obtener los ids antes de alterar el archivo: desbloquear la copia, tomar `id_of`, y `drop(vault)` antes de escribir con `rusqlite`.

- [ ] **Step 2: Ejecutar**

Run: `cargo test -p vault-core --test vault`
Expected: 8 passed (4 existentes + 4 nuevas).

- [ ] **Step 3: Romper cada prueba a propósito**

Uno por uno, sin commitear: cambiar `"Nube"` por `"Banco"` en el orden esperado; quitar la llamada a `flip_byte` del secreto; quitar la llamada a `swap_secrets`; usar la contraseña correcta en `fixture_v1_wrong_password`. Cada vez, `cargo test -p vault-core --test vault <nombre>` debe fallar. Revertir con `git diff` vacío para esas líneas.

- [ ] **Step 4: Lint, formato y commit**

Run: `cargo fmt --all && cargo clippy -p vault-core --all-targets -- -D warnings`
Expected: sin advertencias; `git status` muestra solo `core/tests/vault.rs` (el `.db` del repositorio sin cambios).

```bash
git add core/tests/vault.rs
git commit -m "test: bóveda de ejemplo, contraseña incorrecta, byte alterado y secreto intercambiado"
```

---

### Task 4: Respaldo restaurado y flujo completo

**Files:**
- Modify: `core/tests/vault.rs`

**Interfaces:**
- Consumes: `fixture_copy`, `unlocked_fixture`, `id_of`, `FIXTURE_V1_PASSWORD` (Task 3); `password`, `temp_path` (existentes en el archivo).

- [ ] **Step 1: Escribir las dos pruebas**

```rust
#[test]
fn backup_restore_recovers_everything() {
    // unlocked_fixture(); export a dir/respaldo.db; drop(vault);
    // fs::remove_file(path); Vault::import(&backup, &path, FIXTURE_V1_PASSWORD)
    // report vacío; servicios ordenados == ["Banco", "Correo Electrónico", "Nube"];
    // reveal de las tres con contraseñas y notas exactas;
    // ninguna "Eliminada".
}

#[test]
fn full_flow_through_public_api() {
    // create(path, "maestra-vieja"); add "gmail" (url "https://mail.google.com",
    //   usuario "yo@example.com", clave "clave-gmail") y "banco" (clave "clave-banco");
    // lock(); entries() == Err(Locked); unlock("maestra-vieja");
    // search("gmial") → primer resultado "gmail" (error de escritura);
    // reveal(gmail) == "clave-gmail";
    // change_master_password("maestra-vieja", "maestra-nueva"); lock();
    // unlock("maestra-vieja") == Err(WrongPassword); unlock("maestra-nueva") Ok;
    // export(dir/respaldo.db); drop(vault); remove_file(path);
    // import(respaldo, path, "maestra-nueva"): reveal de las dos con sus claves.
}
```

- [ ] **Step 2: Ejecutar**

Run: `cargo test -p vault-core --test vault`
Expected: 10 passed.

- [ ] **Step 3: Romper cada prueba a propósito**

Esperar `"clave-nube"` como contraseña de "Banco" en el respaldo; importar con `"maestra-vieja"` en el flujo. Cada una debe fallar; revertir.

- [ ] **Step 4: Lint, formato y commit**

Run: `cargo fmt --all && cargo clippy -p vault-core --all-targets -- -D warnings`

```bash
git add core/tests/vault.rs
git commit -m "test: respaldo restaurado y flujo completo por la API pública"
```

---

### Task 5: Documentación

**Files:**
- Modify: `docs/especificacion.md` ("Estrategia de pruebas", línea de bóvedas de ejemplo)
- Modify: `docs/roadmap.md` (casilla "Pruebas" de la etapa 1)
- Modify: `ARCHITECTURE.md` (comentario de `fixtures/` en el mapa)

- [ ] **Step 1: Editar**

- Especificación: tras "Bóvedas de ejemplo fijas en el repositorio...", agregar "Decidido el 2026-11-07: una por versión de formato, `core/tests/fixtures/vault-vN.db` con N igual a la `format_version`; nunca se modifican ni se borran; se generan una sola vez con `KdfParams::MIN`, y las pruebas de integración (`core/tests/vault.rs`) las leen sobre una copia con la API pública (detalle en `docs/superpowers/specs/2026-11-07-pruebas-integracion-design.md`)."
- Roadmap: `- [x] Pruebas: ...`.
- Arquitectura: `└── fixtures/       # bóvedas de ejemplo fijas (vault-vN.db, ver su README)`.

- [ ] **Step 2: Commit**

```bash
git add docs/especificacion.md docs/roadmap.md ARCHITECTURE.md
git commit -m "docs: registrar las bóvedas de ejemplo en la especificación, la arquitectura y el roadmap"
```

---

### Task 6: Verificación final

- [ ] **Step 1: Pruebas, lint y formato del workspace**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`
Expected: todo pasa; en la salida de `vault-core` (lib) aparece `1 ignored` (el generador).

- [ ] **Step 2: El archivo versionado sigue intacto**

Run: `git status --porcelain && git diff dev --stat`
Expected: árbol limpio; `vault-v1.db` aparece una sola vez como `Bin`.
