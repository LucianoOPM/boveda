import 'package:flutter/material.dart';

import '../theme/extensions.dart';

/// Etiqueta de un campo, encima de él: [text] en MAYÚSCULAS, en `meta` y
/// `textSecondary` (`docs/DESIGN.md`, 3 y 10).
///
/// Es solo lo que se ve. Quien la usa pone aparte la etiqueta semántica del
/// campo con el texto original, para que el lector de pantalla no lea
/// mayúsculas.
class FieldLabel extends StatelessWidget {
  const FieldLabel(this.text, {super.key});

  final String text;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Text(
      text.toUpperCase(),
      style: theme.extension<BovedaText>()!.meta.copyWith(
        color: theme.colorScheme.onSurfaceVariant,
      ),
    );
  }
}
