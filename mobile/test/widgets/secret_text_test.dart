// Pruebas de `SecretText`: la contraseña revelada, coloreada por tipo de
// carácter (docs/DESIGN.md, 6.3).

import 'package:boveda/theme/accents.dart';
import 'package:boveda/theme/colors.dart';
import 'package:boveda/theme/extensions.dart';
import 'package:boveda/theme/spacing.dart';
import 'package:boveda/theme/theme.dart';
import 'package:boveda/widgets/secret_text.dart';
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart';

typedef Run = ({String text, Color? color});

Future<void> pumpSecret(
  WidgetTester tester,
  String text, {
  BovedaAccent accent = defaultAccent,
  double textScale = 1,
}) async {
  tester.view.physicalSize = const Size(360, 640);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(
    MaterialApp(
      theme: buildTheme(accent),
      builder: (context, child) => MediaQuery(
        data: MediaQuery.of(context)
            .copyWith(textScaler: TextScaler.linear(textScale)),
        child: child!,
      ),
      home: Scaffold(
        body: Padding(
          padding: const EdgeInsets.all(16),
          child: SecretText(text: text),
        ),
      ),
    ),
  );
}

/// El `Text.rich` que dibuja `SecretText`.
Finder richText() =>
    find.descendant(of: find.byType(SecretText), matching: find.byType(Text));

/// Los tramos (texto y color) que dibuja `SecretText`.
List<Run> runs(WidgetTester tester) {
  final root = tester.widget<Text>(richText()).textSpan! as TextSpan;
  return [
    for (final span in root.children ?? const <InlineSpan>[])
      (text: (span as TextSpan).text!, color: span.style?.color),
  ];
}

const syntax = SyntaxColors.standard;

void main() {
  testWidgets('colorea cada tipo de carácter y dibuja el espacio como ␣', (
    tester,
  ) async {
    await pumpSecret(tester, 'Xk9#mP2\$vL7!qR4 0Ol1');

    expect(runs(tester), [
      (text: 'Xk', color: syntax.letter),
      (text: '9', color: syntax.digit),
      (text: '#', color: syntax.symbol),
      (text: 'mP', color: syntax.letter),
      (text: '2', color: syntax.digit),
      (text: '\$', color: syntax.symbol),
      (text: 'vL', color: syntax.letter),
      (text: '7', color: syntax.digit),
      (text: '!', color: syntax.symbol),
      (text: 'qR', color: syntax.letter),
      (text: '4', color: syntax.digit),
      (text: '␣', color: syntax.space),
      (text: '0', color: syntax.digit),
      (text: 'Ol', color: syntax.letter),
      (text: '1', color: syntax.digit),
    ]);
  });

  testWidgets('los tramos consecutivos del mismo tipo se agrupan', (
    tester,
  ) async {
    await pumpSecret(tester, 'abc123');

    expect(runs(tester), [
      (text: 'abc', color: syntax.letter),
      (text: '123', color: syntax.digit),
    ]);
  });

  testWidgets('las letras acentuadas y la ñ son letras', (tester) async {
    await pumpSecret(tester, 'ñandú');
    expect(runs(tester), [(text: 'ñandú', color: syntax.letter)]);

    await pumpSecret(tester, 'Ébano');
    expect(runs(tester), [(text: 'Ébano', color: syntax.letter)]);
  });

  testWidgets('un emoji, también compuesto, es un solo símbolo', (
    tester,
  ) async {
    await pumpSecret(tester, '🔑');
    expect(runs(tester), [(text: '🔑', color: syntax.symbol)]);

    await pumpSecret(tester, 'a👩‍💻b');
    expect(runs(tester), [
      (text: 'a', color: syntax.letter),
      (text: '👩‍💻', color: syntax.symbol),
      (text: 'b', color: syntax.letter),
    ]);
  });

  testWidgets('el texto plano es el de la contraseña salvo los espacios', (
    tester,
  ) async {
    await pumpSecret(tester, 'una clave  con espacios');

    final root = tester.widget<Text>(richText()).textSpan!;
    expect(root.toPlainText(), 'una␣clave␣␣con␣espacios');
  });

  for (final accent in BovedaAccent.values) {
    testWidgets('con ${accent.name}: JetBrains Mono, surface1 y radio 12', (
      tester,
    ) async {
      await pumpSecret(tester, 'Abc123', accent: accent);

      final style = tester.widget<Text>(richText()).style!;
      final secret = buildTheme(accent).extension<BovedaText>()!.secret;
      expect(style.fontFamily, 'JetBrainsMono');
      expect(style.fontSize, secret.fontSize);
      expect(style.fontWeight, secret.fontWeight);
      final box = tester.widget<Container>(
        find.byKey(const Key('secret-text')),
      );
      final decoration = box.decoration! as BoxDecoration;
      expect(decoration.color, BovedaColors.surface1);
      expect(decoration.borderRadius, BorderRadius.circular(Radii.control));
      expect(box.padding, const EdgeInsets.all(Space.s14));
    });
  }

  testWidgets('no se puede seleccionar ni editar', (tester) async {
    await pumpSecret(tester, 'secreta-de-prueba');

    // Seleccionar permitiría copiar sin la marca de sensible.
    expect(find.byType(SelectableText), findsNothing);
    expect(find.byType(TextField), findsNothing);
    expect(find.byType(EditableText), findsNothing);
    expect(find.byType(SelectionArea), findsNothing);
  });

  for (final scale in [1.0, 2.0]) {
    testWidgets('120 caracteres sin espacios a 360 dp y ${scale * 100} %: '
        'se parten en líneas y no desbordan', (tester) async {
      await pumpSecret(tester, 'a1#' * 40, textScale: scale);

      expect(tester.takeException(), isNull);
      final lines = tester.renderObject<RenderParagraph>(richText());
      // Más de una línea: la contraseña se parte por caracteres.
      expect(lines.size.height, greaterThan(40));
      final width = tester.getSize(find.byKey(const Key('secret-text'))).width;
      expect(width, 328);
    });
  }

  testWidgets('con espacios y todos los tipos mezclados no desborda', (
    tester,
  ) async {
    await pumpSecret(tester, 'Mi clave ñ é 🔑 #1 ' * 8, textScale: 2);

    expect(tester.takeException(), isNull);
  });

  testWidgets('vacío no falla y no tiene tramos', (tester) async {
    await pumpSecret(tester, '');

    expect(tester.takeException(), isNull);
    expect(runs(tester), isEmpty);
  });

  testWidgets('sin las extensiones del tema usa los estilos estándar', (
    tester,
  ) async {
    await tester.pumpWidget(
      const MaterialApp(
        home: Scaffold(body: SecretText(text: 'Ab1#')),
      ),
    );

    expect(tester.takeException(), isNull);
    expect(runs(tester), [
      (text: 'Ab', color: syntax.letter),
      (text: '1', color: syntax.digit),
      (text: '#', color: syntax.symbol),
    ]);
  });
}
