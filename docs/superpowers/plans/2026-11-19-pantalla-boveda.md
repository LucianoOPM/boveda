# Pantalla de Bóveda Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Que la pantalla principal (encabezado, búsqueda, tarjetas, aviso de dañadas, botón "+" y estados vacíos) se vea como sus boards de Penpot y como pide `docs/DESIGN.md`, sin cambiar su comportamiento.

**Architecture:** Tres widgets nuevos y reutilizables en `lib/widgets/` (`TagBadge`, `VaultCard`, `VaultHeader`) más el restilizado de `DamagedEntriesBanner` y del `build` de `SearchScreen`. Los íconos de Lucide ya empaquetados ganan cinco constantes. La lógica de búsqueda, las rutas, los `Key`s y las cadenas del código no se tocan.

**Tech Stack:** Flutter (Material 3), `flutter_test`. Sin dependencias nuevas.

**Spec:** `docs/superpowers/specs/2026-11-19-pantalla-boveda-design.md`. Valores visuales: `docs/DESIGN.md` 2 a 4, 6.1, 6.2, 6.6 a 6.8, 7.4, 9, 10 y 12, y los boards de Penpot *Bóveda*, *Bóveda · sin resultados*, *Bóveda · vacía* y *Componentes* (página *Android*).

## Global Constraints

- Sin dependencias nuevas en `pubspec.yaml`. Los íconos nuevos se leen del `lucide.ttf` que ya está en el repositorio; no se descarga nada.
- No cambia la búsqueda, el orden de la lista, las rutas, los `Key`s, los parámetros públicos ni las cadenas del código. "Buscar", "Sin resultados para «x»", "Todavía no hay contraseñas guardadas. Toca + para agregar la primera.", "No se pudo buscar", "Reintentar", "N entradas no se pudieron leer", "Entendido", "Agregar contraseña" y "Borrar búsqueda" se muestran exactos. Solo las etiquetas de las insignias y el texto de `meta` nuevos van en MAYÚSCULAS, con `toUpperCase()` al dibujarlos.
- Ningún `Color(0x…)`, `Colors.` ni `fontSize:` fuera de `mobile/lib/theme/` (`design_guard_test.dart`). Los colores nuevos de `DESIGN.md` entran a `lib/theme/colors.dart`.
- La tarjeta solo usa servicio, usuario, URL y etiquetas (los datos que `EntrySummary` ya trae); el texto buscado nunca va a la semántica ni a un log.
- Se conservan `ActivityTextField`, el foco inicial (el campo no se enfoca al llegar), `keyboardDismissBehavior: onDrag`, el `Navigator` raíz y el aviso de dañadas dentro del árbol de la pantalla.
- Los widgets nuevos usan los valores estándar si el tema no trae la extensión (`BovedaText.standard`, `StatusColors.standard`, `AccentColors.of(defaultAccent)`), porque muchos tests montan la pantalla en un `MaterialApp` sin `buildTheme`.
- Un solo botón principal por pantalla; áreas táctiles de 48 dp o más; con `MediaQuery.disableAnimations` no hay escala al presionar.
- Un test existente solo se ajusta si el cambio visual deliberado lo exige; cada ajuste se anota en el ledger y en el PR.
- Identificadores en inglés; comentarios, documentación y commits en español, Conventional Commits.
- No usar el emulador. Comandos de Flutter desde `mobile/`; Rust y git desde la raíz.
- Los tests que prueban la presentación montan `MaterialApp(theme: buildTheme(accent))`.

## Review Focus

- Texto del sistema al 200 % a 360 dp de ancho: encabezado, tarjeta con dos insignias y `+N`, aviso de dañadas y los tres estados no desbordan (Tasks 3 a 6).
- Datos extremos en la tarjeta: servicio largo sin espacios, emoji compuesto como inicial, cinco etiquetas y URL larga ni desbordan ni rompen la inicial (Task 3).
- Foco y teclado intactos: el campo no se enfoca al llegar ni al volver de otra ruta, y la lista sigue cerrando el teclado al arrastrar (Task 5; los tests existentes lo cubren).
- El engranaje y el "+" miden 48 dp o más de área táctil y tienen tooltip; la inicial de la tarjeta no se lee aparte del servicio y la cápsula se anuncia una vez (Tasks 3 y 5).
- El aviso de dañadas se oculta con "Entendido" y no vuelve hasta el siguiente desbloqueo (Task 4; el test existente lo cubre).

---

### Task 1: Rama y documentos

- [ ] **Step 1: Crear la rama desde `dev`.** `git fetch origin && git switch -c feat/pantalla-boveda origin/dev` (sin número de issue; si el usuario da uno, `feat/<n>-pantalla-boveda`). El diseño y este plan, sin seguimiento, viajan con el cambio de rama.
- [ ] **Step 2: Commit.** `git add docs/superpowers/specs/2026-11-19-pantalla-boveda-design.md docs/superpowers/plans/2026-11-19-pantalla-boveda.md && git commit -m "docs: diseño y plan de la pantalla de Bóveda"`

### Task 2: Íconos, token y base del botón "+"

**Files:**
- Modify: `mobile/lib/theme/icons.dart`, `mobile/lib/theme/colors.dart`, `mobile/lib/theme/spacing.dart`, `mobile/lib/theme/theme.dart`, `mobile/lib/widgets/primary_glow.dart`, `mobile/test/theme/icons_test.dart`, `mobile/test/theme/theme_test.dart`, `mobile/test/widgets/shared_widgets_test.dart`, `docs/DESIGN.md`

**Interfaces:**
- Produces:
  - `BovedaIcons.search` (0xe151), `x` (0xe1b2), `settings` (0xe154), `plus` (0xe13d) y `vault` (0xe58f), todos `IconData(<código>, fontFamily: 'Lucide')`.
  - `BovedaColors.borderPressed = #52525B`; `Radii.tag = 6`.
  - `PrimaryGlow({super.key, required this.enabled, this.radius = Radii.control, required this.child})`.
  - `buildTheme(accent).floatingActionButtonTheme`: fondo `accent`, texto e ícono `onAccent`, radio 16, `elevation` 0 en todos los estados, 56 × 56.

- [ ] **Step 1: Comprobar los códigos.** Con un script de Node en el scratchpad (no en el repo) que lea la tabla `cmap` y la tabla `post` de `mobile/assets/fonts/lucide.ttf`, comprobar que `search`, `x`, `settings`, `plus` y `vault` existen con esos códigos; registrar el resultado en el ledger.
- [ ] **Step 2: Escribir los tests (fallan).** `icons_test.dart`: los cinco nuevos tienen `fontFamily == 'Lucide'`, código mayor que 0 y son distintos de los cinco anteriores y entre sí (10 códigos únicos). `shared_widgets_test.dart`: `PrimaryGlow` con `radius: 16` redondea con 16 y, sin `radius`, con `Radii.control`. `theme_test.dart`, para los cuatro acentos: `floatingActionButtonTheme` con `backgroundColor == accent`, `foregroundColor == onAccent`, `elevation`, `focusElevation`, `hoverElevation` y `highlightElevation` en 0, y `shape` con radio 16.
- [ ] **Step 3: Ver que fallan.** Run: `flutter test test/theme test/widgets/shared_widgets_test.dart`. Expected: FAIL (no compila).
- [ ] **Step 4: Implementar** las constantes, `borderPressed`, `Radii.tag`, el radio de `PrimaryGlow` y el `floatingActionButtonTheme` en `buildTheme`.
- [ ] **Step 5: Documentar.** `DESIGN.md` 2.1: fila `borderPressed` (`#52525B`, "Borde de una tarjeta presionada (6.2)").
- [ ] **Step 6: Ver que pasan.** Run: `flutter test`. Expected: PASS.
- [ ] **Step 7: Commit.** `git add mobile/lib/theme mobile/lib/widgets/primary_glow.dart mobile/test docs/DESIGN.md && git commit -m "feat: agregar íconos, el borde presionado y el tema del botón +"`

### Task 3: Insignia, tarjeta y encabezado

**Files:**
- Create: `mobile/lib/widgets/tag_badge.dart`, `mobile/lib/widgets/vault_card.dart`, `mobile/lib/widgets/vault_header.dart`, `mobile/test/widgets/vault_widgets_test.dart`

**Interfaces:**
- Consumes: `BovedaIcons`, `BovedaColors.borderPressed`, `Radii.tag`, `StatusCapsule` (Task 2 y PR #82).
- Produces:
  - `TagBadge({super.key, required String label})`: `label.toUpperCase()` en `meta` (peso 700) y color `accent`, fondo `accentDim`, borde `accentBorder`, radio `Radii.tag`.
  - `VaultCard({super.key, required String service, required List<String> tags, String? detail, required VoidCallback onTap})`: la tarjeta de `DESIGN.md` 6.2. La inicial es el primer grafema de `service` en mayúscula (`'?'` si está vacío) dentro de una caja con `Key('vault-card-initial')` y excluida de la semántica. Servicio y `detail` en una línea con puntos suspensivos (`maxLines: 1`, `TextOverflow.ellipsis`); `detail` ausente no ocupa línea. Hasta dos `TagBadge`; si hay más, `+N` (N = el resto) en `meta` y `textSecondary`; sin etiquetas, sin fila. Presionada: borde `borderPressed` y `AnimatedScale` 0.98 en 100 ms, sin escala con `disableAnimations`. `Semantics(button: true, label: servicio o "servicio, detalle", onTapHint: 'abrir')`.
  - `VaultHeader({super.key, this.onOpenSettings})`: `BÓVEDA` en `brand` y debajo `XCHACHA20-POLY1305 • ARGON2ID` en `meta` y `textSecondary`; `StatusCapsule(text: 'CONOCIMIENTO CERO', semanticsLabel: 'Bóveda abierta')`; y, solo con `onOpenSettings`, el engranaje: cuadro visible de 40 × 40 (`surfaceElevated`, radio 12, `BovedaIcons.settings`) con área táctil de 48 × 48, `Key('open-settings')` y tooltip "Ajustes". El texto de la marca y del subtítulo se parte en dos líneas con letra grande; no desborda.

- [ ] **Step 1: Escribir los tests (fallan)** con `buildTheme` y los cuatro acentos donde aplique: `TagBadge` en mayúsculas, con `accentDim`, `accentBorder` y radio 6; `VaultCard` con inicial en `metric` y `accent` dentro de la caja de 40 × 40 (`iconBox`, radio 12), servicio, detalle y `onTap`; dos etiquetas muestran dos insignias y cinco muestran dos más `+3`; sin etiquetas no hay fila ni insignias; sin detalle no hay segunda línea de texto; presionada (con `tester.startGesture`) el borde es `borderPressed` y la escala 0.98, y con `disableAnimations` la escala sigue en 1; el árbol semántico anuncia `"Gmail, ana@gmail.com"`, es botón y lleva la pista "abrir", y la inicial no se lee aparte; datos extremos a 360 dp y `textScaler` 2.0 (servicio de 60 caracteres sin espacios, `'👩‍💻 Trabajo'` como inicial `👩‍💻`, cinco etiquetas, URL larga con una sola línea) sin desbordes. `VaultHeader`: textos y cápsula (anunciada como "Bóveda abierta", una sola vez), engranaje de 48 × 48 con tooltip y `Key`, que llama a `onOpenSettings`, y sin engranaje cuando es `null`; a 200 % a 360 dp no desborda.
- [ ] **Step 2: Ver que fallan.** Run: `flutter test test/widgets/vault_widgets_test.dart`. Expected: FAIL (no compila).
- [ ] **Step 3: Implementar los tres widgets** con las firmas de arriba. La tarjeta es un `Material` de `surface2` con borde y `InkWell`; `onHighlightChanged` lleva el estado de presionada.
- [ ] **Step 4: Ver que pasan.** Run: el archivo y luego `flutter test`. Expected: PASS.
- [ ] **Step 5: Commit.** `git add mobile/lib/widgets mobile/test && git commit -m "feat: agregar la insignia, la tarjeta de entrada y el encabezado"`

### Task 4: Aviso de entradas dañadas

**Files:**
- Modify: `mobile/lib/widgets/damaged_entries_banner.dart`, `mobile/test/widgets_test.dart` (casos nuevos), `mobile/test/search_screen_test.dart` si un `find` lo exige

**Interfaces:**
- Consumes: `BovedaIcons.triangleAlert` (PR #82).
- Produces: `DamagedEntriesBanner({super.key, required int count})` con la misma API.

- [ ] **Step 1: Escribir los tests (fallan)**, con `buildTheme`: fondo `errorContainer`, borde `error` al 38 %, radio 16, ícono `BovedaIcons.triangleAlert` en `error`; el texto exacto ("1 entrada no se pudo leer" y "N entradas no se pudieron leer") en `textPrimary`; "Entendido" sigue siendo un `TextButton` que lo oculta; con `count: 0` no muestra nada; margen lateral de 16; a 360 dp y 200 % no desborda.
- [ ] **Step 2: Ver que fallan.** Run: `flutter test test/widgets_test.dart`. Expected: FAIL solo los casos nuevos.
- [ ] **Step 3: Implementar** con un `Material` (no un `MaterialBanner`) de color `errorContainer` y el mismo estado `_dismissed`; no cambia su comportamiento ni su posición dentro del árbol de la pantalla.
- [ ] **Step 4: Ver que pasa todo.** Run: `flutter test`. Expected: PASS; si un test existente falla solo por la presentación (por ejemplo, busca `MaterialBanner`), ajustar su `find` y anotarlo.
- [ ] **Step 5: Commit.** `git add mobile/lib/widgets mobile/test && git commit -m "feat: restilizar el aviso de entradas dañadas"`

### Task 5: Pantalla: encabezado, búsqueda, lista y botón "+"

**Files:**
- Modify: `mobile/lib/screens/search_screen.dart` (`build`, `_body` de la lista y `_EntryTile`), `mobile/test/search_screen_test.dart` y `mobile/test/app_test.dart` (ajustes), `mobile/test/search_screen_design_test.dart` (nuevo)

**Interfaces:**
- Consumes: `VaultHeader`, `VaultCard`, `BovedaIcons`, `PrimaryGlow` con `radius` (Tasks 2 y 3).

- [ ] **Step 1: Escribir los tests nuevos (fallan)** en `search_screen_design_test.dart`, con `buildTheme` y los cuatro acentos donde aplique: el encabezado está arriba y el engranaje (`Key('open-settings')`) está dentro de él, no junto al campo, y sigue llamando a `onOpenSettings`; el campo conserva `hintText: 'Buscar'`, `BovedaIcons.search` de prefijo y, con texto, `BovedaIcons.x` con el tooltip "Borrar búsqueda"; la lista pinta una `VaultCard` por entrada, en el orden recibido, con las etiquetas de `EntrySummary.tags`, y separadas 10; el botón "+" conserva el tooltip "Agregar contraseña", mide 56 × 56, usa `BovedaIcons.plus` y está envuelto en `PrimaryGlow(radius: 16)`; el campo sigue sin enfocarse al llegar; a 360 dp y 200 % la pantalla con cinco entradas, etiquetas y aviso de dañadas no desborda.
- [ ] **Step 2: Ajustar los tests existentes por el cambio visual.** `search_screen_test.dart`: el caso de la tercera entrada deja de leer `ListTile.title` y `subtitle` (la entrada "C" sin detalle ahora se comprueba con la tercera `VaultCard`: su servicio es `'C'` y no tiene segunda línea de texto), y el de la inicial pasa de `CircleAvatar` a `Key('vault-card-initial')`. `app_test.dart`: los tres `find.widgetWithText(ListTile, 'Gmail')` pasan a `find.widgetWithText(VaultCard, 'Gmail')`. Anotar cada ajuste.
- [ ] **Step 3: Ver que fallan.** Run: `flutter test test/search_screen_design_test.dart`. Expected: FAIL.
- [ ] **Step 4: Implementar** el `build`: `VaultHeader` arriba (con `widget.onOpenSettings`), el campo de búsqueda con `ActivityTextField` y los mismos parámetros de hoy (solo cambian los íconos y el padding), `DamagedEntriesBanner`, la lista con `VaultCard` (`padding` lateral de 16, separación de 10, el espacio de 88 abajo) y el `FloatingActionButton` dentro de `PrimaryGlow(radius: Radii.card, enabled: true)`. Borrar `_EntryTile`: su inicial, `Key(ValueKey(entry.id))` y `onTap` pasan a la `VaultCard`. No tocar la lógica de búsqueda ni de rutas.
- [ ] **Step 5: Ver que pasa todo.** Run: `flutter test`. Expected: PASS.
- [ ] **Step 6: Commit.** `git add mobile/lib/screens/search_screen.dart mobile/test && git commit -m "feat: restilizar el encabezado, la búsqueda y la lista de la bóveda"`

### Task 6: Estados vacío, sin resultados y error

**Files:**
- Modify: `mobile/lib/screens/search_screen.dart` (solo `_body`), `mobile/test/search_screen_design_test.dart` (casos nuevos)

- [ ] **Step 1: Escribir los tests (fallan)**: vacío (sin texto buscado) con una caja de 72 × 72 (`iconBox`, radio 16) con `BovedaIcons.vault` en `accent`, el texto de hoy exacto y un `OutlinedButton` "Agregar la primera" que abre el formulario de crear (la misma ruta que el "+"); con texto buscado y sin resultados, `BovedaIcons.search` y el texto "Sin resultados para «x»" exacto, sin el botón; error de búsqueda con el texto "No se pudo buscar" y el botón "Reintentar" exactos y `BovedaIcons.triangleAlert`; los tres a 360 dp y 200 % sin desbordes y desplazables si no caben.
- [ ] **Step 2: Ver que fallan.** Run: `flutter test test/search_screen_design_test.dart`. Expected: FAIL solo los casos nuevos.
- [ ] **Step 3: Implementar** el estilo de cada estado en `_body`, con los textos y la lógica de hoy; el botón reutiliza `_openCreate`.
- [ ] **Step 4: Ver que pasa todo.** Run: `flutter test`. Expected: PASS.
- [ ] **Step 5: Commit.** `git add mobile/lib/screens/search_screen.dart mobile/test && git commit -m "feat: restilizar los estados vacío, sin resultados y de error"`

### Task 7: Guardia, documentación y verificación

**Files:**
- Modify: `mobile/test/theme/design_guard_test.dart`, `docs/DESIGN.md` (si hace falta), `mobile/CLAUDE.md` (si hace falta)

- [ ] **Step 1: Ampliar la guardia.** En `migrated files use Lucide icons, not Material Icons`, sumar `lib/screens/search_screen.dart`, `lib/widgets/damaged_entries_banner.dart`, `lib/widgets/vault_card.dart`, `lib/widgets/vault_header.dart` y `lib/widgets/tag_badge.dart`. Run: `flutter test test/theme/design_guard_test.dart`. Expected: PASS. Comprobar con una mutación (agregar `// Icons.add` a uno y ver que falla; revertir con `git checkout` del archivo) que la guardia los cubre.
- [ ] **Step 2: Documentación.** Revisar que `DESIGN.md` 6.2 mencione las insignias sin el ` // ` literal que el diseño de Penpot no usa (una frase) y que `mobile/CLAUDE.md` no necesite una regla nueva. Commit: `docs: ajustar el diseño de la tarjeta de entrada`.
- [ ] **Step 3: Verificación completa.** Desde `mobile/`: `flutter test` y `flutter analyze`; desde la raíz: `cargo test --workspace` y `cargo clippy --workspace --all-targets -- -D warnings`, cada uno con su código de salida visible. Expected: todo termina en 0.
- [ ] **Step 4: Preguntar al usuario si quiere probarlo en su teléfono** (compilar el APK de depuración e instalarlo con `adb install -r`, ver la nota `instalar-en-el-telefono`). El recorrido lo hace él.

### Task 8: Publicar

- [ ] **Step 1: Pedir confirmación al usuario** y publicar con `git push -u origin feat/pantalla-boveda`.
- [ ] **Step 2: Abrir el PR contra `dev`** con `gh pr create --base dev`, título `feat: restilizar la pantalla de Bóveda` y el formato del PR #82 (Resumen, Decisiones de implementación, Lo que el diseño pide y el código no tiene, Tests existentes ajustados, Criterios de aceptación con casillas y Verificación local). La sección de lo que el diseño pide y el código no tiene lista: los textos distintos (pista del campo, estado vacío, sin resultados y "Entendido"), la barra de navegación y "Enviar a PC" (etapa 3), deslizar para enviar (etapa 4) y el pulso del LED. Anotar los tests ajustados y las notas de la revisión. No esperar la CI; dejar su casilla sin marcar.
