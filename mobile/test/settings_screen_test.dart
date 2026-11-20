// Pruebas de la pantalla de ajustes (#44): exportar el respaldo.

import 'dart:async';

import 'package:boveda/screens/settings_screen.dart';
import 'package:boveda/services/backup_files.dart';
import 'package:boveda/src/rust/api/error.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'fakes.dart';

late FakeVaultSession session;
late FakeBackupFiles files;
late List<String> notices;

Future<void> pumpSettings(WidgetTester tester) async {
  await tester.pumpWidget(
    MaterialApp(
      home: SettingsScreen(
        session: session,
        files: files,
        biometric: FakeBiometricService(),
        onImported: (_, {bool reenableBiometric = false}) {},
        onImportFailed: (_) {},
        onNotice: (text, {bool isError = false}) => notices.add(text),
      ),
    ),
  );
}

/// Deja terminar los `Future` pendientes sin `pumpAndSettle`.
Future<void> settle(WidgetTester tester) async {
  for (var i = 0; i < 10; i++) {
    await tester.pump();
  }
}

Future<void> tapExport(WidgetTester tester) async {
  await tester.tap(find.byKey(const Key('settings-export')));
  await settle(tester);
}

void main() {
  setUp(() {
    session = FakeVaultSession()..locked = false;
    files = FakeBackupFiles();
    notices = [];
  });

  testWidgets('exportar crea el temporal, exporta a él, guarda con el nombre '
      'del día y lo borra', (tester) async {
    await pumpSettings(tester);

    await tapExport(tester);

    expect(files.calls, ['newTempPath', 'saveTo', 'deleteTemp']);
    expect(session.exportCalls.single, files.tempPath);
    expect(files.deleted.single, files.tempPath);
    expect(files.savedName, matches(RegExp(r'^boveda-\d{4}-\d{2}-\d{2}\.db$')));
    expect(find.text('Respaldo guardado'), findsOneWidget);
  });

  testWidgets('cancelar el selector no avisa y borra el temporal', (
    tester,
  ) async {
    files.onSaveTo = () async => false;
    await pumpSettings(tester);

    await tapExport(tester);

    expect(find.byType(SnackBar), findsNothing);
    expect(files.deleted.single, files.tempPath);
  });

  testWidgets('exportBackup que falla avisa, no abre el selector y borra el '
      'temporal', (tester) async {
    session.onExportBackup = (_) async => throw VaultError.locked;
    await pumpSettings(tester);

    await tapExport(tester);

    expect(find.text('No se pudo guardar el respaldo'), findsOneWidget);
    expect(files.calls, ['newTempPath', 'deleteTemp']);
  });

  testWidgets('un saveTo que lanza avisa y borra el temporal', (tester) async {
    files.onSaveTo = () async =>
        throw const BackupFilesException(BackupFailure.failed);
    await pumpSettings(tester);

    await tapExport(tester);

    expect(find.text('No se pudo guardar el respaldo'), findsOneWidget);
    expect(files.deleted.single, files.tempPath);
  });

  testWidgets('doble toque: un solo flujo y ambos botones desactivados', (
    tester,
  ) async {
    final done = Completer<bool>();
    files.onSaveTo = () => done.future;
    await pumpSettings(tester);

    await tapExport(tester);
    await tester.tap(find.byKey(const Key('settings-export')));
    await settle(tester);

    expect(files.calls.where((c) => c == 'newTempPath'), hasLength(1));
    for (final key in ['settings-export', 'settings-import']) {
      expect(tester.widget<ListTile>(find.byKey(Key(key))).enabled, isFalse);
    }

    done.complete(true);
    await settle(tester);
    expect(
      tester.widget<ListTile>(find.byKey(const Key('settings-export'))).enabled,
      isTrue,
    );
  });
}
