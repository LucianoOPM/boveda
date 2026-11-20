// Pruebas de la presentación del detalle de entrada (docs/DESIGN.md, 7.5 y los
// boards "Detalle" de Penpot). El comportamiento sigue en
// `entry_detail_screen_test.dart`.

import 'package:boveda/screens/entry_detail_screen.dart';
import 'package:boveda/src/rust/api/entries.dart';
import 'package:boveda/theme/accents.dart';
import 'package:boveda/theme/colors.dart';
import 'package:boveda/theme/icons.dart';
import 'package:boveda/theme/spacing.dart';
import 'package:boveda/theme/theme.dart';
import 'package:boveda/widgets/tag_badge.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'fakes.dart';

late FakeVaultSession session;

final gmail = entry(
  'Gmail',
  username: 'ana@example.com',
  url: 'https://mail.example.com',
  tags: ['correo', 'personal'],
);

Future<bool?> fakeForm(
  BuildContext context,
  EntrySummary entry, {
  required bool repair,
}) async => true;

/// Monta el detalle como segunda ruta, para poder probar la flecha de volver.
Future<void> pumpDetail(
  WidgetTester tester, {
  EntrySummary? of,
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
      home: Builder(
        builder: (context) => Scaffold(
          body: Center(
            child: TextButton(
              onPressed: () => Navigator.of(context).push<void>(
                MaterialPageRoute(
                  builder: (_) => EntryDetailScreen(
                    session: session,
                    entry: of ?? gmail,
                    openForm: fakeForm,
                    clipboard: FakeLocalClipboard(),
                  ),
                ),
              ),
              child: const Text('abrir'),
            ),
          ),
        ),
      ),
    ),
  );
  await tester.tap(find.text('abrir'));
  await tester.pumpAndSettle();
}

Finder field(String name) => find.byKey(Key('detail-field-$name'));

Color cardColor(WidgetTester tester, Finder card) =>
    ((tester.widget<Container>(card)).decoration! as BoxDecoration).color!;

void main() {
  setUp(() => session = FakeVaultSession());

  for (final accent in BovedaAccent.values) {
    testWidgets('con ${accent.name}: barra con íconos de Lucide', (
      tester,
    ) async {
      await pumpDetail(tester, accent: accent);

      // El título sigue siendo el servicio.
      expect(find.widgetWithText(AppBar, 'Gmail'), findsOneWidget);
      expect(
        find.descendant(
          of: find.byTooltip('Editar'),
          matching: find.byIcon(BovedaIcons.pencil),
        ),
        findsOneWidget,
      );
      expect(
        find.descendant(
          of: find.byTooltip('Eliminar'),
          matching: find.byIcon(BovedaIcons.trash),
        ),
        findsOneWidget,
      );
      expect(
        find.descendant(
          of: find.byType(BackButton),
          matching: find.byIcon(BovedaIcons.chevronLeft),
        ),
        findsOneWidget,
      );
    });
  }

  testWidgets('cada fila es una tarjeta surface2 con borde y radio 16', (
    tester,
  ) async {
    await pumpDetail(tester);

    for (final name in ['usuario', 'url', 'etiquetas', 'contraseña', 'notas']) {
      final card = field(name);
      expect(card, findsOneWidget, reason: name);
      final decoration =
          tester.widget<Container>(card).decoration! as BoxDecoration;
      expect(decoration.color, BovedaColors.surface2, reason: name);
      expect(
        decoration.borderRadius,
        BorderRadius.circular(Radii.card),
        reason: name,
      );
      expect(
        (decoration.border! as Border).top.color,
        BovedaColors.borderCard,
        reason: name,
      );
    }
  });

  testWidgets('la etiqueta de cada fila va encima, en mayúsculas', (
    tester,
  ) async {
    await pumpDetail(tester);

    for (final (label, name) in [
      ('USUARIO', 'usuario'),
      ('URL', 'url'),
      ('ETIQUETAS', 'etiquetas'),
      ('CONTRASEÑA', 'contraseña'),
      ('NOTAS', 'notas'),
    ]) {
      final text = find.descendant(of: field(name), matching: find.text(label));
      expect(text, findsOneWidget, reason: label);
      expect(tester.widget<Text>(text).style!.fontFamily, 'JetBrainsMono');
    }
    expect(find.text('Usuario'), findsNothing);
  });

  testWidgets('las etiquetas salen como TagBadge, todas y sin Chip', (
    tester,
  ) async {
    await pumpDetail(
      tester,
      of: entry('Gmail', tags: ['a', 'b', 'c', 'd', 'e']),
    );

    expect(find.byType(TagBadge), findsNWidgets(5));
    expect(find.byType(Chip), findsNothing);
    expect(find.text('A'), findsOneWidget);
    expect(find.text('E'), findsOneWidget);
  });

  testWidgets(
    'las acciones siguen siendo TextButton con fondo surfaceElevated',
    (tester) async {
      await pumpDetail(tester);

      final copy = tester.widget<TextButton>(
        find.byKey(const Key('copy-username')),
      );
      expect(
        copy.style!.backgroundColor!.resolve(const {}),
        BovedaColors.surfaceElevated,
      );
      expect(find.widgetWithText(TextButton, 'Mostrar'), findsOneWidget);
      expect(find.widgetWithText(TextButton, 'Mostrar notas'), findsOneWidget);
    },
  );

  testWidgets('a 100 % la acción va a la derecha del valor', (tester) async {
    await pumpDetail(tester);

    final value = tester.getRect(find.text('ana@example.com'));
    final action = tester.getRect(find.byKey(const Key('copy-username')));
    expect(action.left, greaterThanOrEqualTo(value.right));
  });

  testWidgets('a 200 % la acción pasa debajo del valor y no desborda', (
    tester,
  ) async {
    await pumpDetail(tester, textScale: 2);

    final value = tester.getRect(find.text('ana@example.com'));
    final action = tester.getRect(find.byKey(const Key('copy-username')));
    expect(action.top, greaterThanOrEqualTo(value.bottom));
    expect(tester.takeException(), isNull);
  });

  testWidgets('la confirmación de eliminar usa error en "Eliminar"', (
    tester,
  ) async {
    await pumpDetail(tester);

    await tester.tap(find.byTooltip('Eliminar'));
    await tester.pumpAndSettle();

    expect(find.text('¿Eliminar «Gmail»?'), findsOneWidget);
    final delete = tester.widget<TextButton>(
      find.descendant(
        of: find.byType(AlertDialog),
        matching: find.widgetWithText(TextButton, 'Eliminar'),
      ),
    );
    expect(
      delete.style!.foregroundColor!.resolve(const {}),
      BovedaColors.error,
    );
    // "Cancelar" no es destructivo: conserva su estilo.
    final cancel = tester.widget<TextButton>(
      find.descendant(
        of: find.byType(AlertDialog),
        matching: find.widgetWithText(TextButton, 'Cancelar'),
      ),
    );
    expect(cancel.style, isNull);
  });

  testWidgets('con 200 % y datos largos la pantalla no desborda', (
    tester,
  ) async {
    await pumpDetail(
      tester,
      textScale: 2,
      of: entry(
        'S' * 50,
        username: 'u' * 60,
        url: 'https://ejemplo.com/${'a' * 100}',
        tags: ['uno', 'dos', 'tres', 'cuatro', 'cinco'],
      ),
    );

    expect(tester.takeException(), isNull);
  });
}
