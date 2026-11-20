import 'package:flutter/material.dart';

import '../theme/extensions.dart';
import '../theme/spacing.dart';
import '../theme/theme.dart';

/// Insignia de etiqueta de la tarjeta de entrada (`docs/DESIGN.md`, 6.2):
/// [label] en MAYÚSCULAS, en `meta` y en el acento, sobre `accentDim` con
/// borde `accentBorder` y radio 6.
///
/// Las mayúsculas son solo lo que se ve: la etiqueta guardada no cambia.
class TagBadge extends StatelessWidget {
  const TagBadge({super.key, required this.label});

  final String label;

  /// Relleno vertical de la insignia: no está en la escala base de 4.
  static const _verticalPadding = 2.0;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final accent =
        theme.extension<AccentColors>() ?? AccentColors.of(defaultAccent);
    final meta = (theme.extension<BovedaText>() ?? BovedaText.standard).meta;
    return Container(
      padding: const EdgeInsets.symmetric(
        horizontal: Space.sm,
        vertical: _verticalPadding,
      ),
      decoration: BoxDecoration(
        color: accent.dim,
        borderRadius: BorderRadius.circular(Radii.tag),
        border: Border.all(color: accent.border),
      ),
      child: Text(
        label.toUpperCase(),
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
        style: meta.copyWith(
          color: theme.colorScheme.primary,
          fontWeight: FontWeight.w700,
        ),
      ),
    );
  }
}
