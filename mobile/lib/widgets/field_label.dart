import 'package:flutter/material.dart';

import '../theme/colors.dart';
import '../theme/extensions.dart';

/// Etiqueta de un campo, encima de él: [text] en MAYÚSCULAS, en `meta` y
/// `textSecondary` (`docs/DESIGN.md`, 3 y 10). Con [enabled] en `false` va en
/// `textDisabled`, como el campo al que acompaña.
///
/// Es solo lo que se ve. Quien la usa pone aparte la etiqueta semántica del
/// campo con el texto original, para que el lector de pantalla no lea
/// mayúsculas.
class FieldLabel extends StatelessWidget {
  const FieldLabel(this.text, {super.key, this.enabled = true});

  final String text;
  final bool enabled;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Text(
      text.toUpperCase(),
      style: (theme.extension<BovedaText>() ?? BovedaText.standard).meta
          .copyWith(
            color: enabled
                ? theme.colorScheme.onSurfaceVariant
                : BovedaColors.textDisabled,
          ),
    );
  }
}
