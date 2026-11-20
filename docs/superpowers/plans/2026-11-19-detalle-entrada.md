# Detalle de entrada Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Que el detalle de una entrada se vea como sus boards de Penpot y como pide `docs/DESIGN.md` 6.3 y 7.5, restilizando solo lo que la pantalla ya tiene, y dejar escritas las diferencias entre el diseño y la app para convertirlas después en features.

**Architecture:** Un widget nuevo (`SecretText`, la contraseña revelada coloreada por tipo de carácter), un estilo de botón secundario compartido, tres íconos de Lucide y el ícono de volver por el tema. El `build` y los helpers de presentación de `EntryDetailScreen` se restilizan; su estado, sus flujos de revelar, copiar, editar y eliminar, sus `Key`s y sus cadenas no se tocan.

**Tech Stack:** Flutter (Material 3), `flutter_test`. Sin dependencias nuevas.

**Spec:** `docs/superpowers/specs/2026-11-19-detalle-entrada-design.md`. Valores visuales: `docs/DESIGN.md` 2, 3, 4, 6.3, 6.8, 6.11, 7.5 y 12, y los boards de Penpot *Detalle de entrada*, *Detalle · contraseña revelada* y *Detalle · secreto dañado* (página *Android*).

## Global Constraints

- **Regla del usuario:** se trabaja sobre lo ya desarrollado y no se cambia nada más. No se agrega ni se mueve nada aunque el diseño lo muestre: ni el título "Detalle" ni el bloque de cabecera (caja de inicial grande, servicio, dominio), ni el botón al pie "Copiar contraseña", ni "Sin notas" antes de revelar, ni los doce puntos (son ocho), ni el orden "Mostrar, Copiar" (es "Copiar, Mostrar"), ni mover "Eliminar" al formulario. Todo eso va a `docs/diferencias-diseno.md`.
- Sin dependencias nuevas en `pubspec.yaml`. Los íconos nuevos se leen del `lucide.ttf` que ya está en el repositorio.
- No cambia la lógica, los flujos (revelar, ocultar, copiar, editar, reparar, eliminar), el bloqueo, el borrado del portapapeles, los `Key`s, los parámetros públicos ni las cadenas del código. Solo las etiquetas de fila pasan a MAYÚSCULAS al dibujarse, con `FieldLabel`.
- Una contraseña o unas notas reveladas se muestran con `Text`, nunca con `SelectableText` ni en un campo; no se guardan fuera del estado de la pantalla ni van a la semántica oculta, a un log o a un mensaje de error (`mobile/CLAUDE.md`).
- Ningún `Color(0x…)`, `Colors.` ni `fontSize:` fuera de `mobile/lib/theme/` (`design_guard_test.dart`).
- Los widgets nuevos usan los valores estándar si el tema no trae la extensión (`BovedaText.standard`, `SyntaxColors.standard`), porque muchos tests montan la pantalla en un `MaterialApp` sin `buildTheme`.
- Áreas táctiles de 48 dp o más; con `isLargeText(context)` (texto del sistema por encima del 110 %) las acciones de una fila pasan debajo de su valor.
- Un test existente solo se ajusta si el cambio visual deliberado lo exige (por ejemplo, `Chip` pasa a `TagBadge` y las etiquetas de fila a mayúsculas); cada ajuste se anota en el ledger y en el PR.
- Identificadores en inglés; comentarios, documentación y commits en español, Conventional Commits.
- No usar el emulador. Comandos de Flutter desde `mobile/`; Rust y git desde la raíz.
- Los tests de presentación montan `MaterialApp(theme: buildTheme(accent))`.

## Review Focus

- La contraseña revelada es un `Text.rich` y no se puede seleccionar ni copiar: ningún `SelectableText` ni campo editable en el detalle, y el texto de `SecretText` es el de la contraseña salvo los espacios (`␣`) (Tasks 3 y 5).
- Contraseñas extremas: 120 caracteres sin espacios, con espacios, con letras acentuadas, `ñ`, emoji y los tres tipos mezclados ni desbordan ni pierden caracteres, a 360 dp y al 200 % (Task 3).
- La lógica del detalle quedó idéntica: solo cambian `build`, los helpers de presentación y los imports; los estados, los flujos y los `Key`s de `entry_detail_screen.dart` no (revisión de `git diff`) (Tasks 4 y 5).
- Con texto al 200 % a 360 dp las filas con acciones ("Copiar" más "Mostrar") no estrechan el valor: las acciones pasan debajo y nada desborda (Tasks 4 y 5).
- Accesibilidad: la contraseña oculta se anuncia "Contraseña oculta" y no los puntos; los botones de la barra y de las filas mantienen tooltip o texto y 48 dp de área táctil (Tasks 4 y 5).

---

### Task 1: Rama y documentos

- [ ] **Step 1: Crear la rama desde `dev`.** `git fetch origin && git switch -c feat/detalle-entrada origin/dev` (sin número de issue; si el usuario da uno, `feat/<n>-detalle-entrada`). El diseño y este plan, sin seguimiento, viajan con el cambio de rama.
- [ ] **Step 2: Commit.** `git add docs/superpowers/specs/2026-11-19-detalle-entrada-design.md docs/superpowers/plans/2026-11-19-detalle-entrada.md && git commit -m "docs: diseño y plan del detalle de entrada"`

### Task 2: Íconos, ícono de volver y estilo de botón secundario

**Files:**
- Modify: `mobile/lib/theme/icons.dart`, `mobile/lib/theme/theme.dart`, `mobile/test/theme/icons_test.dart`, `mobile/test/theme/theme_test.dart`
- Create: `mobile/lib/theme/button_styles.dart`, `mobile/test/theme/button_styles_test.dart`

**Interfaces:**
- Produces:
  - `BovedaIcons.pencil` (0xe1f9), `trash` (0xe18e, el ícono `trash-2` de Lucide) y `chevronLeft` (0xe06e), todos `IconData(<código>, fontFamily: 'Lucide')`.
  - `ThemeData.actionIconTheme` con `backButtonIconBuilder: (context) => const Icon(BovedaIcons.chevronLeft)`.
  - `ButtonStyle secondaryActionStyle({Color? foreground})` en `lib/theme/button_styles.dart`: botón secundario pequeño del diseño, para un `TextButton`: fondo `surfaceElevated`, texto `foreground` (por omisión `textPrimary`), deshabilitado en `textDisabled`, radio 12, alto mínimo 40 y ancho mínimo 64, sin cambiar el `tapTargetSize` (48 dp táctiles).

- [ ] **Step 1: Comprobar los códigos.** Con el script de Node del scratchpad (lee las tablas `cmap` y `post` de `mobile/assets/fonts/lucide.ttf`), comprobar que `pencil`, `trash-2` y `chevron-left` existen con esos códigos; registrar el resultado en el ledger.
- [ ] **Step 2: Escribir los tests (fallan).** `icons_test.dart`: los tres nuevos con `fontFamily == 'Lucide'`, código mayor que 0, los códigos de arriba y distintos de los otros diez. `theme_test.dart`, para los cuatro acentos: `actionIconTheme.backButtonIconBuilder` devuelve un `Icon` con `BovedaIcons.chevronLeft`; y un test de widget que empuja una ruta con `buildTheme`, comprueba que el `BackButton` muestra ese ícono y que `tester.pageBack()` sigue volviendo. `button_styles_test.dart`: `secondaryActionStyle()` resuelve fondo `surfaceElevated`, texto `textPrimary` y, deshabilitado, texto `textDisabled`; con `foreground: error` el texto es `error`; la forma es un `RoundedRectangleBorder` de radio 12; el alto mínimo es 40 y `tapTargetSize` no es `shrinkWrap`.
- [ ] **Step 3: Ver que fallan.** Run: `flutter test test/theme`. Expected: FAIL (no compila).
- [ ] **Step 4: Implementar** las constantes, el `actionIconTheme` en `buildTheme` y `button_styles.dart`.
- [ ] **Step 5: Ver que pasan.** Run: `flutter test`. Expected: PASS.
- [ ] **Step 6: Commit.** `git add mobile/lib/theme mobile/test/theme && git commit -m "feat: agregar íconos de Lucide, el ícono de volver y el botón secundario"`

### Task 3: `SecretText`

**Files:**
- Create: `mobile/lib/widgets/secret_text.dart`, `mobile/test/widgets/secret_text_test.dart`

**Interfaces:**
- Consumes: `BovedaText.secret`, `SyntaxColors` (tema).
- Produces: `SecretText({super.key, required String text})`: un `Container` con `Key('secret-text')`, fondo `surface1`, radio 12 y relleno 14, que contiene un `Text.rich` en `secret` con un `TextSpan` por tramo de caracteres consecutivos del mismo tipo: letras (cualquier letra Unicode, `\p{L}`) en `syntaxLetter`, dígitos `0-9` en `syntaxDigit`, espacios dibujados como `␣` en `syntaxSpace`, y todo lo demás en `syntaxSymbol`. Recorre grafemas (`characters`) y clasifica por el primer carácter de cada uno, de modo que un emoji compuesto es un solo símbolo. Con salto en cualquier carácter. Nunca `SelectableText`.

- [ ] **Step 1: Escribir los tests (fallan)**, con `buildTheme` y los cuatro acentos donde aplique: `'Xk9#mP2\$vL7!qR4 0Ol1'` produce los tramos esperados con los colores de `SyntaxColors` (letras blancas, dígitos ámbar, símbolos rosa, el espacio como `␣`); los tramos consecutivos del mismo tipo se agrupan en un solo `TextSpan` (`'abc123'` son dos); `'ñandú'` y `'Ébano'` son letras, `'🔑'` y `'👩‍💻'` son símbolos de un solo tramo y sin partirse; el texto plano del `Text.rich` es el de la contraseña salvo que los espacios son `␣`; la fuente es `JetBrainsMono` en el estilo `secret`; el fondo es `surface1` con radio 12; no hay `SelectableText` ni `TextField` en su árbol; con 120 caracteres sin espacios, a 360 dp y también al 200 %, no desborda y ocupa más de una línea; con el campo vacío no falla y no muestra tramos; sin las extensiones del tema (`MaterialApp` sin `buildTheme`) usa los estándar y no falla.
- [ ] **Step 2: Ver que fallan.** Run: `flutter test test/widgets/secret_text_test.dart`. Expected: FAIL (no compila).
- [ ] **Step 3: Implementar `SecretText`.** Si con 120 caracteres sin espacios Flutter no parte la palabra y desborda, anotar el hallazgo en el ledger y resolverlo sin alterar el texto de la contraseña (nada de insertar caracteres invisibles).
- [ ] **Step 4: Ver que pasan.** Run: el archivo y luego `flutter test`. Expected: PASS.
- [ ] **Step 5: Commit.** `git add mobile/lib/widgets/secret_text.dart mobile/test/widgets/secret_text_test.dart && git commit -m "feat: agregar SecretText, la contraseña revelada con colores por tipo"`

### Task 4: Detalle: barra, filas, etiquetas y diálogo

**Files:**
- Modify: `mobile/lib/screens/entry_detail_screen.dart` (`build`, `_Field` y el diálogo de eliminar), `mobile/test/entry_detail_screen_test.dart` (ajustes) y `mobile/test/entry_detail_screen_design_test.dart` (nuevo)

**Interfaces:**
- Consumes: `BovedaIcons.pencil`, `trash`, `secondaryActionStyle`, `FieldLabel`, `TagBadge`, `isLargeText` (Tasks 2 y 3, y los PR anteriores).

- [ ] **Step 1: Escribir los tests nuevos (fallan)** en `entry_detail_screen_design_test.dart`, con `buildTheme` y los cuatro acentos donde aplique: la barra conserva el título con el servicio y los botones "Editar" y "Eliminar" con sus tooltips, ahora con `BovedaIcons.pencil` y `BovedaIcons.trash`; cada fila (Usuario, URL, Etiquetas, Contraseña, Notas) es una tarjeta `surface2` con borde `borderCard` y radio 16, con su etiqueta en mayúsculas encima; las etiquetas salen como `TagBadge` (todas, sin límite) en lugar de `Chip`; las acciones de cada fila siguen siendo `TextButton` con el mismo texto y `Key`, con el fondo `surfaceElevated`; el botón de volver muestra `BovedaIcons.chevronLeft`; el botón "Eliminar" del diálogo de confirmación va en `error` y conserva su texto; a 360 dp y 200 % la fila con "Copiar" (y "Mostrar") pone las acciones debajo del valor y no desborda.
- [ ] **Step 2: Ajustar los tests existentes por el cambio visual.** En `entry_detail_screen_test.dart`: `find.widgetWithText(Chip, 'correo')` y `'personal'` pasan a `find.widgetWithText(TagBadge, 'CORREO')` y `'PERSONAL'`; los `find.text('Usuario')`, `'URL'`, `'Etiquetas'`, `'Contraseña'` y `'Notas'` pasan a `'USUARIO'`, `'URL'`, `'ETIQUETAS'`, `'CONTRASEÑA'` y `'NOTAS'`. Anotar cada ajuste.
- [ ] **Step 3: Ver que fallan.** Run: `flutter test test/entry_detail_screen_design_test.dart test/entry_detail_screen_test.dart`. Expected: FAIL solo los casos nuevos y los ajustados.
- [ ] **Step 4: Implementar** el `build` (íconos de la barra, `_Field` como tarjeta con `FieldLabel`, acciones debajo del valor con `isLargeText`, etiquetas como `TagBadge` en un `Wrap`) y el estilo de "Eliminar" del diálogo, con los mismos textos, `Key`s y condiciones de cuándo aparece cada fila. Los botones de acción usan `secondaryActionStyle()`. No tocar estado ni flujos.
- [ ] **Step 5: Ver que pasa todo.** Run: `flutter test`. Expected: PASS.
- [ ] **Step 6: Commit.** `git add mobile/lib/screens/entry_detail_screen.dart mobile/test && git commit -m "feat: restilizar la barra, las filas y las etiquetas del detalle"`

### Task 5: Detalle: secreto, notas y aviso de dañado

**Files:**
- Modify: `mobile/lib/screens/entry_detail_screen.dart` (`_secretRows`, `_damagedNotice` y `_RevealFailed`), `mobile/test/entry_detail_screen_design_test.dart` (casos nuevos), `mobile/test/entry_detail_screen_test.dart` si un `find` lo exige

**Interfaces:**
- Consumes: `SecretText`, `secondaryActionStyle`, `BovedaIcons.triangleAlert` (Tasks 2 y 3, y los PR anteriores).

- [ ] **Step 1: Escribir los tests (fallan)**: contraseña oculta con los mismos ocho puntos en `secret` y `textSecondary`, anunciada como "Contraseña oculta" y sin los puntos en la semántica; al revelar, la contraseña (la de `fakePassword`) se muestra con `SecretText`, sin `SelectableText` ni `TextField` en el árbol, y al ocultar vuelven los puntos; "Copiar" copia lo mismo que antes y los `Key`s `copy-password` y `copy-username` siguen; notas reveladas en `bodyMedium` y "Sin notas" con el mismo texto; el error de revelar muestra la misma cadena en `error`; el aviso de dañado es una tarjeta `errorContainer` con borde `error` al 38 % y radio 16, con `BovedaIcons.triangleAlert` en `error`, el texto exacto, un `FilledButton` "Reparar" con fondo `surfaceElevated` y un `TextButton` "Eliminar" en `error`, que abren lo mismo que antes; una contraseña de 120 caracteres sin espacios y otra con espacios se revelan a 360 dp y al 200 % sin desbordes.
- [ ] **Step 2: Ver que fallan.** Run: `flutter test test/entry_detail_screen_design_test.dart`. Expected: FAIL solo los casos nuevos.
- [ ] **Step 3: Implementar** los tres helpers con los textos, `Key`s y condiciones de hoy; quitar `TextStyle(fontFamily: 'monospace')`. No tocar estado ni flujos.
- [ ] **Step 4: Ver que pasa todo.** Run: `flutter test`. Expected: PASS; si un test existente falla solo por la presentación, ajustar su `find` y anotarlo.
- [ ] **Step 5: Commit.** `git add mobile/lib/screens/entry_detail_screen.dart mobile/test && git commit -m "feat: restilizar el secreto, las notas y el aviso de dañado del detalle"`

### Task 6: Documento de diferencias

**Files:**
- Create: `docs/diferencias-diseno.md`

- [ ] **Step 1: Escribir el documento**, en español, con una introducción (qué es: lo que el diseño de Penpot y `docs/DESIGN.md` muestran y la app no hace hoy; cada punto es un candidato a feature; la regla de trabajo) y una sección por pantalla con una tabla de **Diseño**, **App hoy** y **Qué haría falta**. Contenido:
  - **General** (de `DESIGN.md` 15): D2 selector de acento en Ajustes; D3 barra de navegación con pestañas (etapa 3); D4 `ClipboardToast` con cuenta regresiva y "Borrar ahora" (necesita un método nuevo en el canal `boveda/clipboard`); D5 generador con modo frase (necesita lista de palabras en el núcleo); D6 acción rápida "Enviar a PC" en la tarjeta; pulso animado del LED de la cápsula; escala al presionar del engranaje y del "+" (0.95 y 0.90); respuesta háptica.
  - **Desbloqueo:** casilla "Desbloquear con huella la próxima vez" (hoy "Usar huella para desbloquear"); etiqueta `HUELLA • KEYSTORE` del botón de huella (hoy "USAR HUELLA").
  - **Primer uso:** aviso `warning` sin casilla (el código exige la casilla de aceptación); "Restaurar un respaldo" no aparece en el diseño; párrafo "Es la única llave de tu bóveda. Se usa para cifrar todo en este celular."; textos de fortaleza en mayúsculas ("MUY CORTA: MÍNIMO 12 CARACTERES", "ACEPTABLE", "FUERTE"); a 360 × 640 la advertencia con la casilla mide 180 dp y "Crear bóveda" no cabe sin desplazar.
  - **Oferta de huella:** título "¿Desbloquear con huella?" y botón "Activar huella" (hoy "¿Activar huella?" y "Activar") y el párrafo del diseño.
  - **Bóveda:** pista del campo "Buscar servicio, usuario o etiqueta" (hoy "Buscar"); estado vacío "Todavía no hay contraseñas" (hoy el texto largo); sin resultados "Nada coincide con «x»" (hoy "Sin resultados para «x»"); aviso de dañadas con botón de cerrar (hoy "Entendido"); a 390 dp o más el subtítulo del encabezado cabe bajo la marca junto a la cápsula como en el board (a 360 dp va en su propia línea); acción "Enviar a PC" de la tarjeta y deslizar para enviar (etapas 3 y 4).
  - **Detalle:** título "Detalle" y bloque de cabecera (caja de inicial de 56, servicio en `titleLarge`, etiquetas y dominio de la URL); botón principal al pie "Copiar contraseña" (hasta la etapa 3); "Sin notas" visible antes de revelar (necesita que la lista o el detalle sepan si hay notas sin descifrarlas); doce puntos en la contraseña oculta (hoy ocho, que no delatan el largo); orden "Mostrar, Copiar" (hoy "Copiar, Mostrar"); "Eliminar" dentro del formulario (hoy en la barra del detalle); "Enviar a PC" en la fila y la acción de copiar (etapa 3).
- [ ] **Step 2: Comprobar** que cada punto es verdadero contra el código y los boards (una lectura final; ningún punto del documento puede describir algo que la app ya hace). Commit: `docs: registrar las diferencias entre el diseño y la app`.

### Task 7: Guardia, documentación y verificación

**Files:**
- Modify: `mobile/test/theme/design_guard_test.dart`, `docs/DESIGN.md` y `mobile/CLAUDE.md` solo si hace falta

- [ ] **Step 1: Ampliar la guardia.** En `migrated files use Lucide icons, not Material Icons`, sumar `lib/screens/entry_detail_screen.dart` y `lib/widgets/secret_text.dart`. Run: `flutter test test/theme/design_guard_test.dart`. Expected: PASS. Comprobar con una mutación (agregar `// Icons.add` a cada uno y ver que falla; revertir con `git checkout`).
- [ ] **Step 2: Documentación.** Si `DESIGN.md` 6.3 o 7.5 dicen algo que la implementación no hace (los doce puntos, el orden "Mostrar, Copiar"), no se cambia la implementación: se agrega una frase que remita a `docs/diferencias-diseno.md`. Commit: `docs: remitir las diferencias del detalle al documento de diferencias`.
- [ ] **Step 3: Verificación completa.** Desde `mobile/`: `flutter test` y `flutter analyze`; desde la raíz: `cargo test --workspace` y `cargo clippy --workspace --all-targets -- -D warnings`, cada uno con su código de salida visible. Expected: todo termina en 0.
- [ ] **Step 4: Preguntar al usuario si quiere probarlo en su teléfono** (compilar el APK de depuración e instalarlo con `adb install -r`, ver la nota `instalar-en-el-telefono`).

### Task 8: Publicar

- [ ] **Step 1: Pedir confirmación al usuario** y publicar con `git push -u origin feat/detalle-entrada`.
- [ ] **Step 2: Abrir el PR contra `dev`** con `gh pr create --base dev`, título `feat: restilizar el detalle de entrada` y el formato del PR #83 (Resumen, Decisiones de implementación, Lo que el diseño pide y el código no tiene —que remite a `docs/diferencias-diseno.md`—, Tests existentes ajustados, Correcciones de la revisión, Criterios de aceptación con casillas y Verificación local). No esperar la CI; dejar su casilla sin marcar.
