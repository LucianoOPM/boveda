// Pruebas de la huella tras importar un respaldo desde Ajustes (#86): si
// estaba activa, `VaultGate` la reactiva sola con la llave de la bóveda
// importada; si no, o si no sale, ofrece como siempre.

import 'dart:async';

import 'package:boveda/app.dart';
import 'package:boveda/screens/biometric_offer_screen.dart';
import 'package:boveda/screens/search_screen.dart';
import 'package:boveda/screens/unlock_screen.dart';
import 'package:boveda/services/biometric_service.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'fakes.dart';
import 'lifecycle.dart';

const password = 'frase larga de prueba';
const backupPassword = 'clave del respaldo';

late FakeVaultSession session;
late FakeBiometricService biometric;
late FakeBackupFiles files;

Future<void> drain(WidgetTester tester) async {
  for (var i = 0; i < 10; i++) {
    await tester.pump();
  }
}

/// Abre la app, desbloquea con la contraseña (sin huella), deja la huella
/// como [enabled] y empieza a importar desde Ajustes: queda el diálogo.
Future<void> startImport(WidgetTester tester, {required bool enabled}) async {
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
  await tester.enterText(find.byKey(const Key('unlock-password')), password);
  await tester.pump();
  await tester.tap(find.byKey(const Key('unlock-submit')));
  await tester.pump();
  await drain(tester);
  expect(find.byType(SearchScreen), findsOneWidget);

  biometric
    ..available = true
    ..enabled = enabled
    ..calls.clear();
  await tester.tap(find.byKey(const Key('open-settings')));
  await tester.pump();
  await tester.pump(const Duration(seconds: 1));
  await tester.tap(find.byKey(const Key('settings-import')));
  await drain(tester);
}

Future<void> submitRestore(WidgetTester tester) async {
  await tester.enterText(
    find.byKey(const Key('restore-password')),
    backupPassword,
  );
  await tester.pump();
  await tester.tap(find.byKey(const Key('restore-submit')));
  await drain(tester);
}

void main() {
  setUp(() {
    session = FakeVaultSession();
    biometric = FakeBiometricService()..available = false;
    files = FakeBackupFiles();
  });

  testWidgets('con la huella activa antes, importar la reactiva y entra a la '
      'bóveda sin la oferta', (tester) async {
    await startImport(tester, enabled: true);

    await submitRestore(tester);

    expect(biometric.calls, containsAllInOrder(['disable', 'beginEnable']));
    expect(biometric.calls, contains('completeEnable'));
    expect(biometric.enabled, isTrue);
    expect(find.byType(BiometricOfferScreen), findsNothing);
    expect(find.byType(SearchScreen), findsOneWidget);
  });

  testWidgets('con la huella inactiva antes, ofrece como siempre y no la '
      'activa sola', (tester) async {
    await startImport(tester, enabled: false);

    await submitRestore(tester);

    expect(biometric.calls, isNot(contains('beginEnable')));
    expect(find.byType(BiometricOfferScreen), findsOneWidget);
  });

  testWidgets('si el usuario cancela la huella, cae en la oferta con la '
      'bóveda abierta', (tester) async {
    biometric.onBeginEnable = () async =>
        throw const BiometricException(BiometricFailure.canceled);
    await startImport(tester, enabled: true);

    await submitRestore(tester);

    expect(find.byType(BiometricOfferScreen), findsOneWidget);
    expect(biometric.enabled, isFalse);
    expect(session.locked, isFalse);
  });

  testWidgets('si la activación falla, cae en la oferta', (tester) async {
    session.onExportVaultKey = () async => throw StateError('sin llave');
    await startImport(tester, enabled: true);

    await submitRestore(tester);

    expect(find.byType(BiometricOfferScreen), findsOneWidget);
    expect(session.locked, isFalse);
  });

  testWidgets('un bloqueo mientras espera la huella no monta la bóveda ni la '
      'oferta', (tester) async {
    final waiting = Completer<void>();
    biometric.onBeginEnable = () => waiting.future;
    await startImport(tester, enabled: true);
    await submitRestore(tester);
    expect(biometric.calls, contains('beginEnable'));

    await setLifecycle(tester, AppLifecycleState.hidden);
    await setLifecycle(tester, AppLifecycleState.resumed);
    await drain(tester);
    expect(find.byType(UnlockScreen), findsOneWidget);

    waiting.complete();
    await drain(tester);

    expect(find.byType(UnlockScreen), findsOneWidget);
    expect(find.byType(SearchScreen), findsNothing);
    expect(find.byType(BiometricOfferScreen), findsNothing);
    expect(session.locked, isTrue);
  });
}
