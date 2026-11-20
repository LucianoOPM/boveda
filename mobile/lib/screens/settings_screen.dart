import 'package:flutter/material.dart';

import '../services/auto_lock.dart';
import '../services/backup_files.dart';
import '../services/biometric_service.dart';
import '../src/rust/api/session.dart';
import '../theme/colors.dart';
import '../theme/icons.dart';
import '../theme/spacing.dart';
import 'restore_dialog.dart';

/// Avisa algo al usuario cuando la pantalla que lo origina ya no está (un
/// bloqueo la quitó): `VaultGate` lo muestra en la pantalla de desbloqueo.
typedef NoticeCallback = void Function(String text, {bool isError});

/// Ajustes mínimos de la etapa 2 (#44): exportar e importar el respaldo con
/// el selector de archivos del sistema. El tiempo de bloqueo y el cambio de
/// contraseña maestra son de la etapa 4.
///
/// Mientras un flujo corre, las dos acciones quedan desactivadas. El
/// selector se envuelve en `runWithoutAutoLock`: abrirlo oculta la app y, sin
/// eso, la bloquearía.
class SettingsScreen extends StatefulWidget {
  const SettingsScreen({
    super.key,
    required this.session,
    required this.files,
    required this.biometric,
    required this.onImported,
    required this.onImportFailed,
    required this.onNotice,
  });

  final VaultSession session;
  final BackupFiles files;
  final BiometricService biometric;

  /// La bóveda quedó reemplazada y desbloqueada, con el total de dañadas.
  final ValueChanged<int> onImported;

  /// La importación falló: `VaultGate` bloquea y muestra el motivo.
  final ValueChanged<String> onImportFailed;
  final NoticeCallback onNotice;

  @override
  State<SettingsScreen> createState() => _SettingsScreenState();
}

class _SettingsScreenState extends State<SettingsScreen> {
  bool _busy = false;

  /// Exporta a un temporal, lo guarda con el selector y lo borra siempre.
  ///
  /// El archivo es texto cifrado que el usuario eligió guardar: si el
  /// bloqueo ocurre mientras el selector está abierto, se escribe igual y el
  /// aviso llega a la pantalla de desbloqueo.
  Future<void> _export() async {
    if (_busy) return;
    final scope = AutoLockScope.maybeOf(context);
    setState(() => _busy = true);
    String? temp;
    String? notice;
    var isError = false;
    try {
      temp = await widget.files.newTempPath();
      await widget.session.exportBackup(dest: temp);
      if (!_visible) {
        // Un bloqueo quitó esta pantalla durante la exportación: no se abre
        // el selector sobre el desbloqueo. El temporal se borra en `finally`.
        notice =
            'La bóveda se bloqueó antes de guardar el respaldo. '
            'Vuelve a exportarlo.';
      } else {
        final path = temp;
        final name = backupFileName(DateTime.now());
        Future<bool> save() => widget.files.saveTo(path, name);
        final saved = await (scope == null
            ? save()
            : scope.runWithoutAutoLock(save));
        if (saved) notice = 'Respaldo guardado';
      }
    } catch (_) {
      // Sin detalles: el error puede traer rutas o datos internos.
      notice = 'No se pudo guardar el respaldo';
      isError = true;
    } finally {
      if (temp != null) {
        try {
          await widget.files.deleteTemp(temp);
        } catch (_) {}
      }
    }
    if (notice != null) _announce(notice, isError);
    if (mounted) setState(() => _busy = false);
  }

  /// Importa un respaldo (ver `runRestore`). Un error de la importación deja
  /// la bóveda bloqueada en Rust: la interfaz se bloquea con el motivo
  /// (#44, opción a; validar sin soltar la bóveda es la #69).
  Future<void> _import() async {
    if (_busy) return;
    setState(() => _busy = true);
    RestoreResult? result;
    try {
      result = await runRestore(
        context,
        session: widget.session,
        files: widget.files,
        biometric: widget.biometric,
        replacesVault: true,
        closeOnError: true,
      );
    } catch (_) {
      result = const RestorePickFailed('No se pudo importar el respaldo.');
    }
    if (mounted) setState(() => _busy = false);
    // Si un bloqueo quitó esta pantalla mientras importaba, la bóveda ya
    // está bloqueada: nunca se monta la principal sobre ella. Se avisa en el
    // desbloqueo, con la contraseña que vale ahora.
    final visible = _visible;
    switch (result) {
      case RestoreImported(:final damagedEntries) when visible:
        widget.onImported(damagedEntries);
      case RestoreImported():
        widget.onNotice(
          'Se importó el respaldo. Desbloquea con la contraseña del respaldo.',
          isError: false,
        );
      case RestoreImportFailed(:final message) when visible:
        widget.onImportFailed(message);
      case RestoreImportFailed(:final message):
        widget.onNotice(message, isError: true);
      case RestorePickFailed(:final message):
        _announce(message, true);
      case null:
        break;
    }
  }

  /// La pantalla sigue a la vista. `mounted` no basta: la ruta quitada con
  /// `removeRoute` sigue montada hasta el siguiente cuadro.
  bool get _visible => mounted && (ModalRoute.of(context)?.isActive ?? true);

  /// Con la pantalla a la vista, un `SnackBar`; si un bloqueo ya la quitó,
  /// el aviso va a `VaultGate`. `mounted` no basta: la ruta quitada con
  /// `removeRoute` sigue montada hasta el siguiente cuadro.
  void _announce(String text, bool isError) {
    if (_visible) {
      ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(text)));
    } else {
      widget.onNotice(text, isError: isError);
    }
  }

  /// Una fila de Ajustes (`docs/DESIGN.md`, 7.9): un `ListTile` con aspecto de
  /// tarjeta, el ícono de Lucide a la izquierda y una flecha decorativa a la
  /// derecha. Conserva su `Key`, sus textos, su `enabled` y su acción.
  Widget _tile({
    required Key key,
    required IconData icon,
    required String title,
    required String subtitle,
    required VoidCallback onTap,
  }) {
    final secondary = Theme.of(context).colorScheme.onSurfaceVariant;
    return ListTile(
      key: key,
      tileColor: BovedaColors.surface2,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(Radii.card),
        side: const BorderSide(color: BovedaColors.borderCard),
      ),
      leading: Icon(icon),
      title: Text(title),
      subtitle: Text(subtitle),
      trailing: Icon(BovedaIcons.chevronRight, color: secondary),
      enabled: !_busy,
      onTap: _busy ? null : onTap,
    );
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('Ajustes')),
      body: ListView(
        padding: const EdgeInsets.all(Space.lg),
        children: [
          _tile(
            key: const Key('settings-export'),
            icon: BovedaIcons.download,
            title: 'Exportar respaldo',
            subtitle: 'Guarda una copia cifrada de la bóveda',
            onTap: _export,
          ),
          const SizedBox(height: Space.s10),
          _tile(
            key: const Key('settings-import'),
            icon: BovedaIcons.refresh,
            title: 'Importar respaldo',
            subtitle: 'Reemplaza la bóveda actual',
            onTap: _import,
          ),
        ],
      ),
    );
  }
}
