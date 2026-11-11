import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';

import '../src/rust/api/entries.dart';
import '../src/rust/api/error.dart';
import '../src/rust/api/session.dart';
import 'entry_form_screen.dart';

/// Abre el formulario de edición de [entry] (en modo reparar si [repair]) y
/// devuelve `true` si guardó.
typedef OpenEntryForm = Future<bool?> Function(
  BuildContext context,
  EntrySummary entry, {
  required bool repair,
});

/// Detalle de una entrada (#41, diseño en
/// `docs/superpowers/specs/2026-11-10-entradas-design.md`).
///
/// Abrirla no llama a Rust: muestra el resumen que ya tiene quien la abre.
/// La contraseña y las notas se descifran solo al tocar "Mostrar" o "Mostrar
/// notas", cada una con su propia llamada, y cada revelado cuenta como uso
/// de la entrada. "Mostrar notas" aparece siempre: sin revelar el secreto no
/// se sabe si hay notas.
///
/// Los textos revelados viven solo en el `State` de esta pantalla. Se
/// descartan al ocultarlos, al abrir el formulario (la pantalla cubierta no
/// retiene nada) y al salir; el bloqueo quita la ruta, así que también pasa
/// por `dispose`. No son seleccionables: copiar es otra acción (#42), que
/// marca el contenido como sensible.
///
/// El secreto es un solo bloque cifrado: si está dañado, lo están la
/// contraseña y las notas, y la pantalla ofrece reparar (escribirlas de
/// nuevo) o eliminar.
class EntryDetailScreen extends StatefulWidget {
  const EntryDetailScreen({
    super.key,
    required this.session,
    required this.entry,
    this.openForm,
  });

  final VaultSession session;

  /// El resumen que ya tiene quien la abre: la lista, o `entry(id)` tras
  /// crear.
  final EntrySummary entry;

  /// Cómo abrir el formulario. Por omisión, [EntryFormScreen] en el
  /// `Navigator` raíz; las pruebas lo reemplazan.
  final OpenEntryForm? openForm;

  /// Apila el formulario en el `Navigator` raíz, que el bloqueo vacía.
  static Future<bool?> _pushForm(
    BuildContext context,
    EntrySummary entry, {
    required bool repair,
    required VaultSession session,
  }) {
    return Navigator.of(context, rootNavigator: true).push<bool>(
      MaterialPageRoute(
        builder: (_) => EntryFormScreen.edit(
          session: session,
          entry: entry,
          repair: repair,
        ),
      ),
    );
  }

  @override
  State<EntryDetailScreen> createState() => _EntryDetailScreenState();
}

/// Estado de una parte del secreto en pantalla.
enum _Reveal { hidden, loading, shown, failed }

class _EntryDetailScreenState extends State<EntryDetailScreen> {
  late EntrySummary _entry = widget.entry;

  _Reveal _passwordState = _Reveal.hidden;
  String? _password;

  _Reveal _notesState = _Reveal.hidden;

  /// `null` con las notas visibles significa "Sin notas".
  String? _notes;

  /// Suben al lanzar un revelado, al ocultar y al abrir el formulario: una
  /// respuesta de una generación vieja se descarta.
  int _passwordGeneration = 0;
  int _notesGeneration = 0;

  /// El secreto no se pudo descifrar.
  bool _damaged = false;

  bool _deleting = false;

  /// La ruta de esta pantalla, para saber si sigue en el `Navigator`.
  ModalRoute<Object?>? _route;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _route = ModalRoute.of(context);
  }

  /// La ruta de esta pantalla sigue en el `Navigator`. Tras cada `await` se
  /// comprueba junto con `mounted`.
  ///
  /// `mounted` no basta tras un `await`: el bloqueo quita la ruta en el
  /// acto, pero el `State` sigue montado hasta el siguiente cuadro, y con la
  /// app oculta no hay cuadros. Una respuesta que llegara en esa ventana
  /// haría `pop` sobre la ruta de debajo (la raíz) o guardaría un secreto en
  /// una pantalla que ya salió.
  bool get _routeActive => _route?.isActive ?? true;

  @override
  void dispose() {
    _password = null;
    _notes = null;
    super.dispose();
  }

  /// Descarta lo revelado y las respuestas pendientes.
  void _discardSecrets() {
    _passwordGeneration++;
    _notesGeneration++;
    _password = null;
    _notes = null;
    _passwordState = _Reveal.hidden;
    _notesState = _Reveal.hidden;
  }

  Future<void> _revealPassword() async {
    final generation = ++_passwordGeneration;
    setState(() => _passwordState = _Reveal.loading);
    try {
      final password = await widget.session.revealPassword(id: _entry.id);
      if (!mounted || !_routeActive) return;
      if (generation != _passwordGeneration) return;
      setState(() {
        _password = password;
        _passwordState = _Reveal.shown;
      });
    } catch (e) {
      if (!mounted || !_routeActive) return;
      if (generation != _passwordGeneration) return;
      _onRevealError(e, () => _passwordState = _Reveal.failed);
    }
  }

  Future<void> _revealNotes() async {
    final generation = ++_notesGeneration;
    setState(() => _notesState = _Reveal.loading);
    try {
      final notes = await widget.session.revealNotes(id: _entry.id);
      if (!mounted || !_routeActive || generation != _notesGeneration) return;
      setState(() {
        _notes = notes;
        _notesState = _Reveal.shown;
      });
    } catch (e) {
      if (!mounted || !_routeActive || generation != _notesGeneration) return;
      _onRevealError(e, () => _notesState = _Reveal.failed);
    }
  }

  /// [markFailed] deja la fila que falló con el mensaje y el botón
  /// disponible; la otra fila no cambia.
  void _onRevealError(Object error, VoidCallback markFailed) {
    switch (error) {
      case VaultError.corruptEntry:
        setState(() {
          _discardSecrets();
          _damaged = true;
        });
      case VaultError.locked:
        // Carrera con el bloqueo: la pantalla ya está saliendo. Sin mensaje,
        // y se descarta también lo que estuviera visible: el núcleo ya no
        // tiene la llave.
        setState(_discardSecrets);
      case VaultError.entryNotFound:
        Navigator.of(context).pop();
      default:
        // Solo el tipo: nunca el mensaje.
        if (kDebugMode && error is! VaultError) {
          debugPrint('reveal: ${error.runtimeType}');
        }
        setState(markFailed);
    }
  }

  void _hidePassword() {
    setState(() {
      _passwordGeneration++;
      _password = null;
      _passwordState = _Reveal.hidden;
    });
  }

  void _hideNotes() {
    setState(() {
      _notesGeneration++;
      _notes = null;
      _notesState = _Reveal.hidden;
    });
  }

  Future<void> _openForm({required bool repair}) async {
    // La pantalla cubierta no retiene nada revelado.
    setState(_discardSecrets);
    final openForm = widget.openForm;
    final saved = openForm != null
        ? await openForm(context, _entry, repair: repair)
        : await EntryDetailScreen._pushForm(
            context,
            _entry,
            repair: repair,
            session: widget.session,
          );
    if (!mounted || !_routeActive || saved != true) return;
    // Tras guardar, el secreto ya no está dañado: lo reemplazó el formulario
    // o se pudo descifrar para conservarlo.
    setState(() => _damaged = false);
    try {
      final fresh = await widget.session.entry(id: _entry.id);
      if (!mounted || !_routeActive) return;
      setState(() => _entry = fresh);
    } on VaultError catch (e) {
      if (!mounted || !_routeActive) return;
      if (e == VaultError.entryNotFound) Navigator.of(context).pop();
      // Con otro error se conserva lo que se mostraba.
    } catch (e) {
      if (kDebugMode) debugPrint('entry: ${e.runtimeType}');
    }
  }

  Future<void> _confirmDelete() async {
    final confirmed = await showDialog<bool>(
      context: context,
      // En el `Navigator` raíz, que el bloqueo vacía.
      useRootNavigator: true,
      builder: (context) => AlertDialog(
        // Un servicio largo no debe dejar los botones fuera de la pantalla.
        scrollable: true,
        title: Text('¿Eliminar «${_entry.service}»?'),
        content: const Text('Esta acción no se puede deshacer.'),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: const Text('Cancelar'),
          ),
          TextButton(
            onPressed: () => Navigator.of(context).pop(true),
            child: const Text('Eliminar'),
          ),
        ],
      ),
    );
    if (confirmed != true || !mounted || !_routeActive) return;

    setState(() => _deleting = true);
    try {
      await widget.session.delete(id: _entry.id);
    } catch (e) {
      if (!mounted || !_routeActive) return;
      // Ya no estaba: para el usuario, quedó eliminada.
      if (e != VaultError.entryNotFound) {
        setState(() => _deleting = false);
        if (e == VaultError.locked) return;
        if (kDebugMode && e is! VaultError) {
          debugPrint('delete: ${e.runtimeType}');
        }
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(
            content: Text('No se pudo eliminar. Intenta de nuevo.'),
          ),
        );
        return;
      }
    }
    if (!mounted || !_routeActive) return;
    Navigator.of(context).pop();
  }

  @override
  Widget build(BuildContext context) {
    final canEdit = !_deleting;
    return Scaffold(
      appBar: AppBar(
        title: Text(_entry.service, overflow: TextOverflow.ellipsis),
        actions: [
          IconButton(
            tooltip: 'Editar',
            icon: const Icon(Icons.edit),
            onPressed: canEdit ? () => _openForm(repair: false) : null,
          ),
          IconButton(
            tooltip: 'Eliminar',
            icon: const Icon(Icons.delete),
            onPressed: _deleting ? null : _confirmDelete,
          ),
        ],
      ),
      body: SafeArea(
        child: ListView(
          keyboardDismissBehavior: ScrollViewKeyboardDismissBehavior.onDrag,
          padding: const EdgeInsets.all(16),
          children: [
            if (_entry.username case final username?)
              _Field(label: 'Usuario', child: Text(username)),
            if (_entry.url case final url?)
              _Field(label: 'URL', child: Text(url)),
            if (_entry.tags.isNotEmpty)
              _Field(
                label: 'Etiquetas',
                child: Wrap(
                  spacing: 8,
                  children: [
                    for (final tag in _entry.tags)
                      Chip(label: Text(tag, overflow: TextOverflow.ellipsis)),
                  ],
                ),
              ),
            if (_damaged) _damagedNotice(canEdit) else ..._secretRows(),
          ],
        ),
      ),
    );
  }

  Widget _damagedNotice(bool canEdit) {
    return Padding(
      padding: const EdgeInsets.only(top: 8),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          const Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Icon(Icons.warning_amber),
              SizedBox(width: 8),
              Expanded(
                child: Text(
                  'La contraseña y las notas de esta entrada están dañadas',
                ),
              ),
            ],
          ),
          const SizedBox(height: 8),
          Row(
            children: [
              FilledButton(
                onPressed: canEdit ? () => _openForm(repair: true) : null,
                child: const Text('Reparar'),
              ),
              const SizedBox(width: 8),
              TextButton(
                onPressed: _deleting ? null : _confirmDelete,
                child: const Text('Eliminar'),
              ),
            ],
          ),
        ],
      ),
    );
  }

  List<Widget> _secretRows() {
    final password = _password;
    final notes = _notes;
    return [
      _Field(
        label: 'Contraseña',
        action: _passwordState == _Reveal.shown
            ? TextButton(
                onPressed: _deleting ? null : _hidePassword,
                child: const Text('Ocultar'),
              )
            : TextButton(
                onPressed: _passwordState == _Reveal.loading || _deleting
                    ? null
                    : _revealPassword,
                child: const Text('Mostrar'),
              ),
        child: switch (_passwordState) {
          _Reveal.shown when password != null => Text(
            password,
            style: const TextStyle(fontFamily: 'monospace'),
          ),
          _Reveal.failed => const _RevealFailed(),
          // Ocho puntos fijos: no delatan el largo. El lector de pantalla
          // dice "Contraseña oculta" en lugar de ocho veces "punto".
          _ => Semantics(
            container: true,
            label: 'Contraseña oculta',
            child: const ExcludeSemantics(child: Text('••••••••')),
          ),
        },
      ),
      _Field(
        label: 'Notas',
        action: _notesState == _Reveal.shown
            ? TextButton(
                onPressed: _deleting ? null : _hideNotes,
                child: const Text('Ocultar notas'),
              )
            : TextButton(
                onPressed: _notesState == _Reveal.loading || _deleting
                    ? null
                    : _revealNotes,
                child: const Text('Mostrar notas'),
              ),
        child: switch (_notesState) {
          _Reveal.shown => Text(notes ?? 'Sin notas'),
          _Reveal.failed => const _RevealFailed(),
          _ => const SizedBox.shrink(),
        },
      ),
    ];
  }
}

/// Una fila del detalle: etiqueta, contenido y, a la derecha, su acción.
class _Field extends StatelessWidget {
  const _Field({required this.label, required this.child, this.action});

  final String label;
  final Widget child;
  final Widget? action;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 16),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(label, style: Theme.of(context).textTheme.labelMedium),
                const SizedBox(height: 4),
                child,
              ],
            ),
          ),
          ?action,
        ],
      ),
    );
  }
}

class _RevealFailed extends StatelessWidget {
  const _RevealFailed();

  @override
  Widget build(BuildContext context) {
    return Text(
      'No se pudo mostrar. Intenta de nuevo.',
      style: TextStyle(color: Theme.of(context).colorScheme.error),
    );
  }
}
