// Dobles escritos a mano para las pruebas de widgets: reemplazan la sesión
// de Rust y la regla de fortaleza sin cargar la librería nativa.

import 'dart:typed_data';

import 'package:boveda/services/biometric_service.dart';
import 'package:boveda/src/rust/api/error.dart';
import 'package:boveda/src/rust/api/entries.dart';
import 'package:boveda/src/rust/api/password.dart';
import 'package:boveda/src/rust/api/session.dart';

/// Reporte de desbloqueo sin entradas dañadas.
const emptyReport = UnlockReport(corrupt: [], unidentifiedCorrupt: 0);

/// Sesión simulada. `create`, `open`, `unlock`, `unlockWithKey`,
/// `exportVaultKey`, `lock` e `isLocked` hacen algo; el resto lanza
/// `UnimplementedError` para que una llamada inesperada falle la prueba.
///
/// Lleva el estado de bloqueo como la fachada: [locked] empieza en `true`,
/// `create`, `unlock` y `unlockWithKey` lo ponen en `false` al terminar con
/// éxito, `lock` en `true`, y `exportVaultKey` con la bóveda bloqueada lanza
/// `VaultError.locked`.
class FakeVaultSession implements VaultSession {
  /// La bóveda está bloqueada en el núcleo simulado.
  bool locked = true;

  /// Llamadas a `lock`.
  int lockCalls = 0;

  /// Si no es `null`, `lock()` lo lanza (después de bloquear).
  Object? lockError;

  /// Contraseñas recibidas por `create`, en orden.
  final List<String> createCalls = [];

  /// Lo que hace `create` después de registrar la llamada. Por omisión,
  /// termina enseguida.
  Future<void> Function(String password) onCreate = (_) async {};

  /// Si no es `null`, `open()` lo lanza.
  Object? openError;

  int openCalls = 0;

  /// Contraseñas recibidas por `unlock`, en orden.
  final List<String> unlockCalls = [];

  /// Lo que hace `unlock` después de registrar la llamada. Por omisión,
  /// devuelve un reporte vacío enseguida.
  Future<UnlockReport> Function(String password) onUnlock = (_) async =>
      emptyReport;

  bool _disposed = false;

  @override
  Future<void> create({required String password}) async {
    createCalls.add(password);
    await onCreate(password);
    locked = false;
  }

  @override
  Future<void> open() async {
    openCalls++;
    if (openError case final error?) throw error;
  }

  @override
  void dispose() => _disposed = true;

  @override
  bool get isDisposed => _disposed;

  @override
  Future<String> add({required EntryInput input, required NewSecret secret}) =>
      throw UnimplementedError();

  @override
  Future<void> delete({required String id}) => throw UnimplementedError();

  @override
  Future<void> edit({
    required String id,
    required EntryInput input,
    NewSecret? newSecret,
  }) => throw UnimplementedError();

  @override
  Future<void> exportBackup({required String dest}) =>
      throw UnimplementedError();

  @override
  Future<UnlockReport> importBackup({
    required String backup,
    required String password,
  }) => throw UnimplementedError();

  /// Llaves entregadas por `exportVaultKey` (la misma referencia), para
  /// comprobar que quien las recibe las pone en ceros.
  final List<Uint8List> exportedKeys = [];

  /// Lo que hace `exportVaultKey`. Por omisión, 32 bytes de valor 9.
  Future<Uint8List> Function() onExportVaultKey = () async =>
      Uint8List.fromList(List.filled(32, 9));

  /// Llaves recibidas por `unlockWithKey` (la misma referencia), en orden.
  final List<List<int>> unlockWithKeyCalls = [];

  /// Lo que hace `unlockWithKey` después de registrar la llamada. Por
  /// omisión, un reporte vacío.
  Future<UnlockReport> Function(List<int> key) onUnlockWithKey = (_) async =>
      emptyReport;

  @override
  Future<Uint8List> exportVaultKey() async {
    if (locked) throw VaultError.locked;
    final key = await onExportVaultKey();
    exportedKeys.add(key);
    return key;
  }

  @override
  Future<UnlockReport> unlockWithKey({required List<int> key}) async {
    unlockWithKeyCalls.add(key);
    final report = await onUnlockWithKey(key);
    locked = false;
    return report;
  }

  @override
  Future<bool> isLocked() async => locked;

  @override
  Future<void> lock() async {
    lockCalls++;
    locked = true;
    if (lockError case final error?) throw error;
  }

  @override
  Future<String?> revealNotes({required String id}) =>
      throw UnimplementedError();

  @override
  Future<String> revealPassword({required String id}) =>
      throw UnimplementedError();

  @override
  Future<List<EntrySummary>> search({required String query}) =>
      throw UnimplementedError();

  @override
  Future<UnlockReport> unlock({required String password}) async {
    unlockCalls.add(password);
    final report = await onUnlock(password);
    locked = false;
    return report;
  }
}

/// La regla del núcleo por longitud. Las pruebas usan solo ASCII, así que
/// `length` coincide con los caracteres que cuenta el núcleo.
PasswordStrength fakeStrength(String password) {
  if (password.length < 12) return PasswordStrength.tooShort;
  if (password.length < 16) return PasswordStrength.acceptable;
  return PasswordStrength.strong;
}

/// Servicio de huella simulado. Registra cada llamada en [calls], en orden;
/// por omisión todo tiene éxito.
class FakeBiometricService implements BiometricService {
  /// Nombre de cada método llamado, en orden.
  final List<String> calls = [];

  /// Lo que devuelve `isAvailable`, salvo que [availableError] no sea `null`.
  bool available = true;
  Object? availableError;

  /// Lo que devuelve `isEnabled`, salvo que [enabledError] no sea `null`.
  /// `completeEnable` lo pone en `true` y `disable` en `false`.
  bool enabled = false;
  Object? enabledError;

  /// Llave recibida por `completeEnable` (la misma referencia).
  Uint8List? receivedKey;

  Future<void> Function() onBeginEnable = () async {};
  Future<void> Function() onCompleteEnable = () async {};
  Future<void> Function() onAbortEnable = () async {};
  Future<void> Function() onDisable = () async {};

  /// Lo que hace `unlock`. Por omisión, 32 bytes de valor 7.
  Future<Uint8List> Function() onUnlock = () async =>
      Uint8List.fromList(List.filled(32, 7));

  @override
  Future<bool> isAvailable() async {
    calls.add('isAvailable');
    if (availableError case final error?) throw error;
    return available;
  }

  @override
  Future<bool> isEnabled() async {
    calls.add('isEnabled');
    if (enabledError case final error?) throw error;
    return enabled;
  }

  @override
  Future<void> beginEnable() {
    calls.add('beginEnable');
    return onBeginEnable();
  }

  @override
  Future<void> completeEnable(Uint8List key) async {
    calls.add('completeEnable');
    receivedKey = key;
    await onCompleteEnable();
    enabled = true;
  }

  @override
  Future<void> abortEnable() {
    calls.add('abortEnable');
    return onAbortEnable();
  }

  @override
  Future<Uint8List> unlock() {
    calls.add('unlock');
    return onUnlock();
  }

  @override
  Future<void> disable() async {
    calls.add('disable');
    await onDisable();
    enabled = false;
  }
}
