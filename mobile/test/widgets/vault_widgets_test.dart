// Pruebas de la insignia, la tarjeta de entrada y el encabezado de la pantalla
// de Bóveda (docs/DESIGN.md, 6.1 y 6.2).

import 'package:boveda/theme/accents.dart';
import 'package:boveda/theme/colors.dart';
import 'package:boveda/theme/extensions.dart';
import 'package:boveda/theme/icons.dart';
import 'package:boveda/theme/spacing.dart';
import 'package:boveda/theme/theme.dart';
import 'package:boveda/widgets/status_capsule.dart';
import 'package:boveda/widgets/tag_badge.dart';
import 'package:boveda/widgets/vault_card.dart';
import 'package:boveda/widgets/vault_header.dart';
import 'package:flutter/material.dart';
import 'package:flutter/semantics.dart';
import 'package:flutter_test/flutter_test.dart';

const initialKey = Key('vault-card-initial');
const surfaceKey = Key('vault-card-surface');
const settingsKey = Key('open-settings');
const settingsBoxKey = Key('open-settings-box');

Future<void> pump(
  WidgetTester tester,
  Widget child, {
  BovedaAccent accent = defaultAccent,
  double textScale = 1,
  bool disableAnimations = false,
}) async {
  tester.view.physicalSize = const Size(360, 640);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(
    MaterialApp(
      theme: buildTheme(accent),
      builder: (context, child) => MediaQuery(
        data: MediaQuery.of(context).copyWith(
          textScaler: TextScaler.linear(textScale),
          disableAnimations: disableAnimations,
        ),
        child: child!,
      ),
      home: Scaffold(
        body: Padding(padding: const EdgeInsets.all(16), child: child),
      ),
    ),
  );
}

VaultCard card({
  String service = 'Gmail',
  List<String> tags = const [],
  String? detail = 'ana@gmail.com',
  VoidCallback? onTap,
}) => VaultCard(
  service: service,
  tags: tags,
  detail: detail,
  onTap: onTap ?? () {},
);

/// Los `Text` de una insignia o de la fila de etiquetas de la tarjeta.
Finder badges() => find.byType(TagBadge);

void main() {
  group('TagBadge', () {
    for (final accent in BovedaAccent.values) {
      testWidgets('con ${accent.name}: mayúsculas, acento y radio 6', (
        tester,
      ) async {
        await pump(tester, const TagBadge(label: 'personal'), accent: accent);

        expect(find.text('PERSONAL'), findsOneWidget);
        final text = tester.widget<Text>(find.text('PERSONAL'));
        expect(text.style!.color, accent.accent);
        expect(text.style!.fontFamily, 'JetBrainsMono');
        final box = tester.widget<Container>(
          find.descendant(
            of: find.byType(TagBadge),
            matching: find.byType(Container),
          ),
        );
        final decoration = box.decoration! as BoxDecoration;
        expect(decoration.color, accent.accentDim);
        expect(decoration.borderRadius, BorderRadius.circular(Radii.tag));
        expect((decoration.border! as Border).top.color, accent.accentBorder);
      });
    }
  });

  group('VaultCard', () {
    for (final accent in BovedaAccent.values) {
      testWidgets('con ${accent.name}: inicial, servicio y detalle', (
        tester,
      ) async {
        var taps = 0;
        await pump(tester, card(onTap: () => taps++), accent: accent);

        final box = find.byKey(initialKey);
        expect(tester.getSize(box), const Size(40, 40));
        final initial = tester.widget<Text>(
          find.descendant(of: box, matching: find.text('G')),
        );
        expect(initial.style!.color, accent.accent);
        expect(initial.style!.fontFamily, 'JetBrainsMono');
        final decoration =
            tester.widget<Container>(box).decoration! as BoxDecoration;
        expect(decoration.color, BovedaColors.iconBox);
        expect(decoration.borderRadius, BorderRadius.circular(Radii.control));
        expect(find.text('Gmail'), findsOneWidget);
        expect(find.text('ana@gmail.com'), findsOneWidget);

        await tester.tap(find.byType(VaultCard));
        expect(taps, 1);
      });
    }

    testWidgets('dos etiquetas muestran dos insignias', (tester) async {
      await pump(tester, card(tags: ['personal', 'correo']));

      expect(badges(), findsNWidgets(2));
      expect(find.text('PERSONAL'), findsOneWidget);
      expect(find.text('CORREO'), findsOneWidget);
      expect(find.textContaining('+'), findsNothing);
    });

    testWidgets('cinco etiquetas muestran dos insignias y +3', (tester) async {
      await pump(tester, card(tags: ['a', 'b', 'c', 'd', 'e']));

      expect(badges(), findsNWidgets(2));
      expect(find.text('A'), findsOneWidget);
      expect(find.text('B'), findsOneWidget);
      expect(find.text('+3'), findsOneWidget);
    });

    testWidgets('sin etiquetas no hay insignias', (tester) async {
      await pump(tester, card());

      expect(badges(), findsNothing);
      expect(find.textContaining('+'), findsNothing);
    });

    testWidgets('sin detalle no hay segunda línea de texto', (tester) async {
      await pump(tester, card(detail: null));

      expect(find.text('Gmail'), findsOneWidget);
      // La inicial y el servicio son los únicos textos.
      expect(find.byType(Text), findsNWidgets(2));
    });

    testWidgets('servicio y detalle van en una línea con puntos suspensivos', (
      tester,
    ) async {
      await pump(tester, card(detail: 'https://ejemplo.com/${'a' * 80}'));

      final service = tester.widget<Text>(find.text('Gmail'));
      expect(service.maxLines, 1);
      expect(service.overflow, TextOverflow.ellipsis);
      final detail = tester.widget<Text>(find.textContaining('https://'));
      expect(detail.maxLines, 1);
      expect(detail.overflow, TextOverflow.ellipsis);
    });

    testWidgets('presionada: borde borderPressed y escala 0.98', (
      tester,
    ) async {
      await pump(tester, card());
      Color border() =>
          ((tester.widget<Material>(find.byKey(surfaceKey)).shape!)
                  as RoundedRectangleBorder)
              .side
              .color;
      double scale() => tester
          .widget<AnimatedScale>(
            find.descendant(
              of: find.byType(VaultCard),
              matching: find.byType(AnimatedScale),
            ),
          )
          .scale;
      expect(border(), BovedaColors.borderCard);
      expect(scale(), 1);

      final gesture = await tester.startGesture(
        tester.getCenter(find.byType(VaultCard)),
      );
      await tester.pump(const Duration(milliseconds: 150));

      expect(border(), BovedaColors.borderPressed);
      expect(scale(), 0.98);

      await gesture.up();
      await tester.pump(const Duration(milliseconds: 150));
      expect(border(), BovedaColors.borderCard);
      expect(scale(), 1);
    });

    testWidgets('con las animaciones reducidas no hay escala', (tester) async {
      await pump(tester, card(), disableAnimations: true);

      final gesture = await tester.startGesture(
        tester.getCenter(find.byType(VaultCard)),
      );
      await tester.pump(const Duration(milliseconds: 150));

      final scale = tester
          .widget<AnimatedScale>(
            find.descendant(
              of: find.byType(VaultCard),
              matching: find.byType(AnimatedScale),
            ),
          )
          .scale;
      expect(scale, 1);
      await gesture.up();
      await tester.pump();
    });

    testWidgets('anuncia servicio y detalle como botón "abrir"', (
      tester,
    ) async {
      final handle = tester.ensureSemantics();
      await pump(tester, card());

      // Desde la superficie: el nodo de `Semantics` envuelve a la tarjeta.
      final node = tester.getSemantics(find.byKey(surfaceKey));
      expect(node.label, 'Gmail, ana@gmail.com');
      expect(node.flagsCollection.isButton, isTrue);
      expect(node.hintOverrides?.onTapHint, 'abrir');
      // La inicial no se lee aparte del servicio.
      expect(find.bySemanticsLabel('G'), findsNothing);
      handle.dispose();
    });

    testWidgets('conserva la acción de tocar en su nodo de semántica', (
      tester,
    ) async {
      final handle = tester.ensureSemantics();
      var taps = 0;
      await pump(tester, card(onTap: () => taps++));

      final data = tester
          .getSemantics(find.byKey(surfaceKey))
          .getSemanticsData();
      // Con `excludeSemantics`, sin `onTap` en el `Semantics` la tarjeta no se
      // podría activar con TalkBack.
      expect(data.hasAction(SemanticsAction.tap), isTrue);
      tester.semantics.tap(find.semantics.byLabel('Gmail, ana@gmail.com'));
      await tester.pump();
      expect(taps, 1);
      handle.dispose();
    });

    testWidgets('sin detalle anuncia solo el servicio', (tester) async {
      final handle = tester.ensureSemantics();
      await pump(tester, card(detail: null));

      expect(tester.getSemantics(find.byKey(surfaceKey)).label, 'Gmail');
      handle.dispose();
    });

    testWidgets('servicio vacío usa "?" como inicial', (tester) async {
      await pump(tester, card(service: ''));

      expect(
        find.descendant(of: find.byKey(initialKey), matching: find.text('?')),
        findsOneWidget,
      );
    });

    testWidgets('emoji compuesto: la inicial es el grafema entero', (
      tester,
    ) async {
      await pump(tester, card(service: '👩‍💻 Trabajo'));

      expect(
        find.descendant(
          of: find.byKey(initialKey),
          matching: find.text('👩‍💻'),
        ),
        findsOneWidget,
      );
    });

    testWidgets('datos extremos a 200 % a 360 dp no desbordan', (tester) async {
      await pump(
        tester,
        card(
          service: 'S' * 60,
          tags: ['uno', 'dos', 'tres', 'cuatro', 'cinco'],
          detail: 'https://ejemplo.com/${'a' * 120}',
        ),
        textScale: 2,
      );

      expect(tester.takeException(), isNull);
      expect(badges(), findsNWidgets(2));
      expect(find.text('+3'), findsOneWidget);
    });
  });

  group('VaultHeader', () {
    for (final accent in BovedaAccent.values) {
      testWidgets('con ${accent.name}: marca, subtítulo y cápsula', (
        tester,
      ) async {
        await pump(tester, VaultHeader(onOpenSettings: () {}), accent: accent);

        expect(find.text('BÓVEDA'), findsOneWidget);
        expect(find.text('XCHACHA20-POLY1305 • ARGON2ID'), findsOneWidget);
        final capsule = find.byType(StatusCapsule);
        expect(
          find.descendant(
            of: capsule,
            matching: find.text('CONOCIMIENTO CERO'),
          ),
          findsOneWidget,
        );
        final text = tester.widget<Text>(find.text('CONOCIMIENTO CERO'));
        expect(text.style!.color, accent.accent);
      });
    }

    testWidgets('la cápsula se anuncia una sola vez como "Bóveda abierta"', (
      tester,
    ) async {
      final handle = tester.ensureSemantics();
      await pump(tester, VaultHeader(onOpenSettings: () {}));

      expect(find.bySemanticsLabel('Bóveda abierta'), findsOneWidget);
      expect(find.bySemanticsLabel('CONOCIMIENTO CERO'), findsNothing);
      handle.dispose();
    });

    testWidgets('el engranaje mide 40 visible y 48 táctil, con tooltip', (
      tester,
    ) async {
      var taps = 0;
      await pump(tester, VaultHeader(onOpenSettings: () => taps++));

      expect(tester.getSize(find.byKey(settingsKey)), const Size(48, 48));
      expect(tester.getSize(find.byKey(settingsBoxKey)), const Size(40, 40));
      final decoration =
          tester.widget<Container>(find.byKey(settingsBoxKey)).decoration!
              as BoxDecoration;
      expect(decoration.color, BovedaColors.surfaceElevated);
      expect(decoration.borderRadius, BorderRadius.circular(Radii.control));
      expect(
        find.descendant(
          of: find.byKey(settingsKey),
          matching: find.byIcon(BovedaIcons.settings),
        ),
        findsOneWidget,
      );
      expect(find.byTooltip('Ajustes'), findsOneWidget);

      await tester.tap(find.byKey(settingsKey));
      expect(taps, 1);
    });

    testWidgets('el engranaje tiene su propio nodo: botón, tooltip y toque', (
      tester,
    ) async {
      final handle = tester.ensureSemantics();
      var taps = 0;
      await pump(tester, VaultHeader(onOpenSettings: () => taps++));

      // Desde la caja visible: el `SizedBox` de 48 pertenece al nodo padre.
      final data = tester
          .getSemantics(find.byKey(settingsBoxKey))
          .getSemanticsData();
      expect(data.flagsCollection.isButton, isTrue);
      expect(data.tooltip, 'Ajustes');
      expect(data.hasAction(SemanticsAction.tap), isTrue);
      // El nodo es solo del engranaje: no se mezcla con la marca ni la cápsula.
      expect(data.label, isNot(contains('BÓVEDA')));
      expect(data.label, isNot(contains('Bóveda abierta')));
      handle.dispose();
    });

    testWidgets('sin onOpenSettings no hay engranaje', (tester) async {
      await pump(tester, const VaultHeader());

      expect(find.byKey(settingsKey), findsNothing);
      expect(find.byIcon(BovedaIcons.settings), findsNothing);
    });

    testWidgets('a 200 % a 360 dp no desborda', (tester) async {
      await pump(tester, VaultHeader(onOpenSettings: () {}), textScale: 2);

      expect(tester.takeException(), isNull);
      expect(find.byKey(settingsKey), findsOneWidget);
    });

    testWidgets('a 100 % es una sola fila baja', (tester) async {
      // `Align` suelta la altura que el `Scaffold` le impone al hijo.
      await pump(
        tester,
        Align(
          alignment: Alignment.topLeft,
          child: VaultHeader(onOpenSettings: () {}),
        ),
      );

      // Marca, subtítulo y cápsula caben en una fila de unos 60 dp.
      expect(tester.getSize(find.byType(VaultHeader)).height, lessThan(90));
    });

    testWidgets('a 200 % se apila y no se vuelve altísimo', (tester) async {
      await pump(
        tester,
        Align(
          alignment: Alignment.topLeft,
          child: VaultHeader(onOpenSettings: () {}),
        ),
        textScale: 2,
      );

      // La marca no se parte letra por letra en una columna estrecha: la
      // cápsula pasa debajo y el encabezado sigue siendo manejable.
      expect(tester.getSize(find.byType(VaultHeader)).height, lessThan(260));
      final brand = tester.getRect(find.text('BÓVEDA'));
      final capsule = tester.getRect(find.byType(StatusCapsule));
      expect(capsule.top, greaterThanOrEqualTo(brand.bottom));
    });

    testWidgets('sin tema usa los estilos estándar', (tester) async {
      tester.view.physicalSize = const Size(360, 640);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: Column(
              children: [
                VaultHeader(onOpenSettings: () {}),
                card(tags: ['a']),
              ],
            ),
          ),
        ),
      );

      expect(tester.takeException(), isNull);
      expect(
        Theme.of(tester.element(find.byType(VaultHeader)))
            .extension<BovedaText>(),
        isNull,
      );
    });
  });
}
