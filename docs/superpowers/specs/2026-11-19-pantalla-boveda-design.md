# Pantalla de Bóveda: encabezado, búsqueda, tarjetas y estados vacíos (PR 3)

Fecha: 2026-11-19. Diseño corto acordado en la conversación. Continúa `2026-11-19-pantallas-arranque-design.md` (PR #82, ya fusionado).

## Intención

Que la pantalla principal (`search_screen.dart` y `damaged_entries_banner.dart`) se vea como sus boards de Penpot (*Bóveda*, *Bóveda · sin resultados*, *Bóveda · vacía* y los componentes de la página *Componentes*) y como pide `docs/DESIGN.md` 6.1, 6.2, 6.6, 6.7, 6.8 y 7.4, **sin cambiar su comportamiento**: misma búsqueda, mismos `Key`s, mismas rutas y mismas cadenas. Lo que el diseño pide y el código no tiene se lista en el PR.

## Metas

- **Íconos** en `BovedaIcons`: `search` (e151), `x` (e1b2), `settings` (e154), `plus` (e13d) y `vault` (e58f), con los códigos leídos del `lucide.ttf` que ya está en el repo. Sin descargas.
- **Encabezado** (`VaultHeader`, `lib/widgets/`): marca `BÓVEDA` en `brand`, debajo `XCHACHA20-POLY1305 • ARGON2ID` en `meta`, la cápsula `CONOCIMIENTO CERO` (`StatusCapsule`, semántica "Bóveda abierta") y el engranaje de Ajustes de 40 × 40 (`surfaceElevated`, radio 12, área táctil de 48). El engranaje conserva `Key('open-settings')`, su tooltip y su acción; solo se mueve del campo de búsqueda al encabezado. Sin `onOpenSettings`, no hay engranaje.
- **Tarjeta de entrada** (`VaultCard`, `lib/widgets/`): `surface2`, borde `borderCard`, radio 16, relleno 14; caja `iconBox` de 40 × 40 con la inicial en `metric` y `accent`; servicio en `titleMedium`; hasta dos etiquetas como insignias (`TagBadge`: `meta`, `accentDim`, `accentBorder`, radio 6, en mayúsculas) y `+N` para el resto; usuario o URL en `bodySmall`. Al presionar: borde `#52525B` y escala 0.98 (sin escala con `disableAnimations`). `Semantics` con servicio y usuario y "abrir" como pista del toque. Las etiquetas salen de `EntrySummary.tags`, que ya llega con la lista.
- **Barra de búsqueda**: `surface1`, borde `outline`, radio 12, alto 52, con lupa y ✕ de Lucide; sigue sin enfocarse al llegar.
- **Botón "+"**: 56 × 56, radio 16, `accent` con `PrimaryGlow` (que gana un parámetro de radio) y `+` de Lucide. Mismo tooltip y misma acción.
- **Aviso de dañadas**: tarjeta `errorContainer`, borde `error` al 38 %, radio 16, ícono de alerta en `error`, el mismo texto y el mismo botón "Entendido".
- **Estados**: vacío con caja de 72 y el ícono `vault`, el texto de hoy y un botón secundario "Agregar la primera" que llama a la misma acción que el "+"; sin resultados con la lupa de Lucide y el texto de hoy; error de búsqueda con el mismo texto y "Reintentar".
- Tests de widgets de los estados nuevos con los cuatro acentos y con texto al 200 % a 360 dp.

## No metas

- Cambiar la búsqueda, la generación de resultados, las rutas, los `Key`s, el orden de la lista o las cadenas del código.
- Barra de navegación, "Enviar a PC" en la tarjeta (etapa 3), deslizar para enviar (etapa 4), pulso animado del LED y el selector de acento.
- Migrar el resto de los `Icons.*` de otras pantallas.

## Anti-metas

- `Color(0x…)`, `Colors.` o `fontSize:` fuera de `lib/theme/` (la guardia lo exige).
- Mostrar o registrar el texto buscado, o mostrar un secreto: la tarjeta solo usa servicio, usuario, URL y etiquetas, como hoy.
- Cambiar el foco inicial, el cierre del teclado al arrastrar o el bloqueo por inactividad (`ActivityTextField`).
- Que el aviso de dañadas se arrastre entre desbloqueos: sigue dentro del árbol de la pantalla.

## Restricciones

- Sin dependencias nuevas.
- Las cadenas del código no cambian. Las etiquetas de las insignias pasan a mayúsculas solo al dibujarse.
- Un test existente solo se ajusta por el cambio visual deliberado y se anota: los de `app_test.dart` y `search_screen_test.dart` que buscan `ListTile` o `CircleAvatar`.
- Token nuevo en `lib/theme/colors.dart` y en las tablas de `DESIGN.md`: `borderPressed` (`#52525B`).

## Enfoques

- **Una sola pantalla en un PR**, elegido: el encabezado, la tarjeta, el aviso y los estados comparten tokens y widgets, y se prueban juntos.
- **Íconos de la lista y el campo con Lucide** y el resto de `Icons.*` del repositorio para sus PR, elegido: mantiene el PR acotado.

## Lo que el diseño pide y el código no tiene (a listar en el PR)

- **Textos distintos** (se conserva el del código): la pista del campo ("Buscar", frente a "Buscar servicio, usuario o etiqueta"), el estado vacío ("Todavía no hay contraseñas guardadas. Toca + para agregar la primera.", frente a "Todavía no hay contraseñas"), el sin resultados ("Sin resultados para «x»", frente a "Nada coincide con «x»") y el aviso de dañadas ("Entendido", frente al botón de cerrar).
- **Etapa 3 y 4:** barra de navegación, acción "Enviar a PC" de la tarjeta y deslizar para enviar.
- **Pulso del LED** de la cápsula (el LED es fijo).

## Decisiones mías (a revisar)

- Las insignias van una al lado de otra con 6 de separación, como en Penpot, sin el ` // ` literal que menciona `DESIGN.md` 6.2.
- La cápsula del encabezado siempre dice `CONOCIMIENTO CERO`: la pantalla solo existe con la bóveda abierta.
- El botón "Agregar la primera" usa el estilo secundario del diseño y no se muestra con texto buscado.
- `PrimaryGlow` gana un radio opcional (12 por omisión) en vez de crear otro widget para el "+".

## Lo que queda al implementador

Nombres y firmas exactos de `VaultHeader`, `VaultCard` y `TagBadge`, el orden de los commits y qué pruebas existentes se ajustan. Verificación final: `flutter test` y `flutter analyze` en `mobile/`, `cargo test --workspace` y `cargo clippy` en la raíz. La revisión visual en el teléfono se hace con tu recorrido manual (ver la nota de instalación).
