import 'package:flutter/material.dart';

import 'accents.dart';
import 'colors.dart';
import 'extensions.dart';
import 'spacing.dart';

/// Acento que usa la app. Elegir otro es el selector de Ajustes (D2), que
/// todavía no existe.
const defaultAccent = BovedaAccent.cyberLime;

/// Arma el tema oscuro de la app (`docs/DESIGN.md`, sección 5.1) con el
/// `ColorScheme` hecho a mano, sin `fromSeed` ni color dinámico.
///
/// Correspondencias con Material que el resto de la app da por hechas:
/// `primary` es el acento, `tertiary` es `warning` (fortaleza "Aceptable") y
/// `surfaceContainerLow`, `surfaceContainer` y `surfaceContainerHigh` son
/// `surface1`, `surface2` y `surfaceElevated`.
ThemeData buildTheme(BovedaAccent accent) {
  final scheme = ColorScheme(
    brightness: Brightness.dark,
    primary: accent.accent,
    onPrimary: accent.onAccent,
    secondary: accent.accent,
    onSecondary: accent.onAccent,
    tertiary: BovedaColors.warning,
    onTertiary: accent.onAccent,
    error: BovedaColors.error,
    onError: accent.onAccent,
    errorContainer: BovedaColors.errorContainer,
    onErrorContainer: BovedaColors.error,
    surface: BovedaColors.bgBase,
    onSurface: BovedaColors.textPrimary,
    onSurfaceVariant: BovedaColors.textSecondary,
    outline: BovedaColors.outline,
    outlineVariant: BovedaColors.borderCard,
    surfaceContainerLowest: BovedaColors.bgCanvas,
    surfaceContainerLow: BovedaColors.surface1,
    surfaceContainer: BovedaColors.surface2,
    surfaceContainerHigh: BovedaColors.surfaceElevated,
    surfaceContainerHighest: BovedaColors.surfaceElevated,
  );

  const sans = BovedaText.sans;
  final base = ThemeData.dark().textTheme.apply(
    fontFamily: sans,
    bodyColor: BovedaColors.textPrimary,
    displayColor: BovedaColors.textPrimary,
  );
  final textTheme = base.copyWith(
    headlineSmall: base.headlineSmall!.copyWith(
      fontSize: 22,
      fontWeight: FontWeight.w800,
      letterSpacing: -0.44,
    ),
    titleLarge: base.titleLarge!.copyWith(
      fontSize: 18,
      fontWeight: FontWeight.w700,
      letterSpacing: -0.18,
    ),
    titleMedium: base.titleMedium!.copyWith(
      fontSize: 14,
      fontWeight: FontWeight.w700,
      letterSpacing: 0,
    ),
    bodyLarge: base.bodyLarge!.copyWith(
      fontSize: 16,
      fontWeight: FontWeight.w400,
      letterSpacing: 0,
    ),
    bodyMedium: base.bodyMedium!.copyWith(
      fontSize: 13,
      fontWeight: FontWeight.w400,
      height: 1.5,
      letterSpacing: 0,
      color: BovedaColors.textBody,
    ),
    bodySmall: base.bodySmall!.copyWith(
      fontSize: 12,
      fontWeight: FontWeight.w400,
      height: 1.5,
      letterSpacing: 0,
      color: BovedaColors.textSecondary,
    ),
    labelLarge: base.labelLarge!.copyWith(
      fontSize: 14,
      fontWeight: FontWeight.w700,
      letterSpacing: 0,
    ),
  );

  final controlShape = RoundedRectangleBorder(
    borderRadius: BorderRadius.circular(Radii.control),
  );
  final cardShape = RoundedRectangleBorder(
    borderRadius: BorderRadius.circular(Radii.card),
    side: const BorderSide(color: BovedaColors.borderCard),
  );
  OutlineInputBorder inputBorder(Color color, double width) =>
      OutlineInputBorder(
        borderRadius: BorderRadius.circular(Radii.control),
        borderSide: BorderSide(color: color, width: width),
      );

  return ThemeData(
    useMaterial3: true,
    brightness: Brightness.dark,
    colorScheme: scheme,
    fontFamily: sans,
    textTheme: textTheme,
    scaffoldBackgroundColor: BovedaColors.bgBase,
    canvasColor: BovedaColors.bgBase,
    dividerColor: BovedaColors.borderSubtle,
    extensions: [
      StatusColors.standard,
      SyntaxColors.standard,
      AccentColors.of(accent),
      BovedaText.standard,
    ],
    appBarTheme: AppBarTheme(
      backgroundColor: BovedaColors.bgBase,
      foregroundColor: BovedaColors.textPrimary,
      surfaceTintColor: Colors.transparent,
      elevation: 0,
      scrolledUnderElevation: 0,
      centerTitle: false,
      titleTextStyle: textTheme.titleLarge,
    ),
    dividerTheme: const DividerThemeData(color: BovedaColors.borderSubtle),
    // Principal: un solo botón por pantalla, alto 52, radio 12.
    filledButtonTheme: FilledButtonThemeData(
      style: FilledButton.styleFrom(
        backgroundColor: accent.accent,
        foregroundColor: accent.onAccent,
        disabledBackgroundColor: BovedaColors.surfaceElevated,
        disabledForegroundColor: BovedaColors.textDisabled,
        minimumSize: const Size(64, 52),
        shape: controlShape,
        textStyle: textTheme.labelLarge,
      ),
    ),
    // Secundario: `surfaceElevated` con borde `borderInteractive`.
    outlinedButtonTheme: OutlinedButtonThemeData(
      style: OutlinedButton.styleFrom(
        backgroundColor: BovedaColors.surfaceElevated,
        foregroundColor: BovedaColors.textPrimary,
        disabledForegroundColor: BovedaColors.textDisabled,
        side: const BorderSide(color: BovedaColors.borderInteractive),
        minimumSize: const Size(64, 52),
        shape: controlShape,
        textStyle: textTheme.labelLarge,
      ),
    ),
    textButtonTheme: TextButtonThemeData(
      style: TextButton.styleFrom(
        foregroundColor: accent.accent,
        disabledForegroundColor: BovedaColors.textDisabled,
        minimumSize: const Size(64, 48),
        shape: controlShape,
        textStyle: textTheme.labelLarge,
      ),
    ),
    inputDecorationTheme: InputDecorationThemeData(
      filled: true,
      fillColor: BovedaColors.surface1,
      contentPadding: const EdgeInsets.symmetric(
        horizontal: Space.lg,
        vertical: Space.s14,
      ),
      border: inputBorder(BovedaColors.outline, 1),
      enabledBorder: inputBorder(BovedaColors.outline, 1),
      focusedBorder: inputBorder(accent.accent, 2),
      errorBorder: inputBorder(BovedaColors.error, 1),
      focusedErrorBorder: inputBorder(BovedaColors.error, 2),
      disabledBorder: inputBorder(BovedaColors.borderInteractive, 1),
      helperStyle: textTheme.bodySmall,
      errorStyle: textTheme.bodySmall!.copyWith(color: BovedaColors.error),
    ),
    cardTheme: CardThemeData(
      color: BovedaColors.surface2,
      surfaceTintColor: Colors.transparent,
      elevation: 0,
      shape: cardShape,
    ),
    dialogTheme: DialogThemeData(
      backgroundColor: BovedaColors.surfaceElevated,
      surfaceTintColor: Colors.transparent,
      elevation: 0,
      shape: cardShape,
    ),
    snackBarTheme: SnackBarThemeData(
      behavior: SnackBarBehavior.floating,
      backgroundColor: BovedaColors.surfaceElevated,
      contentTextStyle: textTheme.bodyMedium!.copyWith(
        color: BovedaColors.textPrimary,
      ),
      actionTextColor: accent.accent,
      shape: cardShape,
    ),
    progressIndicatorTheme: ProgressIndicatorThemeData(
      color: accent.accent,
      linearTrackColor: BovedaColors.borderInteractive,
    ),
  );
}
