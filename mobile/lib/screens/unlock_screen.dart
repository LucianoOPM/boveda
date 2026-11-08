import 'package:flutter/material.dart';

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
/// después del primer cuadro; para reintentar está "Usar huella". El campo
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
  });

  final VaultSession session;
  final BiometricService biometric;

  /// Se llama con la bóveda desbloqueada y el total de entradas dañadas
  /// (con y sin id) del `UnlockReport`.
  final ValueChanged<int> onUnlocked;

  @override
  State<UnlockScreen> createState() => _UnlockScreenState();
}

class _UnlockScreenState extends State<UnlockScreen> {
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

  @override
  void initState() {
    super.initState();
    // Después del primer cuadro, para que el diálogo no choque con la
    // construcción de la pantalla. Corre una sola vez por instancia.
    WidgetsBinding.instance.addPostFrameCallback((_) => _start());
  }

  @override
  void dispose() {
    _password
      ..clear()
      ..dispose();
    _focus.dispose();
    super.dispose();
  }

  bool get _canUnlock => !_busy && _password.text.isNotEmpty;

  Future<void> _start() async {
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
    final outcome = await unlockWithBiometric(widget.session, widget.biometric);
    if (!mounted) return;
    switch (outcome) {
      case BiometricUnlocked(:final damagedEntries):
        widget.onUnlocked(damagedEntries);
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
      setState(() => _activating = true);
      final outcome = await enableBiometric(widget.session, widget.biometric);
      if (outcome == EnableOutcome.failed) {
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
