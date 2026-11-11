import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';

import '../src/rust/api/entries.dart';
import '../src/rust/api/error.dart';
import '../src/rust/api/session.dart';
import '../widgets/activity_text_field.dart';
import '../widgets/damaged_entries_banner.dart';
import 'entry_detail_screen.dart';
import 'entry_form_screen.dart';

/// Pantalla principal: barra de búsqueda y lista filtrada (#40,
/// detalle en `docs/superpowers/specs/2026-11-10-pantalla-principal-design.md`).
///
/// Toda la búsqueda la hace el núcleo: cada cambio del texto llama a
/// `search`, sin retardo, y la lista muestra el resultado en el orden
/// recibido, sin filtrar ni reordenar. Sin texto, el núcleo devuelve primero
/// las usadas recientemente. Como varias búsquedas pueden estar en curso y
/// responder en otro orden, cada una lleva una generación y solo se muestra
/// la de la última.
///
/// El campo no se enfoca al llegar: el teclado sale cuando el usuario lo
/// toca. Tras desbloquear con huella, Android descartaba la petición del
/// teclado (la ventana del diálogo todavía tenía el foco) y el campo quedaba
/// enfocado sin teclado; ver "Sin foco inicial" en el diseño de la #40.
///
/// Las entradas dañadas no aparecen en la lista: nunca llegan a la caché de
/// búsqueda del núcleo. Solo las cuenta [DamagedEntriesBanner], que va
/// dentro de esta pantalla para salir del árbol al bloquear.
///
/// Tocar una entrada abre su detalle y el botón "+" abre el formulario de
/// crear (#41), apilados en el `Navigator` raíz. Al volver de cualquiera de
/// los dos se repite la búsqueda con el texto actual: la entrada pudo
/// cambiar, eliminarse o subir en el orden de recientes.
class SearchScreen extends StatefulWidget {
  const SearchScreen({
    super.key,
    required this.session,
    required this.damagedEntries,
  });

  final VaultSession session;

  /// Total de entradas dañadas del último desbloqueo (0 al crear).
  final int damagedEntries;

  @override
  State<SearchScreen> createState() => _SearchScreenState();
}

class _SearchScreenState extends State<SearchScreen> {
  final _query = TextEditingController();
  final _focus = FocusNode();
  final _scroll = ScrollController();

  /// Sube con cada búsqueda lanzada; solo cuenta la respuesta de la última.
  int _generation = 0;

  /// `null` hasta la primera respuesta.
  List<EntrySummary>? _results;

  /// Texto al que corresponden los resultados en pantalla.
  String _shownQuery = '';

  bool _failed = false;

  @override
  void initState() {
    super.initState();
    _search('');
  }

  @override
  void dispose() {
    _query.dispose();
    _focus.dispose();
    _scroll.dispose();
    super.dispose();
  }

  Future<void> _search(String query) async {
    final generation = ++_generation;
    try {
      final results = await widget.session.search(query: query);
      if (!mounted || generation != _generation) return; // respuesta vieja
      final otherText = query != _shownQuery;
      setState(() {
        _results = results;
        _shownQuery = query;
        _failed = false;
      });
      // Con resultados de otro texto, el mejor tiene que verse arriba
      // aunque la lista estuviera desplazada.
      if (otherText && _scroll.hasClients) _scroll.jumpTo(0);
    } on VaultError catch (e) {
      // Carrera con el bloqueo: la pantalla ya está saliendo.
      if (e == VaultError.locked) return;
      _fail(generation);
    } catch (e) {
      // Solo el tipo: ni el mensaje ni el texto buscado.
      if (kDebugMode) debugPrint('search: ${e.runtimeType}');
      _fail(generation);
    }
  }

  void _fail(int generation) {
    if (!mounted || generation != _generation) return;
    setState(() => _failed = true);
  }

  void _onChanged(String text) {
    // El ✕ sigue al texto, no a la respuesta.
    setState(() {});
    _search(text);
  }

  void _clear() {
    _query.clear();
    setState(() {});
    _focus.requestFocus();
    _search('');
  }

  /// Al volver de otra ruta: la entrada pudo cambiar, eliminarse o usarse
  /// (revelar cambia el orden de recientes). Como el texto es el mismo, la
  /// lista no vuelve al principio.
  void _refresh() {
    if (mounted) _search(_query.text);
  }

  /// Las rutas van en el `Navigator` raíz, que el bloqueo vacía.
  MaterialPageRoute<void> _detailRoute(EntrySummary entry) {
    return MaterialPageRoute<void>(
      builder: (_) => EntryDetailScreen(session: widget.session, entry: entry),
    );
  }

  Future<void> _openDetail(EntrySummary entry) async {
    await Navigator.of(context, rootNavigator: true).push(_detailRoute(entry));
    _refresh();
  }

  Future<void> _openCreate() async {
    final navigator = Navigator.of(context, rootNavigator: true);
    var created = false;
    await navigator.push<void>(
      MaterialPageRoute(
        builder: (_) => EntryFormScreen.create(
          session: widget.session,
          onCreated: (entry) async {
            created = true;
            // El detalle de la entrada nueva ocupa el lugar del formulario:
            // muestra lo que el núcleo guardó, aunque el texto de búsqueda
            // no coincida con ella. Este futuro es el del detalle y se
            // completa al volver a la lista.
            await navigator.pushReplacement<void, void>(_detailRoute(entry));
            _refresh();
          },
        ),
      ),
    );
    // Con `pushReplacement`, el futuro del formulario se completa en el
    // reemplazo, con el detalle todavía encima (`navigator.dart`, Flutter
    // 3.47.9: la ruta reemplazada recibe `didComplete`). Solo cuenta si el
    // formulario salió sin crear.
    if (!created) _refresh();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      floatingActionButton: FloatingActionButton(
        tooltip: 'Agregar contraseña',
        onPressed: _openCreate,
        child: const Icon(Icons.add),
      ),
      body: SafeArea(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            DamagedEntriesBanner(count: widget.damagedEntries),
            Padding(
              padding: const EdgeInsets.all(16),
              child: ActivityTextField(
                controller: _query,
                focusNode: _focus,
                textInputAction: TextInputAction.search,
                // La tecla "buscar" no quita el foco.
                onEditingComplete: () {},
                onChanged: _onChanged,
                // Con false, Flutter marca el campo en Android como contraseña
                // visible (VISIBLE_PASSWORD) y el teclado muestra la fila de
                // números. Sin aprendizaje del teclado, sus sugerencias no
                // guardan lo que se busca.
                autocorrect: false,
                enableIMEPersonalizedLearning: false,
                decoration: InputDecoration(
                  hintText: 'Buscar',
                  prefixIcon: const Icon(Icons.search),
                  suffixIcon: _query.text.isEmpty
                      ? null
                      : IconButton(
                          tooltip: 'Borrar búsqueda',
                          icon: const Icon(Icons.close),
                          onPressed: _clear,
                        ),
                ),
              ),
            ),
            Expanded(child: _body()),
          ],
        ),
      ),
    );
  }

  Widget _body() {
    if (_failed) {
      return Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            const Text('No se pudo buscar'),
            const SizedBox(height: 8),
            TextButton(
              onPressed: () => _search(_query.text),
              child: const Text('Reintentar'),
            ),
          ],
        ),
      );
    }
    final results = _results;
    if (results == null) {
      return const Center(child: CircularProgressIndicator());
    }
    if (results.isEmpty) {
      final text = _shownQuery.trim();
      return Center(
        child: Padding(
          padding: const EdgeInsets.all(16),
          child: Text(
            text.isEmpty
                ? 'Todavía no hay contraseñas guardadas. '
                      'Toca + para agregar la primera.'
                : 'Sin resultados para «$text»',
            textAlign: TextAlign.center,
          ),
        ),
      );
    }
    return ListView.builder(
      controller: _scroll,
      keyboardDismissBehavior: ScrollViewKeyboardDismissBehavior.onDrag,
      // Espacio para que el botón "+" no tape la última entrada.
      padding: const EdgeInsets.only(bottom: 88),
      itemCount: results.length,
      itemBuilder: (context, index) {
        final entry = results[index];
        return _EntryTile(entry: entry, onTap: () => _openDetail(entry));
      },
    );
  }
}

/// Un elemento de la lista: inicial, servicio y usuario (o la URL).
class _EntryTile extends StatelessWidget {
  _EntryTile({required this.entry, required this.onTap})
    : super(key: ValueKey(entry.id));

  final EntrySummary entry;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final service = entry.service;
    // El primer grafema, para acentos y emoji compuestos. El núcleo no
    // admite servicios vacíos; '?' es solo defensa.
    final initial = service.isEmpty
        ? '?'
        : service.characters.first.toUpperCase();
    final detail = entry.username ?? entry.url;
    return ListTile(
      onTap: onTap,
      // El lector de pantalla no lee la letra antes del servicio.
      leading: ExcludeSemantics(child: CircleAvatar(child: Text(initial))),
      title: Text(service),
      subtitle: detail == null
          ? null
          : Text(detail, maxLines: 1, overflow: TextOverflow.ellipsis),
    );
  }
}
