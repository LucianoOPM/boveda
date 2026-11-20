import 'package:flutter/material.dart';

import '../theme/colors.dart';
import '../theme/extensions.dart';
import '../theme/spacing.dart';

/// Cápsula de estado (`docs/DESIGN.md`, 6.1): píldora negra con un LED de 6 px
/// y un texto corto en `meta`.
///
/// Abierta, el LED y el texto van en el acento; [locked], el LED va en
/// `errorStrong` y el texto en `error`. El LED es fijo: el pulso animado de la
/// bóveda abierta no está implementado. Es solo presentación: no sabe si la
/// bóveda está abierta, lo dice quien la usa.
///
/// El lector de pantalla anuncia [semanticsLabel] (por omisión, [text]): un
/// texto en mayúsculas suelto no se lee bien.
class StatusCapsule extends StatelessWidget {
  const StatusCapsule({
    super.key,
    required this.text,
    this.locked = false,
    this.semanticsLabel,
  });

  final String text;
  final bool locked;
  final String? semanticsLabel;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final status = theme.extension<StatusColors>() ?? StatusColors.standard;
    final meta = (theme.extension<BovedaText>() ?? BovedaText.standard).meta;
    final ledColor = locked ? status.errorStrong : theme.colorScheme.primary;
    final textColor = locked ? theme.colorScheme.error : ledColor;
    return Semantics(
      container: true,
      label: semanticsLabel ?? text,
      child: ExcludeSemantics(
        child: DecoratedBox(
          decoration: BoxDecoration(
            color: BovedaColors.capsuleBg,
            borderRadius: BorderRadius.circular(Radii.full),
            border: Border.all(color: BovedaColors.capsuleBorder),
          ),
          child: Padding(
            padding: const EdgeInsets.symmetric(
              horizontal: Space.md,
              vertical: Space.sm - 2,
            ),
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Container(
                  key: const Key('status-led'),
                  width: 6,
                  height: 6,
                  decoration: BoxDecoration(
                    color: ledColor,
                    shape: BoxShape.circle,
                  ),
                ),
                const SizedBox(width: Space.sm - 2),
                // `Flexible`: con texto grande el texto se parte en dos líneas
                // en vez de desbordar.
                Flexible(
                  child: Text(
                    text,
                    style: meta.copyWith(
                      color: textColor,
                      fontWeight: FontWeight.w700,
                    ),
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
