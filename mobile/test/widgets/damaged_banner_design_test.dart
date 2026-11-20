// Pruebas de la presentación del aviso de entradas dañadas (docs/DESIGN.md,
// 6.6). El comportamiento sigue en `widgets_test.dart`.

import 'package:boveda/theme/accents.dart';
import 'package:boveda/theme/colors.dart';
import 'package:boveda/theme/icons.dart';
import 'package:boveda/theme/spacing.dart';
import 'package:boveda/theme/theme.dart';
import 'package:boveda/widgets/damaged_entries_banner.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

const bannerKey = Key('damaged-banner');

Future<void> pump(
  WidgetTester tester,
  int count, {
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
      home: Scaffold(body: DamagedEntriesBanner(count: count)),
    ),
  );
}

void main() {
  for (final accent in BovedaAccent.values) {
    testWidgets('con ${accent.name}: errorContainer, borde y radio', (
      tester,
    ) async {
      await pump(tester, 3, accent: accent);

      final banner = tester.widget<Material>(find.byKey(bannerKey));
      expect(banner.color, BovedaColors.errorContainer);
      final shape = banner.shape! as RoundedRectangleBorder;
      expect(shape.borderRadius, BorderRadius.circular(Radii.card));
      expect(shape.side.color, BovedaColors.error.withValues(alpha: 0.38));
    });
  }

  testWidgets('ícono de alerta en error y texto en textPrimary', (
    tester,
  ) async {
    await pump(tester, 3);

    final icon = tester.widget<Icon>(
      find.descendant(
        of: find.byKey(bannerKey),
        matching: find.byIcon(BovedaIcons.triangleAlert),
      ),
    );
    expect(icon.color, BovedaColors.error);
    final text = tester.widget<Text>(
      find.text('3 entradas no se pudieron leer'),
    );
    expect(text.style!.color, BovedaColors.textPrimary);
  });

  testWidgets('los textos de siempre, en singular y en plural', (tester) async {
    await pump(tester, 1);
    expect(find.text('1 entrada no se pudo leer'), findsOneWidget);

    await pump(tester, 7);
    expect(find.text('7 entradas no se pudieron leer'), findsOneWidget);
  });

  testWidgets('"Entendido" sigue siendo un TextButton que oculta el aviso', (
    tester,
  ) async {
    await pump(tester, 2);
    expect(find.widgetWithText(TextButton, 'Entendido'), findsOneWidget);

    await tester.tap(find.text('Entendido'));
    await tester.pump();

    expect(find.byKey(bannerKey), findsNothing);
  });

  testWidgets('con 0 no muestra nada', (tester) async {
    await pump(tester, 0);

    expect(find.byKey(bannerKey), findsNothing);
  });

  testWidgets('margen lateral de 16', (tester) async {
    await pump(tester, 2);

    final rect = tester.getRect(find.byKey(bannerKey));
    expect(rect.left, 16);
    expect(rect.right, 360 - 16);
  });

  testWidgets('a 200 % a 360 dp no desborda', (tester) async {
    await pump(tester, 12, textScale: 2);

    expect(tester.takeException(), isNull);
    expect(find.text('Entendido'), findsOneWidget);
  });
}
