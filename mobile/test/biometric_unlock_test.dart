// Pruebas de los flujos de activar y desbloquear con huella, con la sesión y
// el servicio simulados.

import 'dart:typed_data';

import 'package:boveda/services/biometric_service.dart';
import 'package:boveda/services/biometric_unlock.dart';
import 'package:boveda/src/rust/api/error.dart';
import 'package:boveda/src/rust/api/session.dart';
import 'package:flutter_test/flutter_test.dart';

import 'fakes.dart';

bool zeroed(List<int> key) => key.every((b) => b == 0);

BiometricException failure(BiometricFailure f) => BiometricException(f);

void main() {
  late FakeVaultSession session;
  late FakeBiometricService biometric;

  setUp(() {
    // Activar la huella exige la bóveda desbloqueada.
    session = FakeVaultSession()..locked = false;
    biometric = FakeBiometricService();
  });

  group('enableBiometric', () {
    test('activa en orden y pone la llave en ceros', () async {
      expect(await enableBiometric(session, biometric), EnableOutcome.enabled);
      expect(biometric.calls, ['beginEnable', 'completeEnable']);
      expect(
        identical(biometric.receivedKey, session.exportedKeys.single),
        isTrue,
      );
      expect(zeroed(biometric.receivedKey!), isTrue);
    });

    test('beginEnable cancelado no exporta la llave', () async {
      biometric.onBeginEnable = () async =>
          throw failure(BiometricFailure.canceled);
      expect(await enableBiometric(session, biometric), EnableOutcome.canceled);
      expect(session.exportedKeys, isEmpty);
      expect(biometric.calls, ['beginEnable']);
    });

    test('beginEnable con lockout o failed da failed', () async {
      for (final f in [BiometricFailure.lockout, BiometricFailure.failed]) {
        biometric.onBeginEnable = () async => throw failure(f);
        expect(await enableBiometric(session, biometric), EnableOutcome.failed);
      }
      expect(session.exportedKeys, isEmpty);
    });

    test('exportVaultKey que falla llama a abortEnable', () async {
      session.onExportVaultKey = () async => throw VaultError.locked;
      expect(await enableBiometric(session, biometric), EnableOutcome.failed);
      expect(biometric.calls, ['beginEnable', 'abortEnable']);
    });

    test('una llave no modificable activa igual', () async {
      session.onExportVaultKey = () async =>
          Uint8List.fromList(List.filled(32, 9)).asUnmodifiableView();
      expect(await enableBiometric(session, biometric), EnableOutcome.enabled);
    });

    test(
      'completeEnable que falla da failed y pone la llave en ceros',
      () async {
        biometric.onCompleteEnable = () async =>
            throw failure(BiometricFailure.failed);
        expect(await enableBiometric(session, biometric), EnableOutcome.failed);
        expect(zeroed(session.exportedKeys.single), isTrue);
      },
    );
  });

  group('unlockWithBiometric', () {
    test('con éxito suma las dañadas y pone la llave en ceros', () async {
      session.onUnlockWithKey = (_) async =>
          UnlockReport(corrupt: ['a' * 32, 'b' * 32], unidentifiedCorrupt: 1);
      final outcome = await unlockWithBiometric(session, biometric);
      expect(outcome, isA<BiometricUnlocked>());
      expect((outcome as BiometricUnlocked).damagedEntries, 3);
      expect(zeroed(session.unlockWithKeyCalls.single), isTrue);
    });

    test('invalidated desactiva', () async {
      biometric.onUnlock = () async =>
          throw failure(BiometricFailure.invalidated);
      expect(
        await unlockWithBiometric(session, biometric),
        isA<BiometricDisabled>(),
      );
      expect(biometric.calls, contains('disable'));
      expect(session.unlockWithKeyCalls, isEmpty);
    });

    test('wrongVaultKey desactiva y pone la llave en ceros', () async {
      session.onUnlockWithKey = (_) async => throw VaultError.wrongVaultKey;
      expect(
        await unlockWithBiometric(session, biometric),
        isA<BiometricDisabled>(),
      );
      expect(biometric.calls, contains('disable'));
      expect(zeroed(session.unlockWithKeyCalls.single), isTrue);
    });

    for (final f in [
      BiometricFailure.canceled,
      BiometricFailure.lockout,
      BiometricFailure.failed,
      BiometricFailure.busy,
    ]) {
      test('${f.name} vuelve a la contraseña sin desactivar', () async {
        biometric.onUnlock = () async => throw failure(f);
        expect(
          await unlockWithBiometric(session, biometric),
          isA<BiometricFallback>(),
        );
        expect(biometric.calls, isNot(contains('disable')));
      });
    }

    test(
      'alreadyUnlocked cuenta como éxito y pone la llave en ceros',
      () async {
        session.onUnlockWithKey = (_) async => throw VaultError.alreadyUnlocked;
        final outcome = await unlockWithBiometric(session, biometric);
        expect((outcome as BiometricUnlocked).damagedEntries, 0);
        expect(zeroed(session.unlockWithKeyCalls.single), isTrue);
      },
    );

    test('storageIo y una excepción ajena dan coreError y ponen la llave en '
        'ceros', () async {
      for (final Object error in [VaultError.storageIo, StateError('x')]) {
        session.onUnlockWithKey = (_) async => throw error;
        expect(
          await unlockWithBiometric(session, biometric),
          isA<BiometricCoreError>(),
        );
        expect(zeroed(session.unlockWithKeyCalls.last), isTrue);
      }
      expect(biometric.calls, isNot(contains('disable')));
    });

    test('una llave no modificable desbloquea igual', () async {
      // El canal real entrega un `_UnmodifiableUint8ArrayView`: ponerlo en
      // ceros lanza y no debe romper el desbloqueo.
      biometric.onUnlock = () async =>
          Uint8List.fromList(List.filled(32, 7)).asUnmodifiableView();
      expect(
        await unlockWithBiometric(session, biometric),
        isA<BiometricUnlocked>(),
      );
    });

    test('disable que falla igual da disabled', () async {
      biometric.onUnlock = () async =>
          throw failure(BiometricFailure.invalidated);
      biometric.onDisable = () async => throw failure(BiometricFailure.failed);
      expect(
        await unlockWithBiometric(session, biometric),
        isA<BiometricDisabled>(),
      );
    });
  });
}
