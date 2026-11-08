import 'package:flutter/services.dart';

/// Por qué falló una operación de huella. Solo [invalidated] desactiva la
/// huella (la llave del Keystore quedó invalidada o no existe, o la copia es
/// ilegible); el resto vuelve a la contraseña.
enum BiometricFailure { invalidated, canceled, lockout, failed, busy }

/// Error de una operación de huella. Solo lleva el código: ningún texto de
/// Kotlin cruza a Dart.
class BiometricException implements Exception {
  const BiometricException(this.failure);

  final BiometricFailure failure;
}

/// Huella y copia de la llave de bóveda en el Keystore.
///
/// La llave cruza solo como bytes. Quien la recibe o la entrega la pone en
/// ceros apenas termina (ver `biometric_unlock.dart`).
abstract class BiometricService {
  /// API 30 o más y huella fuerte registrada. `false` si algo falla.
  Future<bool> isAvailable();

  /// Existe la copia cifrada. `false` si algo falla.
  Future<bool> isEnabled();

  /// Crea la llave del Keystore y pide la huella. Lanza [BiometricException].
  Future<void> beginEnable();

  /// Cifra [key] con la llave autenticada en [beginEnable] y guarda la copia.
  Future<void> completeEnable(Uint8List key);

  /// Descarta una activación a medias.
  Future<void> abortEnable();

  /// Pide la huella y devuelve la llave de bóveda.
  Future<Uint8List> unlock();

  /// Borra la copia y la llave del Keystore. No falla si no existen.
  Future<void> disable();
}

/// [BiometricService] sobre el canal `boveda/biometric` de `MainActivity`.
class PlatformBiometricService implements BiometricService {
  const PlatformBiometricService();

  static const _channel = MethodChannel('boveda/biometric');

  @override
  Future<bool> isAvailable() => _flag('isAvailable');

  @override
  Future<bool> isEnabled() => _flag('isEnabled');

  @override
  Future<void> beginEnable() => _invoke<void>('beginEnable');

  /// Envía el `Uint8List` tal cual; ponerlo en ceros es de quien llama.
  @override
  Future<void> completeEnable(Uint8List key) =>
      _invoke<void>('completeEnable', key);

  @override
  Future<void> abortEnable() => _invoke<void>('abortEnable');

  @override
  Future<Uint8List> unlock() async {
    final key = await _invoke<Uint8List>('unlock');
    if (key == null) throw const BiometricException(BiometricFailure.failed);
    return key;
  }

  @override
  Future<void> disable() => _invoke<void>('disable');

  Future<bool> _flag(String method) async {
    try {
      return await _channel.invokeMethod<bool>(method) ?? false;
    } catch (_) {
      return false;
    }
  }

  /// Traduce los errores del canal a [BiometricException], sin detalles.
  Future<T?> _invoke<T>(String method, [Object? arguments]) async {
    try {
      return await _channel.invokeMethod<T>(method, arguments);
    } on PlatformException catch (error) {
      throw BiometricException(_failureFor(error.code));
    } on MissingPluginException {
      throw const BiometricException(BiometricFailure.failed);
    }
  }

  static BiometricFailure _failureFor(String code) {
    for (final failure in BiometricFailure.values) {
      if (failure.name == code) return failure;
    }
    return BiometricFailure.failed;
  }
}
