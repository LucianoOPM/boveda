// Pruebas de los respaldos con `BovedaApp` (#44): Ajustes, exportar e importar
// con el bloqueo automático y el aviso de la pantalla de desbloqueo.

import 'dart:async';

import 'package:boveda/app.dart';
import 'package:boveda/screens/biometric_offer_screen.dart';
import 'package:boveda/screens/restore_dialog.dart';
import 'package:boveda/screens/search_screen.dart';
import 'package:boveda/screens/settings_screen.dart';
import 'package:boveda/screens/unlock_screen.dart';
import 'package:boveda/services/backup_files.dart';
import 'package:boveda/src/rust/api/error.dart';
import 'package:boveda/src/rust/api/session.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'fakes.dart';
import 'lifecycle.dart';

const password = 'frase larga de prueba';

late FakeVaultSession session;
late FakeBiometricService biometric;
late FakeBackupFiles files;

Future<void> drain(WidgetTester tester) async {
  for (var i = 0; i < 10; i++) {
    await tester.pump();
  }
}

Future<void> submitPassword(WidgetTester tester) async {
  await tester.enterText(find.byKey(const Key('unlock-password')), password);
  await tester.pump();
  await tester.tap(find.byKey(const Key('unlock-submit')));
  await tester.pump();
  await drain(tester);
}

/// Abre la app con bóveda y la desbloquea.
Future<void> pumpUnlocked(WidgetTester tester) async {
  await tester.pumpWidget(
    BovedaApp(
      session: session,
      vaultExists: () async => true,
      strengthOf: fakeStrength,
      biometric: biometric,
      files: files,
    ),
  );
  await drain(tester);
  await submitPassword(tester);
  expect(find.byType(SearchScreen), findsOneWidget);
}

Future<void> openSettings(WidgetTester tester) async {
  await tester.tap(find.byKey(const Key('open-settings')));
  await tester.pump();
  await tester.pump(const Duration(seconds: 1));
  expect(find.byType(SettingsScreen), findsOneWidget);
}

void main() {
  setUp(() {
    session = FakeVaultSession();
    biometric = FakeBiometricService()..available = false;
    files = FakeBackupFiles();
  });

  group('ajustes', () {
    testWidgets('el engranaje abre Ajustes', (tester) async {
      await pumpUnlocked(tester);

      await openSettings(tester);

      expect(find.byKey(const Key('settings-export')), findsOneWidget);
      expect(find.byKey(const Key('settings-import')), findsOneWidget);
    });
  });

  group('exportar con el bloqueo automático', () {
    testWidgets('con la bóveda ya bloqueada al volver del selector, el archivo '
        'se guarda y el aviso sale una sola vez en el desbloqueo', (
      tester,
    ) async {
      final saving = Completer<bool>();
      files.onSaveTo = () => saving.future;
      await pumpUnlocked(tester);
      await openSettings(tester);
      await tester.tap(find.byKey(const Key('settings-export')));
      await drain(tester);

      // El selector oculta la app y no vuelve en 2 minutos: bloquea.
      await setLifecycle(tester, AppLifecycleState.hidden);
      await tester.pump(const Duration(minutes: 2));
      expect(session.lockCalls, 1);
      await setLifecycle(tester, AppLifecycleState.resumed);
      await drain(tester);
      expect(find.byType(UnlockScreen), findsOneWidget);

      saving.complete(true);
      await drain(tester);
      expect(find.text('Respaldo guardado'), findsOneWidget);

      // Un solo uso: tras desbloquear y volver a bloquear, ya no está.
      await submitPassword(tester);
      expect(find.byType(SearchScreen), findsOneWidget);
      await setLifecycle(tester, AppLifecycleState.hidden);
      await setLifecycle(tester, AppLifecycleState.resumed);
      await drain(tester);
      expect(find.byType(UnlockScreen), findsOneWidget);
      expect(find.text('Respaldo guardado'), findsNothing);
    });
  });
  group('importar desde Ajustes', () {
    const backupPassword = 'clave del respaldo';

    /// Abre Ajustes, toca "Importar respaldo" y deja que el selector responda.
    Future<void> startImport(WidgetTester tester) async {
      await pumpUnlocked(tester);
      await openSettings(tester);
      await tester.tap(find.byKey(const Key('settings-import')));
      await drain(tester);
    }

    Future<void> submitRestore(WidgetTester tester) async {
      await tester.enterText(
        find.byKey(const Key('restore-password')),
        backupPassword,
      );
      await tester.pump();
      await tester.tap(find.byKey(const Key('restore-submit')));
      await drain(tester);
    }

    testWidgets('importar feliz: contraseña del respaldo, temporal borrado, '
        'huella desactivada y oferta de activarla', (tester) async {
      biometric.available = true;
      await startImport(tester);
      expect(find.byType(RestoreDialog), findsOneWidget);
      expect(find.text('Reemplaza la bóveda actual.'), findsWidgets);

      await submitRestore(tester);

      expect(session.importCalls.single, (
        backup: files.tempPath,
        password: backupPassword,
      ));
      expect(files.deleted, [files.tempPath]);
      // La oferta se evalúa después de desactivar la huella.
      expect(
        biometric.calls.lastIndexOf('isAvailable'),
        greaterThan(biometric.calls.indexOf('disable')),
      );
      expect(find.byType(BiometricOfferScreen), findsOneWidget);
      expect(find.byType(SettingsScreen), findsNothing);
    });

    testWidgets('sin huella disponible llega a la principal', (tester) async {
      await startImport(tester);

      await submitRestore(tester);

      expect(find.byType(SearchScreen), findsOneWidget);
      expect(find.byType(SettingsScreen), findsNothing);
      expect(biometric.calls, contains('disable'));
    });

    testWidgets('el aviso de dañadas conserva el total tras la oferta', (
      tester,
    ) async {
      biometric.available = true;
      session.onImportBackup = (_, _) async =>
          const UnlockReport(corrupt: ['aa', 'bb'], unidentifiedCorrupt: 0);
      await startImport(tester);

      await submitRestore(tester);
      expect(find.byType(BiometricOfferScreen), findsOneWidget);
      await tester.tap(find.byKey(const Key('offer-skip')));
      await drain(tester);

      expect(find.byType(SearchScreen), findsOneWidget);
      expect(find.text('2 entradas no se pudieron leer'), findsOneWidget);
    });

    for (final (error, message) in [
      (VaultError.wrongPassword, 'Contraseña del respaldo incorrecta.'),
      (VaultError.notAVault, 'El archivo no es una bóveda.'),
      (
        VaultError.unsupportedFormatVersion,
        'Este respaldo es de una versión que la app no admite.',
      ),
    ]) {
      testWidgets('importación fallida (${error.name}): desbloqueo con el '
          'motivo, temporal borrado antes de bloquear', (tester) async {
        session.onImportBackup = (_, _) async => throw error;
        var lockCallsAtDelete = -1;
        files.onDeleteTemp = (_) async => lockCallsAtDelete = session.lockCalls;
        await startImport(tester);

        await submitRestore(tester);

        expect(find.byType(UnlockScreen), findsOneWidget);
        expect(find.text(message), findsOneWidget);
        expect(find.textContaining('/tmp'), findsNothing);
        expect(find.byType(SettingsScreen), findsNothing);
        expect(files.deleted, [files.tempPath]);
        expect(lockCallsAtDelete, 0);
        expect(session.lockCalls, 1);
      });
    }

    testWidgets('el diálogo no se puede cerrar con la importación en curso', (
      tester,
    ) async {
      final importing = Completer<UnlockReport>();
      session.onImportBackup = (_, _) => importing.future;
      await startImport(tester);

      await tester.enterText(
        find.byKey(const Key('restore-password')),
        backupPassword,
      );
      await tester.pump();
      await tester.tap(find.byKey(const Key('restore-submit')));
      await tester.pump();
      await tester.binding.handlePopRoute();
      await tester.pump();
      await tester.tapAt(const Offset(4, 4));
      await tester.pump();
      expect(find.byType(RestoreDialog), findsOneWidget);

      importing.complete(emptyReport);
      await drain(tester);
      expect(find.byType(RestoreDialog), findsNothing);
      expect(find.byType(SearchScreen), findsOneWidget);
    });

    testWidgets('si el tope vence mientras se elige el archivo no se importa', (
      tester,
    ) async {
      final picking = Completer<String?>();
      files.onPickToTemp = () => picking.future;
      await pumpUnlocked(tester);
      await openSettings(tester);
      await tester.tap(find.byKey(const Key('settings-import')));
      await drain(tester);

      await setLifecycle(tester, AppLifecycleState.hidden);
      await tester.pump(const Duration(minutes: 2));
      await setLifecycle(tester, AppLifecycleState.resumed);
      await drain(tester);
      expect(find.byType(UnlockScreen), findsOneWidget);

      picking.complete(files.tempPath);
      await drain(tester);

      expect(session.importCalls, isEmpty);
      expect(files.deleted, [files.tempPath]);
      expect(find.byType(RestoreDialog), findsNothing);
    });

    testWidgets('un archivo demasiado grande avisa sin bloquear', (
      tester,
    ) async {
      files.onPickToTemp = () async =>
          throw const BackupFilesException(BackupFailure.tooLarge);
      await startImport(tester);

      expect(
        find.text('El archivo es demasiado grande para ser un respaldo.'),
        findsOneWidget,
      );
      expect(session.lockCalls, 0);
      expect(find.byType(SettingsScreen), findsOneWidget);
      expect(session.importCalls, isEmpty);
    });
  });
}
