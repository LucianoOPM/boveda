// Pruebas del tema (docs/DESIGN.md, secciones 2 a 5): esquema oscuro, contraste
// de la tabla 2.5 con los cuatro acentos, extensiones y temas de componentes.

import 'package:boveda/theme/accents.dart';
import 'package:boveda/theme/colors.dart';
import 'package:boveda/theme/extensions.dart';
import 'package:boveda/theme/spacing.dart';
import 'package:boveda/theme/theme.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'contrast.dart';

/// Un par de la tabla 2.5: texto o borde sobre su fondo.
typedef Pair = ({
  String name,
  Color fg,
  Color bg,
  double min,
  // Valor documentado, si no depende del acento.
  double? table,
});

/// Pares de la tabla 2.5 que no dependen del acento, leídos del tema.
List<Pair> fixedPairs(ThemeData t) {
  final scheme = t.colorScheme;
  final status = t.extension<StatusColors>()!;
  final syntax = t.extension<SyntaxColors>()!;
  final bgBase = t.scaffoldBackgroundColor;
  return [
    (
      name: 'textPrimary/bgBase',
      fg: scheme.onSurface,
      bg: bgBase,
      min: 4.5,
      table: 18.00,
    ),
    (
      name: 'textBody/bgBase',
      fg: t.textTheme.bodyMedium!.color!,
      bg: bgBase,
      min: 4.5,
      table: 13.38,
    ),
    (
      name: 'textSecondary/surface2',
      fg: scheme.onSurfaceVariant,
      bg: scheme.surfaceContainer,
      min: 4.5,
      table: 7.15,
    ),
    (
      name: 'error/surfaceElevated',
      fg: scheme.error,
      bg: scheme.surfaceContainerHigh,
      min: 4.5,
      table: 6.04,
    ),
    (
      name: 'error/errorContainer',
      fg: scheme.error,
      bg: scheme.errorContainer,
      min: 4.5,
      table: 6.57,
    ),
    (
      name: 'warning/surfaceElevated',
      fg: status.warning,
      bg: scheme.surfaceContainerHigh,
      min: 4.5,
      table: 10.01,
    ),
    (
      name: 'success/surfaceElevated',
      fg: status.success,
      bg: scheme.surfaceContainerHigh,
      min: 4.5,
      table: 9.59,
    ),
    (
      name: 'syntaxDigit/surface1',
      fg: syntax.digit,
      bg: scheme.surfaceContainerLow,
      min: 4.5,
      table: 11.18,
    ),
    (
      name: 'syntaxSymbol/surfaceElevated',
      fg: syntax.symbol,
      bg: scheme.surfaceContainerHigh,
      min: 4.5,
      table: 4.74,
    ),
    // No está en la tabla: el error se escribe sobre el fondo de pantalla.
    (name: 'error/bgBase', fg: scheme.error, bg: bgBase, min: 4.5, table: null),
    (
      name: 'outline/surface1',
      fg: scheme.outline,
      bg: scheme.surfaceContainerLow,
      min: 3.0,
      table: 3.86,
    ),
  ];
}

/// Contrastes de la tabla 2.5 que dependen del acento:
/// `onAccent`/`accent` y `accent`/`surface2`.
const accentTable = {
  BovedaAccent.cyberLime: (onAccent: 16.46, onSurface: 15.07),
  BovedaAccent.electricEmerald: (onAccent: 13.85, onSurface: 12.69),
  BovedaAccent.solarAmber: (onAccent: 9.75, onSurface: 8.93),
  BovedaAccent.neonCrimson: (onAccent: 5.64, onSurface: 5.17),
};

void main() {
  for (final accent in BovedaAccent.values) {
    group('buildTheme(${accent.name})', () {
      final t = buildTheme(accent);

      test('uses a dark scheme and the accent', () {
        expect(t.brightness, Brightness.dark);
        expect(t.useMaterial3, isTrue);
        expect(t.scaffoldBackgroundColor, BovedaColors.bgBase);
        expect(t.colorScheme.primary, accent.accent);
        expect(t.colorScheme.onPrimary, accent.onAccent);
        expect(t.colorScheme.error, BovedaColors.error);
        // La fortaleza "Aceptable" usa `tertiary`.
        expect(t.colorScheme.tertiary, BovedaColors.warning);
      });

      test('contrast pairs meet WCAG', () {
        final pairs = [
          ...fixedPairs(t),
          (
            name: 'onAccent/accent',
            fg: t.colorScheme.onPrimary,
            bg: t.colorScheme.primary,
            min: 4.5,
            table: null,
          ),
          (
            name: 'accent/surface2',
            fg: t.colorScheme.primary,
            bg: t.colorScheme.surfaceContainer,
            min: 4.5,
            table: null,
          ),
        ];
        for (final p in pairs) {
          expect(
            contrast(p.fg, p.bg),
            greaterThanOrEqualTo(p.min),
            reason: '${p.name} con ${accent.name}',
          );
        }
      });

      test('contrast values match the documented table', () {
        for (final p in fixedPairs(t).where((p) => p.table != null)) {
          expect(contrast(p.fg, p.bg), closeTo(p.table!, 0.02), reason: p.name);
        }
        final expected = accentTable[accent]!;
        expect(
          contrast(t.colorScheme.onPrimary, t.colorScheme.primary),
          closeTo(expected.onAccent, 0.02),
          reason: 'onAccent/accent',
        );
        expect(
          contrast(t.colorScheme.primary, t.colorScheme.surfaceContainer),
          closeTo(expected.onSurface, 0.02),
          reason: 'accent/surface2',
        );
      });

      test('extensions are present', () {
        expect(t.extension<StatusColors>(), isNotNull);
        expect(t.extension<SyntaxColors>(), isNotNull);
        expect(t.extension<AccentColors>(), isNotNull);
        final text = t.extension<BovedaText>()!;
        expect(text.secret.fontFamily, 'JetBrainsMono');
        expect(text.secret.fontSize, 17);
        expect(text.secret.fontWeight, FontWeight.w700);
        expect(t.textTheme.bodyLarge!.fontFamily, 'PlusJakartaSans');
        expect(t.extension<AccentColors>()!.border, accent.accentBorder);
      });

      test('text styles use only declared families', () {
        const declared = {'PlusJakartaSans', 'JetBrainsMono'};
        final text = t.extension<BovedaText>()!;
        final styles = [
          t.textTheme.displayLarge,
          t.textTheme.headlineSmall,
          t.textTheme.titleLarge,
          t.textTheme.titleMedium,
          t.textTheme.bodyLarge,
          t.textTheme.bodyMedium,
          t.textTheme.bodySmall,
          t.textTheme.labelLarge,
          t.textTheme.labelSmall,
          text.brand,
          text.secret,
          text.metric,
          text.meta,
        ];
        for (final style in styles) {
          expect(declared, contains(style!.fontFamily));
        }
      });

      test('disabled primary button is distinguishable', () {
        final style = t.filledButtonTheme.style!;
        final disabled = {WidgetState.disabled};
        expect(
          style.foregroundColor!.resolve(disabled),
          BovedaColors.textDisabled,
        );
        expect(style.backgroundColor!.resolve(disabled), isNot(accent.accent));
        expect(style.backgroundColor!.resolve({}), accent.accent);
        expect(style.foregroundColor!.resolve({}), accent.onAccent);
      });

      test('disabled primary button stands out on every surface', () {
        // Dentro de un diálogo (surfaceElevated), sobre la pantalla (bgBase) y
        // sobre una tarjeta (surface2): el fondo deshabilitado no puede
        // confundirse con ninguna.
        final disabled = t.filledButtonTheme.style!.backgroundColor!.resolve({
          WidgetState.disabled,
        })!;
        for (final surface in [
          BovedaColors.bgBase,
          BovedaColors.surface2,
          BovedaColors.surfaceElevated,
        ]) {
          expect(
            contrast(Color.alphaBlend(disabled, surface), surface),
            greaterThan(1.1),
            reason: 'sobre $surface con ${accent.name}',
          );
        }
      });

      test('controls follow the design', () {
        final button = t.filledButtonTheme.style!;
        expect(button.minimumSize!.resolve({})!.height, 52);
        final shape = button.shape!.resolve({}) as RoundedRectangleBorder;
        expect(shape.borderRadius, BorderRadius.circular(Radii.control));

        final input = t.inputDecorationTheme;
        expect(input.filled, isTrue);
        expect(input.fillColor, BovedaColors.surface1);
        final enabled = input.enabledBorder! as OutlineInputBorder;
        expect(enabled.borderSide.color, BovedaColors.outline);
        expect(enabled.borderSide.width, 1);
        expect(enabled.borderRadius, BorderRadius.circular(Radii.control));
        final focused = input.focusedBorder! as OutlineInputBorder;
        expect(focused.borderSide.color, accent.accent);
        expect(focused.borderSide.width, 2);

        final dialog = t.dialogTheme;
        expect(dialog.backgroundColor, BovedaColors.surfaceElevated);
        final dialogShape = dialog.shape! as RoundedRectangleBorder;
        expect(dialogShape.borderRadius, BorderRadius.circular(Radii.card));
        expect(dialogShape.side.color, BovedaColors.borderCard);
      });
    });
  }
}
