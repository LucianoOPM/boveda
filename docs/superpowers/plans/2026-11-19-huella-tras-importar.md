# Huella tras importar un respaldo Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Que un usuario con la huella activa que importa un respaldo desde Ajustes la tenga activa al terminar, con una sola verificación de huella y sin pasar por la pantalla de oferta.

**Architecture:** `runRestore` anota si la huella estaba activa antes de importar (solo con `replacesVault`) y lo devuelve en `RestoreImported`. `SettingsScreen` y `FirstRunScreen` lo pasan a `VaultGate._onImported`, que activa la huella con `enableBiometric` (sin cambios) y entra a la bóveda; si el usuario cancela o falla, sigue por `_afterCreate` (la oferta de hoy).

**Tech Stack:** Flutter (Material 3), `flutter_test`. Sin dependencias nuevas ni cambios en `core/` ni en Kotlin.

**Spec:** `docs/superpowers/specs/2026-11-19-huella-tras-importar-design.md` (corrige el punto 6 de "Tras importar con éxito" de `2026-11-14-respaldos-selector-design.md`).

## Global Constraints

- No cambia la criptografía ni el núcleo: se reutiliza `enableBiometric` (`beginEnable`, `exportVaultKey`, `completeEnable`) tal cual; la llave de la bóveda sale del núcleo solo después de una huella aceptada.
- La copia vieja del Keystore se sigue borrando con `biometric.disable()` en el mismo punto de hoy (después de importar, antes de activar la nueva).
- Cancelar la huella o un fallo de activación nunca bloquea la bóveda ni la cierra: se cae en la oferta de hoy.
- Se conservan `_isAway`, `_enterUnlocked` y la comprobación de `ModalRoute.isActive` de hoy: un bloqueo durante la activación no monta nada sobre el desbloqueo.
- Cadenas, `Key`s y textos de la pantalla de oferta no cambian.
- Un test existente solo se ajusta por el cambio de firma de los callbacks; cada ajuste se anota en el PR.
- Identificadores en inglés; comentarios, documentación y commits en español, Conventional Commits.
- No usar el emulador. Comandos de Flutter desde `mobile/`; Rust y git desde la raíz.

## Review Focus

- Importar desde Ajustes con la huella activa la deja activa y entra a la bóveda sin la oferta; con la huella inactiva ofrece como hoy (Task 3).
- Cancelar la huella o un fallo de `enableBiometric` deja en la oferta, con la bóveda desbloqueada (Task 3).
- Una importación fallida no toca la huella ni cambia el flujo de hoy (Task 2).
- Un bloqueo mientras la activación espera la huella no monta la bóveda ni la oferta (Task 3).
- El primer uso no reactiva aunque exista una copia vieja (Task 3).

---

### Task 1: Rama y documentos

- [ ] **Step 1:** La rama `fix/86-huella-tras-importar` ya existe desde `origin/dev`; el diseño y este plan, sin seguimiento, viajan con ella.
- [ ] **Step 2: Commit.** `git add docs/superpowers/specs/2026-11-19-huella-tras-importar-design.md docs/superpowers/plans/2026-11-19-huella-tras-importar.md && git commit -m "docs: diseño y plan de la huella tras importar"`

### Task 2: `runRestore` informa si la huella estaba activa

**Files:**
- Modify: `mobile/lib/screens/restore_dialog.dart` (`RestoreImported`, `runRestore`)
- Create: `mobile/test/restore_reenable_test.dart`

**Interfaces:**
- Produces: `RestoreImported(int damagedEntries, {bool reenableBiometric = false})` con el campo `reenableBiometric`; `runRestore` lo pone en `true` solo con `replacesVault` y `biometric.isEnabled()` en `true` leído **antes** de abrir el diálogo.

- [ ] **Step 1: Escribir los tests (fallan)** en `restore_reenable_test.dart`, montando un botón que llame a `runRestore` (como `backup_gate_test.dart`) con `FakeVaultSession`, `FakeBackupFiles` y `FakeBiometricService`: (a) `replacesVault: true` y `biometric.enabled = true` → el resultado es `RestoreImported` con `reenableBiometric == true`, y `biometric.calls` tiene `disable` después de importar; (b) `replacesVault: true` y `enabled = false` → `false`; (c) `replacesVault: false` y `enabled = true` → `false`; (d) `isEnabled` lanza (`enabledError`) → `false` y la importación sigue; (e) contraseña incorrecta (`VaultError.wrongPassword`) → `RestoreImportFailed` y `disable` **no** se llama.
- [ ] **Step 2: Ver que fallan.** Run: `flutter test test/restore_reenable_test.dart`. Expected: FAIL (no compila: falta el campo).
- [ ] **Step 3: Implementar.** Añadir el campo con valor por omisión `false` a `RestoreImported`. En `runRestore`, antes de abrir el diálogo (después de elegir el archivo y de comprobar que la pantalla sigue a la vista), leer `isEnabled()` con `try/catch` solo si `replacesVault`, y devolver `RestoreImported(damaged, reenableBiometric: wasEnabled)`. `_runImport` (que construye `RestoreImported` dentro del diálogo) no cambia; `runRestore` reconstruye el resultado con el indicador.
- [ ] **Step 4: Ver que pasan.** Run: `flutter test`. Expected: PASS (los demás tests no usan el campo).
- [ ] **Step 5: Commit.** `git add mobile/lib mobile/test && git commit -m "feat: informar si la huella estaba activa al importar"`

### Task 3: Las pantallas pasan el indicador y `VaultGate` reactiva la huella

**Files:**
- Modify: `mobile/lib/screens/settings_screen.dart`, `mobile/lib/screens/first_run_screen.dart`, `mobile/lib/app.dart`
- Modify (firma de los callbacks): `mobile/test/settings_screen_test.dart`, `mobile/test/settings_screen_design_test.dart`, `mobile/test/first_run_screen_test.dart`, `mobile/test/first_run_screen_design_test.dart`
- Create: `mobile/test/biometric_after_import_test.dart`

**Interfaces:**
- Consumes: `RestoreImported.reenableBiometric` (Task 2), `enableBiometric(session, biometric)` y `EnableOutcome`.
- Produces: `onImported` y `onRestored` pasan a `void Function(int damagedEntries, {bool reenableBiometric})`; `_onImported(int damaged, {bool reenableBiometric = false})` en `VaultGate`.

- [ ] **Step 1: Escribir los tests (fallan)** en `biometric_after_import_test.dart`, con `BovedaApp` y los fakes (como `backup_gate_test.dart`), abriendo Ajustes e importando: (a) huella activa antes: tras importar y escribir la contraseña, `biometric.calls` contiene `beginEnable` y `completeEnable`, `biometric.enabled` queda `true`, **no** aparece la pantalla de oferta y se ve la pantalla principal; (b) huella inactiva: aparece la oferta como hoy y no se llama a `beginEnable`; (c) `beginEnable` lanza `BiometricFailure.canceled` → aparece la oferta y la bóveda sigue desbloqueada; (d) `exportVaultKey` falla (`failed`) → aparece la oferta; (e) un bloqueo (la app pasa a segundo plano) mientras `beginEnable` espera → no se monta la pantalla principal ni la oferta, y se ve el desbloqueo; (f) primer uso con `enabled = true` y una copia vieja: se ofrece como hoy, sin `beginEnable` automático.
- [ ] **Step 2: Ver que fallan.** Run: `flutter test test/biometric_after_import_test.dart`. Expected: FAIL.
- [ ] **Step 3: Implementar.** Cambiar los dos callbacks a la nueva firma y pasar `reenableBiometric` desde `RestoreImported`; en `VaultGate._onImported`, tras quitar lo apilado y subir `_epoch` como hoy, si `reenableBiometric` y el dispositivo la admite, llamar a `enableBiometric`: con `EnableOutcome.enabled` entrar con `_enterUnlocked(VaultGateState.unlocked, damagedEntries: damaged)`; con `canceled` o `failed` seguir por `_afterCreate(damaged: damaged)`. Comprobar `mounted` y `_isAway` después del `await`. Ajustar las construcciones de las pantallas en los cuatro tests existentes a `(_, {bool reenableBiometric = false}) {}`.
- [ ] **Step 4: Ver que pasan.** Run: `flutter test`. Expected: PASS.
- [ ] **Step 5: Commit.** `git add mobile && git commit -m "fix: reactivar la huella tras importar un respaldo (#86)"`

### Task 4: Documentación y verificación

**Files:**
- Modify: `docs/superpowers/specs/2026-11-14-respaldos-selector-design.md` (punto 6 y la línea 21), `docs/especificacion.md` solo si contradice

- [ ] **Step 1: Documentación.** En el diseño de la #44, el punto 6 y la línea 21 dicen que la huella "queda desactivada hasta volver a activarla": añadir que desde Ajustes, si estaba activa, se reactiva sola (remitir al diseño nuevo). Comprobar que `docs/especificacion.md` (líneas 248 a 252: "al restaurar se vuelve a activar la huella") ya concuerda.
- [ ] **Step 2: Verificación completa.** Desde `mobile/`: `flutter test` y `flutter analyze`; desde la raíz: `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` y `cargo fmt --all --check`, cada uno con su código de salida visible. Expected: todo en 0.
- [ ] **Step 3: Preguntar al usuario si quiere probarlo en su teléfono** (APK de depuración e `adb install -r`).
- [ ] **Step 4: Commit.** `git add docs && git commit -m "docs: la huella se reactiva tras importar desde Ajustes"`

### Task 5: Publicar

- [ ] **Step 1: Pedir confirmación** y `git push -u origin fix/86-huella-tras-importar`.
- [ ] **Step 2: Abrir el PR contra `dev`** con el formato de los anteriores, título `fix: reactivar la huella tras importar un respaldo`, `Cierra #86` y la casilla de la CI sin marcar.
