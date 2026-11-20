// Pruebas de la presentación de las etiquetas de una entrada (docs/DESIGN.md,
// 7.6 y el board "Formulario de entrada"). El comportamiento sigue en
// `tags_field_test.dart`.

import 'package:boveda/theme/accents.dart';
import 'package:boveda/theme/icons.dart';
import 'package:boveda/theme/spacing.dart';
import 'package:boveda/theme/theme.dart';
import 'package:boveda/widgets/tags_field.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

late List<String> tags;
late TextEditingController pending;

final input = find.byType(TextField);

Future<void> pumpTags(
  WidgetTester tester, {
  List<String> initial = const [],
  BovedaAccent accent = defaultAccent,
  bool enabled = true,
  String? errorText,
  double textScale = 1,
}) async {
  tester.view.physicalSize = const Size(360, 640);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  tags = [...initial];
  pending = TextEditingController();
  addTearDown(pending.dispose);
  await tester.pumpWidget(
    MaterialApp(
      theme: buildTheme(accent),
      builder: (context, child) => MediaQuery(
        data: MediaQuery.of(context)
            .copyWith(textScaler: TextScaler.linear(textScale)),
        child: child!,
      ),
      home: Scaffold(
        body: StatefulBuilder(
          builder: (context, setState) => Padding(
            padding: const EdgeInsets.all(16),
            child: TagsField(
              tags: tags,
              onChanged: (next) => setState(() => tags = next),
              controller: pending,
              enabled: enabled,
              errorText: errorText,
            ),
          ),
        ),
      ),
    ),
  );
}

EditableText editable(WidgetTester tester) =>
    tester.widget<EditableText>(find.byType(EditableText));

void main() {
  for (final accent in BovedaAccent.values) {
    testWidgets('con ${accent.name}: InputChip con el estilo de insignia', (
      tester,
    ) async {
      await pumpTags(tester, initial: ['personal', 'correo'], accent: accent);

      final chips = tester.widgetList<InputChip>(find.byType(InputChip));
      expect(chips, hasLength(2));
      expect(find.text('PERSONAL'), findsOneWidget);
      expect(find.text('CORREO'), findsOneWidget);
      for (final chip in chips) {
        expect(chip.backgroundColor, accent.accentDim);
        expect(chip.side, BorderSide(color: accent.accentBorder));
        final shape = chip.shape! as RoundedRectangleBorder;
        expect(shape.borderRadius, BorderRadius.circular(Radii.tag));
        expect(chip.labelStyle!.color, accent.accent);
        expect(chip.labelStyle!.fontFamily, 'JetBrainsMono');
        expect(chip.labelStyle!.fontWeight, FontWeight.w700);
        final icon = chip.deleteIcon! as Icon;
        expect(icon.icon, BovedaIcons.x);
        expect(icon.color, accent.accent);
        expect(chip.deleteButtonTooltipMessage, 'Quitar etiqueta');
        // El área táctil es la de Material: no se recorta.
        expect(
          chip.materialTapTargetSize,
          isNot(MaterialTapTargetSize.shrinkWrap),
        );
      }
    });
  }

  testWidgets('la ✕ quita la etiqueta', (tester) async {
    await pumpTags(tester, initial: ['a', 'b']);
    expect(find.byTooltip('Quitar etiqueta'), findsNWidgets(2));

    await tester.tap(find.byTooltip('Quitar etiqueta').first);
    await tester.pump();

    expect(tags, ['b']);
    expect(find.text('A'), findsNothing);
    expect(find.text('B'), findsOneWidget);
  });

  testWidgets('desactivado, no se pueden quitar las etiquetas', (tester) async {
    await pumpTags(tester, initial: ['a'], enabled: false);

    expect(tester.widget<InputChip>(find.byType(InputChip)).onDeleted, isNull);
  });

  testWidgets('el campo tiene su etiqueta encima: AGREGAR ETIQUETA', (
    tester,
  ) async {
    await pumpTags(tester);

    expect(find.text('AGREGAR ETIQUETA'), findsOneWidget);
    expect(find.text('Agregar etiqueta'), findsNothing);
    expect(
      tester.getTopLeft(find.text('AGREGAR ETIQUETA')).dy,
      lessThan(tester.getTopLeft(input).dy),
    );
    // Sin etiqueta flotante.
    expect(tester.widget<TextField>(input).decoration!.labelText, isNull);
  });

  testWidgets('el campo anuncia la etiqueta original, no las mayúsculas', (
    tester,
  ) async {
    final handle = tester.ensureSemantics();
    await pumpTags(tester);

    expect(tester.getSemantics(input).label, contains('Agregar etiqueta'));
    expect(find.bySemanticsLabel('AGREGAR ETIQUETA'), findsNothing);
    handle.dispose();
  });

  testWidgets('tocar la etiqueta enfoca el campo', (tester) async {
    await pumpTags(tester);

    await tester.tap(find.text('AGREGAR ETIQUETA'));
    await tester.pump();

    expect(editable(tester).focusNode.hasFocus, isTrue);
  });

  testWidgets('conserva la ayuda y muestra el error', (tester) async {
    await pumpTags(tester, errorText: 'Demasiadas etiquetas');

    expect(find.text('Enter o coma para agregar'), findsNothing);
    expect(find.text('Demasiadas etiquetas'), findsOneWidget);

    await pumpTags(tester);
    expect(find.text('Enter o coma para agregar'), findsOneWidget);
  });

  testWidgets('Enter y la coma siguen agregando, sin perder el foco', (
    tester,
  ) async {
    await pumpTags(tester);
    await tester.tap(input);
    await tester.enterText(input, 'trabajo');
    await tester.testTextInput.receiveAction(TextInputAction.done);
    await tester.pump();

    expect(tags, ['trabajo']);
    expect(editable(tester).focusNode.hasFocus, isTrue);

    await tester.enterText(input, 'casa,');
    await tester.pump();

    expect(tags, ['trabajo', 'casa']);
    expect(pending.text, isEmpty);
  });

  testWidgets('cinco etiquetas largas a 200 % a 360 dp no desbordan', (
    tester,
  ) async {
    await pumpTags(
      tester,
      initial: [
        for (final n in ['a', 'b', 'c', 'd', 'e']) n * 25,
      ],
      textScale: 2,
      errorText: 'Demasiadas etiquetas',
    );

    expect(tester.takeException(), isNull);
    expect(find.byType(InputChip), findsNWidgets(5));
  });
}
