import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';

import '../src/rust/api/entries.dart';
import '../src/rust/api/error.dart';
import '../src/rust/api/session.dart';
import '../widgets/activity_text_field.dart';
import '../widgets/password_field.dart';
import '../widgets/tags_field.dart';

/// Campo del formulario al que corresponde un error de validación.
enum _FormField { service, username, url, tags, password, notes }

/// Formulario de una entrada (#41, diseño en
/// `docs/superpowers/specs/2026-11-10-entradas-design.md`), en tres modos:
///
/// - **Crear:** todos los campos vacíos. Al guardar entrega el resumen de
///   la entrada nueva a [onCreated] y no navega: lo decide quien lo abrió.
/// - **Editar:** los datos de búsqueda llegan llenos desde el resumen, sin
///   llamar a Rust, y el secreto empieza cerrado. "Cambiar" abre un campo
///   vacío para la contraseña nueva, sin revelar la actual; "Editar notas"
///   las carga (cuenta como uso de la entrada). Una sección abierta se envía
///   siempre y una cerrada nunca, así que la parte que no se toca no sale
///   del núcleo. Al guardar, sale con `true`.
/// - **Reparar:** el secreto está dañado y hay que escribirlo de nuevo. Se
///   entra con `repair` o desde el modo editar, cuando guardar o cargar las
///   notas da `corruptEntry`; lo ya escrito se conserva.
///
/// Dart no valida, no recorta y no conoce los límites: envía los textos tal
/// cual y muestra en su campo el error que devuelve el núcleo. Tampoco hay
/// regla de fortaleza: esa es solo de la contraseña maestra.
///
/// La contraseña y las notas viven solo en sus controladores, que se vacían
/// al salir. Salir con cambios pregunta antes; el bloqueo automático quita
/// la ruta sin preguntar y los cambios se pierden. Es intencional: seguridad
/// antes que comodidad.
class EntryFormScreen extends StatefulWidget {
  const EntryFormScreen.create({
    super.key,
    required this.session,
    required ValueChanged<EntrySummary> this.onCreated,
  }) : entry = null,
       repair = false;

  const EntryFormScreen.edit({
    super.key,
    required this.session,
    required EntrySummary this.entry,
    this.repair = false,
  }) : onCreated = null;

  final VaultSession session;

  /// La entrada que se edita; `null` al crear.
  final EntrySummary? entry;

  /// Empezar en modo reparar.
  final bool repair;

  /// Recibe la entrada recién creada, tal como la guardó el núcleo.
  final ValueChanged<EntrySummary>? onCreated;

  @override
  State<EntryFormScreen> createState() => _EntryFormScreenState();
}

class _EntryFormScreenState extends State<EntryFormScreen> {
  late final _service = TextEditingController(text: widget.entry?.service);
  late final _username = TextEditingController(text: widget.entry?.username);
  late final _url = TextEditingController(text: widget.entry?.url);
  final _pendingTag = TextEditingController();
  final _password = TextEditingController();
  final _notes = TextEditingController();

  final _focus = {for (final field in _FormField.values) field: FocusNode()};

  late List<String> _tags = [...?widget.entry?.tags];

  late bool _repair = widget.repair;

  /// Secciones del secreto abiertas. Al crear y al reparar, siempre.
  bool _passwordOpen = false;
  bool _notesOpen = false;

  bool _loadingNotes = false;
  bool _notesLoadFailed = false;
  bool _saving = false;

  /// Error de validación del núcleo, para mostrar en su campo.
  VaultError? _fieldError;
  bool _generalError = false;

  bool get _creating => widget.entry == null;

  /// La sección no se puede cerrar: no hay un secreto que conservar.
  bool get _secretRequired => _creating || _repair;

  bool get _showPassword => _secretRequired || _passwordOpen;
  bool get _showNotes => _secretRequired || _notesOpen;

  bool get _busy => _saving || _loadingNotes;

  /// Hay algo escrito que se perdería al salir.
  bool get _dirty {
    final entry = widget.entry;
    return _service.text != (entry?.service ?? '') ||
        _username.text != (entry?.username ?? '') ||
        _url.text != (entry?.url ?? '') ||
        !listEquals(_tags, entry?.tags ?? const <String>[]) ||
        _pendingTag.text.isNotEmpty ||
        _password.text.isNotEmpty ||
        _notes.text.isNotEmpty ||
        _passwordOpen ||
        _notesOpen;
  }

  @override
  void initState() {
    super.initState();
    // El texto pendiente de etiquetas no pasa por `onChanged` de
    // `TagsField` (que avisa de los chips), pero también cuenta como cambio.
    _pendingTag.addListener(_pendingTagChanged);
  }

  void _pendingTagChanged() => _changed(_FormField.tags);

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
  /// haría `pop` sobre la ruta de debajo (la raíz) o llamaría a `onCreated`,
  /// que reemplazaría esa ruta por el detalle.
  bool get _routeActive => _route?.isActive ?? true;

  @override
  void dispose() {
    _pendingTag.removeListener(_pendingTagChanged);
    _service.dispose();
    _username.dispose();
    _url.dispose();
    _pendingTag.dispose();
    // Mejor esfuerzo: Dart no permite borrar sus cadenas.
    _password
      ..clear()
      ..dispose();
    _notes
      ..clear()
      ..dispose();
    for (final node in _focus.values) {
      node.dispose();
    }
    super.dispose();
  }

  /// Un campo cambió: se borra su error (y el mensaje general) y se vuelve a
  /// calcular si hay cambios.
  void _changed(_FormField field) {
    setState(() {
      if (_fieldOf(_fieldError) == field) _fieldError = null;
      _generalError = false;
    });
  }

  static _FormField? _fieldOf(VaultError? error) => switch (error) {
    VaultError.invalidEntryServiceEmpty ||
    VaultError.invalidEntryServiceTooLong => _FormField.service,
    VaultError.invalidEntryUsernameTooLong => _FormField.username,
    VaultError.invalidEntryUrlTooLong => _FormField.url,
    VaultError.invalidEntryTooManyTags ||
    VaultError.invalidEntryTagTooLong => _FormField.tags,
    VaultError.invalidEntryPasswordEmpty ||
    VaultError.invalidEntryPasswordTooLong => _FormField.password,
    VaultError.invalidEntryNotesTooLong => _FormField.notes,
    _ => null,
  };

  /// Sin números: los límites del núcleo son en bytes y holgados.
  static String _messageOf(VaultError error) => switch (error) {
    VaultError.invalidEntryServiceEmpty => 'El servicio es obligatorio',
    VaultError.invalidEntryPasswordEmpty => 'La contraseña es obligatoria',
    VaultError.invalidEntryTooManyTags => 'Demasiadas etiquetas',
    _ => 'Este campo es demasiado largo',
  };

  String? _errorFor(_FormField field) {
    final error = _fieldError;
    if (error == null || _fieldOf(error) != field) return null;
    return _messageOf(error);
  }

  /// Lleva el foco a [field] cuando el campo ya volvió a estar activo: un
  /// campo desactivado no lo acepta.
  ///
  /// También lo desplaza a la vista: si el campo quedó fuera de la pantalla,
  /// el usuario solo vería que el botón vuelve a "Guardar".
  void _focusAfterBuild(_FormField field) {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted || !_routeActive) return;
      final node = _focus[field]!..requestFocus();
      final fieldContext = node.context;
      if (fieldContext != null) {
        Scrollable.ensureVisible(
          fieldContext,
          alignment: 0.2,
          duration: const Duration(milliseconds: 200),
        );
      }
    });
  }

  void _enterRepair() {
    setState(() {
      _repair = true;
      _saving = false;
      _loadingNotes = false;
      _notesLoadFailed = false;
      _fieldError = null;
      _generalError = false;
    });
    _focusAfterBuild(_FormField.password);
  }

  void _closePassword() {
    setState(() {
      _password.clear();
      _passwordOpen = false;
      if (_fieldOf(_fieldError) == _FormField.password) _fieldError = null;
    });
  }

  void _closeNotes() {
    setState(() {
      _notes.clear();
      _notesOpen = false;
      if (_fieldOf(_fieldError) == _FormField.notes) _fieldError = null;
    });
  }

  Future<void> _loadNotes() async {
    setState(() {
      _loadingNotes = true;
      _notesLoadFailed = false;
    });
    try {
      final notes = await widget.session.revealNotes(id: widget.entry!.id);
      if (!mounted || !_routeActive) return;
      setState(() {
        _notes.text = notes ?? '';
        _notesOpen = true;
        _loadingNotes = false;
      });
    } catch (e) {
      if (!mounted || !_routeActive) return;
      if (e == VaultError.corruptEntry) {
        _enterRepair();
        return;
      }
      if (kDebugMode && e is! VaultError) {
        debugPrint('revealNotes: ${e.runtimeType}');
      }
      setState(() {
        _loadingNotes = false;
        // `locked` es la carrera con el bloqueo: silencio.
        _notesLoadFailed = e != VaultError.locked;
      });
    }
  }

  Future<void> _save() async {
    if (_busy) return;
    // El texto pendiente pasa a etiqueta, para no perderlo en silencio.
    final tags = TagsField.withPending(_tags, _pendingTag.text);
    _pendingTag.clear();
    setState(() {
      _tags = tags;
      _saving = true;
      _fieldError = null;
      _generalError = false;
    });

    // Tal cual: recortar y validar es del núcleo.
    final input = EntryInput(
      service: _service.text,
      url: _url.text,
      username: _username.text,
      tags: tags,
    );
    try {
      final entry = widget.entry;
      if (entry == null) {
        await _create(input);
      } else {
        await widget.session.edit(
          id: entry.id,
          input: input,
          newPassword: _showPassword ? _password.text : null,
          newNotes: _showNotes ? _notes.text : null,
        );
        if (!mounted || !_routeActive) return;
        Navigator.of(context).pop(true);
      }
    } catch (e) {
      if (!mounted || !_routeActive) return;
      _onSaveError(e);
    }
  }

  Future<void> _create(EntryInput input) async {
    final id = await widget.session.add(
      input: input,
      secret: NewSecret(password: _password.text, notes: _notes.text),
    );
    if (!mounted || !_routeActive) return;
    final EntrySummary created;
    try {
      created = await widget.session.entry(id: id);
    } catch (e) {
      // La entrada ya está guardada: sin su resumen, se vuelve a la lista.
      if (kDebugMode && e is! VaultError) debugPrint('entry: ${e.runtimeType}');
      if (mounted && _routeActive) Navigator.of(context).pop();
      return;
    }
    if (!mounted || !_routeActive) return;
    widget.onCreated!(created);
  }

  void _onSaveError(Object error) {
    if (error == VaultError.corruptEntry && !_creating) {
      _enterRepair();
      return;
    }
    final field = error is VaultError ? _fieldOf(error) : null;
    if (kDebugMode && error is! VaultError) {
      debugPrint('save: ${error.runtimeType}');
    }
    setState(() {
      _saving = false;
      if (field != null) {
        _fieldError = error as VaultError;
      } else {
        // `locked` es la carrera con el bloqueo: silencio.
        _generalError = error != VaultError.locked;
      }
    });
    if (field != null) _focusAfterBuild(field);
  }

  Future<void> _confirmDiscard() async {
    final discard = await showDialog<bool>(
      context: context,
      // En el `Navigator` raíz, que el bloqueo vacía.
      useRootNavigator: true,
      builder: (context) => AlertDialog(
        title: const Text('¿Descartar cambios?'),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: const Text('Seguir editando'),
          ),
          TextButton(
            onPressed: () => Navigator.of(context).pop(true),
            child: const Text('Descartar'),
          ),
        ],
      ),
    );
    if (discard == true && mounted && _routeActive) Navigator.of(context).pop();
  }

  @override
  Widget build(BuildContext context) {
    final enabled = !_saving;
    return PopScope<bool>(
      canPop: !_dirty && !_saving,
      onPopInvokedWithResult: (didPop, _) {
        // Mientras guarda, "atrás" no hace nada.
        if (didPop || _saving) return;
        _confirmDiscard();
      },
      child: Scaffold(
        appBar: AppBar(
          title: Text(
            _creating
                ? 'Nueva entrada'
                : _repair
                ? 'Reparar entrada'
                : 'Editar entrada',
          ),
        ),
        body: SafeArea(
          child: SingleChildScrollView(
            keyboardDismissBehavior: ScrollViewKeyboardDismissBehavior.onDrag,
            padding: const EdgeInsets.all(16),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                if (_repair) ...[
                  const Row(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Icon(Icons.warning_amber),
                      SizedBox(width: 8),
                      Expanded(
                        child: Text(
                          'La contraseña y las notas de esta entrada están '
                          'dañadas. Escribe una contraseña nueva para '
                          'repararla.',
                        ),
                      ),
                    ],
                  ),
                  const SizedBox(height: 16),
                ],
                _textField(
                  _FormField.service,
                  key: const Key('service'),
                  controller: _service,
                  label: 'Servicio',
                  textCapitalization: TextCapitalization.sentences,
                  enabled: enabled,
                ),
                const SizedBox(height: 16),
                _textField(
                  _FormField.username,
                  key: const Key('username'),
                  controller: _username,
                  label: 'Usuario',
                  keyboardType: TextInputType.emailAddress,
                  enabled: enabled,
                ),
                const SizedBox(height: 16),
                _textField(
                  _FormField.url,
                  key: const Key('url'),
                  controller: _url,
                  label: 'URL',
                  keyboardType: TextInputType.url,
                  enabled: enabled,
                ),
                const SizedBox(height: 16),
                TagsField(
                  tags: _tags,
                  controller: _pendingTag,
                  focusNode: _focus[_FormField.tags],
                  enabled: enabled,
                  errorText: _errorFor(_FormField.tags),
                  onChanged: (tags) {
                    _tags = tags;
                    _changed(_FormField.tags);
                  },
                ),
                const SizedBox(height: 16),
                _passwordSection(enabled),
                const SizedBox(height: 16),
                _notesSection(enabled),
                const SizedBox(height: 24),
                if (_generalError)
                  Padding(
                    padding: const EdgeInsets.only(bottom: 8),
                    child: Text(
                      'No se pudo guardar. Intenta de nuevo.',
                      style: TextStyle(
                        color: Theme.of(context).colorScheme.error,
                      ),
                    ),
                  ),
                TextFieldTapRegion(
                  child: FilledButton(
                    key: const Key('save'),
                    onPressed: _busy ? null : _save,
                    child: Text(_saving ? 'Guardando…' : 'Guardar'),
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }

  /// Un campo de texto que no es secreto: no corrige ni aprende lo escrito.
  /// `enableSuggestions` queda en `true` solo por el teclado: con `false`,
  /// en Android Flutter marca el campo como contraseña visible y Gboard
  /// muestra la fila de números (hallazgo de la #40). Lo que evita que el
  /// teclado aprenda es `enableIMEPersonalizedLearning: false`.
  Widget _textField(
    _FormField field, {
    required Key key,
    required TextEditingController controller,
    required String label,
    required bool enabled,
    TextInputType? keyboardType,
    TextCapitalization textCapitalization = TextCapitalization.none,
    TextInputAction? textInputAction = TextInputAction.next,
    int? minLines,
    int? maxLines = 1,
    bool copyable = true,
  }) {
    return ActivityTextField(
      key: key,
      copyable: copyable,
      controller: controller,
      focusNode: _focus[field],
      enabled: enabled,
      onChanged: (_) => _changed(field),
      textInputAction: textInputAction,
      keyboardType: keyboardType,
      textCapitalization: textCapitalization,
      autocorrect: false,
      enableIMEPersonalizedLearning: false,
      minLines: minLines,
      maxLines: maxLines,
      decoration: InputDecoration(
        labelText: label,
        errorText: _errorFor(field),
      ),
    );
  }

  Widget _passwordSection(bool enabled) {
    if (!_showPassword) {
      return _closedSection(
        'Contraseña: sin cambios',
        TextButton(
          onPressed: enabled
              ? () => setState(() => _passwordOpen = true)
              : null,
          child: const Text('Cambiar'),
        ),
      );
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        PasswordField(
          key: const Key('password'),
          controller: _password,
          focusNode: _focus[_FormField.password],
          label: _secretRequired ? 'Contraseña' : 'Contraseña nueva',
          enabled: enabled,
          errorText: _errorFor(_FormField.password),
          onChanged: (_) => _changed(_FormField.password),
          textInputAction: TextInputAction.next,
        ),
        if (!_secretRequired)
          _cancelButton(
            const Key('cancel-password'),
            enabled ? _closePassword : null,
          ),
      ],
    );
  }

  Widget _notesSection(bool enabled) {
    if (!_showNotes) {
      return Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          _closedSection(
            'Notas: sin cambios',
            TextButton(
              onPressed: enabled && !_loadingNotes ? _loadNotes : null,
              child: const Text('Editar notas'),
            ),
          ),
          if (_notesLoadFailed)
            Text(
              'No se pudieron cargar las notas. Intenta de nuevo.',
              style: TextStyle(color: Theme.of(context).colorScheme.error),
            ),
        ],
      );
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _textField(
          _FormField.notes,
          key: const Key('notes'),
          controller: _notes,
          label: 'Notas',
          enabled: enabled,
          keyboardType: TextInputType.multiline,
          textInputAction: TextInputAction.newline,
          minLines: 3,
          maxLines: 8,
          // Son parte del secreto: no se copian desde el formulario.
          copyable: false,
        ),
        if (!_secretRequired)
          _cancelButton(
            const Key('cancel-notes'),
            enabled ? _closeNotes : null,
          ),
      ],
    );
  }

  /// Una sección del secreto sin abrir: su estado y el botón que la abre,
  /// que no cierra el teclado.
  Widget _closedSection(String text, Widget button) {
    return Row(
      children: [
        Expanded(child: Text(text)),
        TextFieldTapRegion(child: button),
      ],
    );
  }

  Widget _cancelButton(Key key, VoidCallback? onPressed) {
    return Align(
      alignment: Alignment.centerRight,
      child: TextFieldTapRegion(
        child: TextButton(
          key: key,
          onPressed: onPressed,
          child: const Text('Cancelar'),
        ),
      ),
    );
  }
}
