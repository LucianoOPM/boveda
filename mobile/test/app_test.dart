// Pruebas del widget raíz: qué pantalla se muestra según la bóveda.

import 'dart:async';
import 'dart:io';

import 'package:boveda/app.dart';
import 'package:boveda/screens/first_run_screen.dart';
import 'package:boveda/src/rust/api/error.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'fakes.dart';

Future<void> pumpApp(
  WidgetTester tester,
  FakeVaultSession session,
  Future<bool> Function() vaultExists,
) {
  return tester.pumpWidget(
    BovedaApp(
      session: session,
      vaultExists: vaultExists,
      strengthOf: fakeStrength,
    ),
  );
}

void main() {
  late FakeVaultSession session;

  setUp(() => session = FakeVaultSession());

  testWidgets('sin bóveda abre el primer uso', (tester) async {
    await pumpApp(tester, session, () async => false);
    await tester.pumpAndSettle();

    expect(find.byType(FirstRunScreen), findsOneWidget);
    expect(session.openCalls, 0);
  });

  testWidgets('con bóveda abre el marcador de desbloqueo', (tester) async {
    await pumpApp(tester, session, () async => true);
    await tester.pumpAndSettle();

    expect(find.text('Desbloqueo: #37'), findsOneWidget);
    expect(find.byType(FirstRunScreen), findsNothing);
    expect(session.openCalls, 1);
  });

  testWidgets('si open falla muestra el error genérico, nunca el primer uso', (
    tester,
  ) async {
    session.openError = VaultError.notAVault;
    await pumpApp(tester, session, () async => true);
    await tester.pumpAndSettle();

    expect(find.text('No se pudo abrir la bóveda.'), findsOneWidget);
    expect(find.byType(FirstRunScreen), findsNothing);
  });

  testWidgets('si la comprobación del archivo lanza muestra el error '
      'genérico', (tester) async {
    await pumpApp(
      tester,
      session,
      () async => throw const FileSystemException('sin acceso'),
    );
    await tester.pumpAndSettle();

    expect(find.text('No se pudo abrir la bóveda.'), findsOneWidget);
    expect(find.byType(FirstRunScreen), findsNothing);
  });

  testWidgets('mientras comprueba muestra un indicador de carga', (
    tester,
  ) async {
    final exists = Completer<bool>();
    await pumpApp(tester, session, () => exists.future);
    await tester.pump();

    expect(find.byType(CircularProgressIndicator), findsOneWidget);
    expect(find.byType(FirstRunScreen), findsNothing);

    exists.complete(false);
    await tester.pumpAndSettle();
    expect(find.byType(FirstRunScreen), findsOneWidget);
  });

  testWidgets('al crear la bóveda pasa a la pantalla principal', (
    tester,
  ) async {
    await pumpApp(tester, session, () async => false);
    await tester.pumpAndSettle();

    const password = 'frase larga de prueba';
    await tester.enterText(
      find.byKey(const Key('first-run-password')),
      password,
    );
    await tester.enterText(find.byKey(const Key('first-run-confirm')), password);
    await tester.pump();
    await tester.ensureVisible(find.byKey(const Key('first-run-accept')));
    await tester.tap(find.byKey(const Key('first-run-accept')));
    await tester.pump();
    await tester.ensureVisible(find.byKey(const Key('first-run-create')));
    await tester.tap(find.byKey(const Key('first-run-create')));
    await tester.pumpAndSettle();

    expect(find.text('Pantalla principal: #40'), findsOneWidget);
    expect(find.byType(FirstRunScreen), findsNothing);
    expect(session.createCalls, [password]);
  });
}
