// Pruebas del bloqueo automático (#39): por ciclo de vida y por inactividad,
// con `BovedaApp` y los dobles de la sesión y de la huella.

import 'dart:async';

import 'package:boveda/app.dart';
import 'package:boveda/screens/biometric_offer_screen.dart';
import 'package:boveda/screens/first_run_screen.dart';
import 'package:boveda/screens/unlock_screen.dart';
import 'package:boveda/services/biometric_service.dart';
import 'package:boveda/src/rust/api/error.dart';
import 'package:boveda/src/rust/api/session.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'fakes.dart';

const mainText = 'Pantalla principal: #40';
const password = 'frase larga de prueba';

late FakeVaultSession session;
late FakeBiometricService biometric;

Future<void> pumpApp(
  WidgetTester tester, {
  bool vaultExists = true,
  Duration? autoLockTimeout,
}) async {
  await tester.pumpWidget(
    autoLockTimeout == null
        ? BovedaApp(
            session: session,
            vaultExists: () async => vaultExists,
            strengthOf: fakeStrength,
            biometric: biometric,
          )
        : BovedaApp(
            session: session,
            vaultExists: () async => vaultExists,
            strengthOf: fakeStrength,
            biometric: biometric,
            autoLockTimeout: autoLockTimeout,
          ),
  );
  await drain(tester);
}

/// Deja terminar los `Future` pendientes. No usa `pumpAndSettle`: los
/// indicadores de progreso giran sin fin.
Future<void> drain(WidgetTester tester) async {
  for (var i = 0; i < 10; i++) {
    await tester.pump();
  }
}

/// Estados en el orden en que Android los recorre al salir de la app.
const _lifecycleChain = [
  AppLifecycleState.resumed,
  AppLifecycleState.inactive,
  AppLifecycleState.hidden,
  AppLifecycleState.paused,
  AppLifecycleState.detached,
];

/// Lleva el ciclo de vida a [target] paso a paso: en las pruebas el binding
/// no sintetiza los estados intermedios, y `AppLifecycleListener` rechaza
/// las transiciones inválidas. De `detached` se vuelve directo a `resumed`.
void _walkLifecycle(WidgetTester tester, AppLifecycleState target) {
  var current = tester.binding.lifecycleState ?? AppLifecycleState.resumed;
  while (current != target) {
    if (current == AppLifecycleState.detached) {
      current = AppLifecycleState.resumed;
    } else {
      final from = _lifecycleChain.indexOf(current);
      final to = _lifecycleChain.indexOf(target);
      current = _lifecycleChain[from + (to > from ? 1 : -1)];
    }
    tester.binding.handleAppLifecycleStateChanged(current);
  }
}

/// Cambia el ciclo de vida y lo restaura a `resumed` al terminar la prueba:
/// el estado del binding sobrevive entre pruebas.
Future<void> setLifecycle(WidgetTester tester, AppLifecycleState state) async {
  addTearDown(() => _walkLifecycle(tester, AppLifecycleState.resumed));
  _walkLifecycle(tester, state);
  await tester.pump();
}

/// Escribe la contraseña en el desbloqueo y toca "Desbloquear".
Future<void> submitPassword(WidgetTester tester) async {
  await tester.enterText(find.byKey(const Key('unlock-password')), password);
  await tester.pump();
  await tester.tap(find.byKey(const Key('unlock-submit')));
  await tester.pump();
}

/// Abre la app con bóveda y la desbloquea con la contraseña.
Future<void> pumpUnlocked(WidgetTester tester, {Duration? timeout}) async {
  await pumpApp(tester, autoLockTimeout: timeout);
  await submitPassword(tester);
  await drain(tester);
  expect(find.text(mainText), findsOneWidget);
}

/// Llena el primer uso y toca "Crear bóveda".
Future<void> createFromFirstRun(WidgetTester tester) async {
  await tester.enterText(find.byKey(const Key('first-run-password')), password);
  await tester.enterText(find.byKey(const Key('first-run-confirm')), password);
  await tester.pump();
  await tester.ensureVisible(find.byKey(const Key('first-run-accept')));
  await tester.tap(find.byKey(const Key('first-run-accept')));
  await tester.pump();
  await tester.ensureVisible(find.byKey(const Key('first-run-create')));
  await tester.tap(find.byKey(const Key('first-run-create')));
  await tester.pump();
}

/// Abre la app sin bóveda, con huella disponible, y la crea: queda la oferta.
Future<void> pumpOffer(WidgetTester tester, {Duration? timeout}) async {
  biometric.available = true;
  await pumpApp(tester, vaultExists: false, autoLockTimeout: timeout);
  await createFromFirstRun(tester);
  await drain(tester);
  expect(find.byType(BiometricOfferScreen), findsOneWidget);
}

/// Marca la casilla "Usar huella para desbloquear".
Future<void> checkEnableBiometric(WidgetTester tester) async {
  final box = find.byKey(const Key('unlock-enable-biometric'));
  await tester.ensureVisible(box);
  await tester.tap(box);
  await tester.pump();
}

BuildContext mainContext(WidgetTester tester) =>
    tester.element(find.text(mainText));

/// Vuelve a `resumed` y comprueba que se ve el desbloqueo. Con la app en
/// `hidden` el binding no produce cuadros (`framesEnabled` es falso), así
/// que la pantalla nueva se construye al volver, igual que en el celular.
Future<void> expectLocked(WidgetTester tester) async {
  await setLifecycle(tester, AppLifecycleState.resumed);
  await drain(tester);
  expect(find.byType(UnlockScreen), findsOneWidget);
  expect(find.text(mainText), findsNothing);
}

void main() {
  setUp(() {
    session = FakeVaultSession();
    biometric = FakeBiometricService()..available = false;
  });

  group('ciclo de vida', () {
    testWidgets('1 unlocked y hidden bloquea', (tester) async {
      await pumpUnlocked(tester);

      await setLifecycle(tester, AppLifecycleState.hidden);

      expect(session.lockCalls, 1);
      await expectLocked(tester);
    });

    testWidgets('2 inactive no bloquea', (tester) async {
      await pumpUnlocked(tester);

      await setLifecycle(tester, AppLifecycleState.inactive);

      expect(session.lockCalls, 0);
      expect(find.text(mainText), findsOneWidget);
    });

    testWidgets('3 offerBiometric y hidden bloquea', (tester) async {
      await pumpOffer(tester);

      await setLifecycle(tester, AppLifecycleState.hidden);

      expect(session.lockCalls, 1);
      await expectLocked(tester);
      expect(find.byType(BiometricOfferScreen), findsNothing);
    });

    testWidgets('4 lock que lanza igual muestra el desbloqueo', (tester) async {
      session.lockError = StateError('falla');
      await pumpUnlocked(tester);

      await setLifecycle(tester, AppLifecycleState.hidden);

      expect(session.lockCalls, 1);
      expect(tester.takeException(), isNull);
      await expectLocked(tester);
    });

    testWidgets('5 cierra las rutas apiladas y los SnackBar', (tester) async {
      await pumpUnlocked(tester);
      final context = mainContext(tester);
      ScaffoldMessenger.of(context)
          .showSnackBar(const SnackBar(content: Text('copiado')));
      unawaited(
        Navigator.of(context).push(
          MaterialPageRoute<void>(
            builder: (_) => const Scaffold(body: Text('detalle')),
          ),
        ),
      );
      await drain(tester);
      expect(find.text('detalle'), findsOneWidget);

      await setLifecycle(tester, AppLifecycleState.hidden);
      await drain(tester);

      expect(find.text('detalle'), findsNothing);
      expect(find.text('copiado'), findsNothing);
      await expectLocked(tester);
    });

    testWidgets('5b cierra un diálogo', (tester) async {
      await pumpUnlocked(tester);
      unawaited(
        showDialog<void>(
          context: mainContext(tester),
          builder: (_) => const AlertDialog(content: Text('¿Eliminar?')),
        ),
      );
      await drain(tester);
      expect(find.text('¿Eliminar?'), findsOneWidget);

      await setLifecycle(tester, AppLifecycleState.hidden);
      await drain(tester);

      expect(find.text('¿Eliminar?'), findsNothing);
      await expectLocked(tester);
    });

    testWidgets('5c cierra una ruta con PopScope(canPop: false)', (
      tester,
    ) async {
      await pumpUnlocked(tester);
      unawaited(
        Navigator.of(mainContext(tester)).push(
          MaterialPageRoute<void>(
            builder: (_) => const PopScope(
              canPop: false,
              child: Scaffold(body: Text('edición')),
            ),
          ),
        ),
      );
      await drain(tester);
      expect(find.text('edición'), findsOneWidget);

      await setLifecycle(tester, AppLifecycleState.hidden);
      await drain(tester);

      expect(find.text('edición'), findsNothing);
      await expectLocked(tester);
    });

    for (final away in [
      AppLifecycleState.hidden,
      AppLifecycleState.paused,
      AppLifecycleState.detached,
    ]) {
      testWidgets('6 unlock pendiente: hidden no llama a lock y al terminar '
          'en ${away.name} bloquea', (tester) async {
        final pending = Completer<UnlockReport>();
        session.onUnlock = (_) => pending.future;
        await pumpApp(tester);
        await submitPassword(tester);

        await setLifecycle(tester, AppLifecycleState.hidden);
        expect(session.lockCalls, 0);
        await setLifecycle(tester, away);

        pending.complete(emptyReport);
        await drain(tester);

        expect(session.lockCalls, 1);
        expect(session.locked, isTrue);
        await expectLocked(tester);
      });
    }

    testWidgets('6 unlock pendiente: si la app vuelve antes de terminar, '
        'llega a la principal', (tester) async {
      final pending = Completer<UnlockReport>();
      session.onUnlock = (_) => pending.future;
      await pumpApp(tester);
      await submitPassword(tester);

      await setLifecycle(tester, AppLifecycleState.hidden);
      await setLifecycle(tester, AppLifecycleState.resumed);
      pending.complete(emptyReport);
      await drain(tester);

      expect(session.lockCalls, 0);
      expect(find.text(mainText), findsOneWidget);
    });

    testWidgets('6b huella: unlockWithKey pendiente y hidden bloquea al '
        'terminar', (tester) async {
      biometric
        ..available = true
        ..enabled = true;
      final pending = Completer<UnlockReport>();
      session.onUnlockWithKey = (_) => pending.future;
      await pumpApp(tester);
      expect(session.unlockWithKeyCalls, hasLength(1));

      await setLifecycle(tester, AppLifecycleState.hidden);
      expect(session.lockCalls, 0);
      pending.complete(emptyReport);
      await drain(tester);

      expect(session.lockCalls, 1);
      // Al volver, el desbloqueo nuevo pide la huella: se cancela para que se
      // quede en pantalla.
      biometric.onUnlock = () async =>
          throw const BiometricException(BiometricFailure.canceled);
      await expectLocked(tester);
    });

    testWidgets('6c alreadyUnlocked con la contraseña y la app oculta '
        'bloquea', (tester) async {
      final pending = Completer<void>();
      session.onUnlock = (_) async {
        await pending.future;
        throw VaultError.alreadyUnlocked;
      };
      await pumpApp(tester);
      await submitPassword(tester);

      await setLifecycle(tester, AppLifecycleState.hidden);
      pending.complete();
      await drain(tester);

      expect(session.lockCalls, 1);
      await expectLocked(tester);
    });

    testWidgets('6c alreadyUnlocked con la huella y la app oculta bloquea', (
      tester,
    ) async {
      biometric
        ..available = true
        ..enabled = true;
      final prompt = Completer<void>();
      biometric.onUnlock = () async {
        await prompt.future;
        return testKey();
      };
      session.onUnlockWithKey = (_) async => throw VaultError.alreadyUnlocked;
      await pumpApp(tester);

      await setLifecycle(tester, AppLifecycleState.hidden);
      prompt.complete();
      await drain(tester);

      expect(session.lockCalls, 1);
      // Al volver, el desbloqueo nuevo pide la huella: se cancela para que se
      // quede en pantalla.
      biometric.onUnlock = () async =>
          throw const BiometricException(BiometricFailure.canceled);
      await expectLocked(tester);
    });

    testWidgets('7 create pendiente y hidden: al terminar bloquea sin la '
        'oferta', (tester) async {
      biometric.available = true;
      final pending = Completer<void>();
      session.onCreate = (_) => pending.future;
      await pumpApp(tester, vaultExists: false);
      await createFromFirstRun(tester);

      await setLifecycle(tester, AppLifecycleState.hidden);
      expect(session.lockCalls, 0);
      pending.complete();
      await drain(tester);

      expect(session.lockCalls, 1);
      await expectLocked(tester);
      expect(find.byType(BiometricOfferScreen), findsNothing);
    });

    testWidgets('8 activación con la casilla y hidden: bloquea y la llave no '
        'sale de Rust', (tester) async {
      biometric.available = true;
      final begin = Completer<void>();
      biometric.onBeginEnable = () => begin.future;
      await pumpApp(tester);
      await checkEnableBiometric(tester);
      await submitPassword(tester);
      await drain(tester);
      expect(biometric.calls, contains('beginEnable'));
      final before = tester.state(find.byType(UnlockScreen));

      await setLifecycle(tester, AppLifecycleState.hidden);

      expect(session.lockCalls, 1);
      await expectLocked(tester);
      expect(
        tester.state(find.byType(UnlockScreen)),
        isNot(same(before)),
        reason: 'el desbloqueo se monta de nuevo',
      );

      begin.complete();
      await drain(tester);

      expect(biometric.calls, contains('abortEnable'));
      expect(biometric.calls, isNot(contains('completeEnable')));
      expect(session.exportedKeys, isEmpty);
      await expectLocked(tester);
      // La activación vieja termina en `failed` (`Locked`), pero su aviso no
      // sale sobre el desbloqueo nuevo.
      expect(find.text('No se pudo activar la huella.'), findsNothing);
    });

    testWidgets('9 la marca de la activación se reinicia: un desbloqueo '
        'nuevo en curso no llama a lock al ocultarse', (tester) async {
      biometric.available = true;
      await pumpApp(tester);
      await checkEnableBiometric(tester);
      await submitPassword(tester);
      await drain(tester);
      expect(find.text(mainText), findsOneWidget);
      expect(biometric.enabled, isTrue);

      await setLifecycle(tester, AppLifecycleState.hidden);
      expect(session.lockCalls, 1);
      // La huella activa se pide al volver; se cancela para usar la
      // contraseña.
      biometric.onUnlock = () async =>
          throw const BiometricException(BiometricFailure.canceled);
      await setLifecycle(tester, AppLifecycleState.resumed);
      await drain(tester);

      final pending = Completer<UnlockReport>();
      session.onUnlock = (_) => pending.future;
      await submitPassword(tester);
      await setLifecycle(tester, AppLifecycleState.hidden);

      expect(session.lockCalls, 1);
      pending.complete(emptyReport);
      await drain(tester);
    });

    testWidgets('10a oferta: beginEnable pendiente y hidden no activa al '
        'terminar', (tester) async {
      await pumpOffer(tester);
      final begin = Completer<void>();
      biometric.onBeginEnable = () => begin.future;
      await tester.tap(find.byKey(const Key('offer-enable')));
      await tester.pump();

      await setLifecycle(tester, AppLifecycleState.hidden);
      expect(session.lockCalls, 1);
      begin.complete();
      await drain(tester);

      expect(biometric.calls, contains('abortEnable'));
      expect(biometric.calls, isNot(contains('completeEnable')));
      expect(session.exportedKeys, isEmpty);
      await expectLocked(tester);
    });

    testWidgets('10b oferta: exportVaultKey pendiente al bloquear no cambia '
        'de estado', (tester) async {
      await pumpOffer(tester);
      final export = Completer<void>();
      final defaultExport = session.onExportVaultKey;
      session.onExportVaultKey = () async {
        await export.future;
        return defaultExport();
      };
      await tester.tap(find.byKey(const Key('offer-enable')));
      await tester.pump();

      await setLifecycle(tester, AppLifecycleState.hidden);
      export.complete();
      await drain(tester);

      expect(tester.takeException(), isNull);
      await expectLocked(tester);
    });

    testWidgets('hidden repetido con la app ya bloqueada no vuelve a llamar '
        'a lock', (tester) async {
      await pumpUnlocked(tester);

      await setLifecycle(tester, AppLifecycleState.hidden);
      await setLifecycle(tester, AppLifecycleState.inactive);
      await setLifecycle(tester, AppLifecycleState.hidden);
      await drain(tester);

      expect(session.lockCalls, 1);
      await expectLocked(tester);
    });

    testWidgets('hidden en el primer uso no llama a lock ni borra lo '
        'escrito', (tester) async {
      await pumpApp(tester, vaultExists: false);
      await tester.enterText(
        find.byKey(const Key('first-run-password')),
        password,
      );

      await setLifecycle(tester, AppLifecycleState.hidden);
      await setLifecycle(tester, AppLifecycleState.resumed);

      expect(session.lockCalls, 0);
      expect(find.byType(FirstRunScreen), findsOneWidget);
      expect(find.text(password), findsOneWidget);
    });

    testWidgets('hidden con la bóveda sin abrir no llama a lock', (
      tester,
    ) async {
      session.openError = StateError('no abre');
      await pumpApp(tester);

      await setLifecycle(tester, AppLifecycleState.hidden);

      expect(session.lockCalls, 0);
      expect(find.text('No se pudo abrir la bóveda.'), findsOneWidget);
    });
  });

  group('inactividad', () {
    testWidgets('11 bloquea a los 5 minutos', (tester) async {
      await pumpUnlocked(tester);

      await tester.pump(const Duration(minutes: 4, seconds: 59));
      expect(session.lockCalls, 0);
      await tester.pump(const Duration(seconds: 1));

      expect(session.lockCalls, 1);
      await expectLocked(tester);
    });

    testWidgets('12 un toque reinicia la cuenta', (tester) async {
      await pumpUnlocked(tester);

      await tester.pump(const Duration(minutes: 4));
      await tester.tapAt(const Offset(10, 10));
      await tester.pump(const Duration(minutes: 4));
      expect(session.lockCalls, 0);
      await tester.pump(const Duration(minutes: 1));

      expect(session.lockCalls, 1);
    });

    testWidgets('13 una tecla física reinicia la cuenta', (tester) async {
      await pumpUnlocked(tester);

      await tester.pump(const Duration(minutes: 4));
      await tester.sendKeyEvent(LogicalKeyboardKey.keyA);
      await tester.pump(const Duration(minutes: 4));
      expect(session.lockCalls, 0);
      await tester.pump(const Duration(minutes: 1));

      expect(session.lockCalls, 1);
    });

    testWidgets('14 en el desbloqueo no corre', (tester) async {
      await pumpApp(tester);

      await tester.pump(const Duration(minutes: 10));

      expect(session.lockCalls, 0);
    });

    testWidgets('14 en la oferta sí bloquea', (tester) async {
      await pumpOffer(tester);

      await tester.pump(const Duration(minutes: 5));

      expect(session.lockCalls, 1);
      await expectLocked(tester);
    });

    testWidgets('14b tras bloquear por inactividad no queda ningún '
        'temporizador', (tester) async {
      await pumpUnlocked(tester);
      await tester.pump(const Duration(minutes: 5));
      expect(session.lockCalls, 1);

      await tester.pump(const Duration(minutes: 10));

      expect(session.lockCalls, 1);
    });

    testWidgets('14b tras bloquear por hidden y volver no queda ningún '
        'temporizador', (tester) async {
      await pumpUnlocked(tester);
      await setLifecycle(tester, AppLifecycleState.hidden);
      await setLifecycle(tester, AppLifecycleState.resumed);
      expect(session.lockCalls, 1);

      await tester.pump(const Duration(minutes: 10));

      expect(session.lockCalls, 1);
    });

    testWidgets('15 respeta el tiempo inyectado', (tester) async {
      await pumpUnlocked(tester, timeout: const Duration(minutes: 1));

      await tester.pump(const Duration(seconds: 59));
      expect(session.lockCalls, 0);
      await tester.pump(const Duration(seconds: 1));

      expect(session.lockCalls, 1);
    });

    testWidgets('al desmontar no quedan temporizadores ni manejador de '
        'teclado', (tester) async {
      await pumpUnlocked(tester);

      await tester.pumpWidget(const SizedBox());
      // Con el manejador colgado, la tecla crearía un temporizador nuevo y
      // la prueba fallaría por temporizadores pendientes.
      await tester.sendKeyEvent(LogicalKeyboardKey.keyA);

      expect(tester.takeException(), isNull);
    });
  });
}

/// Llave de prueba de la huella.
Uint8List testKey() => Uint8List.fromList(List.filled(32, 7));
