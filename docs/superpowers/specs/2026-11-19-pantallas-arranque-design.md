# Pantallas de arranque: desbloqueo, primer uso y oferta de huella (PR 2)

Fecha: 2026-11-19. Cambio pequeño: diseño corto acordado en la conversación. Continúa `2026-11-19-tema-base-design.md` (PR #81, ya fusionado).

## Intención

Que las tres primeras pantallas que ve el usuario (`unlock_screen.dart`, `first_run_screen.dart`, `biometric_offer_screen.dart`) se vean como sus boards de Penpot (*Desbloqueo*, *Primer uso*, *Oferta de huella*) y como pide `docs/DESIGN.md` 6.4, 6.5, 6.8 y 7.1 a 7.3, **sin cambiar su comportamiento**. Mandan el código y los tests actuales; lo que el diseño pide y el código no tiene se lista en el PR para que el usuario decida si se implementa después.

## Metas

- **Íconos de Lucide** como fuente de íconos: `mobile/assets/fonts/lucide.ttf` (ISC) y `lib/theme/icons.dart` con una clase `BovedaIcons` que expone solo los íconos que se usan hoy (`lock`, `eye`, `eyeOff`, `fingerprint`, `triangleAlert`). Su licencia se registra en `registerFontLicenses`. Sin dependencias nuevas. Los demás `Icons.*` de otras pantallas se migran en sus PR.
- **Widgets compartidos** (en `lib/widgets/`): cápsula de estado (LED de 6 px y texto en `meta`, con las variantes "abierta" y "bloqueada"), resplandor del botón principal (`accentGlow`, blur 25, solo habilitado) y etiqueta de campo en `meta`.
- **`PasswordField`** (lo usan también el formulario de entradas y el diálogo de importar): etiqueta encima en `meta`, texto en `secret`, ojo de Lucide, borde `outline` y `accent` de 2 px con foco. Conserva todo lo que hace hoy (no copiar ni cortar, sin sugerencias, `Key`s y parámetros).
- **Desbloqueo**: fondo `lockScrim`, caja de 72 × 72 con candado, título, cápsula `BLOQUEADA`, error con ícono de alerta, botón principal con resplandor, botón de huella circular de 64, avisos (`UnlockNotice`) en contenedor `error` o `warning`.
- **Primer uso**: cápsula `CONOCIMIENTO CERO` arriba, etiquetas sobre los campos y `StrengthIndicator` de tres segmentos (alto 4, separación 4) con el texto en `meta`.
- **Oferta de huella**: ícono de 64 en círculo `accentDim`, contenido centrado y botones al pie.
- Pruebas de widgets de los estados nuevos con los cuatro acentos y con texto al 200 % a 360 dp de ancho.

## No metas

- Cambiar lógica, flujos, `Key`s, parámetros públicos o textos del código (ni los de la especificación).
- Quitar la casilla "Entiendo que…" del primer uso, quitar "Restaurar un respaldo" o cambiar el texto de "¿Activar huella?".
- Barra de navegación, selector de acento, `ClipboardToast`, pulso animado del LED y cualquier otra función de D2 a D6.
- Migrar los íconos de las demás pantallas.

## Anti-metas

- Un `Color(0x…)`, `Colors.`, `fontSize:` o `TextStyle` con tamaño fijo fuera de `lib/theme/` (la guardia lo exige).
- Dar a un secreto estilos que lo hagan seleccionable o copiable.
- Que una pantalla pierda el foco, el teclado o el bloqueo por inactividad que hoy maneja (`ActivityTextField`, `TextFieldTapRegion`).
- Que una prueba existente cambie de resultado por un motivo distinto del cambio visual deliberado.

## Restricciones

- Sin dependencias nuevas. `lucide.ttf` es un archivo del repositorio; se descarga del release oficial `lucide-icons/lucide` (`lucide-font-1.76.0.zip`) a un directorio temporal, con permiso del usuario, y se borra después. Solo entran al repo el TTF y su licencia.
- Textos: las cadenas del código no cambian. Las etiquetas que el diseño pone en MAYÚSCULAS se muestran con `toUpperCase()` al dibujarlas; si una prueba busca el texto original, se ajusta solo ese `find` y se anota en el PR.
- Un solo botón principal por pantalla, `Semantics` en la cápsula ("Bóveda abierta" o "Bóveda bloqueada") y áreas táctiles de 48 dp o más (`DESIGN.md` 12).

## Enfoques

- **Íconos:** (A) fuente `lucide.ttf` con clase de constantes, elegido; (B) Material Icons, descartado porque `DESIGN.md` pide Lucide; (C) SVG con `flutter_svg`, descartado por ser dependencia nueva.
- **Alcance:** las tres pantallas en un PR (comparten `PasswordField`, la cápsula y el botón con resplandor), elegido; una por PR duplicaría los widgets compartidos.

## Lo que el diseño pide y el código no tiene (a listar en el PR)

- **Primer uso:** el diseño reemplaza la casilla de aceptación por un aviso `warning` con ícono; el código la exige. Se deja la casilla, dentro de un contenedor `warning`. "Restaurar un respaldo" no está en el diseño.
- **Textos distintos** (se conserva el del código): oferta "¿Activar huella?" y "Activar", frente a "¿Desbloquear con huella?" y "Activar huella"; fortaleza "Demasiado corta", frente a "MUY CORTA: MÍNIMO 12 CARACTERES"; párrafo del primer uso; casilla de huella del desbloqueo; botón "Usar huella", frente a `HUELLA • KEYSTORE`.
- **Pulso del LED** de la cápsula con la bóveda abierta (animación).
- **Cápsula de estado en el encabezado de la bóveda abierta** y el resto del `CryptoHeader`: son de la pantalla principal.

## Decisiones mías (a revisar)

- La cápsula del primer uso es decorativa: `CONOCIMIENTO CERO` es el texto de `DESIGN.md` 6.1 y no hay función detrás.
- LED estático (sin pulso) para no agregar animaciones; `MediaQuery.disableAnimations` ya queda cubierto.
- El resplandor va en un widget envolvente del `FilledButton`, no en el tema, porque `ButtonStyle` no admite sombras de color.
- `BovedaIcons` vive en `lib/theme/` junto a los demás tokens.

## Lo que queda al implementador

Códigos exactos de los íconos (se leen del `info.json` del release), nombres y firmas de los widgets compartidos, y cuáles pruebas existentes se ajustan. Verificación final: `flutter test` y `flutter analyze` en `mobile/`, `cargo test --workspace` y `cargo clippy` en la raíz. La revisión visual en emulador se avisa antes de usarlo.
