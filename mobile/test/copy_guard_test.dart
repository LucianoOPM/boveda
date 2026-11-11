// Los campos que muestran un secreto no dejan copiarlo ni cortarlo: copiar
// es una acción aparte, que marca el contenido como sensible (#42).

import 'package:boveda/widgets/activity_text_field.dart';
import 'package:boveda/widgets/password_field.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'fakes.dart';

void main() {
  late TextEditingController controller;

  setUp(() => controller = TextEditingController(text: 'secreto largo'));
  tearDown(() => controller.dispose());

  Future<void> pumpField(WidgetTester tester, Widget field) {
    return tester.pumpWidget(MaterialApp(home: Scaffold(body: field)));
  }

  final field = find.byType(ActivityTextField);

  /// Abre el menú de selección con una pulsación larga sobre el texto.
  Future<void> openMenu(WidgetTester tester) async {
    await tester.longPressAt(
      tester.getTopLeft(find.byType(EditableText)) + const Offset(20, 10),
    );
    await tester.pumpAndSettle();
  }

  group('ActivityTextField por omisión', () {
    testWidgets('Ctrl+C copia la selección', (tester) async {
      final copied = mockClipboard(tester);
      await pumpField(tester, ActivityTextField(controller: controller));
      await selectAllAndPress(tester, field, LogicalKeyboardKey.keyC);

      expect(copied, ['secreto largo']);
    });

    testWidgets('el menú ofrece copiar', (tester) async {
      mockClipboard(tester);
      await pumpField(tester, ActivityTextField(controller: controller));
      await openMenu(tester);

      expect(find.text('Copy'), findsOneWidget);
    });
  });

  group('ActivityTextField con copyable: false', () {
    Widget guarded() =>
        ActivityTextField(controller: controller, copyable: false);

    testWidgets('Ctrl+C no copia nada', (tester) async {
      final copied = mockClipboard(tester);
      await pumpField(tester, guarded());
      await selectAllAndPress(tester, field, LogicalKeyboardKey.keyC);

      expect(copied, isEmpty);
    });

    testWidgets('Ctrl+X no copia ni borra', (tester) async {
      final copied = mockClipboard(tester);
      await pumpField(tester, guarded());
      await selectAllAndPress(tester, field, LogicalKeyboardKey.keyX);

      expect(copied, isEmpty);
      expect(controller.text, 'secreto largo');
    });

    testWidgets('Ctrl+V sí pega', (tester) async {
      mockClipboard(tester, pasteText: 'pegado');
      await pumpField(tester, guarded());
      await selectAllAndPress(tester, field, LogicalKeyboardKey.keyV);
      await tester.pumpAndSettle();

      expect(controller.text, 'pegado');
    });

    testWidgets('el menú solo ofrece pegar y seleccionar todo', (tester) async {
      mockClipboard(tester);
      await pumpField(tester, guarded());
      await openMenu(tester);

      expect(find.text('Copy'), findsNothing);
      expect(find.text('Cut'), findsNothing);
      expect(find.text('Share'), findsNothing);
      expect(find.text('Paste'), findsOneWidget);
      expect(find.text('Select all'), findsOneWidget);
    });
  });

  group('PasswordField', () {
    testWidgets('con la contraseña visible, Ctrl+C no la copia', (
      tester,
    ) async {
      final copied = mockClipboard(tester);
      await pumpField(
        tester,
        PasswordField(controller: controller, label: 'Contraseña'),
      );
      await tester.tap(find.byTooltip('Mostrar contraseña'));
      await tester.pump();
      await selectAllAndPress(tester, field, LogicalKeyboardKey.keyC);

      expect(copied, isEmpty);
    });

    testWidgets('con la contraseña visible, el menú no ofrece copiar', (
      tester,
    ) async {
      mockClipboard(tester);
      await pumpField(
        tester,
        PasswordField(controller: controller, label: 'Contraseña'),
      );
      await tester.tap(find.byTooltip('Mostrar contraseña'));
      await tester.pump();
      await openMenu(tester);

      expect(find.text('Copy'), findsNothing);
      expect(find.text('Cut'), findsNothing);
    });
  });
}
