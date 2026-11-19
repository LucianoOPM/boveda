# Archivos huérfanos en create (#58) — plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fijar con pruebas de regresión que `Vault::create` funciona junto a un `-journal`, un `-wal` y un `-shm` huérfanos reales, sin cambiar código de producción.

**Architecture:** Tres pruebas unitarias en `core/src/vault/tests.rs`, con `make_hot_journal_copy` y `copy_live_wal` de `test_support.rs` y `Vault::create_with_params` con `KdfParams::MIN`.

**Spec:** `docs/superpowers/specs/2026-11-19-huerfanos-en-create-design.md`

## Global Constraints

- Identificadores en inglés; comentarios, documentación y commits en español, Conventional Commits con `(#58)`.
- Sin dependencias nuevas ni cambios en `core/src` fuera de `vault/tests.rs`.
- Ejecutar `cargo test -p vault-core` antes del commit.
- No pretender detectar el `-shm` sobrante.

## Task 1: Pruebas de regresión

**Files:** `core/src/vault/tests.rs`

- [ ] Ayudante `create_next_to_orphans(journal, wal)`: crea la bóveda "vieja" con una entrada, arma los restos reales junto a `boveda.db` y borra la base.
- [ ] Pruebas `create_next_to_orphan_journal`, `create_next_to_orphan_wal_and_shm` y `create_next_to_all_orphans`.
- [ ] Comprobar en cada una: `create` bien, `check_integrity` en `Ok`, desbloqueo con la contraseña nueva, `entries()` vacío y contraseña vieja con `WrongPassword`.
- [ ] `cargo test -p vault-core`, `clippy` y `fmt`.
- [ ] Commit: `test: crear una bóveda junto a un diario y un WAL huérfanos reales (#58)`.

## Task 2: Cierre

- [ ] `cargo test --workspace`, `clippy`, `cargo audit` y `cargo deny check`.
- [ ] Revisión final y PR contra `dev` con la evidencia del experimento y `Closes #58`.
