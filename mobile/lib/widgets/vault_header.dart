import 'package:flutter/material.dart';

import '../theme/colors.dart';
import '../theme/extensions.dart';
import '../theme/icons.dart';
import '../theme/spacing.dart';
import 'status_capsule.dart';

/// Encabezado de la pantalla de la bóveda (`docs/DESIGN.md`, 6.1): la marca
/// `BÓVEDA` con los algoritmos debajo, la cápsula de estado y el engranaje de
/// Ajustes.
///
/// La cápsula siempre dice `CONOCIMIENTO CERO`: la pantalla solo existe con la
/// bóveda abierta. Con [onOpenSettings] en `null` no hay engranaje. El
/// engranaje se ve de 40 × 40 y se toca en 48 × 48.
class VaultHeader extends StatelessWidget {
  const VaultHeader({super.key, this.onOpenSettings});

  /// Abre Ajustes; sin él, el encabezado no muestra el engranaje.
  final VoidCallback? onOpenSettings;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final styles = theme.extension<BovedaText>() ?? BovedaText.standard;
    final openSettings = onOpenSettings;
    return Row(
      children: [
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                'BÓVEDA',
                style: styles.brand.copyWith(
                  color: theme.colorScheme.onSurface,
                ),
              ),
              Text(
                'XCHACHA20-POLY1305 • ARGON2ID',
                style: styles.meta.copyWith(
                  color: theme.colorScheme.onSurfaceVariant,
                ),
              ),
            ],
          ),
        ),
        const SizedBox(width: Space.sm),
        // Más flex que la marca: la cápsula conserva su ancho natural y solo
        // se parte con letra grande.
        const Flexible(
          flex: 2,
          child: StatusCapsule(
            text: 'CONOCIMIENTO CERO',
            semanticsLabel: 'Bóveda abierta',
          ),
        ),
        if (openSettings != null) ...[
          const SizedBox(width: Space.sm),
          SizedBox.square(
            key: const Key('open-settings'),
            dimension: 48,
            child: Tooltip(
              message: 'Ajustes',
              child: Semantics(
                button: true,
                child: InkWell(
                  customBorder: RoundedRectangleBorder(
                    borderRadius: BorderRadius.circular(Radii.control),
                  ),
                  onTap: openSettings,
                  child: Center(
                    child: Container(
                      key: const Key('open-settings-box'),
                      width: 40,
                      height: 40,
                      decoration: BoxDecoration(
                        color: BovedaColors.surfaceElevated,
                        borderRadius: BorderRadius.circular(Radii.control),
                      ),
                      child: Icon(
                        BovedaIcons.settings,
                        size: 20,
                        color: theme.colorScheme.onSurface,
                      ),
                    ),
                  ),
                ),
              ),
            ),
          ),
        ],
      ],
    );
  }
}
