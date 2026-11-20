# Ajustes y diálogo de importar (PR 6)

Fecha: 2026-11-19. Diseño corto acordado en la conversación. Continúa `2026-11-19-formulario-entrada-design.md` (PR #85, ya fusionado).

## Intención

Que Ajustes (`settings_screen.dart`) y el diálogo de importar un respaldo (`restore_dialog.dart`) se vean como el board de Penpot *Ajustes* y como pide `docs/DESIGN.md` 7.9 y 6.11, **restilizando solo lo que la app ya tiene**. Regla del usuario: se trabaja sobre lo ya desarrollado y no se cambia nada más; lo que el diseño muestra y la app no hace hoy (casi todo Ajustes) no se implementa, se registra en `docs/diferencias-diseno.md`.

## Metas

- **Ajustes** (hoy: "Exportar respaldo" e "Importar respaldo"): cada fila sigue siendo un `ListTile` con su `Key` (`settings-export` y `settings-import`), su texto, su subtítulo, su estado `enabled` y su acción, ahora como tarjeta (`surface2`, borde `borderCard`, radio 16) con separación de 10 y margen lateral de 16. Los íconos de Material pasan a Lucide, los del board: `download` para exportar y `refresh-cw` para importar. Cada fila lleva a la derecha un `chevron-right` de Lucide en `textSecondary` (solo decoración). Con la pantalla ocupada (`_busy`) las filas se ven desactivadas, como hoy.
- **Diálogo de importar**: el mismo título, el mismo campo de contraseña del respaldo, los mismos botones y las mismas `Key`s. La advertencia "Reemplaza la bóveda actual." (solo al reemplazar) pasa de un texto suelto a una tarjeta `warningContainer` con ícono de alerta de Lucide. El texto y la lógica no cambian.
- **`docs/diferencias-diseno.md`**: sección de Ajustes y del diálogo de importar con lo que el diseño muestra y la app no hace.
- Tests de widgets de Ajustes y del diálogo con los cuatro acentos, con texto al 200 % a 360 dp y con el estado ocupado.

## No metas

- Agregar filas, secciones, encabezados o textos que la app no tiene. En concreto, **no** se agrega: los encabezados de sección (`SEGURIDAD`, `RESPALDOS`, `APARIENCIA`, `DISPOSITIVOS`), el título grande "Ajustes" en el cuerpo, "Bloqueo automático", "Huella", "Cambiar contraseña maestra", el selector de acento ni "PC emparejada". La barra de navegación es de la etapa 3.
- Cambiar la exportación y la importación, el selector del sistema (`runWithoutAutoLock`), el bloqueo, los avisos (`UnlockNotice`, `SnackBar`) ni los textos ("Exportar respaldo", "Guarda una copia cifrada de la bóveda", "Importar respaldo", "Reemplaza la bóveda actual", el título del diálogo y todos los mensajes de error).
- Tocar `placeholder_screen.dart`: es la pantalla de relleno de un error de arranque y el diseño no la dibuja.

## Anti-metas

- Un `Color(0x…)`, `Colors.` o `fontSize:` fuera de `lib/theme/` (la guardia lo exige).
- Que la contraseña del respaldo se vuelva copiable, se guarde fuera del estado del diálogo o aparezca en la semántica, un log o un mensaje de error.
- Cambiar cuándo se cierra el diálogo, cómo se maneja un bloqueo con el diálogo abierto (`onImporting`) ni el resultado que devuelve.
- Que una prueba existente cambie de resultado por un motivo distinto del cambio visual deliberado.

## Restricciones

- Sin dependencias nuevas. Los tres íconos nuevos (`download`, `refresh-cw` y `chevron-right`) se leen del `lucide.ttf` que ya está en el repositorio.
- Las cadenas del código no cambian.
- Un test existente solo se ajusta por el cambio visual deliberado y se anota.

## Enfoques

- **Solo lo existente** (elegido, por indicación del usuario): se restiliza lo que hay; el resto va al documento de diferencias.

## Decisiones mías (a revisar)

- Los íconos de las filas son los del board (`download` para exportar, `refresh-cw` para importar), no los de Material de hoy (`upload_file` y `download`): el de exportar cambia de significado, pero es el del diseño.
- El `chevron-right` es solo decoración: las dos filas abren un selector de archivos o un diálogo, no una pantalla. Se incluye por fidelidad al board; si prefieres no mostrarlo, se quita sin tocar nada más.
- La advertencia del diálogo pasa a tarjeta `warningContainer` (como el aviso del primer uso); el board no dibuja el diálogo.

## Lo que queda al implementador

Nombres exactos de las constantes de íconos, el orden de los commits y qué pruebas existentes se ajustan. Verificación final: `flutter test` y `flutter analyze` en `mobile/`, `cargo test --workspace` y `cargo clippy` en la raíz. La revisión visual la haces tú en el teléfono.
