// Pruebas de la presentación del primer uso (docs/DESIGN.md, 7.1 y el board
// "Primer uso" de Penpot). El comportamiento sigue en
// `first_run_screen_test.dart`.

import 'dart:async';

import 'package:boveda/screens/first_run_screen.dart';
import 'package:boveda/theme/accents.dart';
import 'package:boveda/theme/colors.dart';
import 'package:boveda/theme/icons.dart';
import 'package:boveda/theme/theme.dart';
import 'package:boveda/widgets/primary_glow.dart';
import 'package:boveda/widgets/status_capsule.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'fakes.dart';

const passwordKey = Key('first-run-password');
const confirmKey = Key('first-run-confirm');
const acceptKey = Key('first-run-accept');
const createKey = Key('first-run-create');
const restoreKey = Key('first-run-restore');
const warningKey = Key('first-run-warning');
const errorKey = Key('first-run-error');
const strong = 'prueba-bóveda-v2-larga';

late FakeVaultSession session;

Future<void> pumpScreen(
  WidgetTester tester, {
  BovedaAccent accent = defaultAccent,
  double textScale = 1,
  Size size = const Size(360, 640),
}) async {
  tester.view.physicalSize = size;
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
      home: FirstRunScreen(
        session: session,
        strengthOf: fakeStrength,
        biometric: FakeBiometricService()..available = false,
        files: FakeBackupFiles(),
        onCreated: () {},
        onAlreadyExists: () {},
        onRestored: (_) {},
      ),
    ),
  );
  await tester.pump();
}

/// Escribe las dos contraseñas y marca la casilla de aceptación.
Future<void> fillForm(WidgetTester tester, {String password = strong}) async {
  await tester.enterText(find.byKey(passwordKey), password);
  await tester.enterText(find.byKey(confirmKey), password);
  await tester.pump();
  await tester.ensureVisible(find.byKey(acceptKey));
  await tester.tap(find.byKey(acceptKey));
  await tester.pump();
}

Future<void> drain(WidgetTester tester) async {
  for (var i = 0; i < 10; i++) {
    await tester.pump();
  }
}

void main() {
  setUp(() => session = FakeVaultSession());

  for (final accent in BovedaAccent.values) {
    testWidgets('con ${accent.name} muestra cápsula, título y etiquetas', (
      tester,
    ) async {
      await pumpScreen(tester, accent: accent);

      final capsule = find.byType(StatusCapsule);
      expect(
        find.descendant(of: capsule, matching: find.text('CONOCIMIENTO CERO')),
        findsOneWidget,
      );
      final title = find.text('Crea tu contraseña maestra');
      expect(
        tester.getTopLeft(capsule).dy,
        lessThan(tester.getTopLeft(title).dy),
      );
      final first = find.text('CONTRASEÑA MAESTRA');
      final second = find.text('CONFIRMA LA CONTRASEÑA');
      // Las claves están en el `PasswordField` entero, etiqueta incluida.
      Finder inputOf(Key key) => find.descendant(
        of: find.byKey(key),
        matching: find.byType(TextField),
      );
      expect(
        tester.getTopLeft(first).dy,
        lessThan(tester.getTopLeft(inputOf(passwordKey)).dy),
      );
      expect(
        tester.getTopLeft(second).dy,
        lessThan(tester.getTopLeft(inputOf(confirmKey)).dy),
      );
    });
  }

  testWidgets(
    'el aviso lleva la casilla de siempre dentro de warningContainer',
    (tester) async {
      await pumpScreen(tester);

      final warning = find.byKey(warningKey);
      expect(
        tester.widget<Material>(find.byKey(warningKey)).color,
        BovedaColors.warningContainer,
      );
      expect(
        find.descendant(
          of: warning,
          matching: find.byIcon(BovedaIcons.triangleAlert),
        ),
        findsOneWidget,
      );
      expect(
        find.descendant(of: warning, matching: find.byKey(acceptKey)),
        findsOneWidget,
      );
      expect(
        find.text(
          'Entiendo que si olvido la contraseña maestra pierdo el acceso a la '
          'bóveda. No se puede recuperar.',
        ),
        findsOneWidget,
      );
    },
  );

  testWidgets(
    'el botón principal sigue siendo un FilledButton con resplandor',
    (tester) async {
      await pumpScreen(tester);
      expect(
        tester.widget<FilledButton>(find.byKey(createKey)).onPressed,
        null,
      );
      expect(
        tester.widget<PrimaryGlow>(find.byType(PrimaryGlow)).enabled,
        false,
      );

      await fillForm(tester);

      expect(
        tester.widget<FilledButton>(find.byKey(createKey)).onPressed,
        isNotNull,
      );
      expect(
        tester.widget<PrimaryGlow>(find.byType(PrimaryGlow)).enabled,
        true,
      );
    },
  );

  testWidgets('"Restaurar un respaldo" sigue en la pantalla', (tester) async {
    await pumpScreen(tester);

    expect(find.byKey(restoreKey), findsOneWidget);
    expect(find.text('Restaurar un respaldo'), findsOneWidget);
  });

  testWidgets('el error de creación lleva el ícono de alerta', (tester) async {
    session.onCreate = (_) async => throw Exception('falla');
    await pumpScreen(tester);
    await fillForm(tester);
    await tester.ensureVisible(find.byKey(createKey));
    await tester.tap(find.byKey(createKey));
    await drain(tester);

    final error = find.byKey(errorKey);
    expect(
      find.descendant(
        of: error,
        matching: find.text('No se pudo crear la bóveda. Intenta de nuevo.'),
      ),
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

  testWidgets('a 360 × 640 el botón "Crear bóveda" se ve sin desplazarse', (
    tester,
  ) async {
    await pumpScreen(tester);
    await fillForm(tester);

    final button = tester.getRect(find.byKey(createKey));
    expect(button.bottom, lessThanOrEqualTo(640));
    expect(button.top, greaterThanOrEqualTo(0));
  });

  testWidgets('el botón ocupado a 200 % a 360 dp no desborda', (tester) async {
    final pending = Completer<void>();
    session.onCreate = (_) => pending.future;
    await pumpScreen(tester, textScale: 2);
    await fillForm(tester);
    await tester.ensureVisible(find.byKey(createKey));
    await tester.tap(find.byKey(createKey));
    await tester.pump();

    expect(find.text('Creando la bóveda…'), findsOneWidget);
    expect(tester.takeException(), isNull);
    pending.complete();
    await drain(tester);
  });

  testWidgets('con texto al 200 % a 360 dp no hay desbordes', (tester) async {
    session.onCreate = (_) async => throw Exception('falla');
    await pumpScreen(tester, textScale: 2);
    await fillForm(tester);
    await tester.ensureVisible(find.byKey(createKey));
    await tester.tap(find.byKey(createKey));
    await drain(tester);

    expect(find.byKey(errorKey), findsOneWidget);
    expect(tester.takeException(), isNull);
  });
}
