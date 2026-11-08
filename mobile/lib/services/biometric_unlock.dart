import 'dart:typed_data';

import '../src/rust/api/error.dart';
import '../src/rust/api/session.dart';
import 'biometric_service.dart';

/// Resultado de activar la huella.
enum EnableOutcome { enabled, canceled, failed }

/// Activa la huella en dos pasos: primero la huella y después la llave sale
/// del núcleo. Si el usuario cancela, la llave nunca sale de Rust.
///
/// El `Uint8List` de `exportVaultKey` es memoria de Dart: se pone en ceros
/// después de entregarlo a Kotlin, también si falla. `invokeMethod` codifica
/// el mensaje antes de su primer `await`, así que los ceros no alcanzan la
/// copia en tránsito. Es de mejor esfuerzo, como todo borrado fuera del
/// núcleo.
Future<EnableOutcome> enableBiometric(
  VaultSession session,
  BiometricService biometric,
) async {
  try {
    await biometric.beginEnable();
  } on BiometricException catch (error) {
    // Kotlin ya borró la llave del Keystore.
    return error.failure == BiometricFailure.canceled
        ? EnableOutcome.canceled
        : EnableOutcome.failed;
  } catch (_) {
    return EnableOutcome.failed;
  }

  final Uint8List key;
  try {
    key = await session.exportVaultKey();
  } catch (_) {
    try {
      await biometric.abortEnable();
    } catch (_) {
      // Sin la copia, la llave sobrante no habilita nada; el siguiente
      // `beginEnable` la borra.
    }
    return EnableOutcome.failed;
  }

  try {
    await biometric.completeEnable(key);
    return EnableOutcome.enabled;
  } catch (_) {
    return EnableOutcome.failed;
  } finally {
    _wipe(key);
  }
}

/// Resultado de desbloquear con huella.
sealed class BiometricUnlockOutcome {
  const BiometricUnlockOutcome();
}

/// La bóveda quedó desbloqueada, con el total de entradas dañadas (con y sin
/// id) del `UnlockReport`.
class BiometricUnlocked extends BiometricUnlockOutcome {
  const BiometricUnlocked(this.damagedEntries);

  final int damagedEntries;
}

/// Cancelada, bloqueo temporal u otro error del diálogo: se usa la
/// contraseña, sin mensaje y sin desactivar la huella.
class BiometricFallback extends BiometricUnlockOutcome {
  const BiometricFallback();
}

/// La copia ya no sirve (llave invalidada o inexistente, copia ilegible o
/// `WrongVaultKey`): la huella quedó desactivada.
class BiometricDisabled extends BiometricUnlockOutcome {
  const BiometricDisabled();
}

/// El núcleo falló por otro motivo: el mensaje genérico de desbloqueo.
class BiometricCoreError extends BiometricUnlockOutcome {
  const BiometricCoreError();
}

/// Pide la huella y desbloquea con la llave que libera el Keystore.
///
/// La llave se pone en ceros en todos los caminos después de entregarla a
/// la fachada. Solo `invalidated` y `WrongVaultKey` desactivan la huella.
Future<BiometricUnlockOutcome> unlockWithBiometric(
  VaultSession session,
  BiometricService biometric,
) async {
  final Uint8List key;
  try {
    key = await biometric.unlock();
  } on BiometricException catch (error) {
    if (error.failure == BiometricFailure.invalidated) {
      return _disable(biometric);
    }
    return const BiometricFallback();
  } catch (_) {
    return const BiometricFallback();
  }

  try {
    final report = await session.unlockWithKey(key: key);
    return BiometricUnlocked(
      report.corrupt.length + report.unidentifiedCorrupt,
    );
  } on VaultError catch (error) {
    if (error == VaultError.alreadyUnlocked) return const BiometricUnlocked(0);
    if (error == VaultError.wrongVaultKey) return _disable(biometric);
    return const BiometricCoreError();
  } catch (_) {
    return const BiometricCoreError();
  } finally {
    _wipe(key);
  }
}

/// Pone la llave en ceros. El canal de Kotlin la entrega como una vista no
/// modificable del mensaje (`_UnmodifiableUint8ArrayView`): ahí no se puede
/// borrar y queda en la excepción de mejor esfuerzo de `StandardMessageCodec`.
/// Nunca lanza, para que un fallo al borrar no rompa el desbloqueo.
void _wipe(Uint8List key) {
  try {
    key.fillRange(0, key.length, 0);
  } on UnsupportedError {
    // Vista no modificable: mejor esfuerzo.
  }
}

/// Desactiva la huella. Si `disable` falla, la copia que quede dará
/// `invalidated` o `WrongVaultKey` en el siguiente intento y se volverá a
/// desactivar.
Future<BiometricUnlockOutcome> _disable(BiometricService biometric) async {
  try {
    await biometric.disable();
  } catch (_) {}
  return const BiometricDisabled();
}
