// Pruebas de la oferta de activar la huella al terminar el primer uso.

import 'dart:async';

import 'package:boveda/screens/biometric_offer_screen.dart';
import 'package:boveda/services/biometric_service.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'fakes.dart';

const enableKey = Key('offer-enable');
const skipKey = Key('offer-skip');
const failedText = 'No se pudo activar la huella.';

void main() {
  late FakeVaultSession session;
  late FakeBiometricService biometric;
  late int done;

  setUp(() {
    session = FakeVaultSession();
    biometric = FakeBiometricService();
    done = 0;
  });

  Future<void> pumpScreen(WidgetTester tester) => tester.pumpWidget(
    MaterialApp(
      home: BiometricOfferScreen(
        session: session,
        biometric: biometric,
        onDone: () => done++,
      ),
    ),
  );

  testWidgets('muestra el título, el texto y los dos botones', (tester) async {
    await pumpScreen(tester);
    expect(find.text('¿Activar huella?'), findsOneWidget);
    expect(
      find.text(
        'Desbloquea la bóveda con tu huella. La contraseña maestra seguirá '
        'funcionando y la necesitarás si registras una huella nueva.',
      ),
      findsOneWidget,
    );
    expect(find.widgetWithText(FilledButton, 'Activar'), findsOneWidget);
    expect(find.widgetWithText(TextButton, 'Ahora no'), findsOneWidget);
  });

  testWidgets('Activar con éxito llama a onDone', (tester) async {
    await pumpScreen(tester);
    await tester.tap(find.byKey(enableKey));
    await tester.pumpAndSettle();
    expect(done, 1);
    expect(biometric.calls, ['beginEnable', 'completeEnable']);
  });

  testWidgets('Activar cancelado se queda sin mensaje', (tester) async {
    biometric.onBeginEnable = () async =>
        throw const BiometricException(BiometricFailure.canceled);
    await pumpScreen(tester);
    await tester.tap(find.byKey(enableKey));
    await tester.pumpAndSettle();
    expect(done, 0);
    expect(find.text(failedText), findsNothing);
    expect(
      tester.widget<FilledButton>(find.byKey(enableKey)).onPressed,
      isNotNull,
    );
  });

  testWidgets('Activar fallido muestra el mensaje y permite reintentar', (
    tester,
  ) async {
    biometric.onBeginEnable = () async =>
        throw const BiometricException(BiometricFailure.failed);
    await pumpScreen(tester);
    await tester.tap(find.byKey(enableKey));
    await tester.pumpAndSettle();
    expect(done, 0);
    expect(find.text(failedText), findsOneWidget);

    biometric.onBeginEnable = () async {};
    await tester.tap(find.byKey(enableKey));
    await tester.pumpAndSettle();
    expect(done, 1);
  });

  testWidgets('Ahora no llama a onDone sin activar', (tester) async {
    await pumpScreen(tester);
    await tester.tap(find.byKey(skipKey));
    await tester.pumpAndSettle();
    expect(done, 1);
    expect(biometric.calls, isEmpty);
  });

  testWidgets('los botones se desactivan mientras activa', (tester) async {
    final pending = Completer<void>();
    biometric.onBeginEnable = () => pending.future;
    await pumpScreen(tester);
    await tester.tap(find.byKey(enableKey));
    await tester.pump();
    expect(
      tester.widget<FilledButton>(find.byKey(enableKey)).onPressed,
      isNull,
    );
    expect(tester.widget<TextButton>(find.byKey(skipKey)).onPressed, isNull);

    pending.complete();
    await tester.pumpAndSettle();
    expect(done, 1);
  });
}
