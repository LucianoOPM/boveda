import 'package:flutter/material.dart';

import 'activity_text_field.dart';

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
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (tags.isNotEmpty)
          // Los chips son parte del formulario: tocarlos no cierra el teclado.
          TextFieldTapRegion(
            child: Padding(
              padding: const EdgeInsets.only(bottom: 8),
              child: Wrap(
                spacing: 8,
                children: [
                  for (final (index, tag) in tags.indexed)
                    InputChip(
                      label: Text(tag, overflow: TextOverflow.ellipsis),
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
        ActivityTextField(
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
            labelText: 'Agregar etiqueta',
            helperText: 'Enter o coma para agregar',
            errorText: errorText,
          ),
        ),
      ],
    );
  }
}
