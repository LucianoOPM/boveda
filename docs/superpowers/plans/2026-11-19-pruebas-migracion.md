# Pruebas de la migración y de la bóveda v2 (#53) — plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Que la prueba de la migración fallida detecte una migración sin transacción y que la de la bóveda v2 detecte un archivo que no sea v2.

**Architecture:** Dos pruebas nuevas, una unitaria en `core/src/vault/tests.rs` y una de la API pública en `core/tests/vault.rs`. Sin cambios en `core/src` fuera de pruebas.

**Spec:** `docs/superpowers/specs/2026-11-19-pruebas-migracion-design.md`

## Global Constraints

- Identificadores en inglés; comentarios, documentación y commits en español, Conventional Commits con `(#53)`.
- No modificar `core/tests/fixtures/*.db` ni código de producción (la mutación de `storage.rs` es temporal y se restaura).
- Ejecutar `cargo test -p vault-core` antes de cada commit.

## Task 1: Migración fallida a mitad de la transacción

**Files:** `core/src/vault/tests.rs`

- [x] Prueba `failed_migration_rolls_back_after_dropping_the_old_header` con la vista `v` sobre `header`.
- [x] Mutación: `COMMIT` tras el `INSERT` en `migrate_header`; la prueba nueva debe fallar y la existente seguir pasando. Restaurar `storage.rs`.
- [x] Commit: `test: la migración fallida a mitad de la transacción se revierte (#53)`.

## Task 2: La bóveda de ejemplo es v2

**Files:** `core/tests/vault.rs`

- [x] Prueba `fixture_v2_unlock_does_not_write`: bytes iguales antes y después de desbloquear, sin revelar secretos.
- [x] Comprobar que la misma lógica con `vault-v1.db` da bytes distintos.
- [x] Commit: `test: la bóveda de ejemplo v2 no escribe al desbloquear (#53)`.

## Task 3: Cierre

- [ ] `cargo test --workspace`, `clippy`, `fmt`, `cargo audit` y `cargo deny check`.
- [ ] Revisión final y PR contra `dev` con `Closes #53`.
