// Pruebas de PlatformBiometricService con un manejador simulado del canal
// `boveda/biometric`. Los mensajes pasan por el codec real del canal.

import 'package:boveda/services/biometric_service.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

const channel = MethodChannel('boveda/biometric');

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const service = PlatformBiometricService();

  void handle(Future<Object?>? Function(MethodCall call)? handler) {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, handler);
  }

  tearDown(() => handle(null));

  Matcher failsWith(BiometricFailure failure) => throwsA(
    isA<BiometricException>().having((e) => e.failure, 'failure', failure),
  );

  test('cada código produce su BiometricFailure', () async {
    for (final failure in BiometricFailure.values) {
      handle((_) async => throw PlatformException(code: failure.name));
      await expectLater(service.unlock(), failsWith(failure));
    }
  });

  test('un código desconocido produce failed', () async {
    handle((_) async => throw PlatformException(code: 'raro'));
    await expectLater(service.unlock(), failsWith(BiometricFailure.failed));
    await expectLater(
      service.beginEnable(),
      failsWith(BiometricFailure.failed),
    );
  });

  test('sin manejador en Kotlin produce failed', () async {
    await expectLater(service.unlock(), failsWith(BiometricFailure.failed));
  });

  // El canal simulado entrega una lista modificable; el motor real entrega
  // una vista no modificable. Ese caso lo cubren las pruebas de
  // biometric_unlock_test.dart (una llave no modificable).
  test('unlock devuelve los bytes', () async {
    handle((call) async {
      expect(call.method, 'unlock');
      return Uint8List.fromList(List.filled(32, 7));
    });
    final key = await service.unlock();
    expect(key, hasLength(32));
    expect(key.every((b) => b == 7), isTrue);
  });

  test('unlock sin bytes produce failed', () async {
    handle((_) async => null);
    await expectLater(service.unlock(), failsWith(BiometricFailure.failed));
  });

  test('completeEnable envía la llave como Uint8List', () async {
    Object? sent;
    handle((call) async {
      expect(call.method, 'completeEnable');
      sent = call.arguments;
      return null;
    });
    await service.completeEnable(Uint8List.fromList(List.filled(32, 9)));
    expect(sent, isA<Uint8List>());
    expect(sent, List.filled(32, 9));
  });

  test('isAvailable e isEnabled devuelven lo que responde Kotlin', () async {
    handle((call) async => call.method == 'isAvailable');
    expect(await service.isAvailable(), isTrue);
    expect(await service.isEnabled(), isFalse);
  });

  test('isAvailable e isEnabled dan false si el canal lanza', () async {
    handle((_) async => throw PlatformException(code: 'failed'));
    expect(await service.isAvailable(), isFalse);
    expect(await service.isEnabled(), isFalse);
    handle(null);
    expect(await service.isAvailable(), isFalse);
    expect(await service.isEnabled(), isFalse);
  });

  test('beginEnable, abortEnable y disable llaman al método del mismo '
      'nombre', () async {
    final calls = <String>[];
    handle((call) async {
      calls.add(call.method);
      return null;
    });
    await service.beginEnable();
    await service.abortEnable();
    await service.disable();
    expect(calls, ['beginEnable', 'abortEnable', 'disable']);
  });
}
