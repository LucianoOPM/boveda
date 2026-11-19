# Pantallas de arranque Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Que el desbloqueo, el primer uso y la oferta de huella se vean como sus boards de Penpot y como pide `docs/DESIGN.md`, sin cambiar su comportamiento.

**Architecture:** Íconos de Lucide como fuente (`lucide.ttf`) con una clase de constantes. Tres widgets compartidos nuevos (cápsula de estado, resplandor del botón principal, etiqueta de campo) y dos widgets existentes restilizados (`PasswordField`, `StrengthIndicator`). Cada pantalla se restiliza dentro de su árbol actual: mismos `Key`s, mismos widgets de control (`FilledButton`, `CheckboxListTile`, `TextField`) y mismas cadenas, porque los tests existentes los buscan así.

**Tech Stack:** Flutter (Material 3), `flutter_test`. Sin dependencias nuevas.

**Spec:** `docs/superpowers/specs/2026-11-19-pantallas-arranque-design.md`. Valores visuales: `docs/DESIGN.md` 2 a 7 y los boards de Penpot *Desbloqueo*, *Primer uso* y *Oferta de huella* (página *Android*).

## Global Constraints

- Sin dependencias nuevas en `pubspec.yaml`. `lucide.ttf` es un archivo del repositorio; la fuente se descarga solo del release oficial `lucide-icons/lucide` (`lucide-font-1.76.0.zip`), a un directorio nuevo del scratchpad, con el permiso que el usuario ya dio, y se borra al terminar la Task 2.
- No cambia lógica, flujos, `Key`s, parámetros públicos ni cadenas del código. La casilla "Entiendo que…", "Restaurar un respaldo", "¿Activar huella?" y "Activar" se quedan. Las cadenas que un test busca (por ejemplo "Demasiado corta", "Usar huella", "Contraseña incorrecta") se muestran exactas, sin pasar a mayúsculas; solo las etiquetas nuevas de campo van con `toUpperCase()`.
- Ningún `Color(0x…)`, `Colors.` ni `fontSize:` fuera de `mobile/lib/theme/` (`design_guard_test.dart`). Los colores nuevos de `DESIGN.md` (cápsula) entran a `lib/theme/colors.dart`.
- Se conservan `ActivityTextField`, `TextFieldTapRegion` (envolviendo el botón principal y las casillas), `copyable: false`, `Text` (nunca `SelectableText`) y el foco que maneja cada pantalla.
- Un solo botón principal por pantalla; áreas táctiles de 48 dp o más; `Semantics` en la cápsula y etiqueta accesible en cada campo.
- Un test existente solo se ajusta si el cambio visual deliberado lo exige (por ejemplo, el `find` de un texto que pasó a mayúsculas); cada ajuste se anota en el ledger y en el PR.
- Identificadores en inglés; comentarios, documentación y commits en español, Conventional Commits.
- No usar el emulador sin avisar antes. Comandos de Flutter desde `mobile/`; Rust y git desde la raíz.
- Los tests que prueban pantallas con el tema nuevo montan `MaterialApp(theme: buildTheme(accent))`, no el tema por omisión.

## Review Focus

- Texto del sistema al 200 % a 360 dp de ancho: las tres pantallas no desbordan (Tasks 5 a 7).
- El spinner dentro del botón principal se ve sobre el fondo deshabilitado con los cuatro acentos, contraste de 3:1 o más (Task 5).
- Primer uso a 360 × 640 dp con el teclado cerrado: el botón "Crear bóveda" se ve sin desplazarse (`DESIGN.md` 7.1). Si no cabe, se anota en el PR en vez de recortar el espaciado (Task 6).
- Ningún `Icons.` en los archivos ya migrados, y cada código de `BovedaIcons` existe en `lucide.ttf` (Tasks 2 y 8).
- La cápsula y el campo de contraseña se anuncian: "Bóveda bloqueada" o la etiqueta del campo, no un texto en mayúsculas suelto (Tasks 3, 4 y 5).

---

### Task 1: Rama y documentos

- [ ] **Step 1: Crear la rama desde `dev`.** `git fetch origin && git switch -c feat/pantallas-arranque origin/dev` (sin número de issue; si el usuario da uno, `feat/<n>-pantallas-arranque`). El diseño y este plan, sin seguimiento, viajan con el cambio de rama.
- [ ] **Step 2: Commit.** `git add docs/superpowers/specs/2026-11-19-pantallas-arranque-design.md docs/superpowers/plans/2026-11-19-pantallas-arranque.md && git commit -m "docs: diseño y plan de las pantallas de arranque"`

### Task 2: Íconos de Lucide

**Files:**
- Create: `mobile/assets/fonts/lucide.ttf`, `mobile/assets/fonts/LICENSE-Lucide.txt`, `mobile/lib/theme/icons.dart`, `mobile/test/theme/icons_test.dart`
- Modify: `mobile/pubspec.yaml`, `mobile/lib/theme/licenses.dart`, `mobile/test/theme/fonts_test.dart`

**Interfaces:**
- Produces: `abstract final class BovedaIcons` con `static const IconData lock`, `eye`, `eyeOff`, `fingerprint` y `triangleAlert`, todos `IconData(<código>, fontFamily: 'Lucide')`.

- [ ] **Step 1: Descargar y extraer.** Del release `lucide-icons/lucide` (`gh api repos/lucide-icons/lucide/releases/latest`), bajar `lucide-font-<versión>.zip` a un directorio nuevo y vacío del scratchpad y descomprimirlo ahí; no ejecutar nada de lo descargado. Copiar a `mobile/assets/fonts/` solo `lucide.ttf` y el texto de la licencia como `LICENSE-Lucide.txt`. Anotar versión y origen para el PR.
- [ ] **Step 2: Obtener los códigos.** Leer del `info.json` del zip el código de `lock`, `eye`, `eye-off`, `fingerprint-pattern` (o el nombre vigente de la huella) y `triangle-alert`. Con un script de Node en el scratchpad (no en el repo), comprobar que cada código existe en la tabla `cmap` de `lucide.ttf`; registrar el resultado en el ledger.
- [ ] **Step 3: Escribir los tests (fallan).** `icons_test.dart`: `BovedaIcons.lock`, `eye`, `eyeOff`, `fingerprint` y `triangleAlert` tienen `fontFamily == 'Lucide'`, `codePoint` mayor que 0 y los cinco son distintos entre sí. En `fonts_test.dart`, `declared font assets exist` pasa de 6 a 7 archivos, `font families` suma `Lucide` y `font licenses are registered` exige también el paquete `Lucide` con texto no vacío.
- [ ] **Step 4: Ver que fallan.** Run: `flutter test test/theme/icons_test.dart test/theme/fonts_test.dart`. Expected: FAIL (no compila o los conteos no coinciden).
- [ ] **Step 5: Implementar.** `pubspec.yaml`: familia `Lucide` con `assets/fonts/lucide.ttf` y la licencia nueva bajo `assets:`. `icons.dart`: `BovedaIcons` con los cinco códigos del Step 2 y un comentario de una línea con el origen de la fuente. `registerFontLicenses` agrega `'Lucide': 'assets/fonts/LICENSE-Lucide.txt'`.
- [ ] **Step 6: Ver que pasan.** Run: los dos archivos y luego `flutter test`. Expected: PASS.
- [ ] **Step 7: Borrar las descargas** del scratchpad (zip y carpeta extraída).
- [ ] **Step 8: Commit.** `git add mobile/assets mobile/pubspec.yaml mobile/lib/theme mobile/test/theme && git commit -m "feat: agregar los íconos de Lucide como fuente"`

### Task 3: Widgets compartidos

**Files:**
- Create: `mobile/lib/widgets/status_capsule.dart`, `primary_glow.dart`, `field_label.dart`; `mobile/test/widgets/shared_widgets_test.dart`
- Modify: `mobile/lib/theme/colors.dart`

**Interfaces:**
- Produces:
  - `BovedaColors.capsuleBg = #000000` y `BovedaColors.capsuleBorder = #24242F` (`DESIGN.md` 6.1).
  - `class StatusCapsule extends StatelessWidget { const StatusCapsule({super.key, required this.text, this.locked = false, this.semanticsLabel}); }`: píldora `capsuleBg` con borde `capsuleBorder`, LED de 6 px y `text` en `meta`. Abierta: LED y texto en `accent`. `locked`: LED `errorStrong` fijo y texto `error`. `semanticsLabel` reemplaza al texto para el lector de pantalla.
  - `class PrimaryGlow extends StatelessWidget { const PrimaryGlow({super.key, required this.enabled, required this.child}); }`: `DecoratedBox` de radio `Radii.control` con `BoxShadow(color: accentGlow, blurRadius: 25)` solo con `enabled`; sin sombra si no. No cambia el tamaño del hijo.
  - `class FieldLabel extends StatelessWidget { const FieldLabel(this.text, {super.key}); }`: `text.toUpperCase()` en `meta` y `textSecondary`.

- [ ] **Step 1: Escribir los tests (fallan)** en `shared_widgets_test.dart`, montando con `MaterialApp(theme: buildTheme(accent))` y recorriendo los cuatro acentos: `StatusCapsule` muestra su texto; el LED es del color de `accent` (abierta) o `errorStrong` (bloqueada); con `semanticsLabel` el árbol semántico anuncia ese texto; `accent`/`capsuleBg` y `error`/`capsuleBg` dan 4.5:1 o más. `PrimaryGlow` con `enabled: true` tiene una sombra con `accentGlow` y blur 25, y con `false` ninguna; `FieldLabel('Contraseña maestra')` muestra `CONTRASEÑA MAESTRA`.
- [ ] **Step 2: Ver que fallan.** Run: `flutter test test/widgets/shared_widgets_test.dart`. Expected: FAIL (no compila).
- [ ] **Step 3: Implementar** los tres widgets y los dos colores.
- [ ] **Step 4: Ver que pasan.** Run: el archivo y luego `flutter test`. Expected: PASS.
- [ ] **Step 5: Commit.** `git add mobile/lib mobile/test && git commit -m "feat: agregar la cápsula de estado, el resplandor y la etiqueta de campo"`

### Task 4: `PasswordField` y `StrengthIndicator`

**Files:**
- Modify: `mobile/lib/widgets/password_field.dart`, `mobile/lib/widgets/strength_indicator.dart`, `mobile/test/widgets_test.dart` (casos nuevos al final del grupo de cada widget)

**Interfaces:**
- Consumes: `FieldLabel`, `BovedaIcons` (Tasks 2 y 3).
- Produces: `PasswordField` y `StrengthIndicator` con la misma API pública de hoy.

- [ ] **Step 1: Escribir los tests (fallan).** `PasswordField`: muestra `FieldLabel` con la etiqueta encima del campo; el campo usa el estilo `BovedaText.secret` (JetBrainsMono); el ojo es `BovedaIcons.eye` oculto y `BovedaIcons.eyeOff` revelado, con los tooltips de hoy; el árbol semántico del campo anuncia la etiqueta original (`'Contraseña maestra'`, no en mayúsculas); `errorText` se sigue mostrando; sigue sin poder copiar ni cortar. `StrengthIndicator`: tres segmentos de alto 4 y separación 4; `tooShort` pinta 1 en `errorStrong` y el texto en `error`, `acceptable` pinta 2 en `warning`, `strong` pinta 3 en `success`; los segmentos vacíos son `borderInteractive`; el texto sigue siendo exactamente "Demasiado corta", "Aceptable" o "Fuerte" en `meta`; el semántico anuncia "Fortaleza: <texto en minúsculas>".
- [ ] **Step 2: Ver que fallan.** Run: `flutter test test/widgets_test.dart`. Expected: FAIL solo los casos nuevos.
- [ ] **Step 3: Implementar `PasswordField`:** `Column` con `FieldLabel` y el `ActivityTextField` actual, sin `labelText`, con la etiqueta dada como etiqueta semántica del campo; texto en `secret`; ojo de `BovedaIcons`. Mismos parámetros, `Key`s y comportamiento.
- [ ] **Step 4: Implementar `StrengthIndicator`:** tres segmentos (`Container`s de alto 4 y radio `Radii.full`) en lugar de `LinearProgressIndicator`. Colores de `StatusColors` y del esquema; `Semantics` con la etiqueta.
- [ ] **Step 5: Ver que pasa todo.** Run: `flutter test`. Expected: PASS; si un test existente falla solo por la presentación, ajustar su `find` y anotarlo.
- [ ] **Step 6: Commit.** `git add mobile/lib/widgets mobile/test && git commit -m "feat: restilizar el campo de contraseña y el indicador de fortaleza"`

### Task 5: Desbloqueo

**Files:**
- Modify: `mobile/lib/screens/unlock_screen.dart` (solo `build` y sus ayudas), `mobile/test/unlock_screen_test.dart` (casos nuevos al final)

**Interfaces:**
- Consumes: `StatusCapsule`, `PrimaryGlow`, `BovedaIcons`, `PasswordField` (Tasks 2 a 4).

- [ ] **Step 1: Escribir los tests (fallan)**, montando la pantalla con `buildTheme`: fondo `BovedaColors.lockScrim`; caja de 72 × 72 con `BovedaIcons.lock`; `StatusCapsule` con `BLOQUEADA` y semántica "Bóveda bloqueada"; el error de contraseña se muestra con `BovedaIcons.triangleAlert` y el mismo texto de hoy; un `UnlockNotice` de error va en `errorContainer` y uno que no es de error en `warningContainer`, con el texto sin cambios; el aviso de huella desactivada va en `warningContainer`; el botón principal conserva `Key('unlock-submit')` y es un `FilledButton`, y `PrimaryGlow` está habilitado solo cuando `onPressed != null`; el botón de huella conserva `Key('unlock-biometric')`, mide 64 × 64, tiene tooltip "Usar huella" y el texto "Usar huella" debajo en `meta`; con "Desbloqueando…" el spinner de 16 contrasta 3:1 o más con el fondo deshabilitado compuesto, con los cuatro acentos; a 360 × 640 y `textScaler` 2.0 no hay desbordes.
- [ ] **Step 2: Ver que fallan.** Run: `flutter test test/unlock_screen_test.dart`. Expected: FAIL solo los casos nuevos.
- [ ] **Step 3: Implementar `build`** según el board *Desbloqueo*: contenido centrado y margen lateral de `Space.lg`; caja `iconBox` con borde `accentBorder` y candado en `accent`; título; cápsula; avisos y error con ícono dentro de contenedores; casilla de huella sin cambios de texto; botón en `PrimaryGlow` dentro del `TextFieldTapRegion` actual; botón de huella circular (`surfaceElevated`, borde `accentBorder`, huella en `accent`). No tocar la lógica ni los controladores.
- [ ] **Step 4: Ver que pasa todo.** Run: `flutter test`. Expected: PASS.
- [ ] **Step 5: Commit.** `git add mobile/lib/screens/unlock_screen.dart mobile/test && git commit -m "feat: restilizar la pantalla de desbloqueo"`

### Task 6: Primer uso

**Files:**
- Modify: `mobile/lib/screens/first_run_screen.dart` (solo `build`), `mobile/test/first_run_screen_test.dart` (casos nuevos al final)

- [ ] **Step 1: Escribir los tests (fallan)**, con `buildTheme`: `StatusCapsule` con `CONOCIMIENTO CERO` arriba; las etiquetas `CONTRASEÑA MAESTRA` y `CONFIRMA LA CONTRASEÑA` sobre sus campos; `CheckboxListTile` con `Key('first-run-accept')` y su texto exacto dentro de un contenedor `warningContainer` con `BovedaIcons.triangleAlert`; el error se muestra con ícono y el mismo texto; el botón conserva `Key('first-run-create')`, es un `FilledButton` y `PrimaryGlow` sigue a `onPressed != null`; "Restaurar un respaldo" sigue (`Key('first-run-restore')`); a 360 × 640 con el teclado cerrado, el botón "Crear bóveda" queda dentro de la pantalla sin desplazarse; a `textScaler` 2.0 no hay desbordes.
- [ ] **Step 2: Ver que fallan.** Run: `flutter test test/first_run_screen_test.dart`. Expected: FAIL solo los casos nuevos.
- [ ] **Step 3: Implementar `build`** según el board *Primer uso* y `DESIGN.md` 7.1, respetando el texto del párrafo y la casilla del código. Si el botón no cabe a 360 × 640, no recortar el espaciado: anotarlo como resultado en el ledger y en el PR, y limitar ese caso del test a lo que sí se cumple.
- [ ] **Step 4: Ver que pasa todo.** Run: `flutter test`. Expected: PASS.
- [ ] **Step 5: Commit.** `git add mobile/lib/screens/first_run_screen.dart mobile/test && git commit -m "feat: restilizar la pantalla de primer uso"`

### Task 7: Oferta de huella

**Files:**
- Modify: `mobile/lib/screens/biometric_offer_screen.dart` (solo `build`), `mobile/test/biometric_offer_screen_test.dart` (casos nuevos al final)

- [ ] **Step 1: Escribir los tests (fallan)**, con `buildTheme`: círculo de 64 en `accentDim` con `BovedaIcons.fingerprint` en `accent`; título y párrafo centrados con los textos de hoy; `FilledButton` con `Key('offer-enable')` y texto "Activar" y `TextButton` con `Key('offer-skip')` y texto "Ahora no", ambos al pie; el error se muestra con ícono; `PrimaryGlow` sigue a `onPressed != null`; a 360 × 640 y `textScaler` 2.0 no hay desbordes.
- [ ] **Step 2: Ver que fallan.** Run: `flutter test test/biometric_offer_screen_test.dart`. Expected: FAIL solo los casos nuevos.
- [ ] **Step 3: Implementar `build`** según el board *Oferta de huella*: contenido centrado y botones al pie, con un `CustomScrollView` y `SliverFillRemaining(hasScrollBody: false)` para que siga desplazándose con texto grande. "Ahora no" en `textSecondary`.
- [ ] **Step 4: Ver que pasa todo.** Run: `flutter test`. Expected: PASS.
- [ ] **Step 5: Commit.** `git add mobile/lib/screens/biometric_offer_screen.dart mobile/test && git commit -m "feat: restilizar la oferta de huella"`

### Task 8: Guardia, documentación y verificación

**Files:**
- Modify: `mobile/test/theme/design_guard_test.dart`, `docs/DESIGN.md` (5.1, línea de íconos), `mobile/CLAUDE.md`

- [ ] **Step 1: Ampliar la guardia (falla si queda un `Icons.`).** En `design_guard_test.dart`, un test `migrated files use Lucide icons` comprueba que `lib/screens/unlock_screen.dart`, `first_run_screen.dart`, `biometric_offer_screen.dart` y `lib/widgets/password_field.dart` no contienen `Icons.`. Run: `flutter test test/theme/design_guard_test.dart`. Expected: PASS (las Tasks 4 a 7 ya migraron).
- [ ] **Step 2: Documentación.** `DESIGN.md` 5.1: la línea de íconos pasa a decir que Lucide va como fuente empaquetada con `BovedaIcons` en `lib/theme/icons.dart` (sin paquete). `mobile/CLAUDE.md`: una línea, los íconos salen de `BovedaIcons`; las pantallas ya migradas no usan `Icons.*` y la guardia lo exige. Commit: `docs: documentar los íconos de Lucide`.
- [ ] **Step 3: Verificación completa.** Desde `mobile/`: `flutter test` y `flutter analyze`; desde la raíz: `cargo test --workspace` y `cargo clippy --workspace --all-targets -- -D warnings`, cada uno con su código de salida visible. Expected: todo termina en 0.
- [ ] **Step 4: Preguntar al usuario si quiere la revisión visual en el emulador** (avisar antes, no compilar a la vez, cerrar emulador, adb y Gradle al terminar). Si no, queda anotado como no verificado.

### Task 9: Publicar

- [ ] **Step 1: Pedir confirmación al usuario** y publicar con `git push -u origin feat/pantallas-arranque`.
- [ ] **Step 2: Abrir el PR contra `dev`** con `gh pr create --base dev`, título `feat: restilizar el desbloqueo, el primer uso y la oferta de huella` y el formato del PR #81 (Resumen, Decisiones de implementación, Criterios de aceptación con casillas y Verificación local). Incluir una sección **Lo que el diseño pide y el código no tiene**: la casilla de aceptación frente al aviso, "Restaurar un respaldo", los textos que difieren (oferta, fortaleza, párrafo del primer uso, casilla y botón de huella), el pulso del LED y, si aplica, que el botón de primer uso no cabe a 360 × 640. Anotar los tests existentes ajustados y la versión y el origen de `lucide.ttf`. No esperar la CI; dejar su casilla sin marcar.
