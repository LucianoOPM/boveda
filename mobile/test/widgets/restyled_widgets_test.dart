// Pruebas de la presentación de `PasswordField` y `StrengthIndicator`
// (docs/DESIGN.md, 6.4 y 6.5). El comportamiento sigue en `widgets_test.dart`.

import 'package:boveda/src/rust/api/password.dart';
import 'package:boveda/theme/accents.dart';
import 'package:boveda/theme/colors.dart';
import 'package:boveda/theme/extensions.dart';
import 'package:boveda/theme/icons.dart';
import 'package:boveda/theme/theme.dart';
import 'package:boveda/widgets/field_label.dart';
import 'package:boveda/widgets/password_field.dart';
import 'package:boveda/widgets/strength_indicator.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

Future<void> pump(WidgetTester tester, Widget child, {BovedaAccent? accent}) {
  return tester.pumpWidget(
    MaterialApp(
      theme: buildTheme(accent ?? defaultAccent),
      home: Scaffold(
        body: Padding(padding: const EdgeInsets.all(16), child: child),
      ),
    ),
  );
}

Color segmentColor(WidgetTester tester, int index) {
  final segment = tester.widget<Container>(
    find.byKey(Key('strength-segment-$index')),
  );
  return (segment.decoration! as BoxDecoration).color!;
}

void main() {
  group('PasswordField', () {
    late TextEditingController controller;

    setUp(() => controller = TextEditingController());
    tearDown(() => controller.dispose());

    testWidgets('pone la etiqueta encima del campo, en mayúsculas', (
      tester,
    ) async {
      await pump(
        tester,
        PasswordField(controller: controller, label: 'Contraseña maestra'),
      );

      expect(find.byType(FieldLabel), findsOneWidget);
      expect(find.text('CONTRASEÑA MAESTRA'), findsOneWidget);
      final label = tester.getTopLeft(find.text('CONTRASEÑA MAESTRA'));
      final field = tester.getTopLeft(find.byType(TextField));
      expect(label.dy, lessThan(field.dy));
    });

    testWidgets('escribe en JetBrains Mono con el estilo secret', (
      tester,
    ) async {
      await pump(
        tester,
        PasswordField(controller: controller, label: 'Contraseña maestra'),
      );

      final style = tester.widget<TextField>(find.byType(TextField)).style!;
      final secret = buildTheme(defaultAccent).extension<BovedaText>()!.secret;
      expect(style.fontFamily, 'JetBrainsMono');
      expect(style.fontSize, secret.fontSize);
      expect(style.fontWeight, secret.fontWeight);
    });

    testWidgets('el ojo es de Lucide y alterna con los mismos tooltips', (
      tester,
    ) async {
      await pump(
        tester,
        PasswordField(controller: controller, label: 'Contraseña maestra'),
      );

      expect(find.byIcon(BovedaIcons.eye), findsOneWidget);
      expect(find.byTooltip('Mostrar contraseña'), findsOneWidget);

      await tester.tap(find.byTooltip('Mostrar contraseña'));
      await tester.pump();

      expect(find.byIcon(BovedaIcons.eyeOff), findsOneWidget);
      expect(find.byTooltip('Ocultar contraseña'), findsOneWidget);
    });

    testWidgets('el campo anuncia la etiqueta original, no las mayúsculas', (
      tester,
    ) async {
      final handle = tester.ensureSemantics();
      await pump(
        tester,
        PasswordField(controller: controller, label: 'Contraseña maestra'),
      );

      final node = tester.getSemantics(find.byType(TextField));
      expect(node.label, contains('Contraseña maestra'));
      expect(find.bySemanticsLabel('CONTRASEÑA MAESTRA'), findsNothing);
      handle.dispose();
    });

    testWidgets('sigue mostrando errorText', (tester) async {
      await pump(
        tester,
        PasswordField(
          controller: controller,
          label: 'Contraseña maestra',
          errorText: 'Las contraseñas no coinciden',
        ),
      );

      expect(find.text('Las contraseñas no coinciden'), findsOneWidget);
    });
  });

  group('StrengthIndicator', () {
    final expectations = {
      PasswordStrength.tooShort: (
        label: 'Demasiado corta',
        filled: 1,
        segment: BovedaColors.errorStrong,
        text: BovedaColors.error,
      ),
      PasswordStrength.acceptable: (
        label: 'Aceptable',
        filled: 2,
        segment: BovedaColors.warning,
        text: BovedaColors.warning,
      ),
      PasswordStrength.strong: (
        label: 'Fuerte',
        filled: 3,
        segment: BovedaColors.success,
        text: BovedaColors.success,
      ),
    };

    for (final MapEntry(key: strength, value: expected)
        in expectations.entries) {
      testWidgets('${strength.name}: ${expected.filled} segmentos', (
        tester,
      ) async {
        await pump(tester, StrengthIndicator(strength: strength));

        for (var i = 0; i < 3; i++) {
          expect(
            segmentColor(tester, i),
            i < expected.filled
                ? expected.segment
                : BovedaColors.borderInteractive,
            reason: 'segmento $i',
          );
        }
        // El texto es el de siempre, sin pasar a mayúsculas.
        final text = tester.widget<Text>(find.text(expected.label));
        expect(text.style!.color, expected.text);
        expect(text.style!.fontFamily, 'JetBrainsMono');
      });
    }

    testWidgets('los segmentos miden 4 de alto y se separan 4', (tester) async {
      await pump(
        tester,
        const StrengthIndicator(strength: PasswordStrength.strong),
      );

      final first = tester.getRect(find.byKey(const Key('strength-segment-0')));
      final second = tester.getRect(
        find.byKey(const Key('strength-segment-1')),
      );
      expect(first.height, 4);
      expect(second.left - first.right, 4);
    });

    testWidgets('anuncia la fortaleza', (tester) async {
      final handle = tester.ensureSemantics();
      await pump(
        tester,
        const StrengthIndicator(strength: PasswordStrength.acceptable),
      );

      expect(find.bySemanticsLabel('Fortaleza: aceptable'), findsOneWidget);
      handle.dispose();
    });
  });
}
