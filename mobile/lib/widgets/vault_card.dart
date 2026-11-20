import 'package:flutter/material.dart';

import '../theme/colors.dart';
import '../theme/extensions.dart';
import '../theme/spacing.dart';
import 'tag_badge.dart';

/// Tarjeta de una entrada en la lista de la bóveda (`docs/DESIGN.md`, 6.2).
///
/// De izquierda a derecha: la inicial del servicio en una caja, y el servicio,
/// hasta dos etiquetas como insignias (`+N` para el resto) y el usuario o la
/// URL en una línea cada uno. Solo muestra lo que la lista ya trae: servicio,
/// usuario o URL, y etiquetas. Nunca un secreto, ni favicons ni íconos de
/// categoría: descargarlos revelaría a qué servicios hay cuenta.
///
/// Presionada, el borde pasa a `borderPressed` y la tarjeta se reduce a 0.98
/// en 100 ms; con las animaciones reducidas (`disableAnimations`) no hay
/// escala. El lector de pantalla la anuncia como un botón "servicio, detalle"
/// con la pista "abrir".
class VaultCard extends StatefulWidget {
  const VaultCard({
    super.key,
    required this.service,
    required this.tags,
    this.detail,
    required this.onTap,
  });

  final String service;
  final List<String> tags;

  /// Usuario o, si no hay, la URL tal como está guardada.
  final String? detail;

  final VoidCallback onTap;

  /// Cuántas insignias se muestran antes del `+N`.
  static const maxBadges = 2;

  /// Separación entre insignias (`DESIGN.md` 6.2): no está en la escala base.
  static const _badgeGap = 6.0;

  @override
  State<VaultCard> createState() => _VaultCardState();
}

class _VaultCardState extends State<VaultCard> {
  bool _pressed = false;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final text = theme.textTheme;
    final styles = theme.extension<BovedaText>() ?? BovedaText.standard;
    final service = widget.service;
    final detail = widget.detail;
    final tags = widget.tags;
    // El primer grafema, para acentos y emoji compuestos. El núcleo no admite
    // servicios vacíos; '?' es solo defensa.
    final initial = service.isEmpty
        ? '?'
        : service.characters.first.toUpperCase();
    final reduceMotion = MediaQuery.disableAnimationsOf(context);
    return AnimatedScale(
      scale: _pressed && !reduceMotion ? 0.98 : 1,
      duration: const Duration(milliseconds: 100),
      child: Semantics(
        button: true,
        label: detail == null ? service : '$service, $detail',
        onTapHint: 'abrir',
        onTap: widget.onTap,
        excludeSemantics: true,
        child: Material(
          key: const Key('vault-card-surface'),
          color: BovedaColors.surface2,
          clipBehavior: Clip.antiAlias,
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(Radii.card),
            side: BorderSide(
              color: _pressed
                  ? BovedaColors.borderPressed
                  : BovedaColors.borderCard,
            ),
          ),
          child: InkWell(
            onTap: widget.onTap,
            onHighlightChanged: (pressed) => setState(() => _pressed = pressed),
            child: Padding(
              padding: const EdgeInsets.all(Space.s14),
              child: Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Container(
                    key: const Key('vault-card-initial'),
                    width: 40,
                    height: 40,
                    alignment: Alignment.center,
                    decoration: BoxDecoration(
                      color: BovedaColors.iconBox,
                      borderRadius: BorderRadius.circular(Radii.control),
                    ),
                    child: Text(
                      initial,
                      style: styles.metric.copyWith(
                        color: theme.colorScheme.primary,
                      ),
                    ),
                  ),
                  const SizedBox(width: Space.md),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          service,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: text.titleMedium,
                        ),
                        if (tags.isNotEmpty) ...[
                          const SizedBox(height: Space.xs),
                          Wrap(
                            spacing: VaultCard._badgeGap,
                            runSpacing: Space.xs,
                            crossAxisAlignment: WrapCrossAlignment.center,
                            children: [
                              for (final tag in tags.take(VaultCard.maxBadges))
                                TagBadge(label: tag),
                              if (tags.length > VaultCard.maxBadges)
                                Text(
                                  '+${tags.length - VaultCard.maxBadges}',
                                  style: styles.meta.copyWith(
                                    color: theme.colorScheme.onSurfaceVariant,
                                  ),
                                ),
                            ],
                          ),
                        ],
                        if (detail != null) ...[
                          const SizedBox(height: Space.xs),
                          Text(
                            detail,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: text.bodySmall,
                          ),
                        ],
                      ],
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}
