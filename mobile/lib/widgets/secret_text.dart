import 'package:flutter/material.dart';

import '../theme/colors.dart';
import '../theme/extensions.dart';
import '../theme/spacing.dart';

/// Una contraseña revelada, en JetBrains Mono y coloreada por tipo de carácter
/// (`docs/DESIGN.md`, 6.3), para no confundir `O` con `0` ni `l` con `1`:
///
/// - letras (cualquier letra Unicode) en `syntaxLetter`,
/// - dígitos `0-9` en `syntaxDigit`,
/// - espacios en `syntaxSpace`, dibujados como `␣` para que se vean,
/// - todo lo demás en `syntaxSymbol`.
///
/// Es solo cómo se dibuja: el texto es el de [text] salvo los espacios. Se
/// dibuja con `Text.rich` y **nunca** con `SelectableText` ni con un campo:
/// seleccionarlo permitiría copiarlo sin la marca de sensible (`mobile/CLAUDE.md`).
///
/// Recorre grafemas y clasifica cada uno por su primer carácter, de modo que un
/// emoji compuesto es un solo símbolo. Un tramo agrupa los caracteres
/// consecutivos del mismo tipo. Una contraseña larga se parte en líneas por
/// caracteres.
class SecretText extends StatelessWidget {
  const SecretText({super.key, required this.text});

  final String text;

  static final _letter = RegExp(r'\p{L}', unicode: true);

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final colors = theme.extension<SyntaxColors>() ?? SyntaxColors.standard;
    final style = (theme.extension<BovedaText>() ?? BovedaText.standard).secret;

    final spans = <TextSpan>[];
    Color? runColor;
    final run = StringBuffer();
    void flush() {
      if (run.isEmpty) return;
      spans.add(
        TextSpan(
          text: run.toString(),
          style: TextStyle(color: runColor),
        ),
      );
      run.clear();
    }

    for (final grapheme in text.characters) {
      final (color, shown) = _classify(grapheme, colors);
      if (color != runColor) {
        flush();
        runColor = color;
      }
      run.write(shown);
    }
    flush();

    return Container(
      key: const Key('secret-text'),
      width: double.infinity,
      padding: const EdgeInsets.all(Space.s14),
      decoration: BoxDecoration(
        color: BovedaColors.surface1,
        borderRadius: BorderRadius.circular(Radii.control),
      ),
      child: Text.rich(TextSpan(children: spans), style: style),
    );
  }

  /// El color de un grafema y lo que se dibuja en su lugar.
  static (Color, String) _classify(String grapheme, SyntaxColors colors) {
    final first = String.fromCharCode(grapheme.runes.first);
    if (first.trim().isEmpty) return (colors.space, '␣');
    if (_letter.hasMatch(first)) return (colors.letter, grapheme);
    final unit = first.codeUnitAt(0);
    if (first.length == 1 && unit >= 0x30 && unit <= 0x39) {
      return (colors.digit, grapheme);
    }
    return (colors.symbol, grapheme);
  }
}
