import 'package:flutter/material.dart';

import '../services/auto_lock.dart';
import '../services/backup_files.dart';
import '../src/rust/api/session.dart';

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
    required this.onNotice,
  });

  final VaultSession session;
  final BackupFiles files;
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
      final path = temp;
      final name = backupFileName(DateTime.now());
      Future<bool> save() => widget.files.saveTo(path, name);
      final saved = await (scope == null
          ? save()
          : scope.runWithoutAutoLock(save));
      if (saved) notice = 'Respaldo guardado';
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

  /// Con la pantalla a la vista, un `SnackBar`; si un bloqueo ya la quitó,
  /// el aviso va a `VaultGate`. `mounted` no basta: la ruta quitada con
  /// `removeRoute` sigue montada hasta el siguiente cuadro.
  void _announce(String text, bool isError) {
    final visible = mounted && (ModalRoute.of(context)?.isActive ?? true);
    if (visible) {
      ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(text)));
    } else {
      widget.onNotice(text, isError: isError);
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('Ajustes')),
      body: ListView(
        children: [
          ListTile(
            key: const Key('settings-export'),
            leading: const Icon(Icons.upload_file),
            title: const Text('Exportar respaldo'),
            subtitle: const Text('Guarda una copia cifrada de la bóveda'),
            enabled: !_busy,
            onTap: _busy ? null : _export,
          ),
          ListTile(
            key: const Key('settings-import'),
            leading: const Icon(Icons.download),
            title: const Text('Importar respaldo'),
            subtitle: const Text('Reemplaza la bóveda actual'),
            enabled: !_busy,
          ),
        ],
      ),
    );
  }
}
