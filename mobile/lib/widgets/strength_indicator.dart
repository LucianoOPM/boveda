import 'package:flutter/material.dart';

import '../src/rust/api/password.dart';
import '../theme/colors.dart';
import '../theme/extensions.dart';
import '../theme/spacing.dart';

/// Calcula la fortaleza de una contraseña. En la app es `passwordStrength`
/// del núcleo; las pruebas la reemplazan.
typedef StrengthOf = PasswordStrength Function(String password);

/// Barra de tres segmentos y texto con la fortaleza de la contraseña maestra
/// (`docs/DESIGN.md`, 6.5).
///
/// El texto es el de siempre ("Demasiado corta", "Aceptable", "Fuerte"), en
/// `meta` y sin pasar a mayúsculas; el diseño pide otros textos y mayúsculas.
class StrengthIndicator extends StatelessWidget {
  const StrengthIndicator({super.key, required this.strength});

  final PasswordStrength strength;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final status = theme.extension<StatusColors>() ?? StatusColors.standard;
    final meta = (theme.extension<BovedaText>() ?? BovedaText.standard).meta;
    final (filled, segment, text, label) = switch (strength) {
      PasswordStrength.tooShort => (
        1,
        status.errorStrong,
        theme.colorScheme.error,
        'Demasiado corta',
      ),
      PasswordStrength.acceptable => (
        2,
        status.warning,
        status.warning,
        'Aceptable',
      ),
      PasswordStrength.strong => (3, status.success, status.success, 'Fuerte'),
    };
    return Semantics(
      label: 'Fortaleza: ${label.toLowerCase()}',
      child: ExcludeSemantics(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                for (var i = 0; i < 3; i++) ...[
                  if (i > 0) const SizedBox(width: Space.xs),
                  Expanded(
                    child: Container(
                      key: Key('strength-segment-$i'),
                      height: 4,
                      decoration: BoxDecoration(
                        color: i < filled
                            ? segment
                            : BovedaColors.borderInteractive,
                        borderRadius: BorderRadius.circular(Radii.full),
                      ),
                    ),
                  ),
                ],
              ],
            ),
            const SizedBox(height: Space.sm),
            Text(label, style: meta.copyWith(color: text)),
          ],
        ),
      ),
    );
  }
}
