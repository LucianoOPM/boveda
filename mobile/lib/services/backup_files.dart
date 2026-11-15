import 'package:flutter/services.dart';

/// Por qué falló una operación de archivos de respaldo. Solo el código:
/// ningún texto de Kotlin cruza a Dart (puede traer rutas).
enum BackupFailure { failed, busy, tooLarge, invalidPath }

/// Error de una operación de [BackupFiles].
class BackupFilesException implements Exception {
  const BackupFilesException(this.failure);

  final BackupFailure failure;
}

/// Archivos del respaldo y selector del sistema (Storage Access Framework).
///
/// Dart solo maneja rutas de temporales dentro de
/// `noBackupFilesDir/backup-tmp/`; la copia entre la URI `content://` y el
/// temporal la hace Kotlin por flujo, sin pasar bytes por Dart (#44, detalle
/// en `docs/superpowers/specs/2026-11-14-respaldos-selector-design.md`).
abstract class BackupFiles {
  /// Una ruta nueva dentro de `backup-tmp/`, sin crear el archivo.
  Future<String> newTempPath();

  /// Abre el selector para crear el archivo [name], copia [tempPath] y
  /// borra el temporal. `false` si el usuario cancela.
  Future<bool> saveTo(String tempPath, String name);

  /// Abre el selector, copia el archivo elegido a un temporal nuevo y
  /// devuelve su ruta. `null` si el usuario cancela. Lanza
  /// [BackupFilesException] (`tooLarge` si pasa de 20 MiB).
  Future<String?> pickToTemp();

  /// Borra un temporal. No hace nada si no existe; rechaza una ruta fuera
  /// de `backup-tmp/` con `invalidPath`.
  Future<void> deleteTemp(String tempPath);
}

/// Nombre sugerido del respaldo: `boveda-AAAA-MM-DD.db`, con la fecha dada.
String backupFileName(DateTime date) {
  String two(int n) => n.toString().padLeft(2, '0');
  return 'boveda-${date.year}-${two(date.month)}-${two(date.day)}.db';
}

/// [BackupFiles] sobre el canal `boveda/backup` de `MainActivity`.
class PlatformBackupFiles implements BackupFiles {
  const PlatformBackupFiles();

  static const _channel = MethodChannel('boveda/backup');

  @override
  Future<String> newTempPath() async {
    final path = await _invoke<String>('newTempPath');
    if (path == null) throw const BackupFilesException(BackupFailure.failed);
    return path;
  }

  @override
  Future<bool> saveTo(String tempPath, String name) async =>
      await _invoke<bool>('saveTo', {'temp': tempPath, 'name': name}) ?? false;

  @override
  Future<String?> pickToTemp() => _invoke<String>('pickToTemp');

  @override
  Future<void> deleteTemp(String tempPath) =>
      _invoke<void>('deleteTemp', tempPath);

  /// Traduce los errores del canal a [BackupFilesException], sin detalles.
  Future<T?> _invoke<T>(String method, [Object? arguments]) async {
    try {
      return await _channel.invokeMethod<T>(method, arguments);
    } on PlatformException catch (error) {
      throw BackupFilesException(_failureFor(error.code));
    } on MissingPluginException {
      throw const BackupFilesException(BackupFailure.failed);
    }
  }

  static BackupFailure _failureFor(String code) {
    for (final failure in BackupFailure.values) {
      if (failure.name == code) return failure;
    }
    return BackupFailure.failed;
  }
}
