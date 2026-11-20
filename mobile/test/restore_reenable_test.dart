// Pruebas de `runRestore`: avisa si la huella estaba activa antes de importar
// (#86), para que quien lo llama la reactive en vez de volver a ofrecerla.

import 'package:boveda/screens/restore_dialog.dart';
import 'package:boveda/src/rust/api/error.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'fakes.dart';

late FakeVaultSession session;
late FakeBackupFiles files;
late FakeBiometricService biometric;
RestoreResult? result;

Future<void> importWith(
  WidgetTester tester, {
  required bool replacesVault,
  String password = 'clave del respaldo',
}) async {
  result = null;
  await tester.pumpWidget(
    MaterialApp(
      home: Builder(
        builder: (context) => Scaffold(
          body: TextButton(
            onPressed: () async {
              result = await runRestore(
                context,
                session: session,
                files: files,
                biometric: biometric,
                replacesVault: replacesVault,
                closeOnError: replacesVault,
              );
            },
            child: const Text('importar'),
          ),
        ),
      ),
    ),
  );
  await tester.tap(find.text('importar'));
  for (var i = 0; i < 10; i++) {
    await tester.pump();
  }
  await tester.enterText(find.byKey(const Key('restore-password')), password);
  await tester.pump();
  await tester.tap(find.byKey(const Key('restore-submit')));
  for (var i = 0; i < 10; i++) {
    await tester.pump();
  }
}

void main() {
  setUp(() {
    session = FakeVaultSession()..locked = false;
    files = FakeBackupFiles();
    biometric = FakeBiometricService();
  });

  testWidgets('con la huella activa al reemplazar la bóveda, pide reactivarla '
      'y borra la copia vieja después de importar', (tester) async {
    biometric.enabled = true;

    await importWith(tester, replacesVault: true);

    expect(result, isA<RestoreImported>());
    expect((result! as RestoreImported).reenableBiometric, isTrue);
    expect(biometric.calls, ['isEnabled', 'disable']);
  });

  testWidgets('con la huella inactiva no pide reactivarla', (tester) async {
    biometric.enabled = false;

    await importWith(tester, replacesVault: true);

    expect((result! as RestoreImported).reenableBiometric, isFalse);
  });

  testWidgets('sin reemplazar la bóveda (primer uso) no pide reactivarla '
      'aunque haya una copia vieja', (tester) async {
    biometric.enabled = true;

    await importWith(tester, replacesVault: false);

    expect((result! as RestoreImported).reenableBiometric, isFalse);
    expect(biometric.calls, ['disable']);
  });

  testWidgets('si isEnabled falla, importa igual y no pide reactivarla', (
    tester,
  ) async {
    biometric.enabledError = StateError('keystore');

    await importWith(tester, replacesVault: true);

    expect(result, isA<RestoreImported>());
    expect((result! as RestoreImported).reenableBiometric, isFalse);
  });

  testWidgets('con la contraseña incorrecta no toca la huella', (tester) async {
    biometric.enabled = true;
    session.onImportBackup = (_, _) async => throw VaultError.wrongPassword;

    await importWith(tester, replacesVault: true);

    expect(result, isA<RestoreImportFailed>());
    expect(biometric.calls, ['isEnabled']);
    expect(biometric.enabled, isTrue);
  });
}
