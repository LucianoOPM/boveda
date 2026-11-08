# Primer uso — plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Al abrir la app sin bóveda, crear la contraseña maestra (mínimo de 12 caracteres en el núcleo, indicador de fortaleza, confirmación y aceptación de que no se recupera) y quedar desbloqueada en un marcador de la pantalla principal.

**Architecture:** La regla de la contraseña (`password_strength`, `PasswordTooShort`) vive en `vault-core` y se aplica en `create` y `change_master_password`. La fachada la expone como función `#[frb(sync)]`. En Dart, un widget raíz (`VaultGate`) elige la pantalla por estado, y `FirstRunScreen` recibe la sesión y la función de fortaleza inyectadas.

**Tech Stack:** Rust (vault-core, secrecy), flutter_rust_bridge 2.13.0, Flutter 3.47 (Material 3), flutter_test.

**Spec:** `docs/superpowers/specs/2026-11-08-primer-uso-design.md`

## Global Constraints

- Mínimo 12 caracteres Unicode (`chars().count()`); `Acceptable` de 12 a 15; `Strong` desde 16. Sin recortar: los espacios cuentan.
- La regla solo se aplica en `create`, `create_with_params`, `change_master_password` y `change_master_password_with_params`; nunca en `unlock`, `unlock_with_key` ni `import`.
- `password_strength` no copia el secreto: cuenta sobre `expose_secret().chars()`.
- Sin dependencias nuevas, ni en Rust ni en Dart. `mocktail` se quita de `dev_dependencies`.
- Ningún error ni `Debug` contiene la contraseña.
- Textos de la interfaz, exactos: "Crea tu contraseña maestra", "Usa una frase de varias palabras. Mínimo 12 caracteres.", "Contraseña maestra", "Confirma la contraseña", "Las contraseñas no coinciden", "Demasiado corta", "Aceptable", "Fuerte", "Entiendo que si olvido la contraseña maestra pierdo el acceso a la bóveda. No se puede recuperar.", "Crear bóveda", "Creando la bóveda…", "La contraseña debe tener al menos 12 caracteres.", "No se pudo crear la bóveda. Intenta de nuevo.", "No se pudo abrir la bóveda.", "Desbloqueo: #37", "Pantalla principal: #40".
- Identificadores en inglés; comentarios, documentación y commits en español (Conventional Commits).
- En `mobile/`, regenerar el puente (`flutter_rust_bridge_codegen generate`) antes de `cargo test` cuando cambie `rust/src/api/`.
- Nunca encadenar `clippy` con `| grep` antes de un commit.

## Review Focus

1. Doble toque rápido en "Crear bóveda": `create` debe llamarse una sola vez (prueba en la Task 4).
2. Cambiar la contraseña después de escribir una confirmación igual: el botón debe volver a desactivarse (prueba en la Task 4).
3. Un error de `create` seguido de un reintento: el segundo intento debe funcionar y llevar a la pantalla principal (prueba en la Task 4).
4. Una contraseña de solo espacios con 12 caracteres cuenta como `Acceptable`: es lo decidido (no se recorta) y debe quedar fijado (prueba en la Task 1).
5. Una contraseña pegada muy larga (10 000 caracteres) no hace panic y da `Strong` (prueba en la Task 1).

---

### Task 1: Regla de la contraseña maestra en el núcleo

**Files:**
- Create: `core/src/vault/password.rs`, `core/src/vault/password/tests.rs`
- Modify: `core/src/error.rs` (variante nueva), `core/src/vault/mod.rs` (módulo, `pub use`, comprobaciones en `create`, `create_with_params`, `change_master_password`, `change_master_password_with_params`)
- Modify (pruebas existentes con contraseñas cortas): `core/src/vault/tests.rs`, `core/src/vault/backup/tests.rs`, `core/tests/vault.rs`
- Modify (mapeo exhaustivo de la fachada): `mobile/rust/src/api/error.rs`, `mobile/rust/src/api/error/tests.rs`; regenerado `mobile/lib/src/rust/api/error.dart` y `mobile/rust/src/frb_generated.rs`
- Modify (docs): `docs/especificacion.md` (sección "API"), `ARCHITECTURE.md` (fila de `vault/` en la tabla del núcleo)

**Interfaces:**
- Produces:
  - `vault_core::vault::PasswordStrength` (`#[derive(Debug, Clone, Copy, PartialEq, Eq)] enum { TooShort, Acceptable, Strong }`)
  - `vault_core::vault::password_strength(password: &SecretString) -> PasswordStrength`
  - `vault_core::vault::MIN_MASTER_PASSWORD_CHARS: usize = 12`, `STRONG_MASTER_PASSWORD_CHARS: usize = 16`
  - `vault_core::vault::Error::PasswordTooShort`, con `#[error("la contraseña maestra es demasiado corta")]`
  - `pub(crate) fn require_master_password(password: &SecretString) -> Result<()>` en `password.rs`: `Err(Error::PasswordTooShort)` si es `TooShort`

- [ ] **Step 1: Escribir las pruebas de `password_strength` que fallan** en `core/src/vault/password/tests.rs`

```rust
fn s(text: &str) -> SecretString { SecretString::from(text) }

#[test] fn empty_is_too_short()      { assert_eq!(password_strength(&s("")), PasswordStrength::TooShort); }
#[test] fn eleven_is_too_short()     { assert_eq!(password_strength(&s(&"a".repeat(11))), PasswordStrength::TooShort); }
#[test] fn twelve_is_acceptable()    { assert_eq!(password_strength(&s(&"a".repeat(12))), PasswordStrength::Acceptable); }
#[test] fn fifteen_is_acceptable()   { assert_eq!(password_strength(&s(&"a".repeat(15))), PasswordStrength::Acceptable); }
#[test] fn sixteen_is_strong()       { assert_eq!(password_strength(&s(&"a".repeat(16))), PasswordStrength::Strong); }
// Cuenta caracteres, no bytes: 12 "ñ" son 24 bytes.
#[test] fn counts_chars_not_bytes()  {
    assert_eq!(password_strength(&s(&"ñ".repeat(11))), PasswordStrength::TooShort);
    assert_eq!(password_strength(&s(&"ñ".repeat(12))), PasswordStrength::Acceptable);
}
#[test] fn emoji_counts_one()        { assert_eq!(password_strength(&s(&format!("{}🔐", "a".repeat(11)))), PasswordStrength::Acceptable); }
// No se recorta: los espacios cuentan (Review Focus 4).
#[test] fn spaces_count()            { assert_eq!(password_strength(&s(&" ".repeat(12))), PasswordStrength::Acceptable); }
// Review Focus 5.
#[test] fn very_long_is_strong()     { assert_eq!(password_strength(&s(&"x".repeat(10_000))), PasswordStrength::Strong); }
#[test] fn require_rejects_short()   {
    assert_eq!(require_master_password(&s(&"a".repeat(11))), Err(Error::PasswordTooShort));
    assert_eq!(require_master_password(&s(&"a".repeat(12))), Ok(()));
}
```

- [ ] **Step 2: Correr y comprobar que falla**

Run: `cargo test -p vault-core password::`
Expected: FAIL de compilación (no existe `password_strength`).

- [ ] **Step 3: Implementar `password.rs`, `Error::PasswordTooShort` y el `pub use`** con las firmas de "Interfaces". `password.rs` termina con `#[cfg(test)] mod tests;`. Documentar en el módulo que la regla es de longitud (NIST SP 800-63B), que cuenta caracteres Unicode y que solo se aplica al crear o cambiar.

- [ ] **Step 4: Correr y comprobar que pasa**

Run: `cargo test -p vault-core password::`
Expected: PASS.

- [ ] **Step 5: Escribir las pruebas que fallan para `create` y `change_master_password`** en `core/src/vault/tests.rs`

```rust
#[test]
fn create_rejects_short_password_before_touching_disk() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("boveda.db");
    // `Vault::create` real: debe regresar antes de calibrar.
    assert_eq!(Vault::create(&path, &password("corta-11ch")).err(), Some(Error::PasswordTooShort));
    assert!(!path.exists());
}

#[test]
fn create_with_params_rejects_short_password() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("boveda.db");
    assert_eq!(
        Vault::create_with_params(&path, &password("corta-11ch"), KdfParams::MIN).err(),
        Some(Error::PasswordTooShort)
    );
    assert!(!path.exists());
}

#[test]
fn change_master_password_rejects_short_before_deriving() {
    // Bóveda creada con una contraseña válida; `DERIVE_CALLS` no aumenta
    // porque la longitud se comprueba antes de verificar la actual.
    // Con la actual incorrecta y la nueva corta, el error es PasswordTooShort.
    // Después, la contraseña anterior sigue desbloqueando.
}

#[test]
fn unlock_and_import_ignore_the_minimum() {
    // Bóveda con contraseña "corta" armada con VaultHeader::create(&pw, KdfParams::MIN)
    // y Storage::create(&path, &header), sin pasar por Vault::create.
    // Vault::open + unlock(&password("corta")) funciona; export a otra ruta y
    // Vault::import(backup, dest, &password("corta")) también.
}
```

La cadena `"corta-11ch"` tiene 10 caracteres; cualquier valor de menos de 12 sirve. En `change_master_password_rejects_short_before_deriving`, comparar `DERIVE_CALLS` antes y después como ya hacen las pruebas existentes que lo usan.

- [ ] **Step 6: Correr y comprobar que fallan**

Run: `cargo test -p vault-core vault::tests::create_rejects vault::tests::change_master_password_rejects`
Expected: FAIL (se crea el archivo o el error no es `PasswordTooShort`).

- [ ] **Step 7: Agregar las comprobaciones** en `vault/mod.rs`: `require_master_password(password)?` como primera línea de `create` (antes de `require_absolute`), de `create_with_params`, y de un punto común que llamen `change_master_password` y `change_master_password_with_params` antes de `verify_current` (por ejemplo, al inicio de ambas). Documentar `PasswordTooShort` en los comentarios de esas funciones.

- [ ] **Step 8: Ajustar las pruebas existentes con contraseñas cortas**

Run: `cargo test -p vault-core`
Expected: fallan con `PasswordTooShort` las que crean o cambian con menos de 12 caracteres. Reemplazarlas por valores de 12 o más que conserven la intención, por ejemplo `"correcta"` → `"correcta-larga"`, `"nueva"` → `"nueva-maestra"`, `"maestra"` → `"maestra-larga"`, `"vieja"` → `"vieja-maestra"`, `"p"` → `"p-de-prueba-12"`, `"respaldo"` → `"respaldo-maestra"`, `"actual"` → `"actual-maestra"`. Las contraseñas usadas solo como incorrectas en `unlock` (`"incorrecta"`) no cambian. Las de las bóvedas de ejemplo (`prueba-bóveda-v1/v2`, 16 caracteres) no cambian. Mantener la coherencia entre `create` y el `unlock` posterior de cada prueba.

- [ ] **Step 9: Mapear el error nuevo en la fachada**

El mapeo de `mobile/rust/src/api/error.rs` es exhaustivo: sin esto el workspace no compila. Agregar `(E::PasswordTooShort, VaultError::PasswordTooShort)` a `maps_every_core_error` en `mobile/rust/src/api/error/tests.rs`; agregar `VaultError::PasswordTooShort` en la parte "uno a uno" del enum y su brazo en el `From`. Regenerar el puente desde `mobile/` (`flutter_rust_bridge_codegen generate`) y comprobar que `lib/src/rust/api/error.dart` trae `passwordTooShort`.

- [ ] **Step 10: Correr todo y el lint**

Run (raíz): `cargo test --workspace` y `cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS y sin advertencias.

- [ ] **Step 11: Documentar**

- `docs/especificacion.md`, sección "API": viñeta "Decidido el 2026-11-08 (#36): ..." con la regla (mínimo de 12 caracteres Unicode, fuerte desde 16, sin recortar; `password_strength` y `PasswordTooShort`; solo en `create` y `change_master_password`, nunca en `unlock`, `unlock_with_key` ni `import`, para que las bóvedas y los respaldos existentes sigan abriendo; se comprueba antes de calibrar y antes de verificar la contraseña actual; cambio de API, no de formato).
- `ARCHITECTURE.md`: agregar `password.rs` a la línea de `vault/` del mapa y "regla de la contraseña maestra (`password.rs`)" a la fila de `vault/` de la tabla.

- [ ] **Step 12: Commit**

```bash
cargo fmt --all
git add core mobile/rust mobile/lib/src/rust docs/especificacion.md ARCHITECTURE.md
git commit -m "feat: longitud mínima de la contraseña maestra en el núcleo (#36)"
```

---

### Task 2: Fortaleza en la fachada

**Files:**
- Create: `mobile/rust/src/api/password.rs`, `mobile/rust/src/api/password/tests.rs`
- Modify: `mobile/rust/src/api/mod.rs` (`pub mod password;`), `mobile/rust/src/api/session/tests.rs`
- Regenerated: `mobile/rust/src/frb_generated.rs`, `mobile/lib/src/rust/**`
- Modify (docs): `mobile/CLAUDE.md`, `ARCHITECTURE.md` (mapa de `api/`)

**Interfaces:**
- Consumes: `vault_core::vault::{password_strength, PasswordStrength}` y `VaultError::PasswordTooShort` (Task 1); `crate::api::session::secret_from(String) -> SecretString`.
- Produces:
  - Rust: `pub enum PasswordStrength { TooShort, Acceptable, Strong }` (fachada, `#[derive(Debug, Clone, Copy, PartialEq, Eq)]`, `From<vault::PasswordStrength>`); `#[frb(sync)] pub fn password_strength(password: String) -> PasswordStrength`.
  - Dart (generado): `lib/src/rust/api/password.dart` con `enum PasswordStrength { tooShort, acceptable, strong }` y `PasswordStrength passwordStrength({required String password})`.

- [ ] **Step 1: Escribir las pruebas que fallan**

`api/password/tests.rs`:

```rust
#[test]
fn delegates_to_core() {
    assert_eq!(password_strength("a".repeat(11)), PasswordStrength::TooShort);
    assert_eq!(password_strength("a".repeat(12)), PasswordStrength::Acceptable);
    assert_eq!(password_strength("a".repeat(16)), PasswordStrength::Strong);
}
```

`api/session/tests.rs`:

```rust
#[test]
fn create_with_short_password_fails_and_leaves_no_vault() {
    // Sesión sobre una ruta nueva en un tempdir.
    assert_eq!(session.create("corta".into()), Err(VaultError::PasswordTooShort));
    assert_eq!(session.is_locked(), Ok(true));
    assert_eq!(session.search(String::new()).err(), Some(VaultError::NoVault));
}
```

- [ ] **Step 2: Correr y comprobar que falla**

Run: `cargo test -p rust_lib_boveda`
Expected: FAIL de compilación.

- [ ] **Step 3: Implementar** `password.rs` (convierte con `secret_from` y delega; doc: es síncrona porque no toca la `Vault` ni el `Mutex` y es instantánea) y `pub mod password;`. Si `secret_from` es `pub(crate)` en `session.rs`, importarla desde ahí; no duplicarla.

- [ ] **Step 4: Regenerar el puente**

Run (desde `mobile/`): `flutter_rust_bridge_codegen generate`
Expected: aparece `lib/src/rust/api/password.dart`. Leer el `.dart` generado y confirmar que `passwordStrength` es síncrona (no `Future`) y que `PasswordStrength` es un `enum` de Dart.

- [ ] **Step 5: Correr y comprobar que pasa**

Run (raíz): `cargo test --workspace` y `cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS y sin advertencias.

- [ ] **Step 6: Documentar**

- `mobile/CLAUDE.md`: la regla de `#[frb(sync)]` pasa a nombrar dos excepciones: `VaultSession::new` y `password_strength`, que no tocan la bóveda ni el `Mutex`.
- `ARCHITECTURE.md`: `password.rs (fortaleza de la contraseña maestra)` en la lista de archivos de `api/`.

- [ ] **Step 7: Commit**

```bash
cargo fmt --all
git add mobile/rust mobile/lib/src/rust mobile/CLAUDE.md ARCHITECTURE.md
git commit -m "feat: fortaleza de la contraseña en la fachada (#36)"
```

---

### Task 3: Campo de contraseña e indicador de fortaleza

**Files:**
- Create: `mobile/lib/widgets/password_field.dart`, `mobile/lib/widgets/strength_indicator.dart`
- Test: `mobile/test/widgets_test.dart`

**Interfaces:**
- Consumes: `PasswordStrength` de `package:boveda/src/rust/api/password.dart` (Task 2).
- Produces:
  - `typedef StrengthOf = PasswordStrength Function(String password);` en `strength_indicator.dart`
  - `class PasswordField extends StatefulWidget { const PasswordField({super.key, required TextEditingController controller, required String label, bool enabled = true, String? errorText, ValueChanged<String>? onChanged}); }` — estado interno: `_obscured` (inicia en `true`); `suffixIcon` es un `IconButton` con `Icons.visibility` / `Icons.visibility_off` y tooltip "Mostrar contraseña" / "Ocultar contraseña".
  - `class StrengthIndicator extends StatelessWidget { const StrengthIndicator({super.key, required PasswordStrength strength}); }` — `LinearProgressIndicator` con valor 1/3, 2/3 o 1 y colores `colorScheme.error`, `colorScheme.tertiary`, `colorScheme.primary`; texto "Demasiado corta", "Aceptable" o "Fuerte".

- [ ] **Step 1: Escribir las pruebas que fallan** en `test/widgets_test.dart` (envolver en `MaterialApp(home: Scaffold(body: ...))`)

```dart
testWidgets('PasswordField configura el teclado para una contraseña', (t) async {
  // pump PasswordField(controller: c, label: 'Contraseña maestra')
  final f = t.widget<TextField>(find.byType(TextField));
  expect(f.obscureText, isTrue);
  expect(f.enableSuggestions, isFalse);
  expect(f.autocorrect, isFalse);
  expect(f.enableIMEPersonalizedLearning, isFalse);
  expect(f.autofillHints, anyOf(isNull, isEmpty));
  expect(f.keyboardType, TextInputType.visiblePassword);
});

testWidgets('el botón de ver alterna obscureText', (t) async {
  // tap en find.byTooltip('Mostrar contraseña') → obscureText false
  // tap en find.byTooltip('Ocultar contraseña') → obscureText true
});

testWidgets('PasswordField desactivado no acepta texto', (t) async {
  // enabled: false → TextField.enabled == false
});

for (final (strength, label) in [
  (PasswordStrength.tooShort, 'Demasiado corta'),
  (PasswordStrength.acceptable, 'Aceptable'),
  (PasswordStrength.strong, 'Fuerte'),
]) {
  testWidgets('StrengthIndicator muestra "$label"', (t) async {
    // pump StrengthIndicator(strength: strength)
    expect(find.text(label), findsOneWidget);
  });
}
```

- [ ] **Step 2: Correr y comprobar que falla**

Run (desde `mobile/`): `flutter test test/widgets_test.dart`
Expected: FAIL de compilación (no existen los widgets).

- [ ] **Step 3: Implementar ambos widgets** con las firmas de "Interfaces". `PasswordField` usa `TextField` con `maxLines: 1`, `decoration: InputDecoration(labelText: label, errorText: errorText, suffixIcon: ...)`; no crea ni libera el controlador.

- [ ] **Step 4: Correr y comprobar que pasa**

Run: `flutter test test/widgets_test.dart` y `flutter analyze`
Expected: PASS y "No issues found!".

- [ ] **Step 5: Commit**

```bash
git add mobile/lib/widgets mobile/test/widgets_test.dart
git commit -m "feat: campo de contraseña e indicador de fortaleza (#36)"
```

---

### Task 4: Pantalla de primer uso

**Files:**
- Create: `mobile/lib/screens/first_run_screen.dart`, `mobile/test/fakes.dart`
- Test: `mobile/test/first_run_screen_test.dart`

**Interfaces:**
- Consumes: `PasswordField`, `StrengthIndicator`, `StrengthOf` (Task 3); `VaultSession`, `VaultError` y `PasswordStrength` generados (Task 2).
- Produces:
  - `class FirstRunScreen extends StatefulWidget { const FirstRunScreen({super.key, required VaultSession session, required StrengthOf strengthOf, required VoidCallback onCreated}); }`
  - Claves: `const Key('first-run-password')`, `const Key('first-run-confirm')`, `const Key('first-run-accept')` (el `CheckboxListTile`), `const Key('first-run-create')` (el `FilledButton`).
  - `test/fakes.dart`:
    - `class FakeVaultSession implements VaultSession` con `final List<String> createCalls`, `Future<void> Function(String password) onCreate` (por omisión, completa enseguida), `Object? openError` (si no es `null`, `open()` lo lanza), `int openCalls`; `dispose()` marca `isDisposed`; los demás métodos lanzan `UnimplementedError`.
    - `PasswordStrength fakeStrength(String p)`: `< 12` → `tooShort`, `< 16` → `acceptable`, si no `strong` (usa `p.length`; las pruebas usan solo ASCII).

- [ ] **Step 1: Escribir el fake y las pruebas que fallan** en `test/first_run_screen_test.dart`. Un ayudante `pumpScreen(t, session, {VoidCallback? onCreated})` monta `MaterialApp(home: FirstRunScreen(session: session, strengthOf: fakeStrength, onCreated: onCreated ?? () {}))`. Un ayudante `fillValid(t)` escribe `'frase larga de prueba'` en ambos campos y marca la casilla.

```dart
FilledButton button(WidgetTester t) => t.widget(find.byKey(const Key('first-run-create')));

testWidgets('muestra el título y la sugerencia', ...);   // textos exactos de Global Constraints

testWidgets('el botón empieza desactivado', ...);        // button(t).onPressed == null

testWidgets('cada condición por separado mantiene el botón desactivado', (t) async {
  // corta ('corta-11ch') + confirmación igual + casilla → desactivado
  // larga + confirmación distinta + casilla → desactivado
  // larga + confirmación igual, sin casilla → desactivado
  // larga + igual + casilla → activado
});

testWidgets('el indicador cambia al escribir y no aparece vacío', (t) async {
  // vacío: find.byType(StrengthIndicator) findsNothing
  // 'corta' → 'Demasiado corta'; 'doce-caracte' (12) → 'Aceptable'; 'dieciseis-caract' (16) → 'Fuerte'
});

testWidgets('confirmación distinta muestra el error', ...); // 'Las contraseñas no coinciden'

// Review Focus 2
testWidgets('cambiar la contraseña tras confirmar vuelve a desactivar', (t) async {
  // fillValid; luego escribir otra cosa en first-run-password → onPressed == null
});

testWidgets('durante la creación: progreso, todo desactivado y volver no sale', (t) async {
  final pending = Completer<void>();
  final s = FakeVaultSession()..onCreate = (_) => pending.future;
  // fillValid, tap crear, pump
  expect(find.text('Creando la bóveda…'), findsOneWidget);
  expect(find.byType(CircularProgressIndicator), findsOneWidget);
  expect(button(t).onPressed, isNull);
  // TextField de ambos campos: enabled == false; CheckboxListTile.onChanged == null
  // final popped = await t.binding.handlePopRoute(); la pantalla sigue (find.byType(FirstRunScreen) findsOneWidget)
  pending.complete();
});

testWidgets('al crear llama a create con la contraseña y a onCreated', (t) async {
  // createCalls == ['frase larga de prueba']; onCreated llamado una vez
});

// Review Focus 1
testWidgets('doble toque llama a create una sola vez', (t) async {
  // con un Completer pendiente: tap, tap (sin pump entre ambos con warnIfMissed: false) → createCalls.length == 1
});

testWidgets('error genérico al fallar create y el formulario vuelve', (t) async {
  // onCreate lanza VaultError.storageIo → 'No se pudo crear la bóveda. Intenta de nuevo.'; botón activado; los campos conservan el texto
});

testWidgets('PasswordTooShort muestra el mensaje de longitud', ...);
// onCreate lanza VaultError.passwordTooShort → 'La contraseña debe tener al menos 12 caracteres.'

// Review Focus 3
testWidgets('reintentar tras un error funciona', (t) async {
  // primer onCreate lanza; se cambia onCreate a uno que completa; segundo tap → onCreated llamado
});
```

- [ ] **Step 2: Correr y comprobar que falla**

Run: `flutter test test/first_run_screen_test.dart`
Expected: FAIL de compilación (no existe `FirstRunScreen`).

- [ ] **Step 3: Implementar `FirstRunScreen`** según la spec ("Pantalla de primer uso"):
  - Estado: dos `TextEditingController`, `bool _accepted`, `bool _creating`, `String? _error`. No guarda la contraseña en ningún campo.
  - `onChanged` de ambos campos llama a `setState` para recalcular la fortaleza (`widget.strengthOf(_password.text)`), la coincidencia y el botón.
  - `_canCreate = !_creating && _accepted && strengthOf(text) != PasswordStrength.tooShort && _password.text == _confirm.text`.
  - `_create()`: sale si `_creating` (Review Focus 1); `setState` con `_creating = true` y `_error = null` antes del `await`; lee la contraseña en una variable local; `await widget.session.create(password: local)`; comprueba `mounted`; con éxito llama a `widget.onCreated()`; con `VaultError.passwordTooShort` o cualquier otro error, `setState` con el mensaje exacto y `_creating = false`.
  - `PopScope(canPop: !_creating, child: Scaffold(...))`, con el contenido en un `SingleChildScrollView` (el teclado ocupa espacio).
  - `dispose()`: `clear()` y después `dispose()` en cada controlador.

- [ ] **Step 4: Correr y comprobar que pasa**

Run: `flutter test test/first_run_screen_test.dart` y `flutter analyze`
Expected: PASS y "No issues found!".

- [ ] **Step 5: Commit**

```bash
git add mobile/lib/screens mobile/test/fakes.dart mobile/test/first_run_screen_test.dart
git commit -m "feat: pantalla de primer uso (#36)"
```

---

### Task 5: Widget raíz, arranque y marcadores

**Files:**
- Create: `mobile/lib/app.dart`, `mobile/lib/screens/placeholder_screen.dart`
- Modify: `mobile/lib/main.dart`, `mobile/pubspec.yaml` (quitar `mocktail`), `mobile/pubspec.lock`
- Delete: `mobile/test/widget_test.dart`
- Test: `mobile/test/app_test.dart`
- Modify (docs): `ARCHITECTURE.md` (mapa de `mobile/lib/`)

**Interfaces:**
- Consumes: `FirstRunScreen`, `StrengthOf`, `FakeVaultSession`, `fakeStrength` (Tasks 3–4); `vaultPath()` de `services/vault_location.dart`.
- Produces:
  - `class BovedaApp extends StatelessWidget { const BovedaApp({super.key, required VaultSession session, required Future<bool> Function() vaultExists, required StrengthOf strengthOf}); }` — `MaterialApp(title: 'Bóveda', home: VaultGate(...))`.
  - `enum VaultGateState { checking, noVault, locked, unlocked, openFailed }`
  - `class VaultGate extends StatefulWidget` (mismos tres parámetros). Las issues #37 y #39 agregarán transiciones a este estado.
  - `class PlaceholderScreen extends StatelessWidget { const PlaceholderScreen({super.key, required String text}); }` — `Scaffold` con el texto centrado.

- [ ] **Step 1: Escribir las pruebas que fallan** en `test/app_test.dart`, montando `BovedaApp` con `FakeVaultSession` y `fakeStrength`:

```dart
testWidgets('sin bóveda abre el primer uso', ...);
// vaultExists: () async => false → find.byType(FirstRunScreen) findsOneWidget; openCalls == 0

testWidgets('con bóveda abre el marcador de desbloqueo', ...);
// true y open sin error → 'Desbloqueo: #37'; FirstRunScreen findsNothing; openCalls == 1

testWidgets('si open falla muestra el error genérico, nunca el primer uso', ...);
// openError: VaultError.notAVault → 'No se pudo abrir la bóveda.'; FirstRunScreen findsNothing

testWidgets('si la comprobación del archivo lanza muestra el error genérico', ...);
// vaultExists: () async => throw const FileSystemException('x') → 'No se pudo abrir la bóveda.'

testWidgets('mientras comprueba muestra un indicador de carga', ...);
// vaultExists con un Completer pendiente → CircularProgressIndicator findsOneWidget

testWidgets('al crear la bóveda pasa a la pantalla principal', ...);
// sin bóveda; llenar y crear → 'Pantalla principal: #40'; FirstRunScreen findsNothing
```

- [ ] **Step 2: Correr y comprobar que falla**

Run: `flutter test test/app_test.dart`
Expected: FAIL de compilación.

- [ ] **Step 3: Implementar** `placeholder_screen.dart`, `app.dart` (el arranque de la spec, en `initState`: `vaultExists()` → `noVault`; si existe, `session.open()` → `locked`; cualquier excepción en esos pasos → `openFailed`; comprobar `mounted` antes de cada `setState`) y `main.dart` (el bloque de "Inyección" de la spec). Borrar `test/widget_test.dart` y quitar `mocktail` de `pubspec.yaml`; correr `flutter pub get`.

- [ ] **Step 4: Correr toda la suite**

Run (desde `mobile/`): `flutter test` y `flutter analyze`
Expected: PASS y "No issues found!". `grep -r mocktail mobile/lib mobile/test` no encuentra nada.

- [ ] **Step 5: Documentar** en `ARCHITECTURE.md`, mapa de `mobile/lib/`: `app.dart (BovedaApp y VaultGate: la pantalla según el estado de la sesión)`; `screens/` con `first_run_screen.dart` y `placeholder_screen.dart` (futuro: desbloqueo, búsqueda, detalle, edición, ajustes); `widgets/` con `password_field.dart` y `strength_indicator.dart`; y en "Componentes → mobile/" una línea sobre el widget raíz por estado, sin go_router.

- [ ] **Step 6: Commit**

```bash
git add mobile/lib mobile/test mobile/pubspec.yaml mobile/pubspec.lock ARCHITECTURE.md
git commit -m "feat: widget raíz por estado y arranque de la app (#36)"
```

---

### Task 6: Verificación completa y prueba manual

**Files:** ninguno nuevo (solo correcciones si algo falla).

- [ ] **Step 1: Verificación de Rust** (raíz)

Run: `cargo fmt --all -- --check`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo deny check`
Expected: todo pasa; `Cargo.lock` sin crates nuevos (`git diff dev -- Cargo.lock` vacío).

- [ ] **Step 2: Verificación de Flutter** (desde `mobile/`)

Run: `flutter_rust_bridge_codegen generate` (no debe cambiar nada: `git status` limpio), `flutter analyze`, `flutter test`
Expected: sin cambios, "No issues found!" y todas las pruebas pasan.

- [ ] **Step 3: Prueba manual en el emulador**

Antes: `rustup run stable rustc --version` coincide con `rust-toolchain.toml` (ver `mobile/CLAUDE.md`). Run: `flutter run -d <emulador>` desde una instalación limpia (`adb uninstall` del paquete si ya estaba).
Comprobar:
- Abre en el primer uso; el botón empieza desactivado.
- El indicador cambia al escribir; el teclado no sugiere palabras.
- Con una frase de 16 o más, confirmada y con la casilla: "Creando la bóveda…", la interfaz responde (la animación del indicador sigue) y termina en "Pantalla principal: #40".
- Cerrar la app y volver a abrirla: "Desbloqueo: #37", no el primer uso.

- [ ] **Step 4: Registrar el resultado** en el cuerpo del PR (sección "Verificación local"). Si algo falla, corregirlo con un commit `fix:` y volver al Step 1.
