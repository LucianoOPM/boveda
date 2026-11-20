import 'package:flutter/material.dart';

import '../services/auto_lock.dart';
import '../services/backup_files.dart';
import '../services/biometric_service.dart';
import '../src/rust/api/error.dart';
import '../src/rust/api/session.dart';
import '../theme/colors.dart';
import '../theme/extensions.dart';
import '../theme/icons.dart';
import '../theme/spacing.dart';
import '../widgets/password_field.dart';

/// Resultado del flujo de importar un respaldo (#44).
sealed class RestoreResult {
  const RestoreResult();
}

/// La bóveda quedó reemplazada y desbloqueada, con el total de entradas
/// dañadas (con y sin id) del `UnlockReport`.
class RestoreImported extends RestoreResult {
  const RestoreImported(this.damagedEntries);

  final int damagedEntries;
}

/// La importación falló: la fachada dejó la bóveda anterior bloqueada (o la
/// sesión sin bóveda). [message] es un texto fijo, sin datos del archivo.
class RestoreImportFailed extends RestoreResult {
  const RestoreImportFailed(this.message);

  final String message;
}

/// No se llegó a importar (el archivo no se pudo leer o era demasiado
/// grande): la bóveda sigue como estaba.
class RestorePickFailed extends RestoreResult {
  const RestorePickFailed(this.message);

  final String message;
}

const _genericImportError = 'No se pudo importar el respaldo.';

/// Texto fijo por error de la importación; nunca detalles del núcleo.
String restoreErrorMessage(Object error) {
  if (error is VaultError) {
    return switch (error) {
      VaultError.wrongPassword => 'Contraseña del respaldo incorrecta.',
      VaultError.notAVault => 'El archivo no es una bóveda.',
      VaultError.unsupportedFormatVersion =>
        'Este respaldo es de una versión que la app no admite.',
      VaultError.storageCorrupt => 'El archivo del respaldo está dañado.',
      _ => _genericImportError,
    };
  }
  return _genericImportError;
}

/// Flujo de importar un respaldo, compartido por Ajustes y el primer uso
/// (#44, detalle en `docs/superpowers/specs/2026-11-14-respaldos-selector-design.md`).
///
/// 1. Elige el archivo con el selector (sin bloqueo automático) y lo copia a
///    un temporal. `null` si el usuario cancela.
/// 2. Si al volver la pantalla ya no está (un bloqueo por el tope de 2
///    minutos la quitó), no toca la bóveda y devuelve `null`.
/// 3. Pide la contraseña **del respaldo** en [RestoreDialog] e importa.
/// 4. Borra el temporal siempre, **antes** de devolver el resultado: quien
///    lo reciba puede bloquear sin dejar nada atrás.
/// 5. Con éxito, desactiva la huella (borra la copia y la llave del
///    Keystore, que protegían la bóveda anterior).
///
/// [closeOnError]: en Ajustes, un error cierra el diálogo y devuelve
/// [RestoreImportFailed] (la bóveda ya quedó bloqueada en Rust); en el primer
/// uso no hay bóveda abierta y el error se muestra en el mismo diálogo.
Future<RestoreResult?> runRestore(
  BuildContext context, {
  required VaultSession session,
  required BackupFiles files,
  required BiometricService biometric,
  required bool replacesVault,
  required bool closeOnError,
}) async {
  final scope = AutoLockScope.maybeOf(context);
  final String? temp;
  try {
    temp = await (scope == null
        ? files.pickToTemp()
        : scope.runWithoutAutoLock(files.pickToTemp));
  } on BackupFilesException catch (error) {
    return RestorePickFailed(
      error.failure == BackupFailure.tooLarge
          ? 'El archivo es demasiado grande para ser un respaldo.'
          : 'No se pudo leer el archivo.',
    );
  } catch (_) {
    return const RestorePickFailed('No se pudo leer el archivo.');
  }
  if (temp == null) return null;

  RestoreResult? result;
  // La importación en vuelo, si el usuario tocó Importar. Si un bloqueo quita
  // el diálogo (su `showDialog` termina con `null`), la importación sigue
  // corriendo en Rust: hay que esperarla, o la bóveda se reemplazaría sin que
  // nadie lo supiera y el temporal se borraría con la copia en marcha.
  Future<RestoreResult>? importing;
  try {
    // `mounted` no basta: el bloqueo quita la ruta con `removeRoute` y el
    // `State` sigue montado hasta el siguiente cuadro.
    if (!context.mounted || !(ModalRoute.of(context)?.isActive ?? true)) {
      return null;
    }
    result = await showDialog<RestoreResult>(
      context: context,
      barrierDismissible: false,
      builder: (_) => RestoreDialog(
        session: session,
        backupPath: temp!,
        replacesVault: replacesVault,
        closeOnError: closeOnError,
        onImporting: (outcome) => importing = outcome,
      ),
    );
    if (result == null && importing != null) result = await importing;
  } finally {
    try {
      await files.deleteTemp(temp);
    } catch (_) {}
  }

  if (result is RestoreImported) {
    try {
      await biometric.disable();
    } catch (_) {}
  }
  return result;
}

/// Pide la contraseña del respaldo e importa. No se puede cerrar con la
/// importación en curso: cancelarla a medias dejaría el estado en duda.
class RestoreDialog extends StatefulWidget {
  const RestoreDialog({
    super.key,
    required this.session,
    required this.backupPath,
    required this.replacesVault,
    required this.closeOnError,
    this.onImporting,
  });

  final VaultSession session;
  final String backupPath;

  /// Recibe la importación en cuanto arranca, para que quien abrió el diálogo
  /// pueda esperarla aunque un bloqueo quite el diálogo antes de que termine.
  final ValueChanged<Future<RestoreResult>>? onImporting;

  /// Advierte que el respaldo reemplaza la bóveda actual (no en el primer
  /// uso, donde no hay nada que reemplazar).
  final bool replacesVault;

  /// Un error cierra el diálogo con [RestoreImportFailed] (Ajustes) o se
  /// muestra en el mismo diálogo y deja reintentar (primer uso).
  final bool closeOnError;

  @override
  State<RestoreDialog> createState() => _RestoreDialogState();
}

class _RestoreDialogState extends State<RestoreDialog> {
  final _password = TextEditingController();
  bool _busy = false;
  String? _error;

  @override
  void dispose() {
    _password
      ..clear()
      ..dispose();
    super.dispose();
  }

  Future<void> _submit() async {
    if (_busy || _password.text.isEmpty) return;
    setState(() {
      _busy = true;
      _error = null;
    });
    final outcome = _runImport(_password.text);
    widget.onImporting?.call(outcome);
    final result = await outcome;
    // Quitado por un bloqueo: quien abrió el diálogo ya espera `outcome`.
    if (!mounted) return;
    if (result is RestoreImportFailed && !widget.closeOnError) {
      setState(() {
        _busy = false;
        _error = result.message;
        _password.clear();
      });
      return;
    }
    Navigator.of(context).pop(result);
  }

  /// Importa y devuelve el resultado; nunca lanza ni usa el `State` después
  /// del primer `await`, así que sigue valiendo con el diálogo quitado.
  Future<RestoreResult> _runImport(String password) async {
    try {
      final report = await widget.session.importBackup(
        backup: widget.backupPath,
        password: password,
      );
      return RestoreImported(
        report.corrupt.length + report.unidentifiedCorrupt,
      );
    } catch (error) {
      return RestoreImportFailed(restoreErrorMessage(error));
    }
  }

  /// La advertencia de que el respaldo reemplaza la bóveda actual: una
  /// tarjeta `warningContainer` con ícono, como el aviso del primer uso.
  Widget _warning() {
    final theme = Theme.of(context);
    final status = theme.extension<StatusColors>() ?? StatusColors.standard;
    return Material(
      key: const Key('restore-warning'),
      color: BovedaColors.warningContainer,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(Radii.control),
        side: BorderSide(color: status.warning.withValues(alpha: 0.38)),
      ),
      child: Padding(
        padding: const EdgeInsets.all(Space.md),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Icon(
              BovedaIcons.triangleAlert,
              size: IconSizes.control,
              color: status.warning,
            ),
            const SizedBox(width: Space.sm),
            Expanded(
              child: Text(
                'Reemplaza la bóveda actual.',
                style: theme.textTheme.bodyMedium?.copyWith(
                  color: theme.colorScheme.onSurface,
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    return PopScope(
      canPop: !_busy,
      child: AlertDialog(
        // Con letra grande o el teclado abierto en una pantalla baja el
        // contenido se desplaza en vez de desbordar.
        scrollable: true,
        title: Text(
          widget.replacesVault ? 'Importar respaldo' : 'Restaurar respaldo',
        ),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            if (widget.replacesVault) ...[
              _warning(),
              const SizedBox(height: Space.lg),
            ],
            PasswordField(
              key: const Key('restore-password'),
              controller: _password,
              label: 'Contraseña del respaldo',
              enabled: !_busy,
              errorText: _error,
              onChanged: (_) => setState(() => _error = null),
              onSubmitted: (_) => _submit(),
            ),
          ],
        ),
        actions: [
          TextButton(
            onPressed: _busy ? null : () => Navigator.of(context).pop(),
            child: const Text('Cancelar'),
          ),
          TextFieldTapRegion(
            child: FilledButton(
              key: const Key('restore-submit'),
              onPressed: _busy || _password.text.isEmpty ? null : _submit,
              child: _busy
                  ? const SizedBox.square(
                      dimension: 16,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : const Text('Importar'),
            ),
          ),
        ],
      ),
    );
  }
}
