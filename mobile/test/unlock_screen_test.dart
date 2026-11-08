// Pruebas de la pantalla de desbloqueo con la sesión simulada.

import 'dart:async';
import 'dart:typed_data';

import 'package:boveda/screens/unlock_screen.dart';
import 'package:boveda/services/biometric_service.dart';
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
const disabledNotice =
    'La huella se desactivó. Desbloquea con tu contraseña maestra.';
const biometricKey = Key('unlock-biometric');
const enableKey = Key('unlock-enable-biometric');

/// Servicio de huella de cada prueba. Sin huella disponible ni activa por
/// omisión: las pruebas del desbloqueo con contraseña no la ven.
late FakeBiometricService biometric;

Future<void> pumpScreen(
  WidgetTester tester,
  FakeVaultSession session, {
  ValueChanged<int>? onUnlocked,
}) async {
  await tester.pumpWidget(
    MaterialApp(
      home: UnlockScreen(
        session: session,
        biometric: biometric,
        onUnlocked: onUnlocked ?? (_) {},
      ),
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

  setUp(() {
    session = FakeVaultSession();
    biometric = FakeBiometricService()..available = false;
  });

  testWidgets('al entrar el campo no tiene el foco', (tester) async {
    // Decisión del diseño ("Sin foco inicial"): Android no abre el teclado
    // durante la animación de arranque en frío; no se vuelve a intentar.
    await pumpScreen(tester, session);
    await tester.pump();
    expect(field(tester).focusNode!.hasFocus, isFalse);
  });

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

  group('huella', () {
    int unlockPrompts() => biometric.calls.where((c) => c == 'unlock').length;

    /// Deja terminar los `Future` del flujo. Tras un éxito el indicador sigue
    /// girando (quien muestra la pantalla es quien la reemplaza), así que no
    /// se usa `pumpAndSettle`.
    Future<void> drain(WidgetTester tester) async {
      for (var i = 0; i < 10; i++) {
        await tester.pump();
      }
    }

    BiometricException fail(BiometricFailure f) => BiometricException(f);

    Future<void> checkEnable(WidgetTester tester) async {
      await tester.ensureVisible(find.byKey(enableKey));
      await tester.tap(find.byKey(enableKey));
      await tester.pump();
    }

    testWidgets('con huella activa pide la huella una vez después del primer '
        'cuadro', (tester) async {
      biometric.enabled = true;
      biometric.onUnlock = () async => throw fail(BiometricFailure.canceled);
      await pumpScreen(tester, session);
      await drain(tester);
      expect(unlockPrompts(), 1);

      // Reconstruir la pantalla no vuelve a abrir el diálogo.
      await type(tester, 'a');
      await drain(tester);
      expect(unlockPrompts(), 1);
    });

    testWidgets('mientras corre la huella el campo y el botón están '
        'desactivados', (tester) async {
      biometric.enabled = true;
      final pending = Completer<Uint8List>();
      biometric.onUnlock = () => pending.future;
      await pumpScreen(tester, session);
      await tester.pump();

      expect(find.text('Desbloqueando…'), findsOneWidget);
      expect(field(tester).enabled, isFalse);
      expect(button(tester).onPressed, isNull);
      await submitWithDone(tester);
      expect(session.unlockCalls, isEmpty);

      pending.completeError(fail(BiometricFailure.canceled));
      await drain(tester);
    });

    testWidgets('huella con éxito llama a onUnlocked con las dañadas', (
      tester,
    ) async {
      biometric.enabled = true;
      session.onUnlockWithKey = (_) async =>
          const UnlockReport(corrupt: [], unidentifiedCorrupt: 2);
      final unlocked = <int>[];
      await pumpScreen(tester, session, onUnlocked: unlocked.add);
      await drain(tester);
      expect(unlocked, [2]);
      expect(session.unlockCalls, isEmpty);
    });

    testWidgets('fallback deja el foco en el campo sin mensaje', (
      tester,
    ) async {
      biometric.enabled = true;
      biometric.onUnlock = () async => throw fail(BiometricFailure.lockout);
      await pumpScreen(tester, session);
      await drain(tester);

      expect(field(tester).enabled, isTrue);
      expect(field(tester).focusNode!.hasFocus, isTrue);
      expect(find.text(wrong), findsNothing);
      expect(find.text(generic), findsNothing);
      expect(find.text(disabledNotice), findsNothing);
      expect(biometric.calls, isNot(contains('disable')));
    });

    testWidgets('disabled muestra el aviso, oculta Usar huella y muestra la '
        'casilla', (tester) async {
      biometric
        ..enabled = true
        ..available = true
        ..onUnlock = () async => throw fail(BiometricFailure.invalidated);
      await pumpScreen(tester, session);
      await drain(tester);

      expect(find.text(disabledNotice), findsOneWidget);
      expect(find.byKey(biometricKey), findsNothing);
      expect(find.byKey(enableKey), findsOneWidget);
      expect(field(tester).focusNode!.hasFocus, isTrue);
    });

    testWidgets('disabled sin huella disponible no muestra ni el botón ni la '
        'casilla', (tester) async {
      biometric
        ..enabled = true
        ..onUnlock = () async => throw fail(BiometricFailure.invalidated);
      await pumpScreen(tester, session);
      await drain(tester);

      expect(find.text(disabledNotice), findsOneWidget);
      expect(find.byKey(biometricKey), findsNothing);
      expect(find.byKey(enableKey), findsNothing);
    });

    testWidgets('coreError muestra el mensaje genérico', (tester) async {
      biometric.enabled = true;
      session.onUnlockWithKey = (_) async => throw VaultError.storageIo;
      await pumpScreen(tester, session);
      await drain(tester);

      expect(find.text(generic), findsOneWidget);
      expect(field(tester).focusNode!.hasFocus, isTrue);
      expect(biometric.calls, isNot(contains('disable')));
    });

    testWidgets('Usar huella vuelve a pedir la huella', (tester) async {
      biometric.enabled = true;
      biometric.onUnlock = () async => throw fail(BiometricFailure.canceled);
      final unlocked = <int>[];
      await pumpScreen(tester, session, onUnlocked: unlocked.add);
      await drain(tester);
      expect(unlockPrompts(), 1);

      biometric.onUnlock = () async => Uint8List.fromList(List.filled(32, 7));
      await tester.ensureVisible(find.byKey(biometricKey));
      await tester.tap(find.byKey(biometricKey));
      await drain(tester);
      expect(unlockPrompts(), 2);
      expect(unlocked, [0]);
    });

    testWidgets('sin huella activa no hay botón Usar huella ni intento '
        'automático', (tester) async {
      biometric.available = true;
      await pumpScreen(tester, session);
      await drain(tester);
      expect(find.byKey(biometricKey), findsNothing);
      expect(unlockPrompts(), 0);
    });

    testWidgets('si isEnabled lanza no hay intento automático', (tester) async {
      biometric
        ..enabled = true
        ..enabledError = StateError('sin canal');
      await pumpScreen(tester, session);
      await drain(tester);

      expect(unlockPrompts(), 0);
      expect(field(tester).enabled, isTrue);
      expect(find.byKey(biometricKey), findsNothing);
    });

    testWidgets('la casilla no aparece sin huella disponible ni con la huella '
        'activa', (tester) async {
      await pumpScreen(tester, session);
      await drain(tester);
      expect(find.byKey(enableKey), findsNothing);

      biometric
        ..available = true
        ..enabled = true
        ..onUnlock = () async => throw fail(BiometricFailure.canceled);
      await tester.pumpWidget(const SizedBox());
      await pumpScreen(tester, session);
      await drain(tester);
      expect(find.byKey(enableKey), findsNothing);
    });

    testWidgets('casilla marcada y contraseña correcta activan antes de '
        'onUnlocked', (tester) async {
      biometric.available = true;
      final order = <String>[];
      biometric.onCompleteEnable = () async => order.add('completeEnable');
      await pumpScreen(
        tester,
        session,
        onUnlocked: (_) => order.add('onUnlocked'),
      );
      await drain(tester);
      await checkEnable(tester);
      await type(tester, password);
      await submitWithButton(tester);
      await drain(tester);

      expect(session.unlockCalls, [password]);
      expect(order, ['completeEnable', 'onUnlocked']);
    });

    testWidgets('mientras activa el botón muestra Activando huella…', (
      tester,
    ) async {
      biometric.available = true;
      final pending = Completer<void>();
      biometric.onBeginEnable = () => pending.future;
      await pumpScreen(tester, session);
      await drain(tester);
      await checkEnable(tester);
      await type(tester, password);
      await submitWithButton(tester);
      await tester.pump();

      expect(find.text('Activando huella…'), findsOneWidget);
      pending.complete();
      await drain(tester);
    });

    testWidgets('casilla marcada y contraseña incorrecta no activan', (
      tester,
    ) async {
      biometric.available = true;
      session.onUnlock = (_) async => throw VaultError.wrongPassword;
      await pumpScreen(tester, session);
      await drain(tester);
      await checkEnable(tester);
      await type(tester, password);
      await submitWithButton(tester);
      await drain(tester);

      expect(find.text(wrong), findsOneWidget);
      expect(biometric.calls, isNot(contains('beginEnable')));
      expect(session.exportedKeys, isEmpty);
    });

    testWidgets('casilla sin marcar no activa', (tester) async {
      biometric.available = true;
      final unlocked = <int>[];
      await pumpScreen(tester, session, onUnlocked: unlocked.add);
      await drain(tester);
      await type(tester, password);
      await submitWithButton(tester);
      await drain(tester);

      expect(unlocked, [0]);
      expect(biometric.calls, isNot(contains('beginEnable')));
    });

    testWidgets('activación cancelada desde el desbloqueo sigue sin mensaje', (
      tester,
    ) async {
      biometric
        ..available = true
        ..onBeginEnable = () async => throw fail(BiometricFailure.canceled);
      final unlocked = <int>[];
      await pumpScreen(tester, session, onUnlocked: unlocked.add);
      await drain(tester);
      await checkEnable(tester);
      await type(tester, password);
      await submitWithButton(tester);
      await drain(tester);

      expect(unlocked, [0]);
      expect(find.byType(SnackBar), findsNothing);
    });

    for (final succeed in [true, false]) {
      final label = succeed ? 'éxito' : 'error';
      testWidgets('desmontada con la huella pendiente no hace nada al terminar '
          '($label)', (tester) async {
        biometric.enabled = true;
        final pending = Completer<Uint8List>();
        biometric.onUnlock = () => pending.future;
        final unlocked = <int>[];
        await pumpScreen(tester, session, onUnlocked: unlocked.add);
        await tester.pump();
        await tester.pumpWidget(const SizedBox());

        if (succeed) {
          pending.complete(Uint8List.fromList(List.filled(32, 7)));
        } else {
          pending.completeError(fail(BiometricFailure.invalidated));
        }
        await drain(tester);
        expect(tester.takeException(), isNull);
        expect(unlocked, isEmpty);
      });
    }
  });
}
