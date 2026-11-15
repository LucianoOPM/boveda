// Pruebas de los respaldos con `BovedaApp` (#44): Ajustes, exportar e importar
// con el bloqueo automático y el aviso de la pantalla de desbloqueo.

import 'dart:async';

import 'package:boveda/app.dart';
import 'package:boveda/screens/search_screen.dart';
import 'package:boveda/screens/settings_screen.dart';
import 'package:boveda/screens/unlock_screen.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'fakes.dart';
import 'lifecycle.dart';

const password = 'frase larga de prueba';

late FakeVaultSession session;
late FakeBiometricService biometric;
late FakeBackupFiles files;

Future<void> drain(WidgetTester tester) async {
  for (var i = 0; i < 10; i++) {
    await tester.pump();
  }
}

Future<void> submitPassword(WidgetTester tester) async {
  await tester.enterText(find.byKey(const Key('unlock-password')), password);
  await tester.pump();
  await tester.tap(find.byKey(const Key('unlock-submit')));
  await tester.pump();
  await drain(tester);
}

/// Abre la app con bóveda y la desbloquea.
Future<void> pumpUnlocked(WidgetTester tester) async {
  await tester.pumpWidget(
    BovedaApp(
      session: session,
      vaultExists: () async => true,
      strengthOf: fakeStrength,
      biometric: biometric,
      files: files,
    ),
  );
  await drain(tester);
  await submitPassword(tester);
  expect(find.byType(SearchScreen), findsOneWidget);
}

Future<void> openSettings(WidgetTester tester) async {
  await tester.tap(find.byKey(const Key('open-settings')));
  await tester.pump();
  await tester.pump(const Duration(seconds: 1));
  expect(find.byType(SettingsScreen), findsOneWidget);
}

void main() {
  setUp(() {
    session = FakeVaultSession();
    biometric = FakeBiometricService()..available = false;
    files = FakeBackupFiles();
  });

  group('ajustes', () {
    testWidgets('el engranaje abre Ajustes', (tester) async {
      await pumpUnlocked(tester);

      await openSettings(tester);

      expect(find.byKey(const Key('settings-export')), findsOneWidget);
      expect(find.byKey(const Key('settings-import')), findsOneWidget);
    });
  });

  group('exportar con el bloqueo automático', () {
    testWidgets('con la bóveda ya bloqueada al volver del selector, el archivo '
        'se guarda y el aviso sale una sola vez en el desbloqueo', (
      tester,
    ) async {
      final saving = Completer<bool>();
      files.onSaveTo = () => saving.future;
      await pumpUnlocked(tester);
      await openSettings(tester);
      await tester.tap(find.byKey(const Key('settings-export')));
      await drain(tester);

      // El selector oculta la app y no vuelve en 2 minutos: bloquea.
      await setLifecycle(tester, AppLifecycleState.hidden);
      await tester.pump(const Duration(minutes: 2));
      expect(session.lockCalls, 1);
      await setLifecycle(tester, AppLifecycleState.resumed);
      await drain(tester);
      expect(find.byType(UnlockScreen), findsOneWidget);

      saving.complete(true);
      await drain(tester);
      expect(find.text('Respaldo guardado'), findsOneWidget);

      // Un solo uso: tras desbloquear y volver a bloquear, ya no está.
      await submitPassword(tester);
      expect(find.byType(SearchScreen), findsOneWidget);
      await setLifecycle(tester, AppLifecycleState.hidden);
      await setLifecycle(tester, AppLifecycleState.resumed);
      await drain(tester);
      expect(find.byType(UnlockScreen), findsOneWidget);
      expect(find.text('Respaldo guardado'), findsNothing);
    });
  });
}
