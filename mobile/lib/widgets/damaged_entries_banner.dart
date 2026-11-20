import 'package:flutter/material.dart';

import '../theme/colors.dart';
import '../theme/icons.dart';
import '../theme/spacing.dart';

/// Aviso de entradas que no se pudieron leer al desbloquear.
///
/// [count] es el total de dañadas del `UnlockReport` (con y sin id). Con 0,
/// o después de tocar "Entendido", no muestra nada.
///
/// Va dentro del árbol de la pantalla, no con
/// `ScaffoldMessenger.showMaterialBanner`: ese método lo muestra en todos los
/// `Scaffold` registrados y podría seguir visible en la pantalla de
/// desbloqueo después de bloquear.
///
/// El estado de oculto vive en este widget. Es correcto porque entre dos
/// desbloqueos siempre se pasa por el bloqueo y el aviso sale del árbol, así
/// que cada desbloqueo lo muestra una vez. La pantalla principal (#40) debe
/// mantenerlo en una posición que salga del árbol al bloquear; en un
/// ancestro que sobreviva al bloqueo, el estado de oculto se arrastraría al
/// siguiente desbloqueo.
class DamagedEntriesBanner extends StatefulWidget {
  const DamagedEntriesBanner({super.key, required this.count});

  final int count;

  @override
  State<DamagedEntriesBanner> createState() => _DamagedEntriesBannerState();
}

class _DamagedEntriesBannerState extends State<DamagedEntriesBanner> {
  bool _dismissed = false;

  @override
  Widget build(BuildContext context) {
    if (_dismissed || widget.count == 0) return const SizedBox.shrink();
    final message = widget.count == 1
        ? '1 entrada no se pudo leer'
        : '${widget.count} entradas no se pudieron leer';
    final theme = Theme.of(context);
    final error = theme.colorScheme.error;
    // Con el texto del sistema más grande que un 110 %, "Entendido" deja al
    // mensaje una columna muy estrecha: pasa debajo, a la derecha.
    final stacked = MediaQuery.textScalerOf(context).scale(10) / 10 > 1.1;
    final text = Row(
      children: [
        Icon(BovedaIcons.triangleAlert, size: 20, color: error),
        const SizedBox(width: Space.sm),
        Expanded(
          child: Text(
            message,
            style: theme.textTheme.bodyMedium?.copyWith(
              color: theme.colorScheme.onSurface,
            ),
          ),
        ),
        if (!stacked) const SizedBox(width: Space.sm),
        if (!stacked) _dismissButton(error),
      ],
    );
    // Una tarjeta de `errorContainer` (`DESIGN.md` 6.6), no un
    // `MaterialBanner`: este no admite radio ni borde.
    return Padding(
      padding: const EdgeInsets.fromLTRB(Space.lg, 0, Space.lg, Space.s10),
      child: Material(
        key: const Key('damaged-banner'),
        color: BovedaColors.errorContainer,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(Radii.card),
          side: BorderSide(color: error.withValues(alpha: 0.38)),
        ),
        child: Padding(
          padding: const EdgeInsets.all(Space.md),
          child: stacked
              ? Column(
                  mainAxisSize: MainAxisSize.min,
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    text,
                    Align(
                      alignment: Alignment.centerRight,
                      child: _dismissButton(error),
                    ),
                  ],
                )
              : text,
        ),
      ),
    );
  }

  Widget _dismissButton(Color error) {
    return TextButton(
      style: TextButton.styleFrom(foregroundColor: error),
      onPressed: () => setState(() => _dismissed = true),
      child: const Text('Entendido'),
    );
  }
}
