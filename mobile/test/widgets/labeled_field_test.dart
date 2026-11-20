// Pruebas de `LabeledField`: la etiqueta encima de un campo, con su texto
// original para el lector de pantalla y el foco al tocarla.

import 'package:boveda/theme/colors.dart';
import 'package:boveda/theme/theme.dart';
import 'package:boveda/widgets/labeled_field.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

Future<void> pumpField(
  WidgetTester tester, {
  bool enabled = true,
  FocusNode? focusNode,
  bool withTheme = true,
}) async {
  tester.view.physicalSize = const Size(360, 640);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  final body = Scaffold(
    body: Padding(
      padding: const EdgeInsets.all(16),
      child: LabeledField(
        label: 'Nombre del servicio',
        enabled: enabled,
        child: TextField(enabled: enabled, focusNode: focusNode),
      ),
    ),
  );
  await tester.pumpWidget(
    withTheme
        ? MaterialApp(theme: buildTheme(defaultAccent), home: body)
        : MaterialApp(home: body),
  );
}

EditableText editable(WidgetTester tester) =>
    tester.widget<EditableText>(find.byType(EditableText));

void main() {
  testWidgets('la etiqueta va encima, en mayúsculas y en meta', (tester) async {
    await pumpField(tester);

    expect(find.text('NOMBRE DEL SERVICIO'), findsOneWidget);
    expect(find.text('Nombre del servicio'), findsNothing);
    final label = tester.widget<Text>(find.text('NOMBRE DEL SERVICIO'));
    expect(label.style!.fontFamily, 'JetBrainsMono');
    expect(label.style!.color, BovedaColors.textSecondary);
    expect(
      tester.getTopLeft(find.text('NOMBRE DEL SERVICIO')).dy,
      lessThan(tester.getTopLeft(find.byType(TextField)).dy),
    );
  });

  testWidgets('el campo anuncia la etiqueta original, no las mayúsculas', (
    tester,
  ) async {
    final handle = tester.ensureSemantics();
    await pumpField(tester);

    final node = tester.getSemantics(find.byType(TextField));
    expect(node.label, contains('Nombre del servicio'));
    expect(find.bySemanticsLabel('NOMBRE DEL SERVICIO'), findsNothing);
    handle.dispose();
  });

  testWidgets('tocar la etiqueta enfoca el campo', (tester) async {
    await pumpField(tester);
    expect(editable(tester).focusNode.hasFocus, isFalse);

    await tester.tap(find.text('NOMBRE DEL SERVICIO'));
    await tester.pump();

    expect(editable(tester).focusNode.hasFocus, isTrue);
  });

  testWidgets('tocar la etiqueta enfoca el focusNode que trae el hijo', (
    tester,
  ) async {
    final focus = FocusNode();
    addTearDown(focus.dispose);
    await pumpField(tester, focusNode: focus);

    await tester.tap(find.text('NOMBRE DEL SERVICIO'));
    await tester.pump();

    expect(focus.hasFocus, isTrue);
  });

  testWidgets('el campo conserva su focusNode por omisión', (tester) async {
    await pumpField(tester);

    // No se le obliga al hijo a recibir un `FocusNode`.
    expect(tester.widget<TextField>(find.byType(TextField)).focusNode, isNull);
  });

  testWidgets('deshabilitado, la etiqueta se atenúa y no enfoca', (
    tester,
  ) async {
    await pumpField(tester, enabled: false);

    final label = tester.widget<Text>(find.text('NOMBRE DEL SERVICIO'));
    expect(label.style!.color, BovedaColors.textDisabled);
    await tester.tap(find.text('NOMBRE DEL SERVICIO'));
    await tester.pump();
    expect(editable(tester).focusNode.hasFocus, isFalse);
  });

  testWidgets('sin las extensiones del tema no falla', (tester) async {
    await pumpField(tester, withTheme: false);

    expect(tester.takeException(), isNull);
    expect(find.text('NOMBRE DEL SERVICIO'), findsOneWidget);
  });
}
