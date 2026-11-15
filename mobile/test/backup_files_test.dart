// Pruebas del servicio de archivos de respaldo (#44) con el canal simulado.

import 'package:boveda/services/backup_files.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

const _channel = MethodChannel('boveda/backup');

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  final calls = <MethodCall>[];

  void mock(Future<Object?> Function(MethodCall call) handler) {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(_channel, (call) {
          calls.add(call);
          return handler(call);
        });
  }

  setUp(calls.clear);
  tearDown(
    () => TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(_channel, null),
  );

  const files = PlatformBackupFiles();

  test('newTempPath devuelve la ruta que da Kotlin', () async {
    mock((_) async => '/data/no_backup/backup-tmp/a.bak');

    expect(await files.newTempPath(), '/data/no_backup/backup-tmp/a.bak');
    expect(calls.single.method, 'newTempPath');
  });

  test('saveTo envía temp y name y devuelve true o false', () async {
    mock((_) async => true);
    expect(await files.saveTo('/t/a.bak', 'boveda-2026-11-14.db'), isTrue);
    expect(calls.single.method, 'saveTo');
    expect(calls.single.arguments, {
      'temp': '/t/a.bak',
      'name': 'boveda-2026-11-14.db',
    });

    mock((_) async => false);
    expect(await files.saveTo('/t/a.bak', 'x.db'), isFalse);
  });

  test('pickToTemp devuelve null al cancelar y la ruta al elegir', () async {
    mock((_) async => null);
    expect(await files.pickToTemp(), isNull);

    mock((_) async => '/t/b.bak');
    expect(await files.pickToTemp(), '/t/b.bak');
  });

  test('deleteTemp envía la ruta', () async {
    mock((_) async => null);
    await files.deleteTemp('/t/a.bak');

    expect(calls.single.method, 'deleteTemp');
    expect(calls.single.arguments, '/t/a.bak');
  });

  test('los códigos de Kotlin se traducen a BackupFailure', () async {
    for (final failure in BackupFailure.values) {
      mock((_) async => throw PlatformException(code: failure.name));
      await expectLater(
        files.pickToTemp(),
        throwsA(
          isA<BackupFilesException>().having(
            (e) => e.failure,
            'failure',
            failure,
          ),
        ),
      );
    }
  });

  test('un código desconocido y MissingPluginException dan failed', () async {
    mock((_) async => throw PlatformException(code: 'otro'));
    await expectLater(
      files.newTempPath(),
      throwsA(
        isA<BackupFilesException>().having(
          (e) => e.failure,
          'failure',
          BackupFailure.failed,
        ),
      ),
    );

    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(_channel, null);
    await expectLater(
      files.pickToTemp(),
      throwsA(
        isA<BackupFilesException>().having(
          (e) => e.failure,
          'failure',
          BackupFailure.failed,
        ),
      ),
    );
  });

  test('backupFileName usa la fecha con ceros', () {
    expect(backupFileName(DateTime(2026, 11, 4)), 'boveda-2026-11-04.db');
    expect(backupFileName(DateTime(2027, 1, 31)), 'boveda-2027-01-31.db');
  });
}
