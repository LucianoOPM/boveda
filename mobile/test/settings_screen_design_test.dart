// Pruebas de la presentación de Ajustes (docs/DESIGN.md, 7.9 y el board
// "Ajustes" de Penpot). El comportamiento sigue en `settings_screen_test.dart`.

import 'dart:async';

import 'package:boveda/screens/settings_screen.dart';
import 'package:boveda/theme/accents.dart';
import 'package:boveda/theme/colors.dart';
import 'package:boveda/theme/icons.dart';
import 'package:boveda/theme/spacing.dart';
import 'package:boveda/theme/theme.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'fakes.dart';

late FakeVaultSession session;
late FakeBackupFiles files;

final exportTile = find.byKey(const Key('settings-export'));
final importTile = find.byKey(const Key('settings-import'));

Future<void> pumpSettings(
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
      home: SettingsScreen(
        session: session,
        files: files,
        biometric: FakeBiometricService(),
        onImported: (_) {},
        onImportFailed: (_) {},
        onNotice: (text, {bool isError = false}) {},
      ),
    ),
  );
}

/// Color con el que se pinta la flecha de la derecha de una fila.
Color? _chevronColor(WidgetTester tester, Finder tile) => tester
    .widget<RichText>(
      find.descendant(
        of: find.descendant(
          of: tile,
          matching: find.byIcon(BovedaIcons.chevronRight),
        ),
        matching: find.byType(RichText),
      ),
    )
    .text
    .style
    ?.color;

Future<void> settle(WidgetTester tester) async {
  for (var i = 0; i < 10; i++) {
    await tester.pump();
  }
}

void main() {
  setUp(() {
    session = FakeVaultSession()..locked = false;
    files = FakeBackupFiles();
  });

  for (final accent in BovedaAccent.values) {
    testWidgets('con ${accent.name}: dos tarjetas con los textos de siempre', (
      tester,
    ) async {
      await pumpSettings(tester, accent: accent);

      expect(find.widgetWithText(AppBar, 'Ajustes'), findsOneWidget);
      expect(tester.widget(exportTile), isA<ListTile>());
      expect(tester.widget(importTile), isA<ListTile>());
      expect(find.text('Exportar respaldo'), findsOneWidget);
      expect(
        find.text('Guarda una copia cifrada de la bóveda'),
        findsOneWidget,
      );
      expect(find.text('Importar respaldo'), findsOneWidget);
      expect(find.text('Reemplaza la bóveda actual'), findsOneWidget);
      for (final tile in [exportTile, importTile]) {
        final list = tester.widget<ListTile>(tile);
        expect(list.tileColor, BovedaColors.surface2);
        final shape = list.shape! as RoundedRectangleBorder;
        expect(shape.borderRadius, BorderRadius.circular(Radii.card));
        expect(shape.side.color, BovedaColors.borderCard);
      }
    });
  }

  testWidgets('las tarjetas tienen 16 de margen lateral y 10 de separación', (
    tester,
  ) async {
    await pumpSettings(tester);

    final first = tester.getRect(exportTile);
    final second = tester.getRect(importTile);
    expect(first.left, 16);
    expect(first.right, 360 - 16);
    expect(second.top - first.bottom, 10);
  });

  testWidgets('íconos de Lucide: descargar, refrescar y la flecha de la '
      'derecha', (tester) async {
    await pumpSettings(tester);

    expect(
      find.descendant(
        of: exportTile,
        matching: find.byIcon(BovedaIcons.download),
      ),
      findsOneWidget,
    );
    expect(
      find.descendant(
        of: importTile,
        matching: find.byIcon(BovedaIcons.refresh),
      ),
      findsOneWidget,
    );
    for (final tile in [exportTile, importTile]) {
      expect(_chevronColor(tester, tile), BovedaColors.textSecondary);
    }
    // Ya no queda ningún ícono de Material.
    expect(find.byIcon(Icons.upload_file), findsNothing);
    expect(find.byIcon(Icons.download), findsNothing);
  });

  testWidgets('tocar "Exportar respaldo" hace lo mismo que antes', (
    tester,
  ) async {
    await pumpSettings(tester);

    await tester.tap(exportTile);
    await settle(tester);

    expect(files.calls, ['newTempPath', 'saveTo', 'deleteTemp']);
    expect(session.exportCalls, hasLength(1));
  });

  testWidgets('tocar "Importar respaldo" abre el selector, como antes', (
    tester,
  ) async {
    // El selector se cancela: no se abre el diálogo.
    files.onPickToTemp = () async => null;
    await pumpSettings(tester);

    await tester.tap(importTile);
    await settle(tester);

    expect(files.calls, ['pickToTemp']);
  });

  testWidgets('con la pantalla ocupada las dos filas quedan desactivadas', (
    tester,
  ) async {
    final done = Completer<bool>();
    files.onSaveTo = () => done.future;
    await pumpSettings(tester);

    await tester.tap(exportTile);
    await settle(tester);

    for (final tile in [exportTile, importTile]) {
      expect(tester.widget<ListTile>(tile).enabled, isFalse);
      // La flecha se apaga igual que el ícono de la izquierda.
      expect(_chevronColor(tester, tile), isNot(BovedaColors.textSecondary));
    }
    done.complete(true);
    await settle(tester);
    expect(tester.widget<ListTile>(exportTile).enabled, isTrue);
  });

  testWidgets('a 200 % a 360 dp no desborda', (tester) async {
    await pumpSettings(tester, textScale: 2);

    expect(tester.takeException(), isNull);
    expect(find.text('Guarda una copia cifrada de la bóveda'), findsOneWidget);
  });
}
