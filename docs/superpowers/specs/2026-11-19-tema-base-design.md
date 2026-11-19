# Tema base de la app (PR 1 de la integración del diseño)

Fecha: 2026-11-19. Cambio pequeño: diseño corto acordado en la conversación.

## Intención

Que toda la app herede el sistema de diseño de `docs/DESIGN.md` (colores, tipografía, espaciado y radios) sin cambiar ninguna función. Es la base de los PR siguientes, que restilizan las pantallas una por una comparando el código con su board de Penpot.

## Metas

- `mobile/lib/theme/` con lo que pide `DESIGN.md` 5.1: `colors.dart`, `accents.dart`, `theme.dart`, `extensions.dart` (`StatusColors`, `SyntaxColors`, `BovedaText`) y `spacing.dart`.
- `MaterialApp` con un solo tema oscuro (`darkTheme` y `themeMode: ThemeMode.dark`).
- Fuentes Plus Jakarta Sans y JetBrains Mono empaquetadas en `mobile/assets/fonts/`, con su licencia (OFL 1.1) registrada en `LicenseRegistry`.
- Prueba que recorre los cuatro `ThemeData` y comprueba que ningún par de la tabla 2.5 baja de su mínimo (`DESIGN.md` 13).

## No metas

- Cambiar el comportamiento o el árbol de widgets de las pantallas. Solo se tocan los 9 usos de color o estilo fijo (en 6 archivos) que se vean mal con el tema oscuro, con el cambio mínimo.
- Selector de acento (D2), barra de navegación (D3), `ClipboardToast` (D4), generador (D5), acción rápida en la tarjeta (D6), resplandor y desenfoque.
- Íconos: se resuelven en el primer PR de pantalla que los necesite.

## Anti-metas

- Un `Color(0x…)` o un `TextStyle` con tamaño fijo fuera de `lib/theme/`.
- Descargar fuentes en tiempo de ejecución o agregar dependencias.
- Que alguna prueba existente cambie de resultado.

## Restricciones

- Sin dependencias nuevas. Las fuentes son archivos del repositorio.
- Las reglas de `mobile/CLAUDE.md` y del `CLAUDE.md` raíz siguen vigentes.
- La especificación manda sobre `DESIGN.md`. D1 (solo tema oscuro) cambia la especificación, que hoy sigue el tema del sistema. Se registra en `docs/especificacion.md` en un commit aparte de este PR. D2 a D6 siguen pendientes.

## Enfoques

- **A. Base primero y luego pantalla por pantalla (elegido).** PR pequeños y revisables; el tema ya mejora todas las pantallas por herencia.
- **B. Una pantalla completa primero.** Descartado: deja el tema a medias.
- **C. Todo en un PR.** Descartado: demasiado grande para una app de contraseñas.

## Decisiones mías (a revisar)

- El enum `BovedaAccent` incluye los cuatro acentos, porque `DESIGN.md` los define y la prueba de contraste los recorre. La app usa `cyberLime` fijo; elegirlo es el selector pendiente (D2).
- Fuentes en TTF estáticos, solo los pesos que usa `DESIGN.md` (400, 500, 700 y 800). Se obtienen de las fuentes oficiales de cada proyecto, con tu visto bueno antes de descargar.
- Los íconos de Lucide irán como fuente de íconos (`lucide.ttf`, ISC) con una clase de constantes, no como SVG: evita `flutter_svg`, que sería una dependencia nueva. Se confirma en el primer PR de pantalla.

## Lo que queda al implementador

Nombres exactos de los tokens, cómo armar el `ColorScheme` a mano y el orden de los commits. Comprobación final: `flutter test` y `flutter analyze` en `mobile/`, además de las pruebas y el lint de Rust de la raíz. La revisión visual en emulador se avisa antes de usarlo.
