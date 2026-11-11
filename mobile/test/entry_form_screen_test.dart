// Pruebas del formulario de una entrada (#41): crear, editar y reparar, con
// la sesión simulada.

import 'dart:async';

import 'package:boveda/screens/entry_form_screen.dart';
import 'package:boveda/src/rust/api/entries.dart';
import 'package:boveda/src/rust/api/error.dart';
import 'package:boveda/widgets/strength_indicator.dart';
import 'package:boveda/widgets/tags_field.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'fakes.dart';

late FakeVaultSession session;

/// Resúmenes entregados por `onCreated`.
final List<EntrySummary> created = [];

/// Resultados con los que salió la ruta del formulario.
final List<bool?> popped = [];

final form = find.byType(EntryFormScreen);
final service = find.byKey(const Key('service'));
final username = find.byKey(const Key('username'));
final url = find.byKey(const Key('url'));
final tagsField = find.byType(TagsField);
final password = find.byKey(const Key('password'));
final notes = find.byKey(const Key('notes'));
final saveButton = find.byKey(const Key('save'));
final changePassword = find.widgetWithText(TextButton, 'Cambiar');
final editNotes = find.widgetWithText(TextButton, 'Editar notas');
final cancelPassword = find.byKey(const Key('cancel-password'));
final cancelNotes = find.byKey(const Key('cancel-notes'));

const repairText =
    'La contraseña y las notas de esta entrada están dañadas. '
    'Escribe una contraseña nueva para repararla.';
const saveFailedText = 'No se pudo guardar. Intenta de nuevo.';
const notesFailedText = 'No se pudieron cargar las notas. Intenta de nuevo.';

final gmail = entry(
  'Gmail',
  username: 'ana@example.com',
  url: 'https://mail.example.com',
  tags: ['correo'],
);

/// Monta el formulario como segunda ruta sobre una pantalla base. Sin
/// [editing], en modo crear.
Future<void> pumpForm(
  WidgetTester tester, {
  EntrySummary? editing,
  bool repair = false,
}) async {
  await tester.pumpWidget(
    MaterialApp(
      home: Builder(
        builder: (context) => Scaffold(
          body: Center(
            child: TextButton(
              onPressed: () async {
                final result = await Navigator.of(context).push<bool>(
                  MaterialPageRoute(
                    builder: (_) => editing == null
                        ? EntryFormScreen.create(
                            session: session,
                            onCreated: created.add,
                          )
                        : EntryFormScreen.edit(
                            session: session,
                            entry: editing,
                            repair: repair,
                          ),
                  ),
                );
                popped.add(result);
              },
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

Future<void> enter(WidgetTester tester, Finder field, String text) async {
  await tester.ensureVisible(field);
  await tester.enterText(field, text);
  await tester.pump();
}

Future<void> tapVisible(WidgetTester tester, Finder finder) async {
  await tester.ensureVisible(finder);
  await tester.pump();
  await tester.tap(finder);
  await tester.pump();
  await tester.pump();
}

Future<void> save(WidgetTester tester) => tapVisible(tester, saveButton);

TextField textFieldOf(WidgetTester tester, Finder field) =>
    tester.widget<TextField>(
      find.descendant(
        of: field,
        matching: find.byType(TextField),
        matchRoot: true,
      ),
    );

String? errorOf(WidgetTester tester, Finder field) =>
    textFieldOf(tester, field).decoration?.errorText;

String textOf(WidgetTester tester, Finder field) =>
    textFieldOf(tester, field).controller!.text;

bool saveEnabled(WidgetTester tester) =>
    tester.widget<ButtonStyleButton>(saveButton).onPressed != null;

void removeFormRoute(WidgetTester tester) {
  final navigator = tester.state<NavigatorState>(find.byType(Navigator));
  Route<dynamic>? top;
  navigator.popUntil((route) {
    top = route;
    return true;
  });
  navigator.removeRoute(top!);
}

void main() {
  setUp(() {
    session = FakeVaultSession();
    created.clear();
    popped.clear();
  });

  group('crear', () {
    testWidgets('envía los textos tal cual y entrega el resumen de '
        'entry(id)', (tester) async {
      session.onEntry = (id) async => entry('Gmail', id: id);
      await pumpForm(tester);
      expect(find.widgetWithText(AppBar, 'Nueva entrada'), findsOneWidget);

      await enter(tester, service, '  Gmail ');
      await enter(tester, username, ' ana ');
      await enter(tester, tagsField, 'correo,');
      await enter(tester, password, ' clave ');
      await enter(tester, notes, ' nota ');
      await save(tester);

      final call = session.addCalls.single;
      expect(call.input.service, '  Gmail ');
      expect(call.input.username, ' ana ');
      expect(call.input.url, '');
      expect(call.input.tags, ['correo']);
      expect(call.secret.password, ' clave ');
      expect(call.secret.notes, ' nota ');
      expect(session.entryCalls, [fakeNewId]);
      expect(created.single.id, fakeNewId);
      // No navega por su cuenta: lo decide quien lo abrió.
      expect(popped, isEmpty);
      expect(form, findsOneWidget);
    });

    testWidgets('vacío, envía los textos vacíos: valida el núcleo', (
      tester,
    ) async {
      session.onAdd = (_, _) async => throw VaultError.invalidEntryServiceEmpty;
      await pumpForm(tester);
      await save(tester);

      final call = session.addCalls.single;
      expect(call.input.service, '');
      expect(call.input.username, '');
      expect(call.input.tags, isEmpty);
      expect(call.secret.password, '');
      expect(call.secret.notes, '');
      expect(errorOf(tester, service), 'El servicio es obligatorio');
    });

    testWidgets('una contraseña "1234" se guarda: no hay regla de la '
        'maestra ni indicador', (tester) async {
      await pumpForm(tester);
      expect(find.byType(StrengthIndicator), findsNothing);

      await enter(tester, service, 'Cajero');
      await enter(tester, password, '1234');
      await save(tester);

      expect(session.addCalls.single.secret.password, '1234');
      expect(created, hasLength(1));
    });

    testWidgets('el texto pendiente de etiquetas se agrega al guardar', (
      tester,
    ) async {
      await pumpForm(tester);
      await enter(tester, service, 'Gmail');
      await enter(tester, tagsField, 'correo,');
      await enter(tester, tagsField, 'pendiente');
      await enter(tester, password, 'clave');
      await save(tester);

      expect(session.addCalls.single.input.tags, ['correo', 'pendiente']);
    });

    testWidgets('una etiqueta pendiente no se duplica ni se pierde si el '
        'primer guardado falla', (tester) async {
      var fail = true;
      session.onAdd = (_, _) async =>
          fail ? throw VaultError.invalidEntryServiceEmpty : fakeNewId;
      await pumpForm(tester);
      await enter(tester, tagsField, 'x');
      await enter(tester, password, 'clave');
      await save(tester);
      expect(session.addCalls.single.input.tags, ['x']);

      fail = false;
      await enter(tester, service, 'Gmail');
      await save(tester);
      expect(session.addCalls.last.input.tags, ['x']);
      expect(created, hasLength(1));
    });

    testWidgets('dos toques en "Guardar" crean una sola entrada', (
      tester,
    ) async {
      final pending = Completer<String>();
      session.onAdd = (_, _) => pending.future;
      await pumpForm(tester);
      await enter(tester, service, 'Gmail');
      await enter(tester, password, 'clave');
      await tester.ensureVisible(saveButton);
      await tester.pump();
      await tester.tap(saveButton);
      await tester.tap(saveButton, warnIfMissed: false);
      await tester.pump();
      await tester.tap(saveButton, warnIfMissed: false);
      await tester.pump();

      expect(session.addCalls, hasLength(1));
      pending.complete(fakeNewId);
      await tester.pump();
      await tester.pump();
      expect(created, hasLength(1));
    });

    testWidgets('si entry(id) falla tras crear, sale sin entregar nada', (
      tester,
    ) async {
      session.onEntry = (_) async => throw VaultError.storageIo;
      await pumpForm(tester);
      await enter(tester, service, 'Gmail');
      await enter(tester, password, 'clave');
      await save(tester);
      await tester.pumpAndSettle();

      expect(created, isEmpty);
      expect(form, findsNothing);
    });

    testWidgets('add que termina tras quitar la ruta no llama a onCreated', (
      tester,
    ) async {
      final pending = Completer<String>();
      session.onAdd = (_, _) => pending.future;
      await pumpForm(tester);
      await enter(tester, service, 'Gmail');
      await enter(tester, password, 'clave');
      await save(tester);

      removeFormRoute(tester);
      // Sin cuadro en medio: al bloquear con la app oculta no los hay.
      pending.complete(fakeNewId);
      await tester.idle();
      await tester.pumpAndSettle();

      expect(tester.takeException(), isNull);
      expect(created, isEmpty);
      expect(session.entryCalls, isEmpty);
      // No sacó la ruta que quedó debajo.
      expect(find.text('abrir'), findsOneWidget);
    });
  });

  group('editar', () {
    testWidgets('llega lleno desde el resumen, sin llamar a Rust', (
      tester,
    ) async {
      await pumpForm(tester, editing: gmail);

      expect(find.widgetWithText(AppBar, 'Editar entrada'), findsOneWidget);
      expect(textOf(tester, service), 'Gmail');
      expect(textOf(tester, username), 'ana@example.com');
      expect(textOf(tester, url), 'https://mail.example.com');
      expect(find.widgetWithText(InputChip, 'correo'), findsOneWidget);
      expect(find.text('Contraseña: sin cambios'), findsOneWidget);
      expect(find.text('Notas: sin cambios'), findsOneWidget);
      expect(password, findsNothing);
      expect(notes, findsNothing);
      expect(session.revealPasswordCalls, isEmpty);
      expect(session.revealNotesCalls, isEmpty);
      expect(session.entryCalls, isEmpty);
    });

    testWidgets('sin tocar el secreto, edit lo conserva y nada se revela', (
      tester,
    ) async {
      await pumpForm(tester, editing: gmail);
      await enter(tester, tagsField, 'personal,');
      await save(tester);
      await tester.pumpAndSettle();

      final call = session.editCalls.single;
      expect(call.id, gmail.id);
      expect(call.input.service, 'Gmail');
      expect(call.input.tags, ['correo', 'personal']);
      expect(call.newPassword, isNull);
      expect(call.newNotes, isNull);
      expect(session.revealPasswordCalls, isEmpty);
      expect(session.revealNotesCalls, isEmpty);
      expect(popped, [true]);
    });

    testWidgets('"Cambiar" abre un campo vacío y envía solo la contraseña', (
      tester,
    ) async {
      await pumpForm(tester, editing: gmail);
      await tapVisible(tester, changePassword);

      expect(session.revealPasswordCalls, isEmpty);
      expect(textOf(tester, password), isEmpty);
      await enter(tester, password, 'nueva');
      await save(tester);

      final call = session.editCalls.single;
      expect(call.newPassword, 'nueva');
      expect(call.newNotes, isNull);
    });

    testWidgets('"Editar notas" las carga con revealNotes y envía solo las '
        'notas', (tester) async {
      await pumpForm(tester, editing: gmail);
      await tapVisible(tester, editNotes);

      expect(session.revealNotesCalls, [gmail.id]);
      expect(session.revealPasswordCalls, isEmpty);
      expect(textOf(tester, notes), fakeNotes);
      await enter(tester, notes, 'otras');
      await save(tester);

      final call = session.editCalls.single;
      expect(call.newPassword, isNull);
      expect(call.newNotes, 'otras');
    });

    testWidgets('una sección abierta se envía aunque no cambie', (
      tester,
    ) async {
      await pumpForm(tester, editing: gmail);
      await tapVisible(tester, editNotes);
      await save(tester);

      expect(session.editCalls.single.newNotes, fakeNotes);
    });

    testWidgets('vaciar las notas envía "" (las quita)', (tester) async {
      await pumpForm(tester, editing: gmail);
      await tapVisible(tester, editNotes);
      await enter(tester, notes, '');
      await save(tester);

      expect(session.editCalls.single.newNotes, '');
    });

    testWidgets('sin notas, "Editar notas" abre el campo vacío', (
      tester,
    ) async {
      session.onRevealNotes = (_) async => null;
      await pumpForm(tester, editing: gmail);
      await tapVisible(tester, editNotes);

      expect(textOf(tester, notes), isEmpty);
    });

    testWidgets('"Cancelar" cierra la sección, la vacía y no se envía', (
      tester,
    ) async {
      await pumpForm(tester, editing: gmail);
      await tapVisible(tester, changePassword);
      await enter(tester, password, 'nueva');
      await tapVisible(tester, editNotes);
      await enter(tester, notes, 'otras');

      await tapVisible(tester, cancelPassword);
      await tapVisible(tester, cancelNotes);
      expect(password, findsNothing);
      expect(notes, findsNothing);

      // Al reabrir, el campo de la contraseña está vacío.
      await tapVisible(tester, changePassword);
      expect(textOf(tester, password), isEmpty);
      await tapVisible(tester, cancelPassword);

      await save(tester);
      final call = session.editCalls.single;
      expect(call.newPassword, isNull);
      expect(call.newNotes, isNull);
    });

    testWidgets('mientras cargan las notas, "Editar notas" y "Guardar" '
        'están desactivados', (tester) async {
      final pending = Completer<String?>();
      session.onRevealNotes = (_) => pending.future;
      await pumpForm(tester, editing: gmail);
      await tapVisible(tester, editNotes);

      expect(tester.widget<TextButton>(editNotes).onPressed, isNull);
      expect(saveEnabled(tester), isFalse);

      pending.complete('cargadas');
      await tester.pump();
      expect(textOf(tester, notes), 'cargadas');
      expect(saveEnabled(tester), isTrue);
    });

    testWidgets('si cargar las notas falla, avisa y deja reintentar', (
      tester,
    ) async {
      var fail = true;
      session.onRevealNotes = (_) async =>
          fail ? throw VaultError.storageIo : 'ahora sí';
      await pumpForm(tester, editing: gmail);
      await tapVisible(tester, editNotes);

      expect(find.text(notesFailedText), findsOneWidget);
      expect(notes, findsNothing);

      fail = false;
      await tapVisible(tester, editNotes);
      expect(find.text(notesFailedText), findsNothing);
      expect(textOf(tester, notes), 'ahora sí');
    });

    testWidgets('locked al cargar las notas no muestra nada y deja '
        'reintentar', (tester) async {
      session.onRevealNotes = (_) async => throw VaultError.locked;
      await pumpForm(tester, editing: gmail);
      await tapVisible(tester, editNotes);

      expect(find.text(notesFailedText), findsNothing);
      expect(find.text(repairText), findsNothing);
      expect(notes, findsNothing);
      expect(tester.widget<TextButton>(editNotes).onPressed, isNotNull);
    });

    testWidgets('revealNotes que termina tras quitar la ruta no lanza', (
      tester,
    ) async {
      final pending = Completer<String?>();
      session.onRevealNotes = (_) => pending.future;
      await pumpForm(tester, editing: gmail);
      await tapVisible(tester, editNotes);

      removeFormRoute(tester);
      // Sin cuadro en medio: al bloquear con la app oculta no los hay.
      pending.complete('tardías');
      await tester.idle();
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      expect(find.text('abrir'), findsOneWidget);
    });
  });

  group('errores al guardar', () {
    const tooLong = 'Este campo es demasiado largo';
    final cases = <VaultError, (Finder, String)>{
      VaultError.invalidEntryServiceEmpty: (
        service,
        'El servicio es obligatorio',
      ),
      VaultError.invalidEntryServiceTooLong: (service, tooLong),
      VaultError.invalidEntryUsernameTooLong: (username, tooLong),
      VaultError.invalidEntryUrlTooLong: (url, tooLong),
      VaultError.invalidEntryPasswordEmpty: (
        password,
        'La contraseña es obligatoria',
      ),
      VaultError.invalidEntryPasswordTooLong: (password, tooLong),
      VaultError.invalidEntryNotesTooLong: (notes, tooLong),
      VaultError.invalidEntryTagTooLong: (tagsField, tooLong),
      VaultError.invalidEntryTooManyTags: (tagsField, 'Demasiadas etiquetas'),
    };

    for (final MapEntry(key: error, value: (field, message)) in cases.entries) {
      testWidgets('${error.name}: el mensaje va en su campo, que recibe el '
          'foco', (tester) async {
        session.onAdd = (_, _) async => throw error;
        await pumpForm(tester);
        await save(tester);
        await tester.pump();

        expect(errorOf(tester, field), message);
        expect(find.text(message), findsOneWidget);
        expect(fieldHasFocus(tester, field), isTrue);
        expect(find.text(saveFailedText), findsNothing);
        expect(saveEnabled(tester), isTrue);
        expect(created, isEmpty);
      });
    }

    testWidgets('el error de un campo se borra al escribir en él', (
      tester,
    ) async {
      session.onAdd = (_, _) async => throw VaultError.invalidEntryServiceEmpty;
      await pumpForm(tester);
      await save(tester);
      expect(errorOf(tester, service), isNotNull);

      await enter(tester, service, 'G');
      expect(errorOf(tester, service), isNull);
    });

    testWidgets('el error de las etiquetas se borra al cambiarlas', (
      tester,
    ) async {
      session.onAdd = (_, _) async => throw VaultError.invalidEntryTooManyTags;
      await pumpForm(tester);
      await save(tester);
      expect(errorOf(tester, tagsField), isNotNull);

      await enter(tester, tagsField, 'a,');
      expect(errorOf(tester, tagsField), isNull);
    });

    for (final error in <Object>[
      VaultError.storageIo,
      VaultError.invalidEntryOther,
      VaultError.entryNotFound,
      StateError('interno'),
    ]) {
      testWidgets('$error: mensaje general, sin detalles', (tester) async {
        session.onEdit = (_, _, _, _) async => throw error;
        await pumpForm(tester, editing: gmail);
        await save(tester);

        expect(find.text(saveFailedText), findsOneWidget);
        expect(find.textContaining('interno'), findsNothing);
        expect(saveEnabled(tester), isTrue);
        expect(popped, isEmpty);

        // Se borra al cambiar cualquier campo.
        await enter(tester, username, 'otro');
        expect(find.text(saveFailedText), findsNothing);
      });
    }

    testWidgets('locked no muestra nada', (tester) async {
      session.onEdit = (_, _, _, _) async => throw VaultError.locked;
      await pumpForm(tester, editing: gmail);
      await save(tester);

      expect(find.text(saveFailedText), findsNothing);
      expect(popped, isEmpty);
    });
  });

  group('reparación', () {
    testWidgets('corruptEntry al guardar pasa a reparar sin perder lo '
        'escrito', (tester) async {
      var repaired = false;
      session.onEdit = (_, _, newPassword, newNotes) async {
        if (newPassword == null || newNotes == null) {
          throw VaultError.corruptEntry;
        }
        repaired = true;
      };
      await pumpForm(tester, editing: gmail);
      await enter(tester, username, 'nuevo@example.com');
      await enter(tester, tagsField, 'personal,');
      await save(tester);
      await tester.pump();

      expect(find.text(repairText), findsOneWidget);
      expect(find.widgetWithText(AppBar, 'Reparar entrada'), findsOneWidget);
      expect(textOf(tester, username), 'nuevo@example.com');
      expect(find.widgetWithText(InputChip, 'personal'), findsOneWidget);
      expect(password, findsOneWidget);
      expect(notes, findsOneWidget);
      expect(cancelPassword, findsNothing);
      expect(cancelNotes, findsNothing);
      expect(fieldHasFocus(tester, password), isTrue);
      expect(popped, isEmpty);

      await enter(tester, password, 'reparada');
      await save(tester);
      await tester.pumpAndSettle();

      final call = session.editCalls.last;
      expect(call.input.username, 'nuevo@example.com');
      expect(call.input.tags, ['correo', 'personal']);
      expect(call.newPassword, 'reparada');
      expect(call.newNotes, '');
      expect(repaired, isTrue);
      expect(popped, [true]);
    });

    testWidgets('conserva una contraseña nueva ya escrita', (tester) async {
      session.onEdit = (_, _, _, _) async => throw VaultError.corruptEntry;
      await pumpForm(tester, editing: gmail);
      await tapVisible(tester, changePassword);
      await enter(tester, password, 'ya-escrita');
      await save(tester);
      await tester.pump();

      expect(find.text(repairText), findsOneWidget);
      expect(textOf(tester, password), 'ya-escrita');
      expect(notes, findsOneWidget);
    });

    testWidgets('corruptEntry en "Editar notas" también pasa a reparar', (
      tester,
    ) async {
      session.onRevealNotes = (_) async => throw VaultError.corruptEntry;
      await pumpForm(tester, editing: gmail);
      await enter(tester, username, 'nuevo@example.com');
      await tapVisible(tester, editNotes);
      await tester.pump();

      expect(find.text(repairText), findsOneWidget);
      expect(textOf(tester, username), 'nuevo@example.com');
      expect(password, findsOneWidget);
      expect(notes, findsOneWidget);
      expect(find.text(notesFailedText), findsNothing);
    });

    testWidgets('modo reparar directo: aviso, secciones abiertas y sin '
        '"Cancelar"; guarda los dos valores', (tester) async {
      await pumpForm(tester, editing: gmail, repair: true);

      expect(find.widgetWithText(AppBar, 'Reparar entrada'), findsOneWidget);
      expect(find.text(repairText), findsOneWidget);
      expect(textOf(tester, service), 'Gmail');
      expect(textOf(tester, password), isEmpty);
      expect(textOf(tester, notes), isEmpty);
      expect(cancelPassword, findsNothing);
      expect(cancelNotes, findsNothing);
      expect(session.revealNotesCalls, isEmpty);

      await enter(tester, password, 'nueva');
      await enter(tester, notes, 'notas nuevas');
      await save(tester);
      await tester.pumpAndSettle();

      final call = session.editCalls.single;
      expect(call.newPassword, 'nueva');
      expect(call.newNotes, 'notas nuevas');
      expect(popped, [true]);
    });
  });

  group('guardando', () {
    testWidgets('todo queda desactivado y "atrás" no hace nada', (
      tester,
    ) async {
      final pending = Completer<void>();
      session.onEdit = (_, _, _, _) => pending.future;
      await pumpForm(tester, editing: gmail);
      await tapVisible(tester, changePassword);
      await enter(tester, password, 'nueva');
      await save(tester);

      expect(find.text('Guardando…'), findsOneWidget);
      expect(saveEnabled(tester), isFalse);
      for (final field in [service, username, url, tagsField, password]) {
        expect(textFieldOf(tester, field).enabled, isFalse);
      }
      expect(tester.widget<TextButton>(editNotes).onPressed, isNull);
      expect(tester.widget<TextButton>(cancelPassword).onPressed, isNull);

      await tester.binding.handlePopRoute();
      await tester.pumpAndSettle();
      expect(form, findsOneWidget);
      expect(find.text('¿Descartar cambios?'), findsNothing);

      pending.complete();
      await tester.pumpAndSettle();
      expect(popped, [true]);
    });

    testWidgets('edit que termina tras quitar la ruta no navega ni lanza', (
      tester,
    ) async {
      final pending = Completer<void>();
      session.onEdit = (_, _, _, _) => pending.future;
      await pumpForm(tester, editing: gmail);
      await save(tester);

      removeFormRoute(tester);
      // Sin cuadro en medio: al bloquear con la app oculta no los hay.
      pending.complete();
      await tester.idle();
      await tester.pumpAndSettle();

      expect(tester.takeException(), isNull);
      expect(find.text('abrir'), findsOneWidget);
    });
  });

  group('descartar cambios', () {
    final discardDialog = find.text('¿Descartar cambios?');

    testWidgets('sin cambios, la flecha sale directo', (tester) async {
      await pumpForm(tester, editing: gmail);
      await tester.tap(find.byType(BackButton));
      await tester.pumpAndSettle();

      expect(discardDialog, findsNothing);
      expect(form, findsNothing);
      expect(popped, [null]);
    });

    testWidgets('sin cambios, el atrás del sistema sale directo', (
      tester,
    ) async {
      await pumpForm(tester);
      await tester.binding.handlePopRoute();
      await tester.pumpAndSettle();

      expect(discardDialog, findsNothing);
      expect(form, findsNothing);
    });

    testWidgets('con cambios, la flecha pregunta; "Seguir editando" '
        'conserva lo escrito', (tester) async {
      await pumpForm(tester, editing: gmail);
      await enter(tester, username, 'otro');
      await tester.tap(find.byType(BackButton));
      await tester.pumpAndSettle();

      expect(discardDialog, findsOneWidget);
      await tester.tap(find.widgetWithText(TextButton, 'Seguir editando'));
      await tester.pumpAndSettle();
      expect(form, findsOneWidget);
      expect(textOf(tester, username), 'otro');
    });

    testWidgets('con cambios, el atrás del sistema pregunta; "Descartar" '
        'sale sin guardar', (tester) async {
      await pumpForm(tester);
      await enter(tester, password, 'algo');
      await tester.binding.handlePopRoute();
      await tester.pumpAndSettle();

      expect(discardDialog, findsOneWidget);
      await tester.tap(find.widgetWithText(TextButton, 'Descartar'));
      await tester.pumpAndSettle();
      expect(form, findsNothing);
      expect(session.addCalls, isEmpty);
      expect(popped, [null]);
    });

    for (final (name, change)
        in <(String, Future<void> Function(WidgetTester))>[
          (
            'un chip quitado',
            (t) => tapVisible(t, find.byTooltip('Quitar etiqueta')),
          ),
          ('texto pendiente de etiquetas', (t) => enter(t, tagsField, 'x')),
          ('una sección abierta', (t) => tapVisible(t, changePassword)),
        ]) {
      testWidgets('$name cuenta como cambio', (tester) async {
        await pumpForm(tester, editing: gmail);
        await change(tester);
        await tester.binding.handlePopRoute();
        await tester.pumpAndSettle();

        expect(discardDialog, findsOneWidget);
      });
    }

    testWidgets('volver al valor inicial deja de contar como cambio', (
      tester,
    ) async {
      await pumpForm(tester, editing: gmail);
      await enter(tester, username, 'otro');
      await enter(tester, username, 'ana@example.com');
      await tapVisible(tester, changePassword);
      await tapVisible(tester, cancelPassword);
      await tester.binding.handlePopRoute();
      await tester.pumpAndSettle();

      expect(discardDialog, findsNothing);
      expect(form, findsNothing);
    });
  });

  group('teclado', () {
    Future<void> tapEmpty(WidgetTester tester) async {
      await tester.tapAt(const Offset(4, 200));
      await tester.pump();
    }

    testWidgets('los controles del formulario no quitan el foco; un '
        'espacio vacío sí', (tester) async {
      await pumpForm(tester, editing: gmail);
      await tester.tap(service);
      await tester.pump();
      expect(fieldHasFocus(tester, service), isTrue);

      await tester.tap(find.widgetWithText(InputChip, 'correo'));
      await tester.pump();
      expect(fieldHasFocus(tester, service), isTrue);

      await tester.tap(changePassword);
      await tester.pump();
      expect(fieldHasFocus(tester, service), isTrue);

      await tester.tap(cancelPassword);
      await tester.pump();
      expect(fieldHasFocus(tester, service), isTrue);

      await tapEmpty(tester);
      expect(fieldHasFocus(tester, service), isFalse);
    });

    testWidgets('tocar "Guardar" no quita el foco', (tester) async {
      // Mientras cargan las notas, "Guardar" está desactivado pero los
      // campos no: es el único momento en que se puede tocar sin que el
      // campo pierda el foco por desactivarse.
      final pending = Completer<String?>();
      session.onRevealNotes = (_) => pending.future;
      await pumpForm(tester, editing: gmail);
      await tester.tap(service);
      await tester.pump();
      await tester.tap(editNotes);
      await tester.pump();
      expect(saveEnabled(tester), isFalse);
      expect(fieldHasFocus(tester, service), isTrue);

      await tester.tap(saveButton);
      await tester.pump();
      expect(fieldHasFocus(tester, service), isTrue);
      expect(session.editCalls, isEmpty);

      pending.complete(null);
      await tester.pump();
    });

    testWidgets('las notas cargadas y la contraseña nueva no se pueden '
        'copiar ni cortar', (tester) async {
      final copied = mockClipboard(tester);
      await pumpForm(tester, editing: gmail);
      await tapVisible(tester, editNotes);
      expect(textOf(tester, notes), fakeNotes);

      await selectAllAndPress(tester, notes, LogicalKeyboardKey.keyC);
      await selectAllAndPress(tester, notes, LogicalKeyboardKey.keyX);
      expect(copied, isEmpty);
      expect(textOf(tester, notes), fakeNotes);

      await tapVisible(tester, changePassword);
      await enter(tester, password, 'nueva-clave');
      await tester.tap(find.byTooltip('Mostrar contraseña'));
      await tester.pump();
      await selectAllAndPress(tester, password, LogicalKeyboardKey.keyC);
      expect(copied, isEmpty);

      // Los datos de búsqueda no son secretos: sí se copian.
      await selectAllAndPress(tester, username, LogicalKeyboardKey.keyC);
      expect(copied, ['ana@example.com']);
    });

    testWidgets('el campo con error queda a la vista aunque estuviera '
        'fuera de la pantalla', (tester) async {
      tester.view.physicalSize = const Size(360, 420);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      session.onAdd = (_, _) async => throw VaultError.invalidEntryServiceEmpty;
      await pumpForm(tester);
      await save(tester);
      await tester.pumpAndSettle();

      final view = tester.getRect(find.byType(SingleChildScrollView));
      // El texto escrito y el mensaje de error quedan dentro de la vista (la
      // etiqueta flotante puede asomar unos píxeles por arriba).
      final field = tester.getRect(
        find.descendant(of: service, matching: find.byType(EditableText)),
      );
      expect(field.top, greaterThanOrEqualTo(view.top));
      expect(field.bottom, lessThanOrEqualTo(view.bottom));
      expect(
        find.text('El servicio es obligatorio').hitTestable(),
        findsOneWidget,
      );
    });

    testWidgets('"Guardar" está en el grupo de los campos', (tester) async {
      await pumpForm(tester);
      expect(
        find.ancestor(
          of: saveButton,
          matching: find.byType(TextFieldTapRegion),
        ),
        findsWidgets,
      );
    });

    testWidgets('la lista cierra el teclado al desplazarse', (tester) async {
      await pumpForm(tester);
      expect(
        tester
            .widget<SingleChildScrollView>(find.byType(SingleChildScrollView))
            .keyboardDismissBehavior,
        ScrollViewKeyboardDismissBehavior.onDrag,
      );
    });

    testWidgets('ningún campo corrige ni aprende; cada uno con su teclado', (
      tester,
    ) async {
      await pumpForm(tester);
      for (final field in [service, username, url, tagsField, notes]) {
        final text = textFieldOf(tester, field);
        expect(text.autocorrect, isFalse);
        expect(text.enableIMEPersonalizedLearning, isFalse);
        // Con false, Android mostraría el teclado de contraseña.
        expect(text.enableSuggestions, isTrue);
      }
      expect(
        textFieldOf(tester, service).textCapitalization,
        TextCapitalization.sentences,
      );
      expect(
        textFieldOf(tester, username).keyboardType,
        TextInputType.emailAddress,
      );
      expect(textFieldOf(tester, url).keyboardType, TextInputType.url);

      final notesField = textFieldOf(tester, notes);
      expect(notesField.keyboardType, TextInputType.multiline);
      expect(notesField.minLines, 3);
      expect(notesField.maxLines, 8);

      final passwordField = textFieldOf(tester, password);
      expect(passwordField.obscureText, isTrue);
      expect(passwordField.enableIMEPersonalizedLearning, isFalse);
      expect(passwordField.enableSuggestions, isFalse);
    });
  });
}
