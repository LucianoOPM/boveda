# Formulario de crear y editar entrada Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Que el formulario de crear, editar y reparar una entrada se vea como su board de Penpot y como pide `docs/DESIGN.md` 6.4 y 7.6, restilizando solo lo que la pantalla ya tiene, y dejar escritas las diferencias entre el diseño y la app.

**Architecture:** Un widget nuevo, `LabeledField` (etiqueta en mayúsculas encima, semántica original y foco al tocarla), que `PasswordField` y los campos del formulario comparten. `TagsField` conserva sus `InputChip` con el estilo de insignia. Las secciones cerradas, los avisos, el error general, el botón de guardar y la confirmación de descartar se restilizan dentro del árbol actual. La lógica, los flujos, los `Key`s y las cadenas no se tocan.

**Tech Stack:** Flutter (Material 3), `flutter_test`. Sin dependencias nuevas.

**Spec:** `docs/superpowers/specs/2026-11-19-formulario-entrada-design.md`. Valores visuales: `docs/DESIGN.md` 2, 3, 4, 6.4, 6.8, 6.11, 7.6 y 12, y el board de Penpot *Formulario de entrada* (página *Android*).

## Global Constraints

- **Regla del usuario:** se trabaja sobre lo ya desarrollado y no se cambia nada más. No se agrega ni se mueve nada aunque el diseño lo muestre: ni "Eliminar entrada", ni la etiqueta "ETIQUETAS" sobre el grupo de etiquetas, ni los puntos en la contraseña cerrada, ni el orden de campos del diseño, ni otros títulos o textos. Todo eso va a `docs/diferencias-diseno.md`.
- Sin dependencias nuevas en `pubspec.yaml`. La ✕ es `BovedaIcons.x`, que ya existe.
- No cambia la lógica, los flujos (guardar, descartar, cargar notas, abrir o cerrar la contraseña y las notas), las validaciones, el bloqueo, los `Key`s, los parámetros públicos ni las cadenas del código. Solo las etiquetas de campo y de insignia pasan a MAYÚSCULAS al dibujarse.
- Se conservan `ActivityTextField`, `TextFieldTapRegion` (envolviendo los botones que no deben cerrar el teclado), `keyboardDismissBehavior: onDrag`, `PopScope` y `copyable: false` en las notas y la contraseña (`mobile/CLAUDE.md`). Una contraseña o unas notas escritas nunca se vuelven copiables ni van a la semántica oculta, a un log o a un mensaje de error.
- Ningún `Color(0x…)`, `Colors.` ni `fontSize:` fuera de `mobile/lib/theme/` (`design_guard_test.dart`).
- Los widgets nuevos usan los valores estándar si el tema no trae la extensión (`BovedaText.standard`, `AccentColors.of(defaultAccent)`), porque muchos tests montan la pantalla en un `MaterialApp` sin `buildTheme`.
- Áreas táctiles de 48 dp o más; con `isLargeText(context)` (texto del sistema por encima del 110 %) las filas de texto y botón pasan a apilarse.
- Un test existente solo se ajusta si el cambio visual deliberado lo exige (el texto de una etiqueta o de un `InputChip` pasa a mayúsculas); cada ajuste se anota en el ledger y en el PR.
- Identificadores en inglés; comentarios, documentación y commits en español, Conventional Commits.
- No usar el emulador. Comandos de Flutter desde `mobile/`; Rust y git desde la raíz.
- Los tests de presentación montan `MaterialApp(theme: buildTheme(accent))`.

## Review Focus

- Foco y teclado intactos: tocar una etiqueta enfoca su campo; "Cambiar", "Editar notas", "Cancelar" y la ✕ de las etiquetas no cierran el teclado; el campo de etiquetas agrega con Enter o coma y conserva el foco (Tasks 3 y 4; los tests existentes lo cubren).
- Secretos: las notas y la contraseña siguen sin poder copiarse desde el formulario y su valor no aparece en la semántica (Tasks 2 y 4).
- Con texto al 200 % a 360 dp, y con el teclado abierto en una pantalla baja, el formulario completo (crear, editar con secciones cerradas, reparar con aviso, con errores) no desborda y se desplaza (Tasks 4 y 5).
- Accesibilidad: cada campo anuncia su etiqueta original y no en mayúsculas; la ✕ de cada etiqueta mantiene el tooltip "Quitar etiqueta" y 48 dp de área táctil; los botones secundarios, 48 dp (Tasks 2, 3 y 4).
- `PasswordField` conserva su comportamiento tras usar `LabeledField`: sus tests y los de las pantallas que lo usan (desbloqueo, primer uso, importar y este formulario) pasan sin cambios (Task 2).

---

### Task 1: Rama y documentos

- [ ] **Step 1: Crear la rama desde `dev`.** `git fetch origin && git switch -c feat/formulario-entrada origin/dev` (sin número de issue; si el usuario da uno, `feat/<n>-formulario-entrada`). El diseño y este plan, sin seguimiento, viajan con el cambio de rama.
- [ ] **Step 2: Commit.** `git add docs/superpowers/specs/2026-11-19-formulario-entrada-design.md docs/superpowers/plans/2026-11-19-formulario-entrada.md && git commit -m "docs: diseño y plan del formulario de entrada"`

### Task 2: `LabeledField` y `PasswordField`

**Files:**
- Create: `mobile/lib/widgets/labeled_field.dart`, `mobile/test/widgets/labeled_field_test.dart`
- Modify: `mobile/lib/widgets/password_field.dart`

**Interfaces:**
- Produces: `LabeledField({super.key, required String label, bool enabled = true, required Widget child})`: una `Column` de ancho completo con la etiqueta (`FieldLabel(label, enabled: enabled)`, excluida de la semántica) encima, `Space.sm` de separación y [child] debajo dentro de `Semantics(label: label)` y de un `Focus` transparente (sin tomar el foco ni entrar al recorrido). Tocar la etiqueta, solo con `enabled`, enfoca el primer nodo enfocable de [child]; no obliga a pasarle un `FocusNode` al campo.
- `PasswordField` conserva su API pública y construye su campo dentro de un `LabeledField`.

- [ ] **Step 1: Escribir los tests (fallan)** en `labeled_field_test.dart`, con `buildTheme` y un `ActivityTextField` o `TextField` como hijo: la etiqueta va encima, en mayúsculas, en `meta` y `textSecondary`, y el texto original no se dibuja; el nodo de semántica del campo anuncia la etiqueta original (`getSemantics(find.byType(TextField)).label` la contiene) y `bySemanticsLabel` no encuentra la versión en mayúsculas; tocar la etiqueta enfoca el campo, también cuando el hijo trae su propio `FocusNode`; con `enabled: false` la etiqueta va en `textDisabled` y tocarla no enfoca; el campo conserva su `focusNode` por omisión (`TextField.focusNode` nulo si no se pasó); sin las extensiones del tema (`MaterialApp` sin `buildTheme`) no falla.
- [ ] **Step 2: Ver que fallan.** Run: `flutter test test/widgets/labeled_field_test.dart`. Expected: FAIL (no compila).
- [ ] **Step 3: Implementar `LabeledField`** moviendo ahí la mecánica que hoy está dentro de `PasswordField` (etiqueta con `FieldLabel`, `Semantics`, el `Focus` transparente y el foco al tocar la etiqueta).
- [ ] **Step 4: Hacer que `PasswordField` use `LabeledField`**, sin cambiar su API, sus parámetros ni su comportamiento.
- [ ] **Step 5: Ver que pasan.** Run: `flutter test`. Expected: PASS, incluidos todos los tests existentes de `PasswordField` (`widgets_test.dart`, `restyled_widgets_test.dart`) y de las pantallas que lo usan, sin modificarlos.
- [ ] **Step 6: Commit.** `git add mobile/lib/widgets mobile/test/widgets && git commit -m "feat: agregar LabeledField y reutilizarlo en el campo de contraseña"`

### Task 3: Etiquetas de la entrada (`TagsField`)

**Files:**
- Modify: `mobile/lib/widgets/tags_field.dart`, `mobile/test/tags_field_test.dart` (ajustes), `mobile/test/entry_form_screen_test.dart` (ajustes) y `mobile/test/widgets/tags_field_design_test.dart` (nuevo)

**Interfaces:**
- Consumes: `LabeledField` (Task 2), `BovedaIcons.x`, `AccentColors`, `BovedaText.meta`.
- `TagsField` conserva su API pública. Cada etiqueta sigue siendo un `InputChip` (mismo tooltip "Quitar etiqueta", mismo `onDeleted`) con: texto en mayúsculas en `meta`, peso 700 y color `accent`; fondo `accentDim`; borde `accentBorder`; forma de radio `Radii.tag`; y `BovedaIcons.x` en `accent` como ícono de quitar. El campo "Agregar etiqueta" va en un `LabeledField` y conserva su ayuda "Enter o coma para agregar", su error y su comportamiento (Enter o coma agregan, sin quitar el foco).

- [ ] **Step 1: Escribir los tests nuevos (fallan)** en `tags_field_design_test.dart`, con `buildTheme` y los cuatro acentos: cada etiqueta es un `InputChip` con su texto en mayúsculas, color `accent`, fuente JetBrains Mono, fondo `accentDim`, borde `accentBorder` y forma de radio 6; el ícono de quitar es `BovedaIcons.x` con el tooltip "Quitar etiqueta" y quita la etiqueta al tocarlo; deshabilitado, el chip no se puede quitar; la etiqueta del campo va encima como `AGREGAR ETIQUETA` y se lee como "Agregar etiqueta"; la ayuda "Enter o coma para agregar" y el error se muestran; Enter y la coma siguen agregando sin perder el foco; con cinco etiquetas largas a 360 dp y 200 % no desborda; el área táctil de la ✕ es de 48 dp o más.
- [ ] **Step 2: Ajustar los tests existentes por el cambio visual.** `tags_field_test.dart`: `find.text('Agregar etiqueta')` pasa a `'AGREGAR ETIQUETA'`. `entry_form_screen_test.dart`: los `find.widgetWithText(InputChip, 'correo')` y `'personal'` pasan a `'CORREO'` y `'PERSONAL'` (también el que toca el chip para quitarlo). Anotar cada ajuste.
- [ ] **Step 3: Ver que fallan.** Run: `flutter test test/widgets/tags_field_design_test.dart test/tags_field_test.dart test/entry_form_screen_test.dart`. Expected: FAIL solo los casos nuevos y los ajustados.
- [ ] **Step 4: Implementar** el estilo de los `InputChip` y el `LabeledField` del campo, sin tocar `withPending`, `_commit` ni la lógica de agregar y quitar.
- [ ] **Step 5: Ver que pasa todo.** Run: `flutter test`. Expected: PASS.
- [ ] **Step 6: Commit.** `git add mobile/lib/widgets/tags_field.dart mobile/test && git commit -m "feat: restilizar las etiquetas de la entrada"`

### Task 4: Formulario: campos y secciones del secreto

**Files:**
- Modify: `mobile/lib/screens/entry_form_screen.dart` (`_textField`, `_closedSection`, `_passwordSection`, `_notesSection` y `_cancelButton`), `mobile/test/entry_form_screen_test.dart` si un `find` lo exige y `mobile/test/entry_form_screen_design_test.dart` (nuevo)

**Interfaces:**
- Consumes: `LabeledField`, `secondaryActionStyle`, `BovedaIcons.triangleAlert`, `isLargeText`.

- [ ] **Step 1: Escribir los tests nuevos (fallan)** en `entry_form_screen_design_test.dart` (montar el formulario como lo hacen los tests existentes de `entry_form_screen_test.dart`), con `buildTheme` y los cuatro acentos donde aplique: Servicio, Usuario, URL y Notas (abierta) tienen su etiqueta encima en mayúsculas y se leen con el texto original; los campos conservan sus `Key`s (`service`, `username`, `url`, `notes`) y siguen siendo `ActivityTextField`; no queda ninguna `labelText` flotante en esos campos; las notas siguen sin poder copiarse (`copyable: false`); editando, las secciones cerradas son una tarjeta `surface1` con borde `outline` y radio 12 con el texto exacto "Contraseña: sin cambios" o "Notas: sin cambios" y el botón "Cambiar" o "Editar notas" como `TextButton` con fondo `surfaceElevated`; abrirlas sigue abriendo el campo; "Cancelar" sigue cerrándolas; el error "No se pudieron cargar las notas. Intenta de nuevo." lleva ícono de alerta y color `error`; los botones de las secciones siguen sin cerrar el teclado; a 360 dp y 200 % las secciones cerradas apilan el botón debajo del texto y nada desborda; con el teclado abierto en una pantalla baja (360 × 400 con 200 dp de teclado) el formulario se desplaza sin desbordar.
- [ ] **Step 2: Ver que fallan.** Run: `flutter test test/entry_form_screen_design_test.dart`. Expected: FAIL.
- [ ] **Step 3: Implementar** los helpers con los mismos textos, `Key`s y condiciones de hoy. No tocar estado ni flujos.
- [ ] **Step 4: Ver que pasa todo.** Run: `flutter test`. Expected: PASS; si un test existente falla solo por la presentación, ajustar su `find` y anotarlo.
- [ ] **Step 5: Commit.** `git add mobile/lib/screens/entry_form_screen.dart mobile/test && git commit -m "feat: restilizar los campos y las secciones del secreto del formulario"`

### Task 5: Formulario: avisos, errores, guardar y descartar

**Files:**
- Modify: `mobile/lib/screens/entry_form_screen.dart` (`build` y `_confirmDiscard`), `mobile/test/entry_form_screen_design_test.dart` (casos nuevos), `mobile/test/entry_form_screen_test.dart` si un `find` lo exige

- [ ] **Step 1: Escribir los tests (fallan)**: reparando, el aviso es una tarjeta `errorContainer` con borde `error` al 38 %, radio 16 y `BovedaIcons.triangleAlert` en `error`, con el texto exacto "La contraseña y las notas de esta entrada están dañadas. Escribe una contraseña nueva para repararla."; el error general "No se pudo guardar. Intenta de nuevo." (provocado con un guardado que falla) lleva ícono de alerta y color `error`; el botón de guardar conserva `Key('save')`, es un `FilledButton` con "Guardar" y, mientras guarda, "Guardando…" y desactivado, dentro de `PrimaryGlow` cuyo `enabled` sigue al botón; la confirmación de descartar (con cambios, al volver) muestra "¿Descartar cambios?", "Seguir editando" sin estilo propio y "Descartar" con texto `error`, y cada una hace lo de siempre; los títulos "Nueva entrada", "Editar entrada" y "Reparar entrada" no cambian; a 360 dp y 200 % con el aviso de reparación y el error visibles no desborda.
- [ ] **Step 2: Ver que fallan.** Run: `flutter test test/entry_form_screen_design_test.dart`. Expected: FAIL solo los casos nuevos.
- [ ] **Step 3: Implementar** los cambios de `build` (aviso en tarjeta con ícono de Lucide, error con ícono, `PrimaryGlow` alrededor del botón de guardar, que sigue dentro del `TextFieldTapRegion`) y el estilo del botón "Descartar", con los mismos textos, `Key`s y condiciones. No tocar el estado ni los flujos de guardar y descartar.
- [ ] **Step 4: Ver que pasa todo.** Run: `flutter test`. Expected: PASS.
- [ ] **Step 5: Commit.** `git add mobile/lib/screens/entry_form_screen.dart mobile/test && git commit -m "feat: restilizar los avisos, los errores y el guardado del formulario"`

### Task 6: Documento de diferencias

**Files:**
- Modify: `docs/diferencias-diseno.md`

- [ ] **Step 1: Agregar la sección "Formulario de crear y editar"** (y quitar al formulario de la lista de pantallas sin restilizar), con una tabla de **Diseño**, **App hoy** y **Qué haría falta**. Contenido, verificado contra el código y el board:
  - Botón "Eliminar entrada" (texto rojo bajo "Guardar", `DESIGN.md` 7.6) frente a la app: eliminar solo está en el detalle; habría que mover o duplicar la acción.
  - Etiqueta "ETIQUETAS" sobre el grupo de etiquetas, con las insignias y el campo dentro de un mismo contenedor; la app tiene las etiquetas y un campo "Agregar etiqueta" aparte, sin etiqueta de grupo.
  - Contraseña cerrada con los puntos `••••••••••••` y botón "Cambiar" dentro de un campo; la app muestra el texto "Contraseña: sin cambios".
  - Notas cerradas con la pista "Sin notas" y "Editar notas"; la app muestra "Notas: sin cambios" (y no puede saber si hay notas sin descifrarlas).
  - Orden de los campos: Servicio, Usuario, Contraseña, URL, Etiquetas, Notas (`DESIGN.md` 7.6 y board); la app: Servicio, Usuario, URL, Etiquetas, Contraseña, Notas.
  - Los obligatorios dicen "Obligatorio" en el texto de ayuda (`DESIGN.md` 7.6); la app no valida vacíos ni límites en el formulario (lo hace el núcleo y el error se muestra en su campo).
  - Botón del generador junto a la contraseña que abre `EntropyGenerator` en una hoja (etapa 4).
  - Aviso de reparación: `DESIGN.md` no lo describe.
- [ ] **Step 2: Comprobar** que cada punto es verdadero contra el código y el board; ninguno puede describir algo que la app ya hace. Commit: `docs: registrar las diferencias del formulario`.

### Task 7: Guardia, documentación y verificación

**Files:**
- Modify: `mobile/test/theme/design_guard_test.dart`, `docs/DESIGN.md` y `mobile/CLAUDE.md` solo si hace falta

- [ ] **Step 1: Ampliar la guardia.** En `migrated files use Lucide icons, not Material Icons`, sumar `lib/screens/entry_form_screen.dart`, `lib/widgets/tags_field.dart` y `lib/widgets/labeled_field.dart`. Run: `flutter test test/theme/design_guard_test.dart`. Expected: PASS. Comprobar con una mutación (agregar `// Icons.add` a cada uno y ver que falla; revertir con `git checkout`).
- [ ] **Step 2: Documentación.** Si `DESIGN.md` 7.6 dice algo que la implementación no hace, no se cambia la implementación: se agrega una frase que remita a `docs/diferencias-diseno.md`. Commit: `docs: remitir las diferencias del formulario al documento de diferencias`.
- [ ] **Step 3: Verificación completa.** Desde `mobile/`: `flutter test` y `flutter analyze`; desde la raíz: `cargo test --workspace` y `cargo clippy --workspace --all-targets -- -D warnings`, cada uno con su código de salida visible. Expected: todo termina en 0.
- [ ] **Step 4: Preguntar al usuario si quiere probarlo en su teléfono** (compilar el APK de depuración e instalarlo con `adb install -r`, ver la nota `instalar-en-el-telefono`).

### Task 8: Publicar

- [ ] **Step 1: Pedir confirmación al usuario** y publicar con `git push -u origin feat/formulario-entrada`.
- [ ] **Step 2: Abrir el PR contra `dev`** con `gh pr create --base dev`, título `feat: restilizar el formulario de entrada` y el formato del PR #84 (Resumen, Decisiones de implementación, Lo que el diseño pide y el código no tiene —que remite a `docs/diferencias-diseno.md`—, Tests existentes ajustados, Correcciones de la revisión, Criterios de aceptación con casillas y Verificación local). No esperar la CI; dejar su casilla sin marcar.
