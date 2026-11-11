// Pruebas de los widgets reutilizables: campo de contraseña, indicador de
// fortaleza, aviso de entradas dañadas, marcador y piezas del bloqueo
// automático. No cargan la librería de
// Rust.

import 'package:boveda/screens/placeholder_screen.dart';
import 'package:boveda/services/auto_lock.dart';
import 'package:boveda/src/rust/api/password.dart';
import 'package:boveda/widgets/activity_text_field.dart';
import 'package:boveda/widgets/damaged_entries_banner.dart';
import 'package:boveda/widgets/password_field.dart';
import 'package:boveda/widgets/strength_indicator.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'fakes.dart';

Future<void> pumpInScaffold(WidgetTester tester, Widget child) {
  return tester.pumpWidget(MaterialApp(home: Scaffold(body: child)));
}

TextField textField(WidgetTester tester) =>
    tester.widget<TextField>(find.byType(TextField));

void main() {
  late TextEditingController controller;

  setUp(() => controller = TextEditingController());
  tearDown(() => controller.dispose());

  testWidgets('PasswordField configura el teclado para una contraseña', (
    tester,
  ) async {
    await pumpInScaffold(
      tester,
      PasswordField(controller: controller, label: 'Contraseña maestra'),
    );

    final field = textField(tester);
    expect(field.obscureText, isTrue);
    expect(field.enableSuggestions, isFalse);
    expect(field.autocorrect, isFalse);
    expect(field.enableIMEPersonalizedLearning, isFalse);
    expect(field.autofillHints, anyOf(isNull, isEmpty));
    expect(field.keyboardType, TextInputType.visiblePassword);
    expect(find.text('Contraseña maestra'), findsOneWidget);
  });

  testWidgets('el botón de ver alterna obscureText', (tester) async {
    await pumpInScaffold(
      tester,
      PasswordField(controller: controller, label: 'Contraseña maestra'),
    );

    await tester.tap(find.byTooltip('Mostrar contraseña'));
    await tester.pump();
    expect(textField(tester).obscureText, isFalse);

    await tester.tap(find.byTooltip('Ocultar contraseña'));
    await tester.pump();
    expect(textField(tester).obscureText, isTrue);
  });

  testWidgets('PasswordField desactivado no acepta texto', (tester) async {
    await pumpInScaffold(
      tester,
      PasswordField(
        controller: controller,
        label: 'Contraseña maestra',
        enabled: false,
      ),
    );
    expect(textField(tester).enabled, isFalse);
  });

  testWidgets('PasswordField muestra el error y avisa los cambios', (
    tester,
  ) async {
    final changes = <String>[];
    await pumpInScaffold(
      tester,
      PasswordField(
        controller: controller,
        label: 'Confirma la contraseña',
        errorText: 'Las contraseñas no coinciden',
        onChanged: changes.add,
      ),
    );
    expect(find.text('Las contraseñas no coinciden'), findsOneWidget);

    await tester.enterText(find.byType(TextField), 'abc');
    expect(changes, ['abc']);
    expect(controller.text, 'abc');
  });

  testWidgets('PasswordField pasa textInputAction, onSubmitted y focusNode', (
    tester,
  ) async {
    final focus = FocusNode();
    addTearDown(focus.dispose);
    final submitted = <String>[];
    await pumpInScaffold(
      tester,
      PasswordField(
        controller: controller,
        label: 'Contraseña maestra',
        textInputAction: TextInputAction.done,
        onSubmitted: submitted.add,
        focusNode: focus,
      ),
    );
    await tester.pump();

    final field = textField(tester);
    expect(field.textInputAction, TextInputAction.done);
    expect(field.focusNode, same(focus));

    await tester.enterText(find.byType(TextField), 'abc');
    expect(focus.hasFocus, isTrue);
    await tester.testTextInput.receiveAction(TextInputAction.done);
    expect(submitted, ['abc']);
  });

  testWidgets('PasswordField sin los parámetros nuevos conserva los valores '
      'por omisión', (tester) async {
    await pumpInScaffold(
      tester,
      PasswordField(controller: controller, label: 'Contraseña maestra'),
    );

    final field = textField(tester);
    expect(field.textInputAction, isNull);
    expect(field.onSubmitted, isNull);
    expect(field.focusNode, isNull);
  });

  for (final (strength, label) in [
    (PasswordStrength.tooShort, 'Demasiado corta'),
    (PasswordStrength.acceptable, 'Aceptable'),
    (PasswordStrength.strong, 'Fuerte'),
  ]) {
    testWidgets('StrengthIndicator muestra "$label"', (tester) async {
      await pumpInScaffold(tester, StrengthIndicator(strength: strength));
      expect(find.text(label), findsOneWidget);
      expect(find.byType(LinearProgressIndicator), findsOneWidget);
    });
  }

  testWidgets('DamagedEntriesBanner con 0 no muestra nada', (tester) async {
    await pumpInScaffold(tester, const DamagedEntriesBanner(count: 0));
    expect(find.byType(MaterialBanner), findsNothing);
  });

  testWidgets('DamagedEntriesBanner con 1 usa el singular', (tester) async {
    await pumpInScaffold(tester, const DamagedEntriesBanner(count: 1));
    expect(find.byType(MaterialBanner), findsOneWidget);
    expect(find.text('1 entrada no se pudo leer'), findsOneWidget);
  });

  testWidgets('DamagedEntriesBanner con 3 usa el plural', (tester) async {
    await pumpInScaffold(tester, const DamagedEntriesBanner(count: 3));
    expect(find.text('3 entradas no se pudieron leer'), findsOneWidget);
  });

  testWidgets('"Entendido" oculta el aviso', (tester) async {
    await pumpInScaffold(tester, const DamagedEntriesBanner(count: 2));
    await tester.tap(find.text('Entendido'));
    await tester.pumpAndSettle();
    expect(find.byType(MaterialBanner), findsNothing);
    expect(find.text('2 entradas no se pudieron leer'), findsNothing);
  });

  testWidgets('PlaceholderScreen muestra el texto', (tester) async {
    await tester.pumpWidget(
      const MaterialApp(
        home: PlaceholderScreen(text: 'No se pudo abrir la bóveda.'),
      ),
    );
    expect(find.text('No se pudo abrir la bóveda.'), findsOneWidget);
  });

  group('bloqueo automático', () {
    /// Monta [child] dentro de un `AutoLockScope` cuyo controlador cuenta
    /// los reportes de actividad.
    Future<List<int>> pumpInScope(WidgetTester tester, Widget child) async {
      final reports = <int>[0];
      final autoLock = AutoLockController()..onActivity = () => reports[0]++;
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: AutoLockScope(controller: autoLock, child: child),
          ),
        ),
      );
      return reports;
    }

    test('defaultAutoLockTimeout es de 5 minutos', () {
      expect(defaultAutoLockTimeout, const Duration(minutes: 5));
    });

    test('AutoLockScope no notifica a sus dependientes', () {
      final a = AutoLockScope(
        controller: AutoLockController(),
        child: const SizedBox(),
      );
      final b = AutoLockScope(
        controller: AutoLockController(),
        child: const SizedBox(),
      );
      expect(b.updateShouldNotify(a), isFalse);
    });

    testWidgets('ActivityTextField reporta actividad y llama a onChanged', (
      tester,
    ) async {
      final changes = <String>[];
      final reports = await pumpInScope(
        tester,
        ActivityTextField(controller: controller, onChanged: changes.add),
      );
      await tester.enterText(find.byType(TextField), 'a');
      expect(reports[0], 1);
      expect(changes, ['a']);
    });

    testWidgets('ActivityTextField fuera del alcance no lanza', (tester) async {
      final changes = <String>[];
      await pumpInScaffold(
        tester,
        ActivityTextField(controller: controller, onChanged: changes.add),
      );
      await tester.enterText(find.byType(TextField), 'a');
      expect(tester.takeException(), isNull);
      expect(changes, ['a']);
    });

    testWidgets('PasswordField reporta actividad', (tester) async {
      final changes = <String>[];
      final reports = await pumpInScope(
        tester,
        PasswordField(
          controller: controller,
          label: 'Contraseña',
          onChanged: changes.add,
        ),
      );
      await tester.enterText(find.byType(TextField), 'a');
      expect(reports[0], 1);
      expect(changes, ['a']);
    });
  });

  group('teclado', () {
    final field = find.byKey(const Key('campo'));

    /// Monta [child] arriba de un espacio vacío de 200 de alto, donde se
    /// toca "fuera".
    Future<void> pumpWithSpace(WidgetTester tester, Widget child) {
      return tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: Column(
              children: [
                child,
                const SizedBox(key: Key('vacío'), height: 200),
              ],
            ),
          ),
        ),
      );
    }

    Future<void> tapEmpty(WidgetTester tester) async {
      await tester.tapAt(tester.getCenter(find.byKey(const Key('vacío'))));
      await tester.pump();
    }

    testWidgets('ActivityTextField: tocar fuera quita el foco', (tester) async {
      final controller = TextEditingController();
      addTearDown(controller.dispose);
      await pumpWithSpace(
        tester,
        ActivityTextField(key: const Key('campo'), controller: controller),
      );
      await tester.tap(field);
      await tester.pump();
      expect(fieldHasFocus(tester, field), isTrue);

      await tapEmpty(tester);
      expect(fieldHasFocus(tester, field), isFalse);
    });

    testWidgets(
      'ActivityTextField: tocar un TextFieldTapRegion vecino no quita el foco',
      (tester) async {
        final controller = TextEditingController();
        addTearDown(controller.dispose);
        var pressed = 0;
        await pumpWithSpace(
          tester,
          Column(
            children: [
              ActivityTextField(
                key: const Key('campo'),
                controller: controller,
              ),
              TextFieldTapRegion(
                child: TextButton(
                  key: const Key('vecino'),
                  onPressed: () => pressed++,
                  child: const Text('Vecino'),
                ),
              ),
            ],
          ),
        );
        await tester.tap(field);
        await tester.pump();

        await tester.tap(find.byKey(const Key('vecino')));
        await tester.pump();
        expect(pressed, 1);
        expect(fieldHasFocus(tester, field), isTrue);
      },
    );

    testWidgets('ActivityTextField pasa onEditingComplete', (tester) async {
      final controller = TextEditingController();
      addTearDown(controller.dispose);
      var completed = 0;
      await pumpWithSpace(
        tester,
        ActivityTextField(
          key: const Key('campo'),
          controller: controller,
          textInputAction: TextInputAction.search,
          onEditingComplete: () => completed++,
        ),
      );
      await tester.tap(field);
      await tester.pump();
      expect(fieldHasFocus(tester, field), isTrue);

      await tester.testTextInput.receiveAction(TextInputAction.search);
      await tester.pump();
      expect(completed, 1);
      expect(fieldHasFocus(tester, field), isTrue);
    });

    testWidgets('PasswordField: el ojo no quita el foco y tocar fuera sí', (
      tester,
    ) async {
      final controller = TextEditingController();
      addTearDown(controller.dispose);
      await pumpWithSpace(
        tester,
        PasswordField(
          key: const Key('campo'),
          controller: controller,
          label: 'Contraseña',
        ),
      );
      await tester.tap(find.byType(TextField));
      await tester.pump();
      expect(fieldHasFocus(tester, field), isTrue);

      await tester.tap(find.byTooltip('Mostrar contraseña'));
      await tester.pump();
      expect(fieldHasFocus(tester, field), isTrue);

      await tapEmpty(tester);
      expect(fieldHasFocus(tester, field), isFalse);
    });
  });
}
