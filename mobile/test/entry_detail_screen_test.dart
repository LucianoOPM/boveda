// Pruebas del detalle de una entrada (#41), con la sesión simulada.

import 'dart:async';

import 'package:boveda/screens/entry_detail_screen.dart';
import 'package:boveda/src/rust/api/entries.dart';
import 'package:boveda/src/rust/api/error.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'fakes.dart';

late FakeVaultSession session;

final detail = find.byType(EntryDetailScreen);
final showPassword = find.widgetWithText(TextButton, 'Mostrar');
final hidePassword = find.widgetWithText(TextButton, 'Ocultar');
final showNotes = find.widgetWithText(TextButton, 'Mostrar notas');
final hideNotes = find.widgetWithText(TextButton, 'Ocultar notas');
final editAction = find.byTooltip('Editar');
final deleteAction = find.byTooltip('Eliminar');
final editButton = find.widgetWithIcon(IconButton, Icons.edit);
final deleteButton = find.widgetWithIcon(IconButton, Icons.delete);

const damagedText = 'La contraseña y las notas de esta entrada están dañadas';
const revealFailedText = 'No se pudo mostrar. Intenta de nuevo.';

final gmail = entry(
  'Gmail',
  username: 'ana@example.com',
  url: 'https://mail.example.com',
  tags: ['correo', 'personal'],
);

/// Llamadas al formulario simulado: la entrada y si era reparación.
final List<({EntrySummary entry, bool repair})> formCalls = [];

/// Lo que devuelve el formulario simulado.
Future<bool?> Function() onForm = () async => true;

Future<bool?> fakeForm(
  BuildContext context,
  EntrySummary entry, {
  required bool repair,
}) {
  formCalls.add((entry: entry, repair: repair));
  return onForm();
}

/// Monta el detalle como segunda ruta, sobre una pantalla base, para poder
/// comprobar cuándo sale.
Future<void> pumpDetail(
  WidgetTester tester, {
  EntrySummary? of,
  OpenEntryForm? openForm = fakeForm,
}) async {
  await tester.pumpWidget(
    MaterialApp(
      home: Builder(
        builder: (context) => Scaffold(
          body: Center(
            child: TextButton(
              onPressed: () => Navigator.of(context).push<void>(
                MaterialPageRoute(
                  builder: (_) => EntryDetailScreen(
                    session: session,
                    entry: of ?? gmail,
                    openForm: openForm,
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

Future<void> tapAndPump(WidgetTester tester, Finder finder) async {
  await tester.tap(finder);
  await tester.pump();
  await tester.pump();
}

bool enabled(WidgetTester tester, Finder button) =>
    tester.widget<ButtonStyleButton>(button).onPressed != null;

/// Quita la ruta del detalle sin animación, como hace el bloqueo.
void removeDetailRoute(WidgetTester tester) {
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
    formCalls.clear();
    onForm = () async => true;
  });

  group('contenido', () {
    testWidgets('abrir no llama a Rust y muestra los datos de búsqueda', (
      tester,
    ) async {
      await pumpDetail(tester);

      expect(session.revealPasswordCalls, isEmpty);
      expect(session.revealNotesCalls, isEmpty);
      expect(session.entryCalls, isEmpty);
      expect(find.widgetWithText(AppBar, 'Gmail'), findsOneWidget);
      expect(find.text('ana@example.com'), findsOneWidget);
      expect(find.text('https://mail.example.com'), findsOneWidget);
      expect(find.widgetWithText(Chip, 'correo'), findsOneWidget);
      expect(find.widgetWithText(Chip, 'personal'), findsOneWidget);
      expect(showPassword, findsOneWidget);
      expect(showNotes, findsOneWidget);
    });

    testWidgets('las filas opcionales aparecen solo con dato', (tester) async {
      await pumpDetail(tester, of: entry('Banco'));

      expect(find.text('Usuario'), findsNothing);
      expect(find.text('URL'), findsNothing);
      expect(find.text('Etiquetas'), findsNothing);
      expect(find.text('Contraseña'), findsOneWidget);
      expect(find.text('Notas'), findsOneWidget);
    });

    testWidgets('la lista cierra el teclado al desplazarse', (tester) async {
      await pumpDetail(tester);
      expect(
        tester.widget<ListView>(find.byType(ListView)).keyboardDismissBehavior,
        ScrollViewKeyboardDismissBehavior.onDrag,
      );
    });

    testWidgets('oculta, la contraseña expone "Contraseña oculta" y no los '
        'puntos', (tester) async {
      final handle = tester.ensureSemantics();
      await pumpDetail(tester);

      expect(find.bySemanticsLabel('Contraseña oculta'), findsOneWidget);
      expect(find.bySemanticsLabel(RegExp('•')), findsNothing);
      handle.dispose();
    });

    testWidgets('textos al límite no desbordan', (tester) async {
      session.onRevealNotes = (_) async => 'línea de notas\n' * 1000;
      session.onRevealPassword = (_) async => 'p' * 1024;
      await pumpDetail(
        tester,
        of: entry(
          's' * 256,
          username: 'u' * 256,
          url: 'h' * 2048,
          tags: [for (var i = 0; i < 32; i++) 'etiqueta-$i-${'x' * 50}'],
        ),
      );
      expect(tester.takeException(), isNull);

      final list = find.byType(Scrollable).first;
      await tester.scrollUntilVisible(showPassword, 300, scrollable: list);
      await tapAndPump(tester, showPassword);
      await tester.scrollUntilVisible(showNotes, 300, scrollable: list);
      await tapAndPump(tester, showNotes);
      expect(find.textContaining('línea de notas'), findsOneWidget);
      expect(tester.takeException(), isNull);
    });
  });

  group('contraseña', () {
    testWidgets('"Mostrar" la revela una vez; "Ocultar" la quita del árbol', (
      tester,
    ) async {
      await pumpDetail(tester);
      await tapAndPump(tester, showPassword);

      expect(session.revealPasswordCalls, [gmail.id]);
      expect(session.revealNotesCalls, isEmpty);
      expect(find.text(fakePassword), findsOneWidget);
      expect(find.byType(SelectableText), findsNothing);
      expect(
        tester.widget<Text>(find.text(fakePassword)).style?.fontFamily,
        'monospace',
      );

      await tapAndPump(tester, hidePassword);
      expect(find.text(fakePassword), findsNothing);
      expect(showPassword, findsOneWidget);
      expect(session.revealPasswordCalls, hasLength(1));
    });

    testWidgets('durante la carga el botón está desactivado', (tester) async {
      final pending = Completer<String>();
      session.onRevealPassword = (_) => pending.future;
      await pumpDetail(tester);
      await tapAndPump(tester, showPassword);

      expect(enabled(tester, showPassword), isFalse);
      await tester.tap(showPassword, warnIfMissed: false);
      expect(session.revealPasswordCalls, hasLength(1));

      pending.complete('tardía');
      await tester.pump();
      expect(find.text('tardía'), findsOneWidget);
    });

    testWidgets('una respuesta tras desmontar no lanza nada', (tester) async {
      final pending = Completer<String>();
      session.onRevealPassword = (_) => pending.future;
      await pumpDetail(tester);
      await tapAndPump(tester, showPassword);

      removeDetailRoute(tester);
      await tester.pump();
      pending.complete('tardía');
      await tester.pump();

      expect(tester.takeException(), isNull);
      expect(find.text('tardía'), findsNothing);
    });
  });

  group('notas', () {
    testWidgets('"Mostrar notas" las revela sin tocar la contraseña', (
      tester,
    ) async {
      await pumpDetail(tester);
      await tapAndPump(tester, showNotes);

      expect(session.revealNotesCalls, [gmail.id]);
      expect(session.revealPasswordCalls, isEmpty);
      expect(find.text(fakeNotes), findsOneWidget);
      expect(find.byType(SelectableText), findsNothing);

      await tapAndPump(tester, hideNotes);
      expect(find.text(fakeNotes), findsNothing);
      expect(showNotes, findsOneWidget);
    });

    testWidgets('sin notas muestra "Sin notas"', (tester) async {
      session.onRevealNotes = (_) async => null;
      await pumpDetail(tester);
      await tapAndPump(tester, showNotes);

      expect(find.text('Sin notas'), findsOneWidget);
      expect(hideNotes, findsOneWidget);
    });
  });

  group('errores al revelar', () {
    testWidgets('corruptEntry en la contraseña: aviso con Reparar y '
        'Eliminar', (tester) async {
      session.onRevealPassword = (_) async => throw VaultError.corruptEntry;
      await pumpDetail(tester);
      await tapAndPump(tester, showPassword);

      expect(find.text(damagedText), findsOneWidget);
      expect(find.widgetWithText(FilledButton, 'Reparar'), findsOneWidget);
      expect(find.widgetWithText(TextButton, 'Eliminar'), findsOneWidget);
      expect(showPassword, findsNothing);
      expect(showNotes, findsNothing);
    });

    testWidgets('corruptEntry en las notas: el mismo aviso y se descarta la '
        'contraseña visible', (tester) async {
      session.onRevealNotes = (_) async => throw VaultError.corruptEntry;
      await pumpDetail(tester);
      await tapAndPump(tester, showPassword);
      await tapAndPump(tester, showNotes);

      expect(find.text(damagedText), findsOneWidget);
      expect(find.text(fakePassword), findsNothing);
    });

    testWidgets('un error genérico deja reintentar', (tester) async {
      var fail = true;
      session.onRevealPassword = (_) async =>
          fail ? throw VaultError.storageIo : fakePassword;
      await pumpDetail(tester);
      await tapAndPump(tester, showPassword);

      expect(find.text(revealFailedText), findsOneWidget);
      expect(enabled(tester, showPassword), isTrue);

      fail = false;
      await tapAndPump(tester, showPassword);
      expect(find.text(revealFailedText), findsNothing);
      expect(find.text(fakePassword), findsOneWidget);
    });

    testWidgets('si fallan las notas, la contraseña visible se conserva', (
      tester,
    ) async {
      session.onRevealNotes = (_) async => throw VaultError.storageIo;
      await pumpDetail(tester);
      await tapAndPump(tester, showPassword);
      await tapAndPump(tester, showNotes);

      expect(find.text(fakePassword), findsOneWidget);
      expect(find.text(revealFailedText), findsOneWidget);
      expect(showNotes, findsOneWidget);
    });

    testWidgets('si falla la contraseña, las notas visibles se conservan', (
      tester,
    ) async {
      session.onRevealPassword = (_) async => throw VaultError.storageIo;
      await pumpDetail(tester);
      await tapAndPump(tester, showNotes);
      await tapAndPump(tester, showPassword);

      expect(find.text(fakeNotes), findsOneWidget);
      expect(find.text(revealFailedText), findsOneWidget);
    });

    testWidgets('locked no muestra nada', (tester) async {
      session.onRevealPassword = (_) async => throw VaultError.locked;
      await pumpDetail(tester);
      await tapAndPump(tester, showPassword);

      expect(find.text(revealFailedText), findsNothing);
      expect(find.text(damagedText), findsNothing);
      expect(detail, findsOneWidget);
    });

    testWidgets('entryNotFound vuelve a la lista', (tester) async {
      session.onRevealNotes = (_) async => throw VaultError.entryNotFound;
      await pumpDetail(tester);
      await tester.tap(showNotes);
      await tester.pumpAndSettle();

      expect(detail, findsNothing);
      expect(find.text('abrir'), findsOneWidget);
    });

    testWidgets('un error que no es VaultError también deja reintentar', (
      tester,
    ) async {
      session.onRevealPassword = (_) async => throw StateError('interno');
      await pumpDetail(tester);
      await tapAndPump(tester, showPassword);

      expect(find.text(revealFailedText), findsOneWidget);
      expect(find.textContaining('interno'), findsNothing);
    });
  });

  group('eliminar', () {
    final dialog = find.byType(AlertDialog);
    final confirm = find.descendant(
      of: dialog,
      matching: find.widgetWithText(TextButton, 'Eliminar'),
    );

    testWidgets('pide confirmación; cancelar no elimina', (tester) async {
      await pumpDetail(tester);
      await tester.tap(deleteAction);
      await tester.pumpAndSettle();

      expect(find.text('¿Eliminar «Gmail»?'), findsOneWidget);
      expect(find.text('Esta acción no se puede deshacer.'), findsOneWidget);

      await tester.tap(find.widgetWithText(TextButton, 'Cancelar'));
      await tester.pumpAndSettle();
      expect(session.deleteCalls, isEmpty);
      expect(detail, findsOneWidget);
    });

    testWidgets('confirmar elimina y vuelve a la lista', (tester) async {
      await pumpDetail(tester);
      await tester.tap(deleteAction);
      await tester.pumpAndSettle();
      await tester.tap(confirm);
      await tester.pumpAndSettle();

      expect(session.deleteCalls, [gmail.id]);
      expect(detail, findsNothing);
    });

    testWidgets('entryNotFound cuenta como eliminada', (tester) async {
      session.onDelete = (_) async => throw VaultError.entryNotFound;
      await pumpDetail(tester);
      await tester.tap(deleteAction);
      await tester.pumpAndSettle();
      await tester.tap(confirm);
      await tester.pumpAndSettle();

      expect(detail, findsNothing);
    });

    testWidgets('un fallo avisa y se queda en el detalle', (tester) async {
      session.onDelete = (_) async => throw VaultError.storageIo;
      await pumpDetail(tester);
      await tester.tap(deleteAction);
      await tester.pumpAndSettle();
      await tester.tap(confirm);
      await tester.pumpAndSettle();

      expect(
        find.text('No se pudo eliminar. Intenta de nuevo.'),
        findsOneWidget,
      );
      expect(detail, findsOneWidget);
      expect(enabled(tester, showPassword), isTrue);
    });

    testWidgets('mientras elimina, las acciones están desactivadas', (
      tester,
    ) async {
      final pending = Completer<void>();
      session.onDelete = (_) => pending.future;
      await pumpDetail(tester);
      await tester.tap(deleteAction);
      await tester.pumpAndSettle();
      await tester.tap(confirm);
      await tester.pumpAndSettle();

      expect(tester.widget<IconButton>(deleteButton).onPressed, isNull);
      expect(tester.widget<IconButton>(editButton).onPressed, isNull);
      expect(enabled(tester, showPassword), isFalse);
      expect(enabled(tester, showNotes), isFalse);

      pending.complete();
      await tester.pumpAndSettle();
      expect(detail, findsNothing);
    });

    testWidgets('delete que termina tras quitar la ruta no navega ni lanza', (
      tester,
    ) async {
      final pending = Completer<void>();
      session.onDelete = (_) => pending.future;
      await pumpDetail(tester);
      await tester.tap(deleteAction);
      await tester.pumpAndSettle();
      await tester.tap(confirm);
      await tester.pumpAndSettle();

      removeDetailRoute(tester);
      await tester.pump();
      pending.complete();
      await tester.pumpAndSettle();

      expect(tester.takeException(), isNull);
      expect(find.text('abrir'), findsOneWidget);
    });

    testWidgets('desde el aviso de dañada también se elimina', (tester) async {
      session.onRevealPassword = (_) async => throw VaultError.corruptEntry;
      await pumpDetail(tester);
      await tapAndPump(tester, showPassword);
      await tester.tap(find.widgetWithText(TextButton, 'Eliminar'));
      await tester.pumpAndSettle();
      await tester.tap(confirm);
      await tester.pumpAndSettle();

      expect(session.deleteCalls, [gmail.id]);
      expect(detail, findsNothing);
    });
  });

  group('editar y reparar', () {
    testWidgets('sin formulario, "Editar" está desactivado', (tester) async {
      await pumpDetail(tester, openForm: null);
      expect(tester.widget<IconButton>(editButton).onPressed, isNull);
    });

    testWidgets('abrir el formulario descarta lo revelado', (tester) async {
      final form = Completer<bool?>();
      onForm = () => form.future;
      await pumpDetail(tester);
      await tapAndPump(tester, showPassword);
      await tapAndPump(tester, showNotes);

      await tapAndPump(tester, editAction);
      expect(formCalls.single.entry.id, gmail.id);
      expect(formCalls.single.repair, isFalse);
      expect(find.text(fakePassword), findsNothing);
      expect(find.text(fakeNotes), findsNothing);

      form.complete(null);
      await tester.pump();
    });

    testWidgets('una respuesta de revelado que llega con el formulario '
        'abierto se descarta', (tester) async {
      final reveal = Completer<String>();
      final form = Completer<bool?>();
      session.onRevealPassword = (_) => reveal.future;
      onForm = () => form.future;
      await pumpDetail(tester);
      await tapAndPump(tester, showPassword);
      await tapAndPump(tester, editAction);

      reveal.complete('tardía');
      await tester.pump();
      expect(find.text('tardía'), findsNothing);

      form.complete(null);
      await tester.pump();
      expect(find.text('tardía'), findsNothing);
      expect(enabled(tester, showPassword), isTrue);
    });

    testWidgets('al volver con true refresca con entry(id)', (tester) async {
      session.onEntry = (id) async =>
          entry('Correo nuevo', id: id, username: 'otra@example.com');
      await pumpDetail(tester);
      await tapAndPump(tester, editAction);

      expect(session.entryCalls, [gmail.id]);
      expect(find.widgetWithText(AppBar, 'Correo nuevo'), findsOneWidget);
      expect(find.text('otra@example.com'), findsOneWidget);
      expect(find.text('ana@example.com'), findsNothing);
    });

    testWidgets('al volver sin guardar no llama a entry', (tester) async {
      onForm = () async => null;
      await pumpDetail(tester);
      await tapAndPump(tester, editAction);

      expect(session.entryCalls, isEmpty);
      expect(find.widgetWithText(AppBar, 'Gmail'), findsOneWidget);
    });

    testWidgets('si la entrada ya no existe al refrescar, vuelve a la lista', (
      tester,
    ) async {
      session.onEntry = (_) async => throw VaultError.entryNotFound;
      await pumpDetail(tester);
      await tester.tap(editAction);
      await tester.pumpAndSettle();

      expect(detail, findsNothing);
    });

    testWidgets('si refrescar falla, conserva lo que mostraba', (tester) async {
      session.onEntry = (_) async => throw VaultError.storageIo;
      await pumpDetail(tester);
      await tapAndPump(tester, editAction);

      expect(detail, findsOneWidget);
      expect(find.widgetWithText(AppBar, 'Gmail'), findsOneWidget);
    });

    testWidgets('"Reparar" abre el formulario en modo reparar y, al volver, '
        'el secreto queda oculto', (tester) async {
      session.onRevealPassword = (_) async => throw VaultError.corruptEntry;
      await pumpDetail(tester);
      await tapAndPump(tester, showPassword);
      session.onRevealPassword = (_) async => fakePassword;

      await tapAndPump(tester, find.widgetWithText(FilledButton, 'Reparar'));
      expect(formCalls.single.repair, isTrue);
      expect(find.text(damagedText), findsNothing);
      expect(showPassword, findsOneWidget);
      expect(showNotes, findsOneWidget);
    });

    testWidgets('si la reparación se abandona, el aviso sigue', (tester) async {
      session.onRevealPassword = (_) async => throw VaultError.corruptEntry;
      onForm = () async => null;
      await pumpDetail(tester);
      await tapAndPump(tester, showPassword);
      await tapAndPump(tester, find.widgetWithText(FilledButton, 'Reparar'));

      expect(find.text(damagedText), findsOneWidget);
    });
  });
}
