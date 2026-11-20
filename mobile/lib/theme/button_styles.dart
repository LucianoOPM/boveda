import 'package:flutter/material.dart';

import 'colors.dart';
import 'spacing.dart';

/// Botón secundario pequeño de las acciones de una fila (`docs/DESIGN.md`,
/// 6.8): fondo `surfaceElevated`, radio 12 y 40 de alto, para un `TextButton`.
///
/// Con borde `borderInteractive` de 1: `surfaceElevated` casi no se separa de
/// la tarjeta `surface2` sobre la que va.
///
/// [foreground] cambia el color del texto (por omisión `textPrimary`; `error`
/// para una acción destructiva). No toca `tapTargetSize`: con el valor por
/// omisión (`padded`) el botón de 40 se toca en 48 dp.
ButtonStyle secondaryActionStyle({Color? foreground}) {
  return TextButton.styleFrom(
    backgroundColor: BovedaColors.surfaceElevated,
    foregroundColor: foreground ?? BovedaColors.textPrimary,
    disabledBackgroundColor: BovedaColors.surfaceElevated,
    disabledForegroundColor: BovedaColors.textDisabled,
    side: const BorderSide(color: BovedaColors.borderInteractive),
    minimumSize: const Size(64, 40),
    padding: const EdgeInsets.symmetric(horizontal: Space.lg),
    shape: RoundedRectangleBorder(
      borderRadius: BorderRadius.circular(Radii.control),
    ),
  );
}
