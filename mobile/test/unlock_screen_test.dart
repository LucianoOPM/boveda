// Pruebas de la pantalla de desbloqueo con la sesión simulada.

import 'dart:async';

import 'package:boveda/screens/unlock_screen.dart';
import 'package:boveda/src/rust/api/error.dart';
import 'package:boveda/src/rust/api/session.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'fakes.dart';

const password = 'prueba-bóveda-v2';
const passwordKey = Key('unlock-password');
const submitKey = Key('unlock-submit');
const wrong = 'Contraseña incorrecta';
const generic = 'No se pudo desbloquear la bóveda. Intenta de nuevo.';

Future<void> pumpScreen(
  WidgetTester tester,
  FakeVaultSession session, {
  ValueChanged<int>? onUnlocked,
}) async {
  await tester.pumpWidget(
    MaterialApp(
      home: UnlockScreen(session: session, onUnlocked: onUnlocked ?? (_) {}),
    ),
  );
  await tester.pump();
}

TextField field(WidgetTester tester) => tester.widget<TextField>(
  find.descendant(
    of: find.byKey(passwordKey),
    matching: find.byType(TextField),
  ),
);

FilledButton button(WidgetTester tester) =>
    tester.widget<FilledButton>(find.byKey(submitKey));

Future<void> type(WidgetTester tester, String text) async {
  await tester.enterText(find.byKey(passwordKey), text);
  await tester.pump();
}

Future<void> submitWithButton(WidgetTester tester) async {
  await tester.tap(find.byKey(submitKey));
  await tester.pump();
}

Future<void> submitWithDone(WidgetTester tester) async {
  await tester.testTextInput.receiveAction(TextInputAction.done);
  await tester.pump();
}

void main() {
  late FakeVaultSession session;

  setUp(() => session = FakeVaultSession());

  testWidgets('botón desactivado con el campo vacío y activo al escribir', (
    tester,
  ) async {
    await pumpScreen(tester, session);
    expect(button(tester).onPressed, isNull);

    await type(tester, password);
    expect(button(tester).onPressed, isNotNull);
  });

  testWidgets('durante el desbloqueo muestra el progreso y desactiva el '
      'formulario', (tester) async {
    final pending = Completer<UnlockReport>();
    session.onUnlock = (_) => pending.future;
    await pumpScreen(tester, session);
    await type(tester, password);
    await submitWithButton(tester);

    expect(find.text('Desbloqueando…'), findsOneWidget);
    expect(find.byType(CircularProgressIndicator), findsOneWidget);
    expect(field(tester).enabled, isFalse);
    expect(button(tester).onPressed, isNull);
    expect(session.unlockCalls, [password]);

    pending.complete(emptyReport);
    await tester.pump();
  });

  testWidgets('éxito llama a onUnlocked con el total de dañadas', (
    tester,
  ) async {
    session.onUnlock = (_) async =>
        UnlockReport(corrupt: ['a' * 32, 'b' * 32], unidentifiedCorrupt: 1);
    final unlocked = <int>[];
    await pumpScreen(tester, session, onUnlocked: unlocked.add);
    await type(tester, password);
    await submitWithButton(tester);
    await tester.pump();

    expect(unlocked, [3]);
  });

  testWidgets('alreadyUnlocked llama a onUnlocked con 0', (tester) async {
    session.onUnlock = (_) async => throw VaultError.alreadyUnlocked;
    final unlocked = <int>[];
    await pumpScreen(tester, session, onUnlocked: unlocked.add);
    await type(tester, password);
    await submitWithButton(tester);
    await tester.pump();

    expect(unlocked, [0]);
    expect(find.text(generic), findsNothing);
  });

  testWidgets('contraseña incorrecta: mensaje, campo vacío, sigue bloqueada y '
      'con foco', (tester) async {
    // Pendiente hasta después de un cuadro, como Argon2id en el celular: el
    // campo se dibuja desactivado y pierde el foco. Si fallara en el acto,
    // nunca se desactivaría y la prueba del foco no probaría nada.
    final pending = Completer<UnlockReport>();
    session.onUnlock = (_) => pending.future;
    final unlocked = <int>[];
    await pumpScreen(tester, session, onUnlocked: unlocked.add);
    await type(tester, 'incorrecta');
    await submitWithButton(tester);
    expect(field(tester).focusNode!.hasFocus, isFalse);

    pending.completeError(VaultError.wrongPassword);
    await tester.pumpAndSettle();

    expect(find.text(wrong), findsOneWidget);
    expect(field(tester).controller!.text, isEmpty);
    expect(field(tester).enabled, isTrue);
    expect(unlocked, isEmpty);
    expect(field(tester).focusNode!.hasFocus, isTrue);
  });

  for (final (label, error) in [
    ('VaultError.storageIo', VaultError.storageIo),
    ('una excepción con una ruta', Exception('/data/ruta/interna')),
  ]) {
    testWidgets('$label muestra el genérico sin detalles y vacía el campo', (
      tester,
    ) async {
      session.onUnlock = (_) async => throw error;
      await pumpScreen(tester, session);
      await type(tester, password);
      await submitWithButton(tester);
      await tester.pumpAndSettle();

      expect(find.text(generic), findsOneWidget);
      expect(find.text(wrong), findsNothing);
      expect(find.textContaining('/data'), findsNothing);
      expect(find.textContaining('storage'), findsNothing);
      expect(field(tester).controller!.text, isEmpty);
    });
  }

  testWidgets('el error se borra al volver a escribir', (tester) async {
    session.onUnlock = (_) async => throw VaultError.wrongPassword;
    await pumpScreen(tester, session);
    await type(tester, 'incorrecta');
    await submitWithButton(tester);
    await tester.pumpAndSettle();
    expect(find.text(wrong), findsOneWidget);

    await type(tester, 'o');
    expect(find.text(wrong), findsNothing);
  });

  testWidgets('reintento tras un fallo desbloquea', (tester) async {
    session.onUnlock = (p) async {
      if (p != password) throw VaultError.wrongPassword;
      return emptyReport;
    };
    final unlocked = <int>[];
    await pumpScreen(tester, session, onUnlocked: unlocked.add);
    await type(tester, 'incorrecta');
    await submitWithButton(tester);
    await tester.pumpAndSettle();

    await type(tester, password);
    await submitWithButton(tester);
    // `pump` y no `pumpAndSettle`: aquí nadie reemplaza la pantalla y el
    // indicador sigue girando.
    await tester.pump();

    expect(session.unlockCalls, ['incorrecta', password]);
    expect(unlocked, [0]);
  });

  testWidgets('la contraseña se envía sin modificar', (tester) async {
    await pumpScreen(tester, session);
    await type(tester, '   ');
    expect(button(tester).onPressed, isNotNull);
    await submitWithDone(tester);

    expect(session.unlockCalls, ['   ']);
  });

  testWidgets('doble envío con Hecho no lanza dos unlock', (tester) async {
    final pending = Completer<UnlockReport>();
    session.onUnlock = (_) => pending.future;
    await pumpScreen(tester, session);
    await type(tester, password);
    await tester.testTextInput.receiveAction(TextInputAction.done);
    await tester.testTextInput.receiveAction(TextInputAction.done);
    await tester.pump();

    expect(session.unlockCalls, hasLength(1));
    pending.complete(emptyReport);
    await tester.pump();
  });

  testWidgets('botón y Hecho seguidos no lanzan dos unlock', (tester) async {
    final pending = Completer<UnlockReport>();
    session.onUnlock = (_) => pending.future;
    await pumpScreen(tester, session);
    await type(tester, password);
    await tester.tap(find.byKey(submitKey));
    await tester.testTextInput.receiveAction(TextInputAction.done);
    await tester.pump();

    expect(session.unlockCalls, hasLength(1));
    pending.complete(emptyReport);
    await tester.pump();
  });

  testWidgets('Hecho con el campo vacío no llama a unlock', (tester) async {
    await pumpScreen(tester, session);
    await submitWithDone(tester);
    expect(session.unlockCalls, isEmpty);
  });

  testWidgets('Hecho justo después de una contraseña incorrecta no llama a '
      'unlock', (tester) async {
    session.onUnlock = (_) async => throw VaultError.wrongPassword;
    await pumpScreen(tester, session);
    await type(tester, 'incorrecta');
    await submitWithButton(tester);
    await tester.pumpAndSettle();
    expect(button(tester).onPressed, isNull);

    await submitWithDone(tester);
    expect(session.unlockCalls, ['incorrecta']);
  });

  testWidgets('al salir el campo queda limpio', (tester) async {
    await pumpScreen(tester, session);
    final controller = field(tester).controller!;
    await type(tester, password);
    await submitWithButton(tester);
    await tester.pump();

    await tester.pumpWidget(const SizedBox());
    expect(controller.text, isEmpty);
  });

  for (final (label, outcome) in [
    ('con éxito', () => emptyReport),
    ('con error', () => throw VaultError.wrongPassword),
  ]) {
    testWidgets('desmontada con unlock pendiente no hace nada al terminar '
        '$label', (tester) async {
      final pending = Completer<UnlockReport>();
      session.onUnlock = (_) => pending.future;
      final unlocked = <int>[];
      await pumpScreen(tester, session, onUnlocked: unlocked.add);
      await type(tester, password);
      await submitWithButton(tester);

      await tester.pumpWidget(const SizedBox());
      try {
        pending.complete(outcome());
      } on VaultError catch (error) {
        pending.completeError(error);
      }
      await tester.pumpAndSettle();

      expect(tester.takeException(), isNull);
      expect(unlocked, isEmpty);
    });
  }
}
