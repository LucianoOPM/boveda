import 'package:flutter/material.dart';

import '../theme/extensions.dart';
import '../theme/spacing.dart';

/// Resplandor de la acción principal de una pantalla (`docs/DESIGN.md`, 4.4):
/// el acento al 28 % con desenfoque de 25, alrededor de [child].
///
/// Va fuera del botón y no en el tema porque `ButtonStyle` no admite sombras
/// de color. Solo brilla con [enabled]: un botón deshabilitado no es la acción
/// principal. No cambia el tamaño de [child]; el radio es el de los botones.
class PrimaryGlow extends StatelessWidget {
  const PrimaryGlow({super.key, required this.enabled, required this.child});

  final bool enabled;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    final glow = Theme.of(context).extension<AccentColors>()!.glow;
    return DecoratedBox(
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(Radii.control),
        boxShadow: enabled
            ? [BoxShadow(color: glow, blurRadius: 25)]
            : const <BoxShadow>[],
      ),
      child: child,
    );
  }
}
