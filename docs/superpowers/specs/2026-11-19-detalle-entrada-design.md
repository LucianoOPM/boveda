# Detalle de entrada (PR 4)

Fecha: 2026-11-19. Diseño corto acordado en la conversación. Continúa `2026-11-19-pantalla-boveda-design.md` (PR #83, ya fusionado).

## Intención

Que el detalle de una entrada (`entry_detail_screen.dart`) se vea como sus boards de Penpot (*Detalle de entrada*, *Detalle · contraseña revelada* y *Detalle · secreto dañado*) y como pide `docs/DESIGN.md` 6.3 y 7.5, **restilizando solo lo que la pantalla ya tiene**. Regla del usuario: se trabaja sobre lo ya desarrollado y no se cambia nada más; lo que el diseño muestra y la app no hace hoy no se implementa, se registra en `docs/diferencias-diseno.md` para convertirlo después en features.

## Metas

- **Barra superior**: el título sigue siendo el nombre del servicio; "Editar" y "Eliminar" siguen donde están, con los mismos tooltips y `Key`s, y pasan a íconos de Lucide (`pencil` y `trash-2`). La flecha de volver pasa a `chevron-left` de Lucide con `actionIconTheme` del tema; el widget sigue siendo el `BackButton` de siempre.
- **Filas** (`_Field`): cada campo (Usuario, URL, Etiquetas, Contraseña, Notas) en una tarjeta `surface2` con borde `borderCard`, radio 16 y relleno 14, con la etiqueta encima en `meta` (`FieldLabel`) y el valor debajo. Mismos campos, mismo orden y misma lógica de cuándo aparecen.
- **Acciones de cada fila** ("Copiar", "Mostrar", "Ocultar", "Mostrar notas", "Ocultar notas"): siguen siendo `TextButton`, con el mismo texto, orden y `Key`s, con el estilo de botón secundario pequeño del diseño (`surfaceElevated`, radio 12, texto `textPrimary`), manteniendo 48 dp de área táctil.
- **Etiquetas**: los `Chip` pasan a `TagBadge` (todas, sin el límite de dos de la tarjeta de la lista).
- **Contraseña oculta**: los mismos ocho puntos, en `secret` y `textSecondary`; el lector de pantalla sigue diciendo "Contraseña oculta".
- **Contraseña revelada** (`SecretText`, nuevo en `lib/widgets/`, `DESIGN.md` 6.3): el mismo texto en `secret`, sobre `surface1` con radio 12, coloreado por tipo de carácter (letras `syntaxLetter`, dígitos `syntaxDigit`, símbolos `syntaxSymbol`, espacios dibujados como `␣` en `syntaxSpace`) y con salto en cualquier carácter. Sigue siendo `Text.rich`: nunca `SelectableText` ni un campo, para que no se pueda copiar sin la marca de sensible.
- **Notas reveladas** y "Sin notas": el mismo `Text`, en `bodyMedium`. El error "No se pudo mostrar. Intenta de nuevo." con la misma cadena y el color `error`.
- **Secreto dañado**: el aviso pasa a una tarjeta `errorContainer` con borde `error` al 38 %, ícono de alerta de Lucide, el mismo texto y las mismas dos acciones: "Reparar" (`FilledButton`, estilo secundario) y "Eliminar" (`TextButton` en `error`).
- **Diálogo de eliminar**: mismo texto; el botón "Eliminar" de la confirmación en `error` (`DESIGN.md` 6.8 y 6.11).
- **Documento de diferencias**: `docs/diferencias-diseno.md` con las diferencias entre el diseño y la app de todas las pantallas ya hechas (desbloqueo, primer uso, oferta de huella, Bóveda) y las del detalle, listas para convertirse en features.
- Tests de widgets de las tres variantes con los cuatro acentos, con texto al 200 % a 360 dp y con contraseñas extremas (muy larga sin espacios, con espacios, con todos los tipos de carácter).

## No metas

- Cambiar títulos, textos, botones, flujos, posiciones o parámetros del código. En concreto, **no** se agrega: el título "Detalle" ni el bloque de cabecera (caja de inicial grande, servicio, dominio); el botón al pie "Copiar contraseña"; "Sin notas" antes de revelar; los doce puntos; el orden "Mostrar, Copiar"; ni se mueve "Eliminar" al formulario.
- Cambiar cuándo se pide, se revela o se descarta un secreto, el borrado del portapapeles o el bloqueo.
- Barra de navegación, "Enviar a PC", `ClipboardToast` y generador.

## Anti-metas

- Un `Color(0x…)`, `Colors.` o `fontSize:` fuera de `lib/theme/` (la guardia lo exige).
- Que la contraseña o las notas se vuelvan seleccionables, se guarden fuera del estado de la pantalla, o aparezcan en la semántica, en un log o en un mensaje de error.
- Que una prueba existente cambie de resultado por un motivo distinto del cambio visual deliberado.

## Restricciones

- Sin dependencias nuevas. Los íconos nuevos (`pencil`, `trash-2`, `chevron-left`) se leen del `lucide.ttf` que ya está en el repositorio.
- Las cadenas del código no cambian; solo las etiquetas de fila pasan a MAYÚSCULAS al dibujarse (`FieldLabel`).
- Un test existente solo se ajusta por el cambio visual deliberado y se anota: los `find` de `Chip` (ahora `TagBadge`) y de las etiquetas de fila (ahora en mayúsculas).

## Enfoques

- **Solo lo existente** (elegido, por indicación del usuario): se restiliza lo que hay; el resto va al documento de diferencias.
- **Seguir el diseño en la cabecera y el título** (descartado): cambiaría la pantalla y obligaría a ajustar muchos tests.

## Decisiones mías (a revisar)

- La flecha de volver cambia por `actionIconTheme` del tema (afecta a todas las pantallas con `BackButton`, solo su ícono), porque un `leading` propio rompería `tester.pageBack()` y no es necesario.
- El espacio se dibuja como `␣` solo en pantalla; `fakePassword` y las pruebas actuales no tienen espacios, y un test nuevo lo cubre.
- `docs/diferencias-diseno.md` queda en `docs/` junto a `DESIGN.md`.

## Lo que queda al implementador

Nombres y firmas exactos de `SecretText`, los códigos de los íconos nuevos (se leen del `lucide.ttf`), el orden de los commits y qué pruebas existentes se ajustan. Verificación final: `flutter test` y `flutter analyze` en `mobile/`, `cargo test --workspace` y `cargo clippy` en la raíz. La revisión visual la haces tú en el teléfono.
