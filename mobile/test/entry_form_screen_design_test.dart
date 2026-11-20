// Pruebas de la presentación del formulario de entrada (docs/DESIGN.md, 6.4 y
// 7.6, y el board "Formulario de entrada" de Penpot). El comportamiento sigue
// en `entry_form_screen_test.dart`.

import 'package:boveda/screens/entry_form_screen.dart';
import 'package:boveda/src/rust/api/entries.dart';
import 'package:boveda/theme/accents.dart';
import 'package:boveda/theme/colors.dart';
import 'package:boveda/theme/icons.dart';
import 'package:boveda/theme/spacing.dart';
import 'package:boveda/theme/theme.dart';
import 'package:boveda/widgets/activity_text_field.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'fakes.dart';

late FakeVaultSession session;

final service = find.byKey(const Key('service'));
final username = find.byKey(const Key('username'));
final url = find.byKey(const Key('url'));
final password = find.byKey(const Key('password'));
final notes = find.byKey(const Key('notes'));
final changePassword = find.widgetWithText(TextButton, 'Cambiar');
final editNotes = find.widgetWithText(TextButton, 'Editar notas');
final passwordClosed = find.byKey(const Key('password-closed'));
final notesClosed = find.byKey(const Key('notes-closed'));

final gmail = entry(
  'Gmail',
  username: 'ana@example.com',
  url: 'https://mail.example.com',
  tags: ['correo', 'personal'],
);

/// Monta el formulario como segunda ruta sobre una pantalla base. Sin
/// [editing], en modo crear.
Future<void> pumpForm(
  WidgetTester tester, {
  EntrySummary? editing,
  bool repair = false,
  BovedaAccent accent = defaultAccent,
  double textScale = 1,
  Size size = const Size(360, 640),
  double keyboard = 0,
}) async {
  tester.view.physicalSize = size;
  tester.view.devicePixelRatio = 1;
  tester.view.viewInsets = FakeViewPadding(bottom: keyboard);
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
              onPressed: () => Navigator.of(context).push<bool>(
                MaterialPageRoute(
                  builder: (_) => editing == null
                      ? EntryFormScreen.create(
                          session: session,
                          onCreated: (_) {},
                        )
                      : EntryFormScreen.edit(
                          session: session,
                          entry: editing,
                          repair: repair,
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

TextField textFieldOf(WidgetTester tester, Finder field) =>
    tester.widget<TextField>(
      find.descendant(
        of: field,
        matching: find.byType(TextField),
        matchRoot: true,
      ),
    );

Future<void> tapVisible(WidgetTester tester, Finder finder) async {
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

void main() {
  setUp(() => session = FakeVaultSession());

  for (final accent in BovedaAccent.values) {
    testWidgets('con ${accent.name}: Servicio, Usuario, URL y Contraseña con '
        'la etiqueta encima', (tester) async {
      await pumpForm(tester, accent: accent);

      for (final (label, field) in [
        ('SERVICIO', service),
        ('USUARIO', username),
        ('URL', url),
        ('CONTRASEÑA', password),
      ]) {
        final text = find.text(label);
        expect(text, findsOneWidget, reason: label);
        expect(
          tester.getTopLeft(text).dy,
          lessThan(tester.getTopLeft(field).dy + 1),
          reason: label,
        );
        expect(tester.widget<Text>(text).style!.fontFamily, 'JetBrainsMono');
      }
      expect(find.text('Servicio'), findsNothing);
    });
  }

  testWidgets('los campos conservan su Key y siguen siendo ActivityTextField, '
      'sin etiqueta flotante', (tester) async {
    await pumpForm(tester);

    for (final field in [service, username, url]) {
      expect(tester.widget(field), isA<ActivityTextField>());
      expect(textFieldOf(tester, field).decoration!.labelText, isNull);
    }
  });

  testWidgets('cada campo anuncia su etiqueta original, no en mayúsculas', (
    tester,
  ) async {
    final handle = tester.ensureSemantics();
    await pumpForm(tester);

    for (final (original, field) in [
      ('Servicio', service),
      ('Usuario', username),
      ('URL', url),
    ]) {
      final node = tester.getSemantics(
        find.descendant(of: field, matching: find.byType(TextField)),
      );
      expect(node.label, contains(original), reason: original);
    }
    expect(find.bySemanticsLabel('SERVICIO'), findsNothing);
    handle.dispose();
  });

  testWidgets('tocar la etiqueta enfoca el campo', (tester) async {
    await pumpForm(tester);

    await tester.tap(find.text('USUARIO'));
    await tester.pump();

    final editable = tester.widget<EditableText>(
      find.descendant(of: username, matching: find.byType(EditableText)),
    );
    expect(editable.focusNode.hasFocus, isTrue);
  });

  testWidgets('las notas abiertas llevan su etiqueta y siguen sin poder '
      'copiarse', (tester) async {
    await pumpForm(tester, editing: gmail);
    await tapVisible(tester, editNotes);

    expect(find.text('NOTAS'), findsOneWidget);
    expect((tester.widget(notes) as ActivityTextField).copyable, isFalse);
    expect(textFieldOf(tester, notes).decoration!.labelText, isNull);
  });

  group('secciones cerradas (editando)', () {
    for (final accent in BovedaAccent.values) {
      testWidgets('con ${accent.name}: tarjeta surface1 con borde outline y '
          'radio 12', (tester) async {
        await pumpForm(tester, editing: gmail, accent: accent);

        for (final (card, text) in [
          (passwordClosed, 'Contraseña: sin cambios'),
          (notesClosed, 'Notas: sin cambios'),
        ]) {
          await tester.scrollUntilVisible(
            card,
            200,
            scrollable: find.byType(Scrollable).first,
          );
          final decoration =
              tester.widget<Container>(card).decoration! as BoxDecoration;
          expect(decoration.color, BovedaColors.surface1, reason: text);
          expect(
            decoration.borderRadius,
            BorderRadius.circular(Radii.control),
            reason: text,
          );
          expect(
            (decoration.border! as Border).top.color,
            BovedaColors.outline,
            reason: text,
          );
          expect(
            find.descendant(of: card, matching: find.text(text)),
            findsOneWidget,
          );
        }
      });
    }

    testWidgets('"Cambiar" y "Editar notas" son TextButton secundarios y '
        'abren su campo', (tester) async {
      await pumpForm(tester, editing: gmail);

      for (final button in [changePassword, editNotes]) {
        await tester.scrollUntilVisible(
          button,
          200,
          scrollable: find.byType(Scrollable).first,
        );
        final style = tester.widget<TextButton>(button).style!;
        expect(
          style.backgroundColor!.resolve(const {}),
          BovedaColors.surfaceElevated,
        );
      }

      await tapVisible(tester, changePassword);
      expect(password, findsOneWidget);
      expect(passwordClosed, findsNothing);
      expect(find.byKey(const Key('cancel-password')), findsOneWidget);

      await tapVisible(tester, find.byKey(const Key('cancel-password')));
      expect(passwordClosed, findsOneWidget);
    });

    testWidgets('los botones de las secciones no quitan el foco del campo', (
      tester,
    ) async {
      await pumpForm(tester, editing: gmail);
      await tester.tap(find.text('SERVICIO'));
      await tester.pump();
      EditableText serviceField() => tester.widget<EditableText>(
        find.descendant(of: service, matching: find.byType(EditableText)),
      );
      expect(serviceField().focusNode.hasFocus, isTrue);

      await tapVisible(tester, changePassword);

      expect(serviceField().focusNode.hasFocus, isTrue);
    });

    testWidgets('si las notas no cargan, el error lleva ícono de alerta', (
      tester,
    ) async {
      session.onRevealNotes = (_) async => throw Exception('falla');
      await pumpForm(tester, editing: gmail);
      await tapVisible(tester, editNotes);

      final failed = find.byKey(const Key('notes-load-failed'));
      expect(
        find.descendant(
          of: failed,
          matching: find.text(
            'No se pudieron cargar las notas. Intenta de nuevo.',
          ),
        ),
        findsOneWidget,
      );
      final icon = tester.widget<Icon>(
        find.descendant(
          of: failed,
          matching: find.byIcon(BovedaIcons.triangleAlert),
        ),
      );
      expect(icon.color, BovedaColors.error);
    });

    testWidgets('a 200 % el botón pasa debajo del texto y no desborda', (
      tester,
    ) async {
      await pumpForm(tester, editing: gmail, textScale: 2);
      await tester.scrollUntilVisible(
        passwordClosed,
        200,
        scrollable: find.byType(Scrollable).first,
      );

      final text = tester.getRect(
        find.descendant(
          of: passwordClosed,
          matching: find.text('Contraseña: sin cambios'),
        ),
      );
      final button = tester.getRect(changePassword);
      expect(button.top, greaterThanOrEqualTo(text.bottom));
      expect(tester.takeException(), isNull);
    });
  });

  testWidgets('con el teclado abierto en una pantalla baja el formulario se '
      'desplaza sin desbordar', (tester) async {
    await pumpForm(
      tester,
      editing: gmail,
      size: const Size(360, 400),
      keyboard: 200,
    );

    expect(tester.takeException(), isNull);
    await tester.drag(find.byType(Scrollable).first, const Offset(0, -300));
    await tester.pump();
    expect(tester.takeException(), isNull);
  });

  testWidgets('a 200 % el formulario de crear no desborda', (tester) async {
    await pumpForm(tester, textScale: 2);

    expect(tester.takeException(), isNull);
  });
}
