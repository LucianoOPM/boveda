import 'package:flutter/material.dart';

import '../theme/extensions.dart';
import '../theme/icons.dart';
import '../theme/spacing.dart';
import '../theme/theme.dart';
import 'activity_text_field.dart';
import 'labeled_field.dart';

/// Etiquetas de una entrada: un chip por etiqueta y un campo para agregar
/// más (#41).
///
/// Enter o una coma agregan lo escrito; pegar "a,b,c" agrega tres. Por eso
/// una etiqueta no puede contener una coma: es una limitación de esta
/// interfaz, no del formato de la bóveda.
///
/// Dart no recorta las etiquetas ni quita las duplicadas: lo hace el núcleo
/// al guardar, con sus propias reglas. Lo único que se descarta aquí es un
/// trozo en blanco, para no dibujar un chip vacío; el núcleo también lo
/// descartaría.
///
/// La lista y el controlador del texto pendiente son de quien usa el campo.
/// Al guardar, [withPending] une los chips con lo que quedó sin agregar.
class TagsField extends StatelessWidget {
  const TagsField({
    super.key,
    required this.tags,
    required this.onChanged,
    required this.controller,
    this.focusNode,
    this.enabled = true,
    this.errorText,
  });

  final List<String> tags;

  /// Recibe la lista nueva; [tags] no se modifica.
  final ValueChanged<List<String>> onChanged;

  /// Texto todavía sin agregar.
  final TextEditingController controller;
  final FocusNode? focusNode;
  final bool enabled;
  final String? errorText;

  /// [tags] más los trozos de [pending], partidos por comas, sin los que
  /// quedan en blanco.
  static List<String> withPending(List<String> tags, String pending) => [
    ...tags,
    for (final piece in pending.split(','))
      if (piece.trim().isNotEmpty) piece,
  ];

  /// Pasa a chips lo escrito y vacía el campo.
  void _commit() {
    final next = withPending(tags, controller.text);
    controller.clear();
    if (next.length != tags.length) onChanged(next);
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final accent = theme.colorScheme.primary;
    final accents =
        theme.extension<AccentColors>() ?? AccentColors.of(defaultAccent);
    final meta = (theme.extension<BovedaText>() ?? BovedaText.standard).meta;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (tags.isNotEmpty)
          // Los chips son parte del formulario: tocarlos no cierra el teclado.
          TextFieldTapRegion(
            child: Padding(
              padding: const EdgeInsets.only(bottom: Space.sm),
              child: Wrap(
                spacing: Space.sm,
                runSpacing: Space.sm,
                children: [
                  for (final (index, tag) in tags.indexed)
                    // Con el estilo de insignia de la tarjeta (`DESIGN.md` 6.2
                    // y 7.6); el texto va en MAYÚSCULAS solo al dibujarse.
                    InputChip(
                      // En mayúsculas solo al dibujarse: el lector de pantalla
                      // lee la etiqueta original.
                      label: Text(
                        tag.toUpperCase(),
                        semanticsLabel: tag,
                        overflow: TextOverflow.ellipsis,
                      ),
                      labelStyle: meta.copyWith(
                        color: accent,
                        fontWeight: FontWeight.w700,
                      ),
                      backgroundColor: accents.dim,
                      // Mientras guarda (desactivado), un \`InputChip\` toma el
                      // fondo de \`disabledColor\`, no de \`backgroundColor\`.
                      disabledColor: accents.dim,
                      side: BorderSide(color: accents.border),
                      shape: RoundedRectangleBorder(
                        borderRadius: BorderRadius.circular(Radii.tag),
                      ),
                      deleteIcon: Icon(
                        BovedaIcons.x,
                        size: IconSizes.inline,
                        color: accent,
                      ),
                      // La zona táctil de la ✕ crece con su caja (Material la
                      // limita a \`24 + ancho / 2\`): con una caja de 48 de ancho
                      // llega a los 48 dp de \`DESIGN.md\` 12. Con el ícono de
                      // 14 por omisión eran unos 26 dp. Una etiqueta muy corta
                      // sigue limitada por Material a la mitad del chip.
                      deleteIconBoxConstraints: const BoxConstraints(
                        minWidth: 48,
                      ),
                      isEnabled: enabled,
                      deleteButtonTooltipMessage: 'Quitar etiqueta',
                      onDeleted: enabled
                          ? () => onChanged([...tags]..removeAt(index))
                          : null,
                    ),
                ],
              ),
            ),
          ),
        LabeledField(
          label: 'Agregar etiqueta',
          enabled: enabled,
          child: ActivityTextField(
            controller: controller,
            focusNode: focusNode,
            enabled: enabled,
            textInputAction: TextInputAction.done,
            // Enter agrega la etiqueta sin quitar el foco.
            onEditingComplete: _commit,
            onChanged: (text) {
              if (text.contains(',')) _commit();
            },
            autocorrect: false,
            enableIMEPersonalizedLearning: false,
            decoration: InputDecoration(
              helperText: 'Enter o coma para agregar',
              errorText: errorText,
            ),
          ),
        ),
      ],
    );
  }
}
