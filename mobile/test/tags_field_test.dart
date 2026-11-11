// Pruebas del campo de etiquetas del formulario de entradas (#41).

import 'package:boveda/widgets/activity_text_field.dart';
import 'package:boveda/widgets/tags_field.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'fakes.dart';

final field = find.byType(TagsField);
final input = find.byType(TextField);

void main() {
  late TextEditingController pending;
  late List<String> tags;

  setUp(() {
    pending = TextEditingController();
    tags = [];
  });
  tearDown(() => pending.dispose());

  /// Monta el campo sobre un espacio vacío; la lista vive en la prueba, como
  /// vivirá en el formulario.
  Future<void> pumpTags(
    WidgetTester tester, {
    List<String> initial = const [],
    bool enabled = true,
    String? errorText,
  }) {
    tags = [...initial];
    return tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: StatefulBuilder(
            builder: (context, setState) => Column(
              children: [
                TagsField(
                  tags: tags,
                  onChanged: (next) => setState(() => tags = next),
                  controller: pending,
                  enabled: enabled,
                  errorText: errorText,
                ),
                const SizedBox(key: Key('vacío'), height: 100),
              ],
            ),
          ),
        ),
      ),
    );
  }

  List<String> chips(WidgetTester tester) => [
    for (final chip in tester.widgetList<InputChip>(find.byType(InputChip)))
      (chip.label as Text).data!,
  ];

  testWidgets('Enter agrega la etiqueta, vacía el campo y deja el foco', (
    tester,
  ) async {
    await pumpTags(tester);
    await tester.tap(input);
    await tester.enterText(input, 'trabajo');
    await tester.testTextInput.receiveAction(TextInputAction.done);
    await tester.pump();

    expect(tags, ['trabajo']);
    expect(chips(tester), ['trabajo']);
    expect(pending.text, isEmpty);
    expect(fieldHasFocus(tester, field), isTrue);
  });

  testWidgets('una coma agrega lo escrito y vacía el campo', (tester) async {
    await pumpTags(tester, initial: ['a']);
    await tester.enterText(input, 'trabajo,');
    await tester.pump();

    expect(tags, ['a', 'trabajo']);
    expect(pending.text, isEmpty);
  });

  testWidgets('pegar "a,b,c" agrega tres', (tester) async {
    await pumpTags(tester);
    await tester.enterText(input, 'a,b,c');
    await tester.pump();

    expect(tags, ['a', 'b', 'c']);
    expect(pending.text, isEmpty);
  });

  testWidgets('un trozo en blanco no crea chip y el contenido no se recorta', (
    tester,
  ) async {
    await pumpTags(tester);
    await tester.enterText(input, ' , ,');
    await tester.pump();
    expect(tags, isEmpty);
    expect(pending.text, isEmpty);

    await tester.enterText(input, ' a ,');
    await tester.pump();
    expect(tags, [' a ']);

    // Enter con el campo en blanco tampoco agrega nada.
    await tester.enterText(input, '  ');
    await tester.testTextInput.receiveAction(TextInputAction.done);
    await tester.pump();
    expect(tags, [' a ']);
  });

  testWidgets('no quita duplicados: lo hace el núcleo al guardar', (
    tester,
  ) async {
    await pumpTags(tester);
    await tester.enterText(input, 'a,a,');
    await tester.pump();
    expect(tags, ['a', 'a']);
  });

  testWidgets('el ✕ quita ese chip', (tester) async {
    await pumpTags(tester, initial: ['a', 'b', 'c']);
    await tester.tap(find.byTooltip('Quitar etiqueta').at(1));
    await tester.pump();

    expect(tags, ['a', 'c']);
    expect(chips(tester), ['a', 'c']);
  });

  testWidgets('tocar un chip o su ✕ no quita el foco; tocar fuera sí', (
    tester,
  ) async {
    await pumpTags(tester, initial: ['a', 'b']);
    await tester.tap(input);
    await tester.pump();
    expect(fieldHasFocus(tester, field), isTrue);

    await tester.tap(find.text('a'));
    await tester.pump();
    expect(fieldHasFocus(tester, field), isTrue);

    await tester.tap(find.byTooltip('Quitar etiqueta').first);
    await tester.pump();
    expect(tags, ['b']);
    expect(fieldHasFocus(tester, field), isTrue);

    await tester.tapAt(tester.getCenter(find.byKey(const Key('vacío'))));
    await tester.pump();
    expect(fieldHasFocus(tester, field), isFalse);
  });

  testWidgets('el campo es un ActivityTextField que no corrige ni aprende', (
    tester,
  ) async {
    await pumpTags(tester);
    expect(find.byType(ActivityTextField), findsOneWidget);
    final text = tester.widget<TextField>(input);
    expect(text.autocorrect, isFalse);
    expect(text.enableIMEPersonalizedLearning, isFalse);
    expect(text.enableSuggestions, isTrue);
    expect(find.text('Agregar etiqueta'), findsOneWidget);
  });

  testWidgets('muestra el error', (tester) async {
    await pumpTags(tester, errorText: 'Demasiadas etiquetas');
    expect(find.text('Demasiadas etiquetas'), findsOneWidget);
  });

  testWidgets('desactivado: no se escribe ni se quitan chips', (tester) async {
    await pumpTags(tester, initial: ['a'], enabled: false);
    expect(tester.widget<TextField>(input).enabled, isFalse);
    expect(tester.widget<InputChip>(find.byType(InputChip)).onDeleted, isNull);
    expect(chips(tester), ['a']);
  });

  testWidgets('una etiqueta larga no desborda', (tester) async {
    await pumpTags(tester, initial: ['e' * 200]);
    expect(tester.takeException(), isNull);
  });

  test('withPending agrega los trozos pendientes, sin los que quedan en '
      'blanco', () {
    expect(TagsField.withPending(['a'], ' b, ,c'), ['a', ' b', 'c']);
    expect(TagsField.withPending(['a'], '   '), ['a']);
    expect(TagsField.withPending([], ''), isEmpty);
    expect(TagsField.withPending(['a'], 'a'), ['a', 'a']);
  });
}
