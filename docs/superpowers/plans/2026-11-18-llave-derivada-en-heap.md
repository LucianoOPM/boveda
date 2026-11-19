# La llave derivada en el heap (#54) — plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Que la llave derivada viva en el heap (`SecretBox`), que Argon2id escriba directamente en ella, que el búfer de `encrypt_vault_key` se borre y que ningún ayudante devuelva el cifrador por valor, sin cambiar el comportamiento ni el formato.

**Architecture:** Solo `core/src/crypto/` (`kdf.rs`, `keys.rs`, `aead.rs`) y sus pruebas, más la documentación de la excepción. Sin dependencias nuevas.

**Spec:** `docs/superpowers/specs/2026-11-18-llave-derivada-en-heap-design.md`

## Global Constraints

- Identificadores en inglés; comentarios, documentación y commits en español, Conventional Commits con `(#54)`.
- Sin dependencias nuevas ni cambios de `Cargo.toml` (las features `zeroize` ya están activas).
- Los vectores fijos de `kdf/tests.rs` (`ascii_password_derives_the_reference_key`, `CAFE_KEY_HEX`) no cambian y deben seguir pasando en cada tarea.
- Eliminar los ayudantes `cipher`/`vault_key_cipher`: esfuerzo razonable, **sin pruebas que pretendan verificar movimientos**.
- No encadenar `cargo clippy` ni `cargo test` con `| grep` o `| tail` antes de un commit: usar un script que imprima los códigos de salida.
- No modificar `core/tests/fixtures/`.

## Review Focus

1. `derive_key` no copia la llave fuera del `SecretBox`, tampoco al fallar (Tarea 1).
2. Ningún `[u8; 32]` secreto se mueve por valor en `crypto/` (Tareas 1 a 3), salvo las excepciones documentadas de las dependencias (Tarea 4).
3. Las pruebas existentes no cambian de comportamiento (solo cómo leen la llave).

## Estructura de archivos

| Archivo | Cambio |
| --- | --- |
| `core/src/crypto/kdf.rs`, `kdf/tests.rs` | `DerivedKey = SecretBox<[u8; 32]>`; `derive_key` con `init_with_mut`; prueba de tamaño de puntero |
| `core/src/crypto/keys.rs`, `keys/tests.rs` | `encrypt_vault_key` con `Zeroizing`; sin `cipher` ni `vault_key_cipher`; pruebas leen la llave con `expose_secret()` |
| `core/src/crypto/aead.rs` | Sin `cipher`; el cifrador se crea y se usa en `seal_part` y `open_part` |
| `core/src/vault/header/tests.rs` | Solo si hace falta adaptar el uso de la llave derivada |
| `ARCHITECTURE.md`, `docs/especificacion.md`, `core/src/crypto/mod.rs` | Excepción con versiones y razón |

### Task 1: `DerivedKey` en el heap

- [ ] Prueba primero en `kdf/tests.rs`: `derived_and_vault_keys_are_pointer_sized` (`size_of::<DerivedKey>() == size_of::<usize>()` y lo mismo para `VaultKey`), con un comentario que diga que garantiza que el tipo es un puntero y que mover no copia la llave, y que no prueba que no haya copias en la pila. Debe fallar con el tipo actual.
- [ ] `kdf.rs`: `pub(crate) type DerivedKey = SecretBox<[u8; DERIVED_KEY_LEN]>;`. En `derive_key`, `SecretBox::init_with_mut` con el resultado de `hash_password_into` capturado en una variable externa (`let mut result = Ok(())`), `result.map_err(|_| Error::InvalidKdfParams)?` después y devolver el `SecretBox`. Quitar `Zeroizing` si ya no se usa en el archivo.
- [ ] Adaptar las pruebas existentes que desreferencian la llave (`*first == *second`, `key.iter()`) a `expose_secret()`, sin cambiar lo que comprueban.
- [ ] Adaptar `keys.rs`: `cipher(derived)` usa `derived.expose_secret()`; `keys/tests.rs` (`derived(byte)`, `assert_eq!(*derived, *expected)`) y `vault/header/tests.rs` con `expose_secret()`.
- [ ] `cargo test -p vault-core`; commit: `refactor: la llave derivada vive en el heap con SecretBox (#54)`.

### Task 2: `encrypt_vault_key` con búfer que se borra

- [ ] `keys.rs`: el búfer de trabajo `Zeroizing<[u8; ENCRYPTED_KEY_LEN]>`; tras cifrar con éxito se copia solo el texto cifrado al `EncryptedKey` (`*buf`, que ya es texto cifrado y etiqueta).
- [ ] Las pruebas existentes de `encrypt_vault_key` y `decrypt_vault_key` pasan sin cambios; `cargo test -p vault-core`; commit: `fix: borrar el búfer de trabajo de encrypt_vault_key si el cifrado falla (#54)`.

### Task 3: Sin ayudantes que devuelvan el cifrador

- [ ] `keys.rs`: eliminar `cipher` y `vault_key_cipher`; en `encrypt_vault_key`, `decrypt_vault_key`, `seal_key_check` y `verify_key_check`, `let cipher = XChaCha20Poly1305::new_from_slice(...).map_err(|_| Error::...)?;` y usarlo en la misma función, con el mismo error que tenía cada ayudante (`WrongPassword`, `WrongVaultKey`). Dejar el comentario de que la longitud es fija.
- [ ] `aead.rs`: lo mismo en `seal_part` y `open_part` con `Error::CorruptEntry`.
- [ ] Comentar en el módulo que es un esfuerzo razonable, sin garantía.
- [ ] `cargo test -p vault-core` y `clippy`; commit: `refactor: crear y usar el cifrador en la misma función (#54)`.

### Task 4: Documentación

- [ ] `docs/especificacion.md` y `ARCHITECTURE.md`: la excepción con el mismo formato que las de `nucleo-matcher` y `nfc()`: qué copia es y dónde vive, las versiones exactas (`argon2` 0.6.0, `chacha20poly1305` 0.11.0), cuándo se revisa y por qué no se evita. Actualizar la nota de `crypto/mod.rs`.
- [ ] Commit: `docs: excepción de borrado de las copias internas de argon2 y chacha20poly1305 (#54)`.

### Task 5: Verificación y cierre

- [ ] Con un script que imprima los códigos de salida: `cargo test --workspace`, `clippy -D warnings`, `fmt --check`, `cargo audit`, `cargo deny check`; `git diff dev -- core/tests/fixtures` vacío; los dos vectores de `kdf` pasan.
- [ ] Revisión final independiente (opus) y menú de cierre.
- [ ] PR contra `dev` con el formato del PR #11, explicando por qué `SecretBox` no deja copias en la pila, y con `Closes #54`. No abrirlo hasta que termine la CI de `dev` (`4c5eb87`).
