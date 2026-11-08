import 'package:flutter/material.dart';

import '../src/rust/api/error.dart';
import '../src/rust/api/session.dart';
import '../widgets/password_field.dart';

/// Desbloqueo con la contraseña maestra.
///
/// La contraseña solo vive en el controlador de esta pantalla y en la
/// variable local con la que se llama a `unlock`. El campo se limpia tras
/// cada intento fallido y al salir de la pantalla (de mejor esfuerzo: Dart no
/// permite borrar sus cadenas).
class UnlockScreen extends StatefulWidget {
  const UnlockScreen({
    super.key,
    required this.session,
    required this.onUnlocked,
  });

  final VaultSession session;

  /// Se llama con la bóveda desbloqueada y el total de entradas dañadas
  /// (con y sin id) del `UnlockReport`.
  final ValueChanged<int> onUnlocked;

  @override
  State<UnlockScreen> createState() => _UnlockScreenState();
}

class _UnlockScreenState extends State<UnlockScreen> {
  final _password = TextEditingController();
  final _focus = FocusNode();
  bool _unlocking = false;
  String? _error;

  @override
  void dispose() {
    _password
      ..clear()
      ..dispose();
    _focus.dispose();
    super.dispose();
  }

  bool get _canUnlock => !_unlocking && _password.text.isNotEmpty;

  Future<void> _unlock() async {
    // Cubre el botón y la tecla "Hecho": nunca corren dos `unlock` a la vez.
    if (!_canUnlock) return;
    setState(() {
      _unlocking = true;
      _error = null;
    });
    final password = _password.text;
    final UnlockReport report;
    try {
      report = await widget.session.unlock(password: password);
    } on VaultError catch (error) {
      if (!mounted) return;
      if (error == VaultError.alreadyUnlocked) {
        // La bóveda ya está desbloqueada: reintentar fallaría siempre.
        widget.onUnlocked(0);
        return;
      }
      _fail(
        error == VaultError.wrongPassword
            ? 'Contraseña incorrecta'
            : 'No se pudo desbloquear la bóveda. Intenta de nuevo.',
      );
      return;
    } catch (_) {
      // Sin detalles: el error puede traer rutas o datos internos.
      if (!mounted) return;
      _fail('No se pudo desbloquear la bóveda. Intenta de nuevo.');
      return;
    }
    if (!mounted) return;
    widget.onUnlocked(report.corrupt.length + report.unidentifiedCorrupt);
  }

  /// Limpia el campo, lo reactiva con el mensaje y le devuelve el foco. El
  /// campo desactivado no acepta el foco, así que se pide después de
  /// reconstruirlo.
  void _fail(String message) {
    setState(() {
      _password.clear();
      _unlocking = false;
      _error = message;
    });
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) _focus.requestFocus();
    });
  }

  void _onChanged(String _) => setState(() => _error = null);

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Scaffold(
      body: SafeArea(
        child: SingleChildScrollView(
          padding: const EdgeInsets.all(24),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Text('Bóveda bloqueada', style: theme.textTheme.headlineSmall),
              const SizedBox(height: 24),
              PasswordField(
                key: const Key('unlock-password'),
                controller: _password,
                label: 'Contraseña maestra',
                enabled: !_unlocking,
                autofocus: true,
                focusNode: _focus,
                textInputAction: TextInputAction.done,
                onChanged: _onChanged,
                onSubmitted: (_) => _unlock(),
              ),
              if (_error case final error?) ...[
                const SizedBox(height: 8),
                Text(error, style: TextStyle(color: theme.colorScheme.error)),
              ],
              const SizedBox(height: 24),
              FilledButton(
                key: const Key('unlock-submit'),
                onPressed: _canUnlock ? _unlock : null,
                child: _unlocking
                    ? const Row(
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          SizedBox.square(
                            dimension: 16,
                            child: CircularProgressIndicator(strokeWidth: 2),
                          ),
                          SizedBox(width: 12),
                          Text('Desbloqueando…'),
                        ],
                      )
                    : const Text('Desbloquear'),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
