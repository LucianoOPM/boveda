import 'package:flutter/material.dart';

import '../services/backup_files.dart';
import '../services/biometric_service.dart';
import '../src/rust/api/error.dart';
import '../src/rust/api/password.dart';
import '../src/rust/api/session.dart';
import '../theme/colors.dart';
import '../theme/extensions.dart';
import '../theme/icons.dart';
import '../theme/spacing.dart';
import '../widgets/password_field.dart';
import '../widgets/primary_glow.dart';
import '../widgets/status_capsule.dart';
import '../widgets/strength_indicator.dart';
import 'restore_dialog.dart';

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
    required this.biometric,
    required this.files,
    required this.onCreated,
    required this.onAlreadyExists,
    required this.onRestored,
  });

  final VaultSession session;
  final StrengthOf strengthOf;

  /// Para borrar, antes de crear, la huella de una bóveda anterior.
  final BiometricService biometric;

  /// Se llama cuando la bóveda quedó creada y desbloqueada.
  final VoidCallback onCreated;

  /// Se llama si `create` encuentra que el archivo ya existe: quien muestra
  /// la pantalla vuelve a evaluar la bóveda en lugar de pedir reintentar.
  final VoidCallback onAlreadyExists;

  /// Archivos de respaldo y selector del sistema, para "Restaurar un
  /// respaldo" (#44).
  final BackupFiles files;

  /// Se llama cuando un respaldo quedó importado y desbloqueado, con el total
  /// de entradas dañadas.
  final ImportedCallback onRestored;

  @override
  State<FirstRunScreen> createState() => _FirstRunScreenState();
}

class _FirstRunScreenState extends State<FirstRunScreen> {
  final _password = TextEditingController();
  final _confirm = TextEditingController();
  bool _accepted = false;
  bool _creating = false;
  bool _restoring = false;
  String? _error;

  /// Fortaleza de la contraseña escrita; `null` con el campo vacío. Se
  /// calcula solo al editar la contraseña: cada cálculo cruza a Rust con
  /// una copia de la contraseña, así que `build` nunca llama a la fachada.
  PasswordStrength? _strength;

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

  bool get _matches => _password.text == _confirm.text;

  bool get _canCreate =>
      !_creating &&
      !_restoring &&
      _accepted &&
      _strength != null &&
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
    // Mejor esfuerzo: que no sobreviva la huella de una bóveda anterior. Si
    // falla, se crea igual: una copia vieja no abre la bóveda nueva
    // (`key_check` la rechaza con `WrongVaultKey` y eso la desactiva), y un
    // Keystore averiado no debe impedir crear la bóveda.
    try {
      await widget.biometric.disable();
    } catch (_) {}
    try {
      await widget.session.create(password: password);
    } on VaultError catch (error) {
      if (error == VaultError.alreadyExists) {
        // El archivo apareció después de comprobarlo: reintentar fallaría
        // siempre. Quien muestra la pantalla vuelve a evaluar la bóveda.
        if (mounted) widget.onAlreadyExists();
        return;
      }
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

  /// Restaura un respaldo en un celular nuevo o un emulador limpio (#44). No
  /// hay bóveda que reemplazar ni abierta: un error de la importación se
  /// muestra en el mismo diálogo y no bloquea nada.
  Future<void> _restore() async {
    if (_creating || _restoring) return;
    setState(() {
      _restoring = true;
      _error = null;
    });
    RestoreResult? result;
    try {
      result = await runRestore(
        context,
        session: widget.session,
        files: widget.files,
        biometric: widget.biometric,
        replacesVault: false,
        closeOnError: false,
      );
    } catch (_) {
      result = const RestorePickFailed('No se pudo importar el respaldo.');
    }
    if (!mounted) return;
    setState(() => _restoring = false);
    switch (result) {
      case RestoreImported(:final damagedEntries, :final reenableBiometric):
        widget.onRestored(damagedEntries, reenableBiometric: reenableBiometric);
      case RestorePickFailed(:final message):
        setState(() => _error = message);
      case RestoreImportFailed() || null:
        break;
    }
  }

  void _fail(String message) {
    if (!mounted) return;
    setState(() {
      _creating = false;
      _error = message;
    });
  }

  void _onPasswordChanged(String text) {
    setState(() {
      _strength = text.isEmpty ? null : widget.strengthOf(text);
      _error = null;
    });
  }

  void _onConfirmChanged(String _) => setState(() => _error = null);

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final status = theme.extension<StatusColors>() ?? StatusColors.standard;
    final showMismatch = _confirm.text.isNotEmpty && !_matches;
    return PopScope(
      canPop: !_creating,
      child: Scaffold(
        body: SafeArea(
          child: SingleChildScrollView(
            padding: const EdgeInsets.all(Space.lg),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                const Align(
                  alignment: Alignment.centerLeft,
                  child: StatusCapsule(
                    text: 'CONOCIMIENTO CERO',
                    semanticsLabel: 'Conocimiento cero',
                  ),
                ),
                const SizedBox(height: Space.lg),
                Text(
                  'Crea tu contraseña maestra',
                  style: theme.textTheme.headlineSmall,
                ),
                const SizedBox(height: Space.sm),
                Text(
                  'Usa una frase de varias palabras. Mínimo 12 caracteres.',
                  style: theme.textTheme.bodyMedium?.copyWith(
                    color: theme.colorScheme.onSurfaceVariant,
                  ),
                ),
                const SizedBox(height: Space.lg),
                PasswordField(
                  key: const Key('first-run-password'),
                  controller: _password,
                  label: 'Contraseña maestra',
                  enabled: !_creating,
                  onChanged: _onPasswordChanged,
                ),
                if (_strength case final strength?) ...[
                  const SizedBox(height: Space.sm),
                  StrengthIndicator(strength: strength),
                ],
                const SizedBox(height: Space.lg),
                PasswordField(
                  key: const Key('first-run-confirm'),
                  controller: _confirm,
                  label: 'Confirma la contraseña',
                  enabled: !_creating,
                  errorText: showMismatch
                      ? 'Las contraseñas no coinciden'
                      : null,
                  onChanged: _onConfirmChanged,
                ),
                const SizedBox(height: Space.lg),
                // La advertencia del diseño, con la casilla que el código
                // exige dentro.
                // `Material` y no un `Container` con color: la casilla es un
                // `ListTile`, que pinta su fondo y su efecto en el `Material`
                // más cercano y un fondo intermedio los taparía.
                Material(
                  key: const Key('first-run-warning'),
                  color: BovedaColors.warningContainer,
                  shape: RoundedRectangleBorder(
                    borderRadius: BorderRadius.circular(Radii.card),
                    side: BorderSide(
                      color: status.warning.withValues(alpha: 0.38),
                    ),
                  ),
                  child: Padding(
                    padding: const EdgeInsets.all(Space.md),
                    child: Row(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Padding(
                          padding: const EdgeInsets.only(top: Space.sm),
                          child: Icon(
                            BovedaIcons.triangleAlert,
                            size: 20,
                            color: status.warning,
                          ),
                        ),
                        const SizedBox(width: Space.sm),
                        Expanded(
                          // En el grupo del campo: tocarlo no cierra el teclado.
                          child: TextFieldTapRegion(
                            child: CheckboxListTile(
                              key: const Key('first-run-accept'),
                              value: _accepted,
                              onChanged: _creating
                                  ? null
                                  : (value) => setState(
                                      () => _accepted = value ?? false,
                                    ),
                              controlAffinity: ListTileControlAffinity.leading,
                              contentPadding: EdgeInsets.zero,
                              title: Text(
                                'Entiendo que si olvido la contraseña maestra '
                                'pierdo el acceso a la bóveda. No se puede '
                                'recuperar.',
                                style: theme.textTheme.bodyMedium?.copyWith(
                                  color: theme.colorScheme.onSurface,
                                ),
                              ),
                            ),
                          ),
                        ),
                      ],
                    ),
                  ),
                ),
                if (_error case final error?) ...[
                  const SizedBox(height: Space.sm),
                  Row(
                    key: const Key('first-run-error'),
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Icon(
                        BovedaIcons.triangleAlert,
                        size: 16,
                        color: theme.colorScheme.error,
                      ),
                      const SizedBox(width: Space.sm),
                      Expanded(
                        child: Text(
                          error,
                          style: theme.textTheme.bodyMedium?.copyWith(
                            color: theme.colorScheme.error,
                          ),
                        ),
                      ),
                    ],
                  ),
                ],
                const SizedBox(height: Space.lg),
                // En el grupo del campo: tocarlo no cierra el teclado.
                TextFieldTapRegion(
                  child: PrimaryGlow(
                    enabled: _canCreate,
                    child: FilledButton(
                      key: const Key('first-run-create'),
                      onPressed: _canCreate ? _create : null,
                      child: _creating
                          ? const Row(
                              mainAxisSize: MainAxisSize.min,
                              children: [
                                SizedBox.square(
                                  dimension: 16,
                                  child: CircularProgressIndicator(
                                    strokeWidth: 2,
                                  ),
                                ),
                                SizedBox(width: Space.md),
                                // `Flexible`: con texto grande se parte en dos
                                // líneas en vez de desbordar.
                                Flexible(child: Text('Creando la bóveda…')),
                              ],
                            )
                          : const Text('Crear bóveda'),
                    ),
                  ),
                ),
                const SizedBox(height: Space.sm),
                TextButton(
                  key: const Key('first-run-restore'),
                  onPressed: _creating || _restoring ? null : _restore,
                  child: const Text('Restaurar un respaldo'),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
