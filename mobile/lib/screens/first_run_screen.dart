import 'package:flutter/material.dart';

import '../src/rust/api/error.dart';
import '../src/rust/api/password.dart';
import '../src/rust/api/session.dart';
import '../widgets/password_field.dart';
import '../widgets/strength_indicator.dart';

/// Primer uso: crear la contraseña maestra y la bóveda.
///
/// La contraseña solo vive en los controladores de esta pantalla y en la
/// variable local con la que se llama a `create`; al salir de la pantalla
/// los controladores se limpian y se liberan (de mejor esfuerzo: Dart no
/// permite borrar sus cadenas).
class FirstRunScreen extends StatefulWidget {
  const FirstRunScreen({
    super.key,
    required this.session,
    required this.strengthOf,
    required this.onCreated,
  });

  final VaultSession session;
  final StrengthOf strengthOf;

  /// Se llama cuando la bóveda quedó creada y desbloqueada.
  final VoidCallback onCreated;

  @override
  State<FirstRunScreen> createState() => _FirstRunScreenState();
}

class _FirstRunScreenState extends State<FirstRunScreen> {
  final _password = TextEditingController();
  final _confirm = TextEditingController();
  bool _accepted = false;
  bool _creating = false;
  String? _error;

  @override
  void dispose() {
    _password
      ..clear()
      ..dispose();
    _confirm
      ..clear()
      ..dispose();
    super.dispose();
  }

  PasswordStrength get _strength => widget.strengthOf(_password.text);

  bool get _matches => _password.text == _confirm.text;

  bool get _canCreate =>
      !_creating &&
      _accepted &&
      _strength != PasswordStrength.tooShort &&
      _matches;

  Future<void> _create() async {
    // Un segundo toque antes de reconstruir la pantalla no crea dos veces.
    if (_creating) return;
    setState(() {
      _creating = true;
      _error = null;
    });
    final password = _password.text;
    try {
      await widget.session.create(password: password);
    } on VaultError catch (error) {
      _fail(
        error == VaultError.passwordTooShort
            ? 'La contraseña debe tener al menos 12 caracteres.'
            : 'No se pudo crear la bóveda. Intenta de nuevo.',
      );
      return;
    } catch (_) {
      _fail('No se pudo crear la bóveda. Intenta de nuevo.');
      return;
    }
    if (!mounted) return;
    widget.onCreated();
  }

  void _fail(String message) {
    if (!mounted) return;
    setState(() {
      _creating = false;
      _error = message;
    });
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final showMismatch = _confirm.text.isNotEmpty && !_matches;
    return PopScope(
      canPop: !_creating,
      child: Scaffold(
        body: SafeArea(
          child: SingleChildScrollView(
            padding: const EdgeInsets.all(24),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Text(
                  'Crea tu contraseña maestra',
                  style: theme.textTheme.headlineSmall,
                ),
                const SizedBox(height: 8),
                const Text(
                  'Usa una frase de varias palabras. Mínimo 12 caracteres.',
                ),
                const SizedBox(height: 24),
                PasswordField(
                  key: const Key('first-run-password'),
                  controller: _password,
                  label: 'Contraseña maestra',
                  enabled: !_creating,
                  onChanged: (_) => setState(() {}),
                ),
                if (_password.text.isNotEmpty) ...[
                  const SizedBox(height: 8),
                  StrengthIndicator(strength: _strength),
                ],
                const SizedBox(height: 16),
                PasswordField(
                  key: const Key('first-run-confirm'),
                  controller: _confirm,
                  label: 'Confirma la contraseña',
                  enabled: !_creating,
                  errorText: showMismatch
                      ? 'Las contraseñas no coinciden'
                      : null,
                  onChanged: (_) => setState(() {}),
                ),
                const SizedBox(height: 16),
                CheckboxListTile(
                  key: const Key('first-run-accept'),
                  value: _accepted,
                  onChanged: _creating
                      ? null
                      : (value) => setState(() => _accepted = value ?? false),
                  controlAffinity: ListTileControlAffinity.leading,
                  contentPadding: EdgeInsets.zero,
                  title: const Text(
                    'Entiendo que si olvido la contraseña maestra pierdo el '
                    'acceso a la bóveda. No se puede recuperar.',
                  ),
                ),
                if (_error case final error?) ...[
                  const SizedBox(height: 8),
                  Text(error, style: TextStyle(color: theme.colorScheme.error)),
                ],
                const SizedBox(height: 24),
                FilledButton(
                  key: const Key('first-run-create'),
                  onPressed: _canCreate ? _create : null,
                  child: _creating
                      ? const Row(
                          mainAxisSize: MainAxisSize.min,
                          children: [
                            SizedBox.square(
                              dimension: 16,
                              child: CircularProgressIndicator(strokeWidth: 2),
                            ),
                            SizedBox(width: 12),
                            Text('Creando la bóveda…'),
                          ],
                        )
                      : const Text('Crear bóveda'),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
