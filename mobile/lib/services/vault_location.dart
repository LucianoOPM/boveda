import 'package:flutter/services.dart';
import 'package:path/path.dart' as p;

const _channel = MethodChannel('boveda/storage');

/// Ruta absoluta de la bóveda: `boveda.db` dentro de `noBackupFilesDir`.
///
/// Ese directorio queda fuera de Auto Backup y de la transferencia entre
/// dispositivos de Android: el único camino de restauración es importar un
/// respaldo.
Future<String> vaultPath() async {
  final dir = await _channel.invokeMethod<String>('noBackupFilesDir');
  if (dir == null) {
    throw StateError('Android no devolvió el directorio de la bóveda');
  }
  return p.join(dir, 'boveda.db');
}
