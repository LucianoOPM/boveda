# Formulario de crear y editar entrada (PR 5)

Fecha: 2026-11-19. Diseño corto acordado en la conversación. Continúa `2026-11-19-detalle-entrada-design.md` (PR #84, ya fusionado).

## Intención

Que el formulario de crear, editar y reparar una entrada (`entry_form_screen.dart` y `tags_field.dart`) se vea como su board de Penpot (*Formulario de entrada*) y como pide `docs/DESIGN.md` 6.4 y 7.6, **restilizando solo lo que la pantalla ya tiene**. Regla del usuario: se trabaja sobre lo ya desarrollado y no se cambia nada más; lo que el diseño muestra y la app no hace hoy no se implementa, se registra en `docs/diferencias-diseno.md`.

## Metas

- **Etiquetas encima de los campos**: Servicio, Usuario, URL, Notas y "Agregar etiqueta" pasan de `labelText` flotante a una etiqueta en mayúsculas encima (`FieldLabel`), como ya hace `PasswordField`. El texto de cada etiqueta no cambia; el lector de pantalla sigue leyendo el original y tocar la etiqueta enfoca el campo. Un widget nuevo, `LabeledField`, reúne esa mecánica (etiqueta, semántica y foco) y `PasswordField` lo usa en vez de repetirla. Los campos siguen siendo `ActivityTextField` con los mismos `Key`s y parámetros; el relleno, el borde y el foco los da el tema.
- **Etiquetas de la entrada** (`TagsField`): siguen siendo `InputChip`, con el mismo texto (en mayúsculas al dibujar), `accentDim`, borde `accentBorder`, radio 6, texto en `meta` y `accent`, y una ✕ de Lucide como botón de quitar con el mismo tooltip "Quitar etiqueta". El campo "Agregar etiqueta" conserva su ayuda "Enter o coma para agregar" y su comportamiento.
- **Secciones cerradas** (contraseña y notas sin abrir): la misma fila de texto y botón ("Contraseña: sin cambios" con "Cambiar", "Notas: sin cambios" con "Editar notas") dentro de una tarjeta con el aspecto de un campo (`surface1`, borde `outline`, radio 12), con el botón secundario pequeño del detalle. Los botones siguen sin cerrar el teclado.
- **Secciones abiertas**: "Cancelar" sigue donde está; el error "No se pudieron cargar las notas…" con ícono de alerta.
- **Aviso de reparación**: el mismo texto, ahora en una tarjeta `errorContainer` con ícono de Lucide, como los avisos de entrada dañada.
- **Error general** ("No se pudo guardar. Intenta de nuevo."): mismo texto, con ícono de alerta.
- **Guardar**: el mismo `FilledButton` con el mismo texto, `Key` y estados, con `PrimaryGlow`.
- **Confirmación de descartar**: mismos textos; el botón "Descartar" en `error` (`DESIGN.md` 6.8 y 6.11).
- **`docs/diferencias-diseno.md`**: sección del formulario con lo que el diseño muestra y la app no hace.
- Tests de widgets de los estados (crear, editar, reparar, secciones cerradas y abiertas, errores, etiquetas) con los cuatro acentos, con texto al 200 % a 360 dp y con el teclado abierto en una pantalla baja.

## No metas

- Cambiar títulos ("Nueva entrada", "Editar entrada", "Reparar entrada"), textos, botones, orden de los campos, validaciones, el guardado, el descarte ni el flujo de la contraseña y las notas. En concreto, **no** se agrega: el botón "Eliminar entrada" (en la app eliminar vive en el detalle), la etiqueta "ETIQUETAS" sobre el grupo de etiquetas, los puntos en la contraseña cerrada, ni se reordenan los campos.
- Cambiar cuándo se pide, se carga o se descarta un secreto, que las notas no se puedan copiar desde el formulario, ni el bloqueo por inactividad.
- Barra de navegación, "Enviar a PC", `ClipboardToast` y generador (etapa 4: el botón del generador junto a la contraseña).

## Anti-metas

- Un `Color(0x…)`, `Colors.` o `fontSize:` fuera de `lib/theme/` (la guardia lo exige).
- Que las notas o la contraseña escritas se vuelvan copiables, se guarden fuera del estado de la pantalla o aparezcan en la semántica, un log o un mensaje de error.
- Perder el foco, el teclado o el cierre del teclado al arrastrar y al tocar fuera (`ActivityTextField`, `TextFieldTapRegion`, `PopScope`).
- Que una prueba existente cambie de resultado por un motivo distinto del cambio visual deliberado.

## Restricciones

- Sin dependencias nuevas. La ✕ es `BovedaIcons.x`, que ya existe.
- Las cadenas del código no cambian; solo las etiquetas de campo y de insignia pasan a MAYÚSCULAS al dibujarse.
- Un test existente solo se ajusta por el cambio visual deliberado y se anota: los que buscan el texto de una etiqueta flotante o de un `InputChip` por su texto original.

## Enfoques

- **Solo lo existente** (elegido, por indicación del usuario): se restiliza lo que hay; el resto va al documento de diferencias.
- **Una etiqueta flotante restilizada, sin moverla encima** (descartado): `PasswordField` ya la tiene encima desde el PR #82 y quedaría una pantalla con dos estilos.

## Decisiones mías (a revisar)

- Extraer `LabeledField` y que `PasswordField` lo use evita repetir la etiqueta, la semántica y el foco en seis campos; el riesgo es tocar un widget con muchos tests, que lo cubren.
- El aviso de reparación va en `errorContainer` (como las demás entradas dañadas); `DESIGN.md` no lo describe.
- El texto de las etiquetas dentro de `InputChip` pasa a mayúsculas, igual que `TagBadge`.

## Lo que queda al implementador

Nombres y firmas exactos de `LabeledField`, el orden de los commits y qué pruebas existentes se ajustan. Verificación final: `flutter test` y `flutter analyze` en `mobile/`, `cargo test --workspace` y `cargo clippy` en la raíz. La revisión visual la haces tú en el teléfono.
