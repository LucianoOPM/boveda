// Pruebas del botón secundario pequeño de las acciones de fila
// (docs/DESIGN.md, 6.8).

import 'package:boveda/theme/button_styles.dart';
import 'package:boveda/theme/colors.dart';
import 'package:boveda/theme/spacing.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  const enabled = <WidgetState>{};
  const disabled = {WidgetState.disabled};

  test('fondo surfaceElevated y texto textPrimary', () {
    final style = secondaryActionStyle();

    expect(
      style.backgroundColor!.resolve(enabled),
      BovedaColors.surfaceElevated,
    );
    expect(style.foregroundColor!.resolve(enabled), BovedaColors.textPrimary);
  });

  test('deshabilitado: texto textDisabled', () {
    final style = secondaryActionStyle();

    expect(style.foregroundColor!.resolve(disabled), BovedaColors.textDisabled);
    expect(
      style.backgroundColor!.resolve(disabled),
      BovedaColors.surfaceElevated,
    );
  });

  test('con foreground: error el texto es error', () {
    final style = secondaryActionStyle(foreground: BovedaColors.error);

    expect(style.foregroundColor!.resolve(enabled), BovedaColors.error);
    expect(style.foregroundColor!.resolve(disabled), BovedaColors.textDisabled);
  });

  test('forma de radio 12 y alto mínimo de 40', () {
    final style = secondaryActionStyle();

    final shape = style.shape!.resolve(enabled)! as RoundedRectangleBorder;
    expect(shape.borderRadius, BorderRadius.circular(Radii.control));
    final size = style.minimumSize!.resolve(enabled)!;
    expect(size.height, 40);
    expect(size.width, 64);
  });

  test('no recorta el área táctil: tapTargetSize no es shrinkWrap', () {
    final style = secondaryActionStyle();

    // Con `padded` (por omisión) el botón de 40 se toca en 48.
    expect(style.tapTargetSize, isNot(MaterialTapTargetSize.shrinkWrap));
  });

  testWidgets('un TextButton con el estilo se toca en 48 dp', (tester) async {
    var taps = 0;
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: Center(
            child: TextButton(
              style: secondaryActionStyle(),
              onPressed: () => taps++,
              child: const Text('Copiar'),
            ),
          ),
        ),
      ),
    );

    expect(
      tester.getSize(find.byType(TextButton)).height,
      greaterThanOrEqualTo(48),
    );
    await tester.tap(find.text('Copiar'));
    expect(taps, 1);
  });
}
