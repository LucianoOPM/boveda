// Pruebas de la pantalla de primer uso con la sesión simulada.

import 'dart:async';

import 'package:boveda/screens/first_run_screen.dart';
import 'package:boveda/src/rust/api/error.dart';
import 'package:boveda/widgets/strength_indicator.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'fakes.dart';

const valid = 'frase larga de prueba';

const passwordKey = Key('first-run-password');
const confirmKey = Key('first-run-confirm');
const acceptKey = Key('first-run-accept');
const createKey = Key('first-run-create');

Future<void> pumpScreen(
  WidgetTester tester,
  FakeVaultSession session, {
  VoidCallback? onCreated,
}) {
  return tester.pumpWidget(
    MaterialApp(
      home: FirstRunScreen(
        session: session,
        strengthOf: fakeStrength,
        onCreated: onCreated ?? () {},
      ),
    ),
  );
}

/// Monta la pantalla encima de una ruta base, para que "volver" pueda
/// sacarla.
Future<void> pumpAsSecondRoute(
  WidgetTester tester,
  FakeVaultSession session,
) async {
  final navigator = GlobalKey<NavigatorState>();
  await tester.pumpWidget(
    MaterialApp(
      navigatorKey: navigator,
      home: const Scaffold(body: Text('base')),
    ),
  );
  navigator.currentState!.push(
    MaterialPageRoute<void>(
      builder: (_) => FirstRunScreen(
        session: session,
        strengthOf: fakeStrength,
        onCreated: () {},
      ),
    ),
  );
  await tester.pumpAndSettle();
}

Future<void> typePassword(WidgetTester tester, String text) async {
  await tester.enterText(find.byKey(passwordKey), text);
  await tester.pump();
}

Future<void> typeConfirm(WidgetTester tester, String text) async {
  await tester.enterText(find.byKey(confirmKey), text);
  await tester.pump();
}

Future<void> toggleAccept(WidgetTester tester) async {
  await tester.ensureVisible(find.byKey(acceptKey));
  await tester.tap(find.byKey(acceptKey));
  await tester.pump();
}

Future<void> fillValid(WidgetTester tester) async {
  await typePassword(tester, valid);
  await typeConfirm(tester, valid);
  await toggleAccept(tester);
}

Future<void> tapCreate(WidgetTester tester) async {
  await tester.ensureVisible(find.byKey(createKey));
  await tester.tap(find.byKey(createKey));
}

/// Deja terminar el `Future` de `create`. Tras un éxito el indicador sigue
/// girando (el `VaultGate` es quien reemplaza la pantalla), así que no se usa
/// `pumpAndSettle`.
Future<void> settleFuture(WidgetTester tester) async {
  await tester.pump();
  await tester.pump();
}

FilledButton button(WidgetTester tester) =>
    tester.widget<FilledButton>(find.byKey(createKey));

TextField fieldOf(WidgetTester tester, Key key) => tester.widget<TextField>(
  find.descendant(of: find.byKey(key), matching: find.byType(TextField)),
);

void main() {
  late FakeVaultSession session;

  setUp(() => session = FakeVaultSession());

  testWidgets('muestra el título y la sugerencia', (tester) async {
    await pumpScreen(tester, session);
    expect(find.text('Crea tu contraseña maestra'), findsOneWidget);
    expect(
      find.text('Usa una frase de varias palabras. Mínimo 12 caracteres.'),
      findsOneWidget,
    );
    expect(find.text('Contraseña maestra'), findsOneWidget);
    expect(find.text('Confirma la contraseña'), findsOneWidget);
    expect(
      find.text(
        'Entiendo que si olvido la contraseña maestra pierdo el acceso a la '
        'bóveda. No se puede recuperar.',
      ),
      findsOneWidget,
    );
    expect(find.text('Crear bóveda'), findsOneWidget);
  });

  testWidgets('el botón empieza desactivado', (tester) async {
    await pumpScreen(tester, session);
    expect(button(tester).onPressed, isNull);
  });

  testWidgets('cada condición por separado mantiene el botón desactivado', (
    tester,
  ) async {
    await pumpScreen(tester, session);

    // Corta, confirmada y con la casilla.
    await typePassword(tester, 'corta-11ch');
    await typeConfirm(tester, 'corta-11ch');
    await toggleAccept(tester);
    expect(button(tester).onPressed, isNull);

    // Larga, con la casilla, pero la confirmación es distinta.
    await typePassword(tester, valid);
    expect(button(tester).onPressed, isNull);

    // Larga y confirmada, sin la casilla.
    await typeConfirm(tester, valid);
    await toggleAccept(tester);
    expect(button(tester).onPressed, isNull);

    // Todo cumplido.
    await toggleAccept(tester);
    expect(button(tester).onPressed, isNotNull);
  });

  testWidgets('el indicador cambia al escribir y no aparece vacío', (
    tester,
  ) async {
    await pumpScreen(tester, session);
    expect(find.byType(StrengthIndicator), findsNothing);

    await typePassword(tester, 'corta');
    expect(find.text('Demasiado corta'), findsOneWidget);

    await typePassword(tester, 'doce-caracte');
    expect(find.text('Aceptable'), findsOneWidget);

    await typePassword(tester, 'dieciseis-caract');
    expect(find.text('Fuerte'), findsOneWidget);

    await typePassword(tester, '');
    expect(find.byType(StrengthIndicator), findsNothing);
  });

  testWidgets('confirmación distinta muestra el error', (tester) async {
    await pumpScreen(tester, session);
    await typePassword(tester, valid);
    expect(find.text('Las contraseñas no coinciden'), findsNothing);

    await typeConfirm(tester, 'otra cosa distinta');
    expect(find.text('Las contraseñas no coinciden'), findsOneWidget);

    await typeConfirm(tester, valid);
    expect(find.text('Las contraseñas no coinciden'), findsNothing);
  });

  testWidgets('cambiar la contraseña tras confirmar vuelve a desactivar', (
    tester,
  ) async {
    await pumpScreen(tester, session);
    await fillValid(tester);
    expect(button(tester).onPressed, isNotNull);

    await typePassword(tester, '$valid cambiada');
    expect(button(tester).onPressed, isNull);
  });

  testWidgets('durante la creación: progreso, todo desactivado y volver no '
      'sale', (tester) async {
    final pending = Completer<void>();
    session.onCreate = (_) => pending.future;
    await pumpScreen(tester, session);
    await fillValid(tester);

    await tapCreate(tester);
    await tester.pump();

    expect(find.text('Creando la bóveda…'), findsOneWidget);
    expect(find.byType(CircularProgressIndicator), findsOneWidget);
    expect(button(tester).onPressed, isNull);
    expect(fieldOf(tester, passwordKey).enabled, isFalse);
    expect(fieldOf(tester, confirmKey).enabled, isFalse);
    expect(
      tester.widget<CheckboxListTile>(find.byKey(acceptKey)).onChanged,
      isNull,
    );

    pending.complete();
    await settleFuture(tester);
  });

  // Como segunda ruta, un "volver" que no se bloquee saca la pantalla; como
  // única ruta, `maybePop` delega en el sistema y la prueba no lo notaría.
  testWidgets('control: sin crear, volver sí saca la pantalla', (
    tester,
  ) async {
    await pumpAsSecondRoute(tester, session);

    await tester.binding.handlePopRoute();
    await tester.pumpAndSettle();
    expect(find.byType(FirstRunScreen), findsNothing);
  });

  testWidgets('durante la creación volver no saca la pantalla', (
    tester,
  ) async {
    final pending = Completer<void>();
    session.onCreate = (_) => pending.future;
    await pumpAsSecondRoute(tester, session);
    await fillValid(tester);
    await tapCreate(tester);
    await tester.pump();

    await tester.binding.handlePopRoute();
    await tester.pump();
    await tester.pump(const Duration(seconds: 1));
    expect(find.byType(FirstRunScreen), findsOneWidget);

    pending.complete();
    await settleFuture(tester);
  });

  testWidgets('al crear llama a create con la contraseña y a onCreated', (
    tester,
  ) async {
    var created = 0;
    await pumpScreen(tester, session, onCreated: () => created++);
    await fillValid(tester);

    await tapCreate(tester);
    await settleFuture(tester);

    expect(session.createCalls, [valid]);
    expect(created, 1);
  });

  testWidgets('doble toque llama a create una sola vez', (tester) async {
    final pending = Completer<void>();
    session.onCreate = (_) => pending.future;
    await pumpScreen(tester, session);
    await fillValid(tester);

    await tester.ensureVisible(find.byKey(createKey));
    await tester.tap(find.byKey(createKey));
    await tester.tap(find.byKey(createKey), warnIfMissed: false);
    await tester.pump();

    expect(session.createCalls, hasLength(1));
    pending.complete();
    await settleFuture(tester);
  });

  testWidgets('error genérico al fallar create y el formulario vuelve', (
    tester,
  ) async {
    session.onCreate = (_) async => throw VaultError.storageIo;
    await pumpScreen(tester, session);
    await fillValid(tester);

    await tapCreate(tester);
    await tester.pumpAndSettle();

    expect(
      find.text('No se pudo crear la bóveda. Intenta de nuevo.'),
      findsOneWidget,
    );
    expect(button(tester).onPressed, isNotNull);
    expect(fieldOf(tester, passwordKey).controller!.text, valid);
    expect(fieldOf(tester, confirmKey).controller!.text, valid);
  });

  testWidgets('PasswordTooShort muestra el mensaje de longitud', (
    tester,
  ) async {
    session.onCreate = (_) async => throw VaultError.passwordTooShort;
    await pumpScreen(tester, session);
    await fillValid(tester);

    await tapCreate(tester);
    await tester.pumpAndSettle();

    expect(
      find.text('La contraseña debe tener al menos 12 caracteres.'),
      findsOneWidget,
    );
  });

  testWidgets('reintentar tras un error funciona', (tester) async {
    var created = 0;
    session.onCreate = (_) async => throw VaultError.storageIo;
    await pumpScreen(tester, session, onCreated: () => created++);
    await fillValid(tester);

    await tapCreate(tester);
    await tester.pumpAndSettle();
    expect(created, 0);

    session.onCreate = (_) async {};
    await tapCreate(tester);
    await settleFuture(tester);

    expect(session.createCalls, [valid, valid]);
    expect(created, 1);
    expect(
      find.text('No se pudo crear la bóveda. Intenta de nuevo.'),
      findsNothing,
    );
  });
}
