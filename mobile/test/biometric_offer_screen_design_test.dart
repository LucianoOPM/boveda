// Pruebas de la presentación de la oferta de huella (docs/DESIGN.md, 7.2 y el
// board "Oferta de huella" de Penpot). El comportamiento sigue en
// `biometric_offer_screen_test.dart`.

import 'dart:async';

import 'package:boveda/screens/biometric_offer_screen.dart';
import 'package:boveda/services/biometric_service.dart';
import 'package:boveda/theme/accents.dart';
import 'package:boveda/theme/colors.dart';
import 'package:boveda/theme/icons.dart';
import 'package:boveda/theme/theme.dart';
import 'package:boveda/widgets/primary_glow.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'fakes.dart';

const enableKey = Key('offer-enable');
const skipKey = Key('offer-skip');
const iconKey = Key('offer-icon');
const errorKey = Key('offer-error');
const failedText = 'No se pudo activar la huella.';
const bodyText =
    'Desbloquea la bóveda con tu huella. La contraseña maestra seguirá '
    'funcionando y la necesitarás si registras una huella nueva.';

late FakeVaultSession session;
late FakeBiometricService biometric;

Future<void> pumpScreen(
  WidgetTester tester, {
  BovedaAccent accent = defaultAccent,
  double textScale = 1,
}) async {
  tester.view.physicalSize = const Size(360, 640);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(
    MaterialApp(
      theme: buildTheme(accent),
      builder: (context, child) => MediaQuery(
        data: MediaQuery.of(context)
            .copyWith(textScaler: TextScaler.linear(textScale)),
        child: child!,
      ),
      home: BiometricOfferScreen(
        session: session,
        biometric: biometric,
        onDone: () {},
      ),
    ),
  );
  await tester.pump();
}

void main() {
  setUp(() {
    session = FakeVaultSession()..locked = false;
    biometric = FakeBiometricService();
  });

  for (final accent in BovedaAccent.values) {
    testWidgets('con ${accent.name} muestra el círculo con la huella', (
      tester,
    ) async {
      await pumpScreen(tester, accent: accent);

      final circle = find.byKey(iconKey);
      expect(tester.getSize(circle), const Size(64, 64));
      final decoration =
          tester.widget<Container>(circle).decoration! as BoxDecoration;
      expect(decoration.shape, BoxShape.circle);
      expect(decoration.color, accent.accentDim);
      final icon = tester.widget<Icon>(
        find.descendant(
          of: circle,
          matching: find.byIcon(BovedaIcons.fingerprint),
        ),
      );
      expect(icon.color, accent.accent);
    });
  }

  testWidgets('el título y el texto van centrados con los textos de hoy', (
    tester,
  ) async {
    await pumpScreen(tester);

    expect(
      tester.widget<Text>(find.text('¿Activar huella?')).textAlign,
      TextAlign.center,
    );
    expect(
      tester.widget<Text>(find.text(bodyText)).textAlign,
      TextAlign.center,
    );
  });

  testWidgets('los botones van al pie con los textos de siempre', (
    tester,
  ) async {
    await pumpScreen(tester);

    expect(find.widgetWithText(FilledButton, 'Activar'), findsOneWidget);
    expect(find.widgetWithText(TextButton, 'Ahora no'), findsOneWidget);
    final enable = tester.getRect(find.byKey(enableKey));
    final skip = tester.getRect(find.byKey(skipKey));
    final body = tester.getRect(find.text(bodyText));
    expect(enable.top, greaterThan(body.bottom));
    expect(skip.top, greaterThan(enable.bottom));
    // Pegados al borde inferior, no justo debajo del texto.
    expect(skip.bottom, greaterThan(640 * 0.8));
  });

  testWidgets('"Ahora no" va en textSecondary', (tester) async {
    await pumpScreen(tester);

    final style = tester.widget<TextButton>(find.byKey(skipKey)).style!;
    expect(style.foregroundColor!.resolve({}), BovedaColors.textSecondary);
  });

  testWidgets('el resplandor sigue al botón principal', (tester) async {
    final pending = Completer<void>();
    biometric.onBeginEnable = () => pending.future;
    await pumpScreen(tester);
    expect(tester.widget<PrimaryGlow>(find.byType(PrimaryGlow)).enabled, true);

    await tester.tap(find.byKey(enableKey));
    await tester.pump();

    expect(tester.widget<FilledButton>(find.byKey(enableKey)).onPressed, null);
    expect(tester.widget<PrimaryGlow>(find.byType(PrimaryGlow)).enabled, false);
    pending.complete();
    await tester.pump();
  });

  testWidgets('el error lleva el ícono de alerta', (tester) async {
    biometric.onBeginEnable = () async =>
        throw const BiometricException(BiometricFailure.failed);
    await pumpScreen(tester);
    await tester.tap(find.byKey(enableKey));
    await tester.pumpAndSettle();

    final error = find.byKey(errorKey);
    expect(
      find.descendant(of: error, matching: find.text(failedText)),
      findsOneWidget,
    );
    expect(
      find.descendant(
        of: error,
        matching: find.byIcon(BovedaIcons.triangleAlert),
      ),
      findsOneWidget,
    );
  });

  testWidgets('con texto al 200 % a 360 dp no desborda y se desplaza', (
    tester,
  ) async {
    biometric.onBeginEnable = () async =>
        throw const BiometricException(BiometricFailure.failed);
    await pumpScreen(tester, textScale: 2);
    await tester.ensureVisible(find.byKey(enableKey));
    await tester.pump();
    await tester.tap(find.byKey(enableKey));
    await tester.pumpAndSettle();

    expect(find.byKey(errorKey), findsOneWidget);
    expect(tester.takeException(), isNull);
    expect(find.byType(Scrollable), findsOneWidget);
  });
}
