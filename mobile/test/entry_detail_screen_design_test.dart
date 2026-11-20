// Pruebas de la presentación del detalle de entrada (docs/DESIGN.md, 7.5 y los
// boards "Detalle" de Penpot). El comportamiento sigue en
// `entry_detail_screen_test.dart`.

import 'package:boveda/screens/entry_detail_screen.dart';
import 'package:boveda/src/rust/api/entries.dart';
import 'package:boveda/src/rust/api/error.dart';
import 'package:boveda/theme/accents.dart';
import 'package:boveda/theme/colors.dart';
import 'package:boveda/theme/icons.dart';
import 'package:boveda/theme/spacing.dart';
import 'package:boveda/theme/theme.dart';
import 'package:boveda/widgets/secret_text.dart';
import 'package:boveda/widgets/tag_badge.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'fakes.dart';

late FakeVaultSession session;
late FakeLocalClipboard clipboard;

/// Llamadas al formulario simulado: si era reparación.
final List<bool> formCalls = [];

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
}) async {
  formCalls.add(repair);
  return true;
}

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
                    clipboard: clipboard,
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
  setUp(() {
    session = FakeVaultSession();
    clipboard = FakeLocalClipboard();
    formCalls.clear();
  });

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

  testWidgets('el lector de pantalla lee las etiquetas de fila con su texto '
      'original, no en mayúsculas', (tester) async {
    final handle = tester.ensureSemantics();
    await pumpDetail(tester);

    for (final label in [
      'Usuario',
      'URL',
      'Etiquetas',
      'Contraseña',
      'Notas',
    ]) {
      expect(find.bySemanticsLabel(label), findsOneWidget, reason: label);
    }
    expect(find.bySemanticsLabel('USUARIO'), findsNothing);
    expect(find.bySemanticsLabel('CONTRASEÑA'), findsNothing);
    handle.dispose();
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

  group('secreto', () {
    final showPassword = find.widgetWithText(TextButton, 'Mostrar');
    final hidePassword = find.widgetWithText(TextButton, 'Ocultar');
    final showNotes = find.widgetWithText(TextButton, 'Mostrar notas');
    final copyPassword = find.byKey(const Key('copy-password'));

    Future<void> tap(WidgetTester tester, Finder finder) async {
      // Con letra grande la fila puede no estar construida todavía.
      await tester.scrollUntilVisible(
        finder,
        200,
        scrollable: find.byType(Scrollable).first,
      );
      await tester.pump();
      await tester.tap(finder);
      await tester.pump();
      await tester.pump();
    }

    testWidgets('oculta: ocho puntos en secret y textSecondary, anunciada '
        'como "Contraseña oculta"', (tester) async {
      final handle = tester.ensureSemantics();
      await pumpDetail(tester);

      final dots = tester.widget<Text>(find.text('••••••••'));
      expect(dots.style!.fontFamily, 'JetBrainsMono');
      expect(dots.style!.color, BovedaColors.textSecondary);
      expect(find.bySemanticsLabel('Contraseña oculta'), findsOneWidget);
      expect(find.bySemanticsLabel(RegExp('•')), findsNothing);
      expect(find.byType(SecretText), findsNothing);
      handle.dispose();
    });

    testWidgets('revelada: SecretText, sin selección, y "Ocultar" vuelve a '
        'los puntos', (tester) async {
      await pumpDetail(tester);

      await tap(tester, showPassword);

      expect(find.byType(SecretText), findsOneWidget);
      expect(find.text(fakePassword), findsOneWidget);
      expect(find.text('••••••••'), findsNothing);
      // Seleccionarla permitiría copiarla sin la marca de sensible.
      expect(find.byType(SelectableText), findsNothing);
      expect(find.byType(TextField), findsNothing);
      expect(find.byType(EditableText), findsNothing);
      expect(find.byType(SelectionArea), findsNothing);

      await tap(tester, hidePassword);

      expect(find.byType(SecretText), findsNothing);
      expect(find.text(fakePassword), findsNothing);
      expect(find.text('••••••••'), findsOneWidget);
    });

    for (final scale in [1.0, 2.0]) {
      testWidgets('al revelar y ocultar, los botones conservan su elemento a '
          '${scale * 100} % (el foco de accesibilidad no salta)', (
        tester,
      ) async {
        await pumpDetail(tester, textScale: scale);
        await tester.scrollUntilVisible(
          copyPassword,
          200,
          scrollable: find.byType(Scrollable).first,
        );
        final copy = tester.element(copyPassword);
        final toggle = tester.element(showPassword);

        await tap(tester, showPassword);

        expect(identical(tester.element(copyPassword), copy), isTrue);
        expect(identical(tester.element(hidePassword), toggle), isTrue);

        await tap(tester, hidePassword);

        expect(identical(tester.element(copyPassword), copy), isTrue);
        expect(identical(tester.element(showPassword), toggle), isTrue);
      });
    }

    testWidgets('revelada, las acciones pasan debajo de la contraseña', (
      tester,
    ) async {
      await pumpDetail(tester);

      await tap(tester, showPassword);

      final secret = tester.getRect(find.byType(SecretText));
      expect(
        tester.getRect(copyPassword).top,
        greaterThanOrEqualTo(secret.bottom),
      );
      expect(
        tester.getRect(hidePassword).top,
        greaterThanOrEqualTo(secret.bottom),
      );
    });

    testWidgets('copiar copia lo mismo que antes, con la marca de sensible', (
      tester,
    ) async {
      await pumpDetail(tester);

      await tap(tester, copyPassword);

      expect(clipboard.calls, [(text: fakePassword, sensitive: true)]);
    });

    testWidgets('las acciones de la contraseña y de las notas usan el botón '
        'secundario', (tester) async {
      await pumpDetail(tester);

      for (final button in [copyPassword, showPassword, showNotes]) {
        final style = tester.widget<TextButton>(button).style!;
        expect(
          style.backgroundColor!.resolve(const {}),
          BovedaColors.surfaceElevated,
        );
      }

      await tap(tester, showPassword);
      await tap(tester, showNotes);
      for (final button in [hidePassword, find.text('Ocultar notas')]) {
        expect(button, findsOneWidget);
      }
      final hideStyle = tester.widget<TextButton>(hidePassword).style!;
      expect(
        hideStyle.backgroundColor!.resolve(const {}),
        BovedaColors.surfaceElevated,
      );
    });

    testWidgets('notas reveladas en bodyMedium y "Sin notas" con su texto', (
      tester,
    ) async {
      await pumpDetail(tester);
      await tap(tester, showNotes);

      expect(find.text(fakeNotes), findsOneWidget);
      expect(tester.widget<Text>(find.text(fakeNotes)).style, isNull);

      await tester.pumpWidget(const SizedBox.shrink());
      session.onRevealNotes = (_) async => null;
      await pumpDetail(tester);
      await tap(tester, showNotes);

      expect(find.text('Sin notas'), findsOneWidget);
    });

    testWidgets('el error de revelar muestra la misma cadena en error', (
      tester,
    ) async {
      session.onRevealPassword = (_) async => throw Exception('falla');
      await pumpDetail(tester);

      await tap(tester, showPassword);

      final message = tester.widget<Text>(
        find.text('No se pudo mostrar. Intenta de nuevo.'),
      );
      expect(message.style!.color, BovedaColors.error);
      expect(find.byType(SecretText), findsNothing);
    });

    for (final accent in BovedaAccent.values) {
      testWidgets('con ${accent.name}: aviso de dañado en errorContainer', (
        tester,
      ) async {
        session.onRevealPassword = (_) async => throw VaultError.corruptEntry;
        await pumpDetail(tester, accent: accent);
        await tap(tester, showPassword);

        final card = find.byKey(const Key('damaged-secret'));
        expect(card, findsOneWidget);
        final material = tester.widget<Material>(card);
        expect(material.color, BovedaColors.errorContainer);
        final shape = material.shape! as RoundedRectangleBorder;
        expect(shape.borderRadius, BorderRadius.circular(Radii.card));
        expect(shape.side.color, BovedaColors.error.withValues(alpha: 0.38));
        final icon = tester.widget<Icon>(
          find.descendant(
            of: card,
            matching: find.byIcon(BovedaIcons.triangleAlert),
          ),
        );
        expect(icon.color, BovedaColors.error);
        expect(
          find.descendant(
            of: card,
            matching: find.text(
              'La contraseña y las notas de esta entrada están dañadas',
            ),
          ),
          findsOneWidget,
        );
      });
    }

    testWidgets('el aviso de dañado: "Reparar" secundario y "Eliminar" en '
        'error, con las acciones de siempre', (tester) async {
      session.onRevealPassword = (_) async => throw VaultError.corruptEntry;
      await pumpDetail(tester);
      await tap(tester, showPassword);

      final repair = tester.widget<FilledButton>(
        find.widgetWithText(FilledButton, 'Reparar'),
      );
      expect(
        repair.style!.backgroundColor!.resolve(const {}),
        BovedaColors.surfaceElevated,
      );
      expect(
        repair.style!.side!.resolve(const {})!.color,
        BovedaColors.borderInteractive,
      );
      final delete = tester.widget<TextButton>(
        find.descendant(
          of: find.byKey(const Key('damaged-secret')),
          matching: find.widgetWithText(TextButton, 'Eliminar'),
        ),
      );
      expect(
        delete.style!.foregroundColor!.resolve(const {}),
        BovedaColors.error,
      );

      await tap(tester, find.widgetWithText(FilledButton, 'Reparar'));

      expect(formCalls, [true]);
    });

    for (final scale in [1.0, 2.0]) {
      testWidgets('contraseñas extremas a ${scale * 100} %: 120 caracteres sin '
          'espacios y con espacios, sin desbordes', (tester) async {
        for (final password in ['a1#' * 40, 'Mi clave ñ é 🔑 #1 ' * 6]) {
          session.onRevealPassword = (_) async => password;
          await pumpDetail(tester, textScale: scale);
          await tap(tester, showPassword);

          expect(tester.takeException(), isNull, reason: password);
          expect(find.byType(SecretText), findsOneWidget);
          await tester.pumpWidget(const SizedBox.shrink());
        }
      });
    }

    testWidgets('daño con 200 % no desborda', (tester) async {
      session.onRevealPassword = (_) async => throw VaultError.corruptEntry;
      await pumpDetail(tester, textScale: 2);
      await tap(tester, showPassword);

      expect(tester.takeException(), isNull);
      expect(find.byKey(const Key('damaged-secret')), findsOneWidget);
    });
  });
}
