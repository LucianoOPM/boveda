// Pruebas del canal `boveda/clipboard` con el codec real.

import 'package:boveda/services/local_clipboard.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

const channel = MethodChannel('boveda/clipboard');

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const clipboard = PlatformLocalClipboard();

  void handle(Future<Object?>? Function(MethodCall call)? handler) {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, handler);
  }

  tearDown(() => handle(null));

  test(
    'copySensitive envía el texto y devuelve si Flutter debe avisar',
    () async {
      handle((call) async {
        expect(call.method, 'copySensitive');
        expect(call.arguments, 'contraseña-de-prueba');
        return true;
      });

      expect(await clipboard.copySensitive('contraseña-de-prueba'), isTrue);
    },
  );

  test('copyPlain usa un método distinto y acepta que Android avise', () async {
    handle((call) async {
      expect(call.method, 'copyPlain');
      expect(call.arguments, 'ana@example.com');
      return false;
    });

    expect(await clipboard.copyPlain('ana@example.com'), isFalse);
  });

  test('una respuesta vacía produce un error genérico', () async {
    handle((_) async => null);

    await expectLater(
      clipboard.copySensitive('no-debe-salir'),
      throwsA(isA<LocalClipboardException>()),
    );
  });

  test(
    'un PlatformException produce un error genérico sin su mensaje',
    () async {
      handle(
        (_) async => throw PlatformException(
          code: 'falló',
          message: 'detalle con contraseña-de-prueba',
        ),
      );

      try {
        await clipboard.copySensitive('contraseña-de-prueba');
        fail('Debió lanzar');
      } on LocalClipboardException catch (error) {
        expect(error.toString(), isNot(contains('contraseña-de-prueba')));
        expect(error.toString(), isNot(contains('detalle')));
      }
    },
  );

  test('sin plugin produce un error genérico', () async {
    await expectLater(
      clipboard.copyPlain('ana@example.com'),
      throwsA(isA<LocalClipboardException>()),
    );
  });
}
