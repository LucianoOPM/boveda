# Limpieza menor tras la #34 (#55) — plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Que `seal_key_check` no devuelva errores engañosos y que `fill_fixture` no dependa del final de línea del archivo fuente.

**Architecture:** Dos cambios aislados, en `core/src/crypto/keys.rs` y `core/src/vault/tests.rs`.

**Spec:** `docs/superpowers/specs/2026-11-19-limpieza-tras-34-design.md`

## Global Constraints

- Identificadores en inglés; comentarios, documentación y commits en español, Conventional Commits con `(#55)`.
- Sin cambios de comportamiento ni de formato; no modificar `core/tests/fixtures/*.db`.
- Ejecutar `cargo test -p vault-core` antes de cada commit.
- Rama sobre `fix/54-llave-derivada-en-heap`; el PR se abre cuando el #74 esté mergeado.

## Task 1: Error de `seal_key_check`

**Files:** `core/src/crypto/keys.rs`

- [x] Los dos errores inalcanzables pasan a `Error::MalformedEncryptedKey`, con el motivo en un comentario.
- [x] `cargo test -p vault-core` y `clippy`.
- [x] Commit: `refactor: seal_key_check devuelve MalformedEncryptedKey en sus errores inalcanzables (#55)`.

## Task 2: Salto de línea de `fill_fixture`

**Files:** `core/src/vault/tests.rs`

- [x] Las notas vuelven a `"Línea uno\nLínea dos"`.
- [x] Comprobar que `git diff` no cambia los bytes del contenido (LF) y que `vault-v2.db` no cambia.
- [x] Commit: `chore: escribir el salto de línea de fill_fixture como \n (#55)`.

## Task 3: Cierre

- [ ] `cargo test --workspace`, `clippy`, `fmt`, `cargo audit` y `cargo deny check`.
- [ ] Revisión final; tras mergear el #74, `git rebase dev`, subir la rama y abrir el PR contra `dev` con `Closes #55`.
