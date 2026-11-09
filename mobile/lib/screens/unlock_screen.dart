import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/scheduler.dart';

import '../services/biometric_service.dart';
import '../services/biometric_unlock.dart';
import '../src/rust/api/error.dart';
import '../src/rust/api/session.dart';
import '../widgets/password_field.dart';

/// Desbloqueo con la contraseña maestra o con huella.
///
/// La contraseña solo vive en el controlador de esta pantalla y en la
/// variable local con la que se llama a `unlock`. El campo se limpia tras
/// cada intento fallido y al salir de la pantalla (de mejor esfuerzo: Dart no
/// permite borrar sus cadenas).
///
/// Con la huella activa, la pantalla la pide una sola vez al abrirse,
/// después del primer cuadro y con la app en `resumed` (si no, espera a que
/// vuelva: no depende de lo que haga cada fabricante con un diálogo pedido
/// en segundo plano, #39); para reintentar está "Usar huella". Si el diálogo
/// se cancela porque la app se ocultó (Kotlin lo cancela en `onStop`), la
/// huella se vuelve a pedir una vez al volver; si la cancela el usuario, se
/// queda en la contraseña. Android 16 cierra el diálogo por su cuenta al
/// tocar inicio, con el mismo código que el usuario y antes de ocultar la
/// app: una cancelación con la app en `inactive` cuenta como del sistema si
/// `hidden` llega dentro de `_systemCancelWindow`. El campo
/// no se enfoca al abrir (Android no abre el teclado durante la animación de
/// arranque, ver el diseño de la #37): se enfoca si la huella no se usa o
/// falla. Sin huella activa, y si el dispositivo la admite, la casilla
/// "Usar huella para desbloquear" la activa al desbloquear con la
/// contraseña.
class UnlockScreen extends StatefulWidget {
  const UnlockScreen({
    super.key,
    required this.session,
    required this.biometric,
    required this.onUnlocked,
    this.onVaultOpened,
  });

  final VaultSession session;
  final BiometricService biometric;

  /// Se llama cuando `unlock` tuvo éxito y la pantalla va a activar la huella
  /// con la casilla: desde aquí Rust está desbloqueado aunque la interfaz
  /// siga en el desbloqueo, y ocultar la app debe bloquear (#39).
  final VoidCallback? onVaultOpened;

  /// Se llama con la bóveda desbloqueada y el total de entradas dañadas
  /// (con y sin id) del `UnlockReport`.
  final ValueChanged<int> onUnlocked;

  @override
  State<UnlockScreen> createState() => _UnlockScreenState();
}

class _UnlockScreenState extends State<UnlockScreen> {
  /// Cuánto se espera `hidden` después de una cancelación con la app en
  /// `inactive`. Android 16 cierra el diálogo al tocar inicio con el mismo
  /// código que una cancelación del usuario, antes de `onPause`, y `hidden`
  /// llega unos 0.5 s después (#39, hallazgo de la prueba a mano).
  static const _systemCancelWindow = Duration(seconds: 1);

  static const _generic = 'No se pudo desbloquear la bóveda. Intenta de nuevo.';

  final _password = TextEditingController();
  final _focus = FocusNode();

  /// Un desbloqueo en curso, con huella o con contraseña: nunca corren dos.
  bool _busy = false;

  /// Activando la huella después de desbloquear con la contraseña.
  bool _activating = false;

  String? _error;

  /// Aviso de huella desactivada.
  String? _notice;

  /// La huella está activa: se muestra "Usar huella".
  bool _biometricEnabled = false;

  /// El dispositivo admite la huella y no está activa: se muestra la casilla.
  bool _canOfferBiometric = false;

  /// Casilla "Usar huella para desbloquear".
  bool _enableBiometric = false;

  late final AppLifecycleListener _lifecycle;

  /// El intento automático espera a que la app vuelva a `resumed`.
  bool _waitingResume = false;

  /// El diálogo de la huella está abierto (o la llave va hacia Rust).
  bool _promptOpen = false;

  /// La app se ocultó con el diálogo abierto: su cancelación no fue del
  /// usuario.
  bool _hiddenDuringPrompt = false;

  /// Pedir la huella otra vez en el siguiente `resumed`.
  bool _retryOnResume = false;

  /// Una cancelación llegó con la app en `inactive`: si `hidden` llega antes
  /// de que venza, la canceló el sistema al ocultarse; si no, el usuario.
  Timer? _cancelWindow;

  @override
  void initState() {
    super.initState();
    _lifecycle = AppLifecycleListener(onHide: _onHide, onResume: _onResume);
    // Después del primer cuadro, para que el diálogo no choque con la
    // construcción de la pantalla. Corre una sola vez por instancia.
    WidgetsBinding.instance.addPostFrameCallback((_) => _start());
  }

  @override
  void dispose() {
    _cancelWindow?.cancel();
    _lifecycle.dispose();
    _password
      ..clear()
      ..dispose();
    _focus.dispose();
    super.dispose();
  }

  bool get _canUnlock => !_busy && _password.text.isNotEmpty;

  bool get _isResumed {
    final state = SchedulerBinding.instance.lifecycleState;
    return state == null || state == AppLifecycleState.resumed;
  }

  void _onHide() {
    if (_promptOpen) _hiddenDuringPrompt = true;
    if (_cancelWindow case final window?) {
      window.cancel();
      _cancelWindow = null;
      _retryOnResume = true;
    }
  }

  void _onResume() {
    if (_waitingResume) {
      _waitingResume = false;
      unawaited(_start());
    } else if (_retryOnResume) {
      _retryOnResume = false;
      unawaited(_unlockWithBiometric());
    }
  }

  Future<void> _start() async {
    if (!_isResumed) {
      _waitingResume = true;
      return;
    }
    final enabled = await _isEnabled();
    if (!mounted) return;
    setState(() => _biometricEnabled = enabled);
    if (enabled) {
      await _unlockWithBiometric();
    } else {
      await _refreshOffer();
    }
  }

  Future<bool> _isEnabled() async {
    try {
      return await widget.biometric.isEnabled();
    } catch (_) {
      return false;
    }
  }

  /// Vuelve a evaluar si se ofrece la casilla.
  Future<void> _refreshOffer() async {
    bool offer;
    try {
      offer =
          await widget.biometric.isAvailable() &&
          !await widget.biometric.isEnabled();
    } catch (_) {
      offer = false;
    }
    if (!mounted) return;
    setState(() => _canOfferBiometric = offer);
  }

  Future<void> _unlockWithBiometric() async {
    if (_busy) return;
    setState(() {
      _busy = true;
      _error = null;
    });
    _hiddenDuringPrompt = false;
    _promptOpen = true;
    final outcome = await unlockWithBiometric(widget.session, widget.biometric);
    _promptOpen = false;
    if (!mounted) return;
    switch (outcome) {
      case BiometricUnlocked(:final damagedEntries):
        widget.onUnlocked(damagedEntries);
      case BiometricFallback() when _hiddenDuringPrompt:
        // La canceló el sistema al ocultarse la app, no el usuario: se pide
        // otra vez al volver, sin enfocar el campo.
        _hiddenDuringPrompt = false;
        setState(() => _busy = false);
        if (_isResumed) {
          unawaited(_unlockWithBiometric());
        } else {
          _retryOnResume = true;
        }
      case BiometricFallback() when !_isResumed:
        // El diálogo tenía el foco: no se sabe todavía quién canceló.
        setState(() => _busy = false);
        _cancelWindow = Timer(_systemCancelWindow, () {
          _cancelWindow = null;
          if (mounted) _release();
        });
      case BiometricFallback():
        _release();
      case BiometricDisabled():
        setState(() {
          _biometricEnabled = false;
          _notice =
              'La huella se desactivó. Desbloquea con tu contraseña maestra.';
        });
        _release();
        await _refreshOffer();
      case BiometricCoreError():
        setState(() => _error = _generic);
        _release();
    }
  }

  /// Reactiva el formulario y le da el foco al campo. El campo desactivado
  /// no acepta el foco, así que se pide después de reconstruirlo.
  void _release() {
    setState(() => _busy = false);
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) _focus.requestFocus();
    });
  }

  Future<void> _unlock() async {
    // Cubre el botón y la tecla "Hecho": nunca corren dos `unlock` a la vez.
    if (!_canUnlock) return;
    // Antes del primer `await`: el `ScaffoldMessenger` raíz sobrevive al
    // cambio de pantalla, así que el aviso de activación fallida se ve en
    // la pantalla principal sin volver a leer el `context`.
    final messenger = ScaffoldMessenger.of(context);
    setState(() {
      _busy = true;
      _error = null;
    });
    final password = _password.text;
    final int damaged;
    try {
      final report = await widget.session.unlock(password: password);
      damaged = report.corrupt.length + report.unidentifiedCorrupt;
    } on VaultError catch (error) {
      if (!mounted) return;
      if (error == VaultError.alreadyUnlocked) {
        // La bóveda ya está desbloqueada: reintentar fallaría siempre.
        widget.onUnlocked(0);
        return;
      }
      _fail(
        error == VaultError.wrongPassword ? 'Contraseña incorrecta' : _generic,
      );
      return;
    } catch (_) {
      // Sin detalles: el error puede traer rutas o datos internos.
      if (!mounted) return;
      _fail(_generic);
      return;
    }
    if (!mounted) return;
    if (_enableBiometric && _canOfferBiometric) {
      widget.onVaultOpened?.call();
      setState(() => _activating = true);
      final outcome = await enableBiometric(widget.session, widget.biometric);
      // Si el bloqueo sacó la pantalla del árbol, la activación terminó en
      // `failed` por `Locked`: el aviso no va sobre el desbloqueo nuevo.
      if (outcome == EnableOutcome.failed && mounted) {
        messenger.showSnackBar(
          const SnackBar(content: Text('No se pudo activar la huella.')),
        );
      }
      if (!mounted) return;
    }
    widget.onUnlocked(damaged);
  }

  /// Limpia el campo, lo reactiva con el mensaje y le devuelve el foco.
  void _fail(String message) {
    setState(() {
      _password.clear();
      _error = message;
    });
    _release();
  }

  void _onChanged(String _) => setState(() => _error = null);

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final errorStyle = TextStyle(color: theme.colorScheme.error);
    return Scaffold(
      body: SafeArea(
        child: SingleChildScrollView(
          padding: const EdgeInsets.all(24),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Text('Bóveda bloqueada', style: theme.textTheme.headlineSmall),
              if (_notice case final notice?) ...[
                const SizedBox(height: 16),
                Text(notice, style: errorStyle),
              ],
              const SizedBox(height: 24),
              PasswordField(
                key: const Key('unlock-password'),
                controller: _password,
                label: 'Contraseña maestra',
                enabled: !_busy,
                focusNode: _focus,
                textInputAction: TextInputAction.done,
                onChanged: _onChanged,
                onSubmitted: (_) => _unlock(),
              ),
              if (_error case final error?) ...[
                const SizedBox(height: 8),
                Text(error, style: errorStyle),
              ],
              if (_canOfferBiometric) ...[
                const SizedBox(height: 8),
                CheckboxListTile(
                  key: const Key('unlock-enable-biometric'),
                  value: _enableBiometric,
                  onChanged: _busy
                      ? null
                      : (value) =>
                            setState(() => _enableBiometric = value ?? false),
                  controlAffinity: ListTileControlAffinity.leading,
                  contentPadding: EdgeInsets.zero,
                  title: const Text('Usar huella para desbloquear'),
                ),
              ],
              const SizedBox(height: 24),
              FilledButton(
                key: const Key('unlock-submit'),
                onPressed: _canUnlock ? _unlock : null,
                child: _busy
                    ? Row(
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          const SizedBox.square(
                            dimension: 16,
                            child: CircularProgressIndicator(strokeWidth: 2),
                          ),
                          const SizedBox(width: 12),
                          Text(
                            _activating
                                ? 'Activando huella…'
                                : 'Desbloqueando…',
                          ),
                        ],
                      )
                    : const Text('Desbloquear'),
              ),
              if (_biometricEnabled) ...[
                const SizedBox(height: 8),
                OutlinedButton(
                  key: const Key('unlock-biometric'),
                  onPressed: _busy ? null : _unlockWithBiometric,
                  child: const Text('Usar huella'),
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }
}
