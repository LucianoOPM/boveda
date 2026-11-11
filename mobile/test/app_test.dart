// Pruebas del widget raíz: qué pantalla se muestra según la bóveda.

import 'dart:async';
import 'dart:io';

import 'package:boveda/app.dart';
import 'package:boveda/screens/biometric_offer_screen.dart';
import 'package:boveda/screens/entry_detail_screen.dart';
import 'package:boveda/screens/entry_form_screen.dart';
import 'package:boveda/screens/first_run_screen.dart';
import 'package:boveda/screens/search_screen.dart';
import 'package:boveda/screens/unlock_screen.dart';
import 'package:boveda/services/biometric_service.dart';
import 'package:boveda/src/rust/api/entries.dart';
import 'package:boveda/src/rust/api/error.dart';
import 'package:boveda/src/rust/api/session.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'fakes.dart';

/// Servicio de huella de cada prueba. Sin huella disponible por omisión, para
/// que crear la bóveda lleve directo a la pantalla principal.
late FakeBiometricService biometric;

Future<void> pumpApp(
  WidgetTester tester,
  FakeVaultSession session,
  Future<bool> Function() vaultExists,
) {
  return tester.pumpWidget(
    BovedaApp(
      session: session,
      vaultExists: vaultExists,
      strengthOf: fakeStrength,
      biometric: biometric,
    ),
  );
}

void main() {
  late FakeVaultSession session;

  setUp(() {
    session = FakeVaultSession();
    biometric = FakeBiometricService()..available = false;
  });

  testWidgets('sin bóveda abre el primer uso', (tester) async {
    await pumpApp(tester, session, () async => false);
    await tester.pumpAndSettle();

    expect(find.byType(FirstRunScreen), findsOneWidget);
    expect(session.openCalls, 0);
  });

  testWidgets('con bóveda abre la pantalla de desbloqueo', (tester) async {
    await pumpApp(tester, session, () async => true);
    await tester.pumpAndSettle();

    expect(find.byType(UnlockScreen), findsOneWidget);
    expect(find.byType(FirstRunScreen), findsNothing);
    expect(session.openCalls, 1);
  });

  testWidgets('si open falla muestra el error genérico, nunca el primer uso', (
    tester,
  ) async {
    session.openError = VaultError.notAVault;
    await pumpApp(tester, session, () async => true);
    await tester.pumpAndSettle();

    expect(find.text('No se pudo abrir la bóveda.'), findsOneWidget);
    expect(find.byType(FirstRunScreen), findsNothing);
  });

  testWidgets('si la comprobación del archivo lanza muestra el error '
      'genérico', (tester) async {
    await pumpApp(
      tester,
      session,
      () async => throw const FileSystemException('sin acceso'),
    );
    await tester.pumpAndSettle();

    expect(find.text('No se pudo abrir la bóveda.'), findsOneWidget);
    expect(find.byType(FirstRunScreen), findsNothing);
  });

  testWidgets('mientras comprueba muestra un indicador de carga', (
    tester,
  ) async {
    final exists = Completer<bool>();
    await pumpApp(tester, session, () => exists.future);
    await tester.pump();

    expect(find.byType(CircularProgressIndicator), findsOneWidget);
    expect(find.byType(FirstRunScreen), findsNothing);

    exists.complete(false);
    await tester.pumpAndSettle();
    expect(find.byType(FirstRunScreen), findsOneWidget);
  });

  testWidgets('al crear la bóveda pasa a la pantalla principal', (
    tester,
  ) async {
    await pumpApp(tester, session, () async => false);
    await tester.pumpAndSettle();

    const password = 'frase larga de prueba';
    await tester.enterText(
      find.byKey(const Key('first-run-password')),
      password,
    );
    await tester.enterText(
      find.byKey(const Key('first-run-confirm')),
      password,
    );
    await tester.pump();
    await tester.ensureVisible(find.byKey(const Key('first-run-accept')));
    await tester.tap(find.byKey(const Key('first-run-accept')));
    await tester.pump();
    await tester.ensureVisible(find.byKey(const Key('first-run-create')));
    await tester.tap(find.byKey(const Key('first-run-create')));
    await tester.pumpAndSettle();

    expect(find.byType(SearchScreen), findsOneWidget);
    expect(find.byType(FirstRunScreen), findsNothing);
    expect(find.byType(MaterialBanner), findsNothing);
    expect(session.createCalls, [password]);
  });

  /// Llena el primer uso y toca "Crear bóveda".
  Future<void> createFromFirstRun(WidgetTester tester) async {
    const password = "frase larga de prueba";
    await tester.enterText(
      find.byKey(const Key("first-run-password")),
      password,
    );
    await tester.enterText(
      find.byKey(const Key("first-run-confirm")),
      password,
    );
    await tester.pump();
    await tester.ensureVisible(find.byKey(const Key("first-run-accept")));
    await tester.tap(find.byKey(const Key("first-run-accept")));
    await tester.pump();
    await tester.ensureVisible(find.byKey(const Key("first-run-create")));
    await tester.tap(find.byKey(const Key("first-run-create")));
    await tester.pumpAndSettle();
  }

  testWidgets("si create da AlreadyExists vuelve a evaluar y abre el "
      "desbloqueo", (tester) async {
    var exists = false;
    session.onCreate = (_) async {
      exists = true; // el archivo apareció después de comprobarlo
      throw VaultError.alreadyExists;
    };
    await pumpApp(tester, session, () async => exists);
    await tester.pumpAndSettle();

    await createFromFirstRun(tester);

    expect(find.byType(UnlockScreen), findsOneWidget);
    expect(find.byType(FirstRunScreen), findsNothing);
    expect(session.openCalls, 1);
  });

  testWidgets("si create da AlreadyExists y open falla muestra el error "
      "genérico", (tester) async {
    var exists = false;
    session.onCreate = (_) async {
      exists = true;
      throw VaultError.alreadyExists;
    };
    session.openError = VaultError.notAVault;
    await pumpApp(tester, session, () async => exists);
    await tester.pumpAndSettle();

    await createFromFirstRun(tester);

    expect(find.text("No se pudo abrir la bóveda."), findsOneWidget);
    expect(find.byType(FirstRunScreen), findsNothing);
  });

  /// Abre la app con bóveda y desbloquea desde la pantalla de desbloqueo.
  Future<void> unlockFromScreen(WidgetTester tester) async {
    await pumpApp(tester, session, () async => true);
    await tester.pumpAndSettle();
    await tester.enterText(
      find.byKey(const Key('unlock-password')),
      'prueba-bóveda-v2',
    );
    await tester.pump();
    await tester.tap(find.byKey(const Key('unlock-submit')));
    await tester.pumpAndSettle();
  }

  testWidgets('desbloquear lleva a la pantalla principal sin aviso', (
    tester,
  ) async {
    await unlockFromScreen(tester);

    expect(find.byType(SearchScreen), findsOneWidget);
    expect(find.byType(UnlockScreen), findsNothing);
    expect(find.byType(MaterialBanner), findsNothing);
    expect(session.unlockCalls, ['prueba-bóveda-v2']);
  });

  testWidgets('con 3 dañadas muestra el aviso en plural', (tester) async {
    session.onUnlock = (_) async =>
        UnlockReport(corrupt: ['a' * 32, 'b' * 32], unidentifiedCorrupt: 1);
    await unlockFromScreen(tester);

    expect(find.byType(SearchScreen), findsOneWidget);
    expect(find.text('3 entradas no se pudieron leer'), findsOneWidget);
  });

  testWidgets('con 1 dañada sin id muestra el aviso en singular', (
    tester,
  ) async {
    session.onUnlock = (_) async =>
        const UnlockReport(corrupt: [], unidentifiedCorrupt: 1);
    await unlockFromScreen(tester);

    expect(find.text('1 entrada no se pudo leer'), findsOneWidget);
  });

  testWidgets('alreadyUnlocked lleva a la pantalla principal sin aviso', (
    tester,
  ) async {
    session.onUnlock = (_) async => throw VaultError.alreadyUnlocked;
    await unlockFromScreen(tester);

    expect(find.byType(SearchScreen), findsOneWidget);
    expect(find.byType(MaterialBanner), findsNothing);
  });

  testWidgets("después de crear, con huella disponible, ofrece activarla", (
    tester,
  ) async {
    biometric.available = true;
    await pumpApp(tester, session, () async => false);
    await tester.pumpAndSettle();
    await createFromFirstRun(tester);

    expect(find.byType(BiometricOfferScreen), findsOneWidget);
    expect(find.byType(SearchScreen), findsNothing);
  });

  testWidgets("sin huella disponible pasa a la pantalla principal", (
    tester,
  ) async {
    await pumpApp(tester, session, () async => false);
    await tester.pumpAndSettle();
    await createFromFirstRun(tester);

    expect(find.byType(BiometricOfferScreen), findsNothing);
    expect(find.byType(SearchScreen), findsOneWidget);
  });

  testWidgets("si isAvailable lanza pasa a la pantalla principal", (
    tester,
  ) async {
    biometric.availableError = StateError("sin canal");
    await pumpApp(tester, session, () async => false);
    await tester.pumpAndSettle();
    await createFromFirstRun(tester);

    expect(find.byType(SearchScreen), findsOneWidget);
  });

  testWidgets("activación fallida desde el desbloqueo muestra el SnackBar en "
      "la pantalla principal", (tester) async {
    biometric
      ..available = true
      ..onBeginEnable = () async =>
          throw const BiometricException(BiometricFailure.failed);
    await pumpApp(tester, session, () async => true);
    await tester.pumpAndSettle();
    await tester.ensureVisible(
      find.byKey(const Key("unlock-enable-biometric")),
    );
    await tester.tap(find.byKey(const Key("unlock-enable-biometric")));
    await tester.enterText(
      find.byKey(const Key("unlock-password")),
      "prueba-bóveda-v2",
    );
    await tester.pump();
    await tester.tap(find.byKey(const Key("unlock-submit")));
    await tester.pumpAndSettle();

    expect(find.byType(SearchScreen), findsOneWidget);
    expect(find.text("No se pudo activar la huella."), findsOneWidget);
  });

  testWidgets("Ahora no lleva a la pantalla principal", (tester) async {
    biometric.available = true;
    await pumpApp(tester, session, () async => false);
    await tester.pumpAndSettle();
    await createFromFirstRun(tester);
    await tester.tap(find.byKey(const Key("offer-skip")));
    await tester.pumpAndSettle();

    expect(find.byType(SearchScreen), findsOneWidget);
    expect(biometric.calls, isNot(contains("beginEnable")));
  });

  group('sin foco al llegar a la pantalla principal', () {
    bool searchHasFocus(WidgetTester tester) =>
        fieldHasFocus(tester, find.byType(SearchScreen));

    testWidgets('tras desbloquear con contraseña', (tester) async {
      await unlockFromScreen(tester);
      expect(find.byType(SearchScreen), findsOneWidget);
      expect(searchHasFocus(tester), isFalse);
    });

    testWidgets('tras desbloquear con huella', (tester) async {
      biometric
        ..available = true
        ..enabled = true;
      await pumpApp(tester, session, () async => true);
      await tester.pumpAndSettle();
      expect(biometric.calls, contains('unlock'));
      expect(find.byType(SearchScreen), findsOneWidget);
      expect(searchHasFocus(tester), isFalse);
    });

    testWidgets('tras crear sin huella', (tester) async {
      await pumpApp(tester, session, () async => false);
      await tester.pumpAndSettle();
      await createFromFirstRun(tester);
      expect(find.byType(SearchScreen), findsOneWidget);
      expect(searchHasFocus(tester), isFalse);
    });

    testWidgets('tras la oferta de la huella', (tester) async {
      biometric.available = true;
      await pumpApp(tester, session, () async => false);
      await tester.pumpAndSettle();
      await createFromFirstRun(tester);
      await tester.tap(find.byKey(const Key('offer-skip')));
      await tester.pumpAndSettle();
      expect(find.byType(SearchScreen), findsOneWidget);
      expect(searchHasFocus(tester), isFalse);
    });
  });

  testWidgets('entradas de punta a punta: crear, ver, editar y eliminar '
      '(#41)', (tester) async {
    // Una bóveda simulada mínima: lo guardado es lo que devuelven la
    // búsqueda y `entry`.
    final stored = <String, EntrySummary>{};
    session.onSearch = (_) async => stored.values.toList();
    session.onAdd = (input, _) async {
      stored[fakeNewId] = entry(input.service.trim(), id: fakeNewId);
      return fakeNewId;
    };
    session.onEdit = (id, input, _, _) async {
      stored[id] = entry(
        input.service.trim(),
        id: id,
        username: input.username,
      );
    };
    session.onEntry = (id) async => stored[id]!;
    session.onDelete = (id) async => stored.remove(id);

    await unlockFromScreen(tester);
    expect(find.textContaining('Toca + para agregar'), findsOneWidget);

    // Crear: el detalle de la nueva ocupa el lugar del formulario.
    await tester.tap(find.byTooltip('Agregar contraseña'));
    await tester.pumpAndSettle();
    await tester.enterText(find.byKey(const Key('service')), ' Gmail ');
    await tester.enterText(find.byKey(const Key('password')), '1234');
    await tester.ensureVisible(find.byKey(const Key('save')));
    await tester.tap(find.byKey(const Key('save')));
    await tester.pumpAndSettle();
    expect(find.byType(EntryFormScreen), findsNothing);
    expect(find.byType(EntryDetailScreen), findsOneWidget);
    expect(find.widgetWithText(AppBar, 'Gmail'), findsOneWidget);

    // Ver: la contraseña solo cruza al tocar "Mostrar".
    expect(session.revealPasswordCalls, isEmpty);
    await tester.tap(find.widgetWithText(TextButton, 'Mostrar'));
    await tester.pumpAndSettle();
    expect(find.text(fakePassword), findsOneWidget);

    // Editar: sin tocar el secreto, no cruza nada.
    await tester.tap(find.byTooltip('Editar'));
    await tester.pumpAndSettle();
    await tester.enterText(find.byKey(const Key('username')), 'ana');
    await tester.ensureVisible(find.byKey(const Key('save')));
    await tester.tap(find.byKey(const Key('save')));
    await tester.pumpAndSettle();
    expect(find.byType(EntryFormScreen), findsNothing);
    expect(find.text('ana'), findsOneWidget);
    expect(find.text(fakePassword), findsNothing);
    expect(session.editCalls.single.newPassword, isNull);
    expect(session.editCalls.single.newNotes, isNull);
    expect(session.revealPasswordCalls, hasLength(1));

    // Atrás: la lista muestra la entrada.
    await tester.pageBack();
    await tester.pumpAndSettle();
    expect(find.byType(SearchScreen), findsOneWidget);
    expect(find.widgetWithText(ListTile, 'Gmail'), findsOneWidget);

    // Eliminar: con confirmación, y desaparece de la lista.
    await tester.tap(find.widgetWithText(ListTile, 'Gmail'));
    await tester.pumpAndSettle();
    await tester.tap(find.byTooltip('Eliminar'));
    await tester.pumpAndSettle();
    await tester.tap(
      find.descendant(
        of: find.byType(AlertDialog),
        matching: find.widgetWithText(TextButton, 'Eliminar'),
      ),
    );
    await tester.pumpAndSettle();
    expect(session.deleteCalls, [fakeNewId]);
    expect(find.byType(EntryDetailScreen), findsNothing);
    expect(find.widgetWithText(ListTile, 'Gmail'), findsNothing);
    expect(find.textContaining('Toca + para agregar'), findsOneWidget);
  });
}
