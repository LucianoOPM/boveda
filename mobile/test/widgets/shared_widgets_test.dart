// Pruebas de los widgets compartidos de las pantallas de arranque: cápsula de
// estado, resplandor del botón principal y etiqueta de campo.

import 'package:boveda/theme/accents.dart';
import 'package:boveda/theme/colors.dart';
import 'package:boveda/theme/extensions.dart';
import 'package:boveda/theme/spacing.dart';
import 'package:boveda/theme/theme.dart';
import 'package:boveda/widgets/field_label.dart';
import 'package:boveda/widgets/primary_glow.dart';
import 'package:boveda/widgets/status_capsule.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import '../theme/contrast.dart';

Future<void> pump(WidgetTester tester, BovedaAccent accent, Widget child) {
  return tester.pumpWidget(
    MaterialApp(
      theme: buildTheme(accent),
      home: Scaffold(body: Center(child: child)),
    ),
  );
}

/// El LED de la cápsula: un círculo de 6 px con la clave `status-led`.
Color ledColor(WidgetTester tester) {
  final led = tester.widget<Container>(find.byKey(const Key('status-led')));
  final decoration = led.decoration! as BoxDecoration;
  expect(decoration.shape, BoxShape.circle);
  return decoration.color!;
}

/// Sombras de la primera `DecoratedBox` dentro de [PrimaryGlow].
List<BoxShadow> glowShadows(WidgetTester tester) {
  final box = tester.widget<DecoratedBox>(
    find
        .descendant(
          of: find.byType(PrimaryGlow),
          matching: find.byType(DecoratedBox),
        )
        .first,
  );
  return (box.decoration as BoxDecoration).boxShadow ?? const [];
}

void main() {
  test('capsule colors are documented tokens', () {
    expect(BovedaColors.capsuleBg, const Color(0xFF000000));
    expect(BovedaColors.capsuleBorder, const Color(0xFF24242F));
  });

  for (final accent in BovedaAccent.values) {
    group('con ${accent.name}', () {
      testWidgets('StatusCapsule abierta usa el acento', (tester) async {
        await pump(
          tester,
          accent,
          const StatusCapsule(text: 'CONOCIMIENTO CERO'),
        );

        expect(find.text('CONOCIMIENTO CERO'), findsOneWidget);
        expect(ledColor(tester), accent.accent);
        final text = tester.widget<Text>(find.text('CONOCIMIENTO CERO'));
        expect(text.style!.color, accent.accent);
        expect(text.style!.fontFamily, 'JetBrainsMono');
      });

      testWidgets('StatusCapsule bloqueada usa errorStrong y error', (
        tester,
      ) async {
        await pump(
          tester,
          accent,
          const StatusCapsule(text: 'BLOQUEADA', locked: true),
        );

        expect(ledColor(tester), BovedaColors.errorStrong);
        final text = tester.widget<Text>(find.text('BLOQUEADA'));
        expect(text.style!.color, BovedaColors.error);
      });

      test('el texto de la cápsula contrasta con su fondo', () {
        expect(
          contrast(accent.accent, BovedaColors.capsuleBg),
          greaterThanOrEqualTo(4.5),
        );
        expect(
          contrast(BovedaColors.error, BovedaColors.capsuleBg),
          greaterThanOrEqualTo(4.5),
        );
      });

      testWidgets('PrimaryGlow brilla solo si está habilitado', (tester) async {
        await pump(
          tester,
          accent,
          const PrimaryGlow(enabled: true, child: SizedBox(width: 80)),
        );
        final shadows = glowShadows(tester);
        expect(shadows, hasLength(1));
        expect(shadows.single.color, accent.accentGlow);
        expect(shadows.single.blurRadius, 25);

        await pump(
          tester,
          accent,
          const PrimaryGlow(enabled: false, child: SizedBox(width: 80)),
        );
        expect(glowShadows(tester), isEmpty);
      });

      testWidgets('PrimaryGlow no cambia el tamaño de su hijo', (tester) async {
        await pump(
          tester,
          accent,
          const PrimaryGlow(
            enabled: true,
            child: SizedBox(width: 120, height: 52),
          ),
        );

        expect(tester.getSize(find.byType(PrimaryGlow)), const Size(120, 52));
      });
    });
  }

  testWidgets(
    'los widgets compartidos funcionan sin las extensiones del tema',
    (tester) async {
      // Muchas pruebas montan pantallas en un `MaterialApp` sin `buildTheme`;
      // en la app, `BovedaApp` siempre lo pone.
      await tester.pumpWidget(
        const MaterialApp(
          home: Scaffold(
            body: Column(
              children: [
                StatusCapsule(text: 'ABIERTA'),
                FieldLabel('Campo'),
                PrimaryGlow(enabled: true, child: SizedBox(width: 80)),
              ],
            ),
          ),
        ),
      );

      expect(tester.takeException(), isNull);
      expect(find.text('ABIERTA'), findsOneWidget);
      expect(find.text('CAMPO'), findsOneWidget);
    },
  );

  testWidgets('StatusCapsule parte su texto con letra grande en vez de '
      'desbordar', (tester) async {
    tester.view.physicalSize = const Size(360, 640);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    await tester.pumpWidget(
      MaterialApp(
        theme: buildTheme(defaultAccent),
        builder: (context, child) => MediaQuery(
          data: MediaQuery.of(context)
              .copyWith(textScaler: const TextScaler.linear(2)),
          child: child!,
        ),
        home: const Scaffold(
          body: Padding(
            padding: EdgeInsets.all(16),
            child: Align(
              alignment: Alignment.centerLeft,
              child: StatusCapsule(text: 'CONOCIMIENTO CERO'),
            ),
          ),
        ),
      ),
    );

    expect(tester.takeException(), isNull);
    expect(find.text('CONOCIMIENTO CERO'), findsOneWidget);
  });

  testWidgets('StatusCapsule anuncia su etiqueta semántica', (tester) async {
    final handle = tester.ensureSemantics();
    await pump(
      tester,
      defaultAccent,
      const StatusCapsule(
        text: 'BLOQUEADA',
        locked: true,
        semanticsLabel: 'Bóveda bloqueada',
      ),
    );

    expect(find.bySemanticsLabel('Bóveda bloqueada'), findsOneWidget);
    // El texto en mayúsculas no se lee aparte.
    expect(find.bySemanticsLabel('BLOQUEADA'), findsNothing);
    handle.dispose();
  });

  testWidgets('StatusCapsule sin etiqueta semántica anuncia su texto', (
    tester,
  ) async {
    final handle = tester.ensureSemantics();
    await pump(tester, defaultAccent, const StatusCapsule(text: 'ABIERTA'));

    expect(find.bySemanticsLabel('ABIERTA'), findsOneWidget);
    handle.dispose();
  });

  testWidgets('PrimaryGlow redondea con el radio de los controles', (
    tester,
  ) async {
    await pump(
      tester,
      defaultAccent,
      const PrimaryGlow(enabled: true, child: SizedBox(width: 80)),
    );

    final box = tester.widget<DecoratedBox>(
      find
          .descendant(
            of: find.byType(PrimaryGlow),
            matching: find.byType(DecoratedBox),
          )
          .first,
    );
    expect(
      (box.decoration as BoxDecoration).borderRadius,
      BorderRadius.circular(Radii.control),
    );
  });

  testWidgets('FieldLabel se atenúa con enabled: false', (tester) async {
    await pump(
      tester,
      defaultAccent,
      const FieldLabel('Contraseña maestra', enabled: false),
    );

    final text = tester.widget<Text>(find.text('CONTRASEÑA MAESTRA'));
    expect(text.style!.color, BovedaColors.textDisabled);
  });

  testWidgets('FieldLabel muestra el texto en mayúsculas, en meta', (
    tester,
  ) async {
    await pump(tester, defaultAccent, const FieldLabel('Contraseña maestra'));

    expect(find.text('CONTRASEÑA MAESTRA'), findsOneWidget);
    final text = tester.widget<Text>(find.text('CONTRASEÑA MAESTRA'));
    final meta = buildTheme(defaultAccent).extension<BovedaText>()!.meta;
    expect(text.style!.fontFamily, meta.fontFamily);
    expect(text.style!.fontSize, meta.fontSize);
    expect(text.style!.color, BovedaColors.textSecondary);
  });
}
