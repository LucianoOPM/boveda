// Pruebas de la pantalla principal: búsqueda enfocada y lista filtrada por
// el núcleo (simulado).

import 'dart:async';

import 'package:boveda/screens/search_screen.dart';
import 'package:boveda/src/rust/api/entries.dart';
import 'package:boveda/src/rust/api/error.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'fakes.dart';

late FakeVaultSession session;

final searchField = find.byType(TextField);
final clearButton = find.byTooltip('Borrar búsqueda');

Future<void> pumpSearch(WidgetTester tester, {int damaged = 0}) async {
  await tester.pumpWidget(
    MaterialApp(
      home: SearchScreen(session: session, damagedEntries: damaged),
    ),
  );
  await tester.pump();
}

/// Respuestas que la prueba completa a mano, una por texto buscado.
Map<String, Completer<List<EntrySummary>>> pendingSearches() {
  final pending = <String, Completer<List<EntrySummary>>>{};
  session.onSearch = (query) => (pending[query] = Completer()).future;
  return pending;
}

Future<void> type(WidgetTester tester, String text) async {
  await tester.enterText(searchField, text);
  await tester.pump();
}

/// [n] entradas con servicios distintos que empiezan con [prefix].
List<EntrySummary> many(int n, [String prefix = 'Servicio']) => [
  for (var i = 0; i < n; i++) entry('$prefix $i', username: 'usuario$i'),
];

bool searchHasFocus(WidgetTester tester) =>
    fieldHasFocus(tester, find.byType(SearchScreen));

Future<void> tapEmpty(WidgetTester tester) async {
  await tester.tapAt(const Offset(1, 1));
  await tester.pump();
}

void main() {
  setUp(() => session = FakeVaultSession());

  testWidgets('primera carga: indicador y luego la lista en el orden '
      'recibido', (tester) async {
    final pending = pendingSearches();
    await pumpSearch(tester);
    expect(find.byType(CircularProgressIndicator), findsOneWidget);

    pending['']!.complete([entry('Zeta'), entry('Alfa')]);
    await tester.pump();
    expect(find.byType(CircularProgressIndicator), findsNothing);
    expect(
      tester.getTopLeft(find.text('Zeta')).dy,
      lessThan(tester.getTopLeft(find.text('Alfa')).dy),
    );
  });

  testWidgets('subtítulo: usuario, si no URL, si no nada', (tester) async {
    const longUrl =
        'https://una.direccion.muy.larga.example.com/ruta/que/no/cabe/en/una/'
        'sola/linea/de/la/pantalla';
    session.onSearch = (_) async => [
      entry('A', username: 'ana', url: 'a.com'),
      entry('B', url: longUrl),
      entry('C'),
    ];
    await pumpSearch(tester);

    expect(find.text('ana'), findsOneWidget);
    expect(find.text('a.com'), findsNothing);
    final url = tester.widget<Text>(find.text(longUrl));
    expect(url.maxLines, 1);
    expect(url.overflow, TextOverflow.ellipsis);
    // "C" también es la inicial: se toma el tercer elemento de la lista.
    final c = tester.widgetList<ListTile>(find.byType(ListTile)).elementAt(2);
    expect((c.title as Text).data, 'C');
    expect(c.subtitle, isNull);
  });

  testWidgets('inicial: primer grafema en mayúscula', (tester) async {
    session.onSearch = (_) async => [
      entry('gmail'),
      entry('Ébano'),
      entry('🔑 Llaves'),
      entry('👩‍💻 Trabajo'),
    ];
    await pumpSearch(tester);

    for (final initial in ['G', 'É', '🔑', '👩‍💻']) {
      expect(
        find.descendant(
          of: find.byType(CircleAvatar),
          matching: find.text(initial),
        ),
        findsOneWidget,
        reason: initial,
      );
    }
  });

  testWidgets('la inicial no se lee con el lector de pantalla', (tester) async {
    final handle = tester.ensureSemantics();
    session.onSearch = (_) async => [entry('gmail')];
    await pumpSearch(tester);

    expect(find.bySemanticsLabel(RegExp('gmail')), findsWidgets);
    // Ni como etiqueta propia ni al principio de una etiqueta combinada.
    expect(find.bySemanticsLabel(RegExp(r'^G')), findsNothing);
    handle.dispose();
  });

  testWidgets('busca en cada cambio', (tester) async {
    await pumpSearch(tester);
    await type(tester, 'g');
    await type(tester, 'gm');
    expect(session.searchCalls, ['', 'g', 'gm']);
  });

  testWidgets('una respuesta vieja no pisa la nueva', (tester) async {
    final pending = pendingSearches();
    await pumpSearch(tester);
    pending['']!.complete([]);
    await type(tester, 'g');
    await type(tester, 'gm');

    pending['gm']!.complete([entry('Gmail')]);
    await tester.pump();
    pending['g']!.complete([entry('Google')]);
    await tester.pump();
    expect(find.text('Gmail'), findsOneWidget);
    expect(find.text('Google'), findsNothing);
  });

  testWidgets('la búsqueda inicial tardía no pisa la primera letra', (
    tester,
  ) async {
    final pending = pendingSearches();
    await pumpSearch(tester);
    await type(tester, 'g');

    pending['g']!.complete([entry('Gmail')]);
    await tester.pump();
    pending['']!.complete([entry('Alfa'), entry('Beta'), entry('Gmail')]);
    await tester.pump();
    expect(find.text('Gmail'), findsOneWidget);
    expect(find.text('Alfa'), findsNothing);
  });

  testWidgets('un fallo viejo no muestra el error', (tester) async {
    final pending = pendingSearches();
    await pumpSearch(tester);
    pending['']!.complete([]);
    await type(tester, 'g');
    await type(tester, 'gm');

    pending['gm']!.complete([entry('Gmail')]);
    await tester.pump();
    pending['g']!.completeError(StateError('falla'));
    await tester.pump();
    expect(find.text('No se pudo buscar'), findsNothing);
    expect(find.text('Gmail'), findsOneWidget);
  });

  testWidgets('bóveda vacía', (tester) async {
    await pumpSearch(tester);
    expect(find.text('Todavía no hay contraseñas guardadas'), findsOneWidget);

    await type(tester, '   ');
    expect(find.text('Todavía no hay contraseñas guardadas'), findsOneWidget);
  });

  testWidgets('sin resultados', (tester) async {
    await pumpSearch(tester);
    await type(tester, 'xyz');
    expect(find.text('Sin resultados para «xyz»'), findsOneWidget);
  });

  testWidgets('fallo: mensaje, Reintentar y recuperación', (tester) async {
    session.onSearch = (_) async => throw StateError('falla');
    await pumpSearch(tester);
    expect(find.text('No se pudo buscar'), findsOneWidget);
    expect(find.text('Reintentar'), findsOneWidget);

    session.onSearch = (_) async => [entry('Gmail')];
    await tester.tap(find.text('Reintentar'));
    await tester.pump();
    expect(session.searchCalls.last, '');
    expect(find.text('No se pudo buscar'), findsNothing);
    expect(find.text('Gmail'), findsOneWidget);

    session.onSearch = (_) async => throw StateError('falla');
    await type(tester, 'g');
    expect(find.text('No se pudo buscar'), findsOneWidget);

    session.onSearch = (_) async => [entry('Gmail')];
    await type(tester, 'gm');
    expect(find.text('No se pudo buscar'), findsNothing);
    expect(find.text('Gmail'), findsOneWidget);
  });

  testWidgets('Locked se ignora en silencio', (tester) async {
    session.onSearch = (_) async => [entry('Gmail')];
    await pumpSearch(tester);

    session.onSearch = (_) async => throw VaultError.locked;
    await type(tester, 'g');
    expect(find.text('No se pudo buscar'), findsNothing);
    expect(find.text('Gmail'), findsOneWidget);
  });

  testWidgets('una respuesta tras desmontar no lanza', (tester) async {
    final pending = pendingSearches();
    await pumpSearch(tester);
    await tester.pumpWidget(const SizedBox());

    pending['']!.complete([entry('Gmail')]);
    await tester.pump();
    expect(tester.takeException(), isNull);
  });

  testWidgets('el ✕ sigue al texto aunque la búsqueda esté pendiente', (
    tester,
  ) async {
    await pumpSearch(tester);
    expect(clearButton, findsNothing);

    session.onSearch = (_) => Completer<List<EntrySummary>>().future;
    await type(tester, 'g');
    expect(clearButton, findsOneWidget);
  });

  testWidgets('el ✕ borra, deja el foco y busca vacío', (tester) async {
    await pumpSearch(tester);
    await type(tester, 'gm');

    await tester.tap(clearButton);
    await tester.pump();
    expect(tester.widget<TextField>(searchField).controller!.text, isEmpty);
    expect(searchHasFocus(tester), isTrue);
    expect(session.searchCalls.last, '');
    expect(clearButton, findsNothing);
  });

  testWidgets('el ✕ devuelve el foco tras perderlo', (tester) async {
    await pumpSearch(tester);
    await type(tester, 'g');
    await tapEmpty(tester);
    expect(searchHasFocus(tester), isFalse);

    await tester.tap(clearButton);
    await tester.pump();
    expect(searchHasFocus(tester), isTrue);
  });

  testWidgets('la tecla buscar no quita el foco', (tester) async {
    await pumpSearch(tester);
    await type(tester, 'g');
    await tester.testTextInput.receiveAction(TextInputAction.search);
    await tester.pump();
    expect(searchHasFocus(tester), isTrue);
  });

  testWidgets('configuración del teclado', (tester) async {
    await pumpSearch(tester);
    final field = tester.widget<TextField>(searchField);
    expect(field.enableSuggestions, isFalse);
    expect(field.autocorrect, isFalse);
    expect(field.enableIMEPersonalizedLearning, isFalse);
    expect(field.textInputAction, TextInputAction.search);
  });

  testWidgets('el campo tiene el foco al montar', (tester) async {
    await pumpSearch(tester);
    expect(searchHasFocus(tester), isTrue);
  });

  testWidgets('desplazar la lista, tocar una entrada o un espacio vacío '
      'cierran el teclado', (tester) async {
    session.onSearch = (_) async => many(40);
    await pumpSearch(tester);
    final list = find.byType(ListView);
    expect(
      tester.widget<ListView>(list).keyboardDismissBehavior,
      ScrollViewKeyboardDismissBehavior.onDrag,
    );
    expect(searchHasFocus(tester), isTrue);

    await tester.drag(list, const Offset(0, -300));
    await tester.pump();
    expect(searchHasFocus(tester), isFalse);

    await tester.tap(searchField);
    await tester.pump();
    expect(searchHasFocus(tester), isTrue);
    await tester.tap(find.byType(ListTile).first);
    await tester.pump();
    expect(searchHasFocus(tester), isFalse);

    await tester.tap(searchField);
    await tester.pump();
    expect(searchHasFocus(tester), isTrue);
    await tapEmpty(tester);
    expect(searchHasFocus(tester), isFalse);
  });

  testWidgets('resultados de otro texto vuelven al principio de la lista', (
    tester,
  ) async {
    session.onSearch = (_) async => many(40);
    await pumpSearch(tester);
    final list = find.byType(ListView);
    await tester.drag(list, const Offset(0, -300));
    await tester.pumpAndSettle();
    final controller = tester.widget<ListView>(list).controller!;
    expect(controller.offset, greaterThan(0));

    session.onSearch = (_) async => many(40, 'Otro');
    await type(tester, 'e');
    await tester.pump();
    expect(controller.offset, 0);
  });

  testWidgets('aviso de dañadas arriba de la lista', (tester) async {
    await pumpSearch(tester, damaged: 2);
    final banner = find.text('2 entradas no se pudieron leer');
    expect(banner, findsOneWidget);
    expect(
      tester.getTopLeft(banner).dy,
      lessThan(tester.getTopLeft(searchField).dy),
    );
  });

  testWidgets('sin dañadas no hay aviso', (tester) async {
    await pumpSearch(tester);
    expect(find.byType(MaterialBanner), findsNothing);
  });

  testWidgets('sin desbordamiento en una pantalla baja con aviso y teclado', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(800, 400);
    tester.view.devicePixelRatio = 1;
    tester.view.viewInsets = const FakeViewPadding(bottom: 200);
    addTearDown(tester.view.reset);
    session.onSearch = (_) async => many(10);

    await pumpSearch(tester, damaged: 3);
    expect(tester.takeException(), isNull);
  });
}
