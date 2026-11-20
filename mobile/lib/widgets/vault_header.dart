import 'package:flutter/material.dart';

import '../theme/colors.dart';
import '../theme/extensions.dart';
import '../theme/icons.dart';
import '../theme/spacing.dart';
import 'status_capsule.dart';

/// Encabezado de la pantalla de la bóveda (`docs/DESIGN.md`, 6.1): la marca
/// `BÓVEDA`, la cápsula de estado y el engranaje de Ajustes, con los
/// algoritmos debajo.
///
/// La cápsula siempre dice `CONOCIMIENTO CERO`: la pantalla solo existe con la
/// bóveda abierta. Con [onOpenSettings] en `null` no hay engranaje. El
/// engranaje se ve de 40 × 40 y se toca en 48 × 48.
///
/// A 360 dp el subtítulo no cabe junto a la marca, la cápsula y el engranaje:
/// va en su propia línea, debajo de la fila. Con el texto del sistema más
/// grande que un 110 %, la cápsula también pasa a su propia línea, para que la
/// marca no se parta en una columna estrecha.
class VaultHeader extends StatelessWidget {
  const VaultHeader({super.key, this.onOpenSettings});

  /// Abre Ajustes; sin él, el encabezado no muestra el engranaje.
  final VoidCallback? onOpenSettings;

  /// Por encima de esta escala del texto, la cápsula se apila bajo la marca.
  static const _stackAbove = 1.1;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final styles = theme.extension<BovedaText>() ?? BovedaText.standard;
    final stacked =
        MediaQuery.textScalerOf(context).scale(10) / 10 > _stackAbove;

    final brand = Text(
      'BÓVEDA',
      style: styles.brand.copyWith(color: theme.colorScheme.onSurface),
    );
    final subtitle = Text(
      'XCHACHA20-POLY1305 • ARGON2ID',
      style: styles.meta.copyWith(color: theme.colorScheme.onSurfaceVariant),
    );
    const capsule = StatusCapsule(
      text: 'CONOCIMIENTO CERO',
      semanticsLabel: 'Bóveda abierta',
    );
    final settings = onOpenSettings == null ? null : _settingsButton(theme);

    if (stacked) {
      return Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Expanded(child: brand),
              ?settings,
            ],
          ),
          const SizedBox(height: Space.sm),
          capsule,
          const SizedBox(height: Space.xs),
          subtitle,
        ],
      );
    }
    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            brand,
            const SizedBox(width: Space.sm),
            // La cápsula conserva su ancho y, si no cabe, se parte.
            const Expanded(
              child: Align(alignment: Alignment.centerRight, child: capsule),
            ),
            if (settings != null) ...[
              const SizedBox(width: Space.sm),
              settings,
            ],
          ],
        ),
        subtitle,
      ],
    );
  }

  Widget _settingsButton(ThemeData theme) {
    return SizedBox.square(
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
            onTap: onOpenSettings,
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
    );
  }
}
