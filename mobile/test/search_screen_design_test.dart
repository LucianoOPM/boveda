// Pruebas de la presentación de la pantalla de Bóveda (docs/DESIGN.md, 7.4 y
// los boards "Bóveda" de Penpot). El comportamiento sigue en
// `search_screen_test.dart`.

import 'package:boveda/screens/entry_form_screen.dart';
import 'package:boveda/screens/search_screen.dart';
import 'package:boveda/src/rust/api/entries.dart';
import 'package:boveda/theme/accents.dart';
import 'package:boveda/theme/colors.dart';
import 'package:boveda/theme/icons.dart';
import 'package:boveda/theme/spacing.dart';
import 'package:boveda/theme/theme.dart';
import 'package:boveda/widgets/damaged_entries_banner.dart';
import 'package:boveda/widgets/primary_glow.dart';
import 'package:boveda/widgets/tag_badge.dart';
import 'package:boveda/widgets/vault_card.dart';
import 'package:boveda/widgets/vault_header.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'fakes.dart';

late FakeVaultSession session;

const settingsKey = Key('open-settings');

final searchField = find.byType(TextField);
final addButton = find.byTooltip('Agregar contraseña');

Future<void> pumpSearch(
  WidgetTester tester, {
  BovedaAccent accent = defaultAccent,
  int damaged = 0,
  double textScale = 1,
  VoidCallback? onOpenSettings,
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
      home: SearchScreen(
        session: session,
        damagedEntries: damaged,
        onOpenSettings: onOpenSettings,
      ),
    ),
  );
  await tester.pump();
}

List<EntrySummary> sample() => [
  entry('Gmail', username: 'ana@gmail.com', tags: ['personal', 'correo']),
  entry('BBVA', username: 'ana.lopez', tags: ['banco']),
  entry('Netflix', url: 'https://netflix.com', tags: ['ocio', 'familia']),
  entry('GitHub', username: 'analopez-dev'),
];

void main() {
  setUp(() => session = FakeVaultSession());

  for (final accent in BovedaAccent.values) {
    testWidgets('con ${accent.name}: encabezado, campo y tarjetas', (
      tester,
    ) async {
      session.onSearch = (_) async => sample();
      await pumpSearch(tester, accent: accent, onOpenSettings: () {});

      expect(find.byType(VaultHeader), findsOneWidget);
      expect(find.text('BÓVEDA'), findsOneWidget);
      expect(find.byType(VaultCard), findsNWidgets(4));
      // Las etiquetas de `EntrySummary.tags` salen como insignias.
      expect(find.text('PERSONAL'), findsOneWidget);
      expect(find.text('BANCO'), findsOneWidget);
      expect(find.byType(TagBadge), findsNWidgets(5));
    });
  }

  testWidgets('el encabezado va arriba y el engranaje dentro de él', (
    tester,
  ) async {
    var opened = 0;
    await pumpSearch(tester, onOpenSettings: () => opened++);

    expect(
      find.descendant(
        of: find.byType(VaultHeader),
        matching: find.byKey(settingsKey),
      ),
      findsOneWidget,
    );
    expect(
      tester.getTopLeft(find.byType(VaultHeader)).dy,
      lessThan(tester.getTopLeft(searchField).dy),
    );

    await tester.tap(find.byKey(settingsKey));
    expect(opened, 1);
  });

  testWidgets('sin onOpenSettings no hay engranaje', (tester) async {
    await pumpSearch(tester);

    expect(find.byKey(settingsKey), findsNothing);
  });

  testWidgets('el campo conserva "Buscar" y usa íconos de Lucide', (
    tester,
  ) async {
    await pumpSearch(tester);

    final field = tester.widget<TextField>(searchField);
    expect(field.decoration!.hintText, 'Buscar');
    expect(
      find.descendant(
        of: searchField,
        matching: find.byIcon(BovedaIcons.search),
      ),
      findsOneWidget,
    );
    expect(find.byIcon(BovedaIcons.x), findsNothing);
    expect(tester.getSize(searchField).height, greaterThanOrEqualTo(52));

    await tester.enterText(searchField, 'gm');
    await tester.pump();

    expect(find.byIcon(BovedaIcons.x), findsOneWidget);
    expect(find.byTooltip('Borrar búsqueda'), findsOneWidget);
  });

  testWidgets('el campo sigue sin enfocarse al llegar', (tester) async {
    await pumpSearch(tester);

    expect(tester.widget<TextField>(searchField).focusNode!.hasFocus, isFalse);
  });

  testWidgets('las tarjetas van en el orden recibido, con 16 de margen y 10 '
      'de separación', (tester) async {
    session.onSearch = (_) async => sample();
    await pumpSearch(tester);

    final cards = tester
        .widgetList<VaultCard>(find.byType(VaultCard))
        .map((c) => c.service)
        .toList();
    expect(cards, ['Gmail', 'BBVA', 'Netflix', 'GitHub']);
    final first = tester.getRect(find.byType(VaultCard).at(0));
    final second = tester.getRect(find.byType(VaultCard).at(1));
    expect(first.left, 16);
    expect(first.right, 360 - 16);
    expect(second.top - first.bottom, 10);
  });

  testWidgets('el usuario va en la tarjeta y, si no hay, la URL', (
    tester,
  ) async {
    session.onSearch = (_) async => sample();
    await pumpSearch(tester);

    final cards = tester.widgetList<VaultCard>(find.byType(VaultCard)).toList();
    expect(cards.map((c) => c.detail), [
      'ana@gmail.com',
      'ana.lopez',
      'https://netflix.com',
      'analopez-dev',
    ]);
  });

  testWidgets('el botón + mide 56, usa Lucide y brilla con radio 16', (
    tester,
  ) async {
    await pumpSearch(tester);

    final fab = find.byType(FloatingActionButton);
    expect(tester.getSize(fab), const Size(56, 56));
    expect(
      find.descendant(of: fab, matching: find.byIcon(BovedaIcons.plus)),
      findsOneWidget,
    );
    final glow = tester.widget<PrimaryGlow>(
      find.ancestor(of: fab, matching: find.byType(PrimaryGlow)),
    );
    expect(glow.enabled, isTrue);
    expect(glow.radius, Radii.card);
    expect(addButton, findsOneWidget);
  });

  testWidgets('el aviso de dañadas va debajo del campo y arriba de la '
      'lista', (tester) async {
    session.onSearch = (_) async => sample();
    await pumpSearch(tester, damaged: 3);

    final banner = find.byType(DamagedEntriesBanner);
    expect(banner, findsOneWidget);
    expect(
      tester.getTopLeft(banner).dy,
      greaterThan(tester.getBottomLeft(searchField).dy),
    );
    expect(
      tester.getTopLeft(banner).dy,
      lessThan(tester.getTopLeft(find.byType(VaultCard).first).dy),
    );
  });

  testWidgets('el fondo de la pantalla es bgBase', (tester) async {
    await pumpSearch(tester);

    expect(
      Theme.of(tester.element(find.byType(Scaffold))).scaffoldBackgroundColor,
      BovedaColors.bgBase,
    );
  });

  group('estados', () {
    const emptyText =
        'Todavía no hay contraseñas guardadas. Toca + para agregar la primera.';

    for (final accent in BovedaAccent.values) {
      testWidgets('con ${accent.name}: vacío con caja, ícono, texto y botón', (
        tester,
      ) async {
        await pumpSearch(tester, accent: accent);

        final box = find.byKey(const Key('empty-icon-box'));
        expect(tester.getSize(box), const Size(72, 72));
        final decoration =
            tester.widget<Container>(box).decoration! as BoxDecoration;
        expect(decoration.color, BovedaColors.surface2);
        expect(decoration.borderRadius, BorderRadius.circular(Radii.card));
        final icon = tester.widget<Icon>(
          find.descendant(of: box, matching: find.byIcon(BovedaIcons.vault)),
        );
        expect(icon.color, accent.accent);
        expect(find.text(emptyText), findsOneWidget);
        expect(
          find.widgetWithText(OutlinedButton, 'Agregar la primera'),
          findsOneWidget,
        );
      });
    }

    testWidgets('"Agregar la primera" abre el formulario de crear', (
      tester,
    ) async {
      await pumpSearch(tester);

      await tester.tap(find.byKey(const Key('empty-add')));
      await tester.pumpAndSettle();

      expect(find.byType(EntryFormScreen), findsOneWidget);
    });

    testWidgets('con texto buscado y sin resultados: lupa y texto, sin botón', (
      tester,
    ) async {
      await pumpSearch(tester);
      await tester.enterText(searchField, 'xyz');
      await tester.pump();

      expect(find.text('Sin resultados para «xyz»'), findsOneWidget);
      expect(
        find.descendant(
          of: find.byKey(const Key('no-results')),
          matching: find.byIcon(BovedaIcons.search),
        ),
        findsOneWidget,
      );
      expect(find.byKey(const Key('empty-add')), findsNothing);
      expect(find.byKey(const Key('empty-icon-box')), findsNothing);
    });

    testWidgets('error de búsqueda: alerta, texto y "Reintentar"', (
      tester,
    ) async {
      session.onSearch = (_) async => throw Exception('falla');
      await pumpSearch(tester);

      final failed = find.byKey(const Key('search-failed'));
      expect(
        find.descendant(of: failed, matching: find.text('No se pudo buscar')),
        findsOneWidget,
      );
      expect(
        find.descendant(
          of: failed,
          matching: find.byIcon(BovedaIcons.triangleAlert),
        ),
        findsOneWidget,
      );
      expect(find.widgetWithText(TextButton, 'Reintentar'), findsOneWidget);

      // Reintentar vuelve a buscar.
      session.onSearch = (_) async => sample();
      await tester.tap(find.text('Reintentar'));
      await tester.pump();
      expect(find.byType(VaultCard), findsNWidgets(4));
    });

    for (final scale in [1.0, 2.0]) {
      testWidgets('vacío a ${scale * 100} % en una pantalla baja con teclado: '
          'no desborda y se desplaza', (tester) async {
        tester.view.physicalSize = const Size(360, 400);
        tester.view.devicePixelRatio = 1;
        tester.view.viewInsets = const FakeViewPadding(bottom: 200);
        addTearDown(tester.view.reset);
        await pumpSearch(tester, textScale: scale, onOpenSettings: () {});

        expect(tester.takeException(), isNull);
        expect(find.text(emptyText), findsOneWidget);
      });

      testWidgets('error a ${scale * 100} % en una pantalla baja: no '
          'desborda', (tester) async {
        session.onSearch = (_) async => throw Exception('falla');
        tester.view.physicalSize = const Size(360, 400);
        tester.view.devicePixelRatio = 1;
        tester.view.viewInsets = const FakeViewPadding(bottom: 200);
        addTearDown(tester.view.reset);
        await pumpSearch(tester, textScale: scale, onOpenSettings: () {});

        expect(tester.takeException(), isNull);
        expect(find.text('No se pudo buscar'), findsOneWidget);
      });
    }

    testWidgets('sin resultados a 200 % no desborda', (tester) async {
      await pumpSearch(tester, textScale: 2);
      await tester.enterText(searchField, 'x' * 40);
      await tester.pump();

      expect(tester.takeException(), isNull);
    });
  });

  testWidgets('con letra grande y aviso, la lista conserva la mitad de la '
      'altura y la zona de arriba se desplaza', (tester) async {
    session.onSearch = (_) async => sample();
    await pumpSearch(tester, textScale: 2, damaged: 12, onOpenSettings: () {});

    // 640 de alto: la zona de arriba no pasa de 320, el resto es de la lista.
    expect(
      tester.getSize(find.byType(ListView)).height,
      greaterThanOrEqualTo(300),
    );
    expect(find.byType(SingleChildScrollView), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets('con texto al 200 % a 360 dp no desborda', (tester) async {
    session.onSearch = (_) async => [
      entry(
        'S' * 60,
        username: 'u' * 60,
        tags: ['uno', 'dos', 'tres', 'cuatro', 'cinco'],
      ),
      ...sample(),
    ];
    await pumpSearch(tester, textScale: 2, damaged: 12, onOpenSettings: () {});

    expect(tester.takeException(), isNull);
    expect(find.byType(VaultHeader), findsOneWidget);
  });
}
