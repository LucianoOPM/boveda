// Dobles escritos a mano para las pruebas de widgets: reemplazan la sesión
// de Rust y la regla de fortaleza sin cargar la librería nativa.

import 'dart:typed_data';

import 'package:boveda/src/rust/api/entries.dart';
import 'package:boveda/src/rust/api/password.dart';
import 'package:boveda/src/rust/api/session.dart';

/// Reporte de desbloqueo sin entradas dañadas.
const emptyReport = UnlockReport(corrupt: [], unidentifiedCorrupt: 0);

/// Sesión simulada. Solo `create`, `open` y `unlock` hacen algo; el resto lanza
/// `UnimplementedError` para que una llamada inesperada falle la prueba.
class FakeVaultSession implements VaultSession {
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
  Future<void> create({required String password}) {
    createCalls.add(password);
    return onCreate(password);
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

  @override
  Future<Uint8List> exportVaultKey() => throw UnimplementedError();

  @override
  Future<UnlockReport> unlockWithKey({required List<int> key}) =>
      throw UnimplementedError();

  @override
  Future<bool> isLocked() => throw UnimplementedError();

  @override
  Future<void> lock() => throw UnimplementedError();

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
  Future<UnlockReport> unlock({required String password}) {
    unlockCalls.add(password);
    return onUnlock(password);
  }
}

/// La regla del núcleo por longitud. Las pruebas usan solo ASCII, así que
/// `length` coincide con los caracteres que cuenta el núcleo.
PasswordStrength fakeStrength(String password) {
  if (password.length < 12) return PasswordStrength.tooShort;
  if (password.length < 16) return PasswordStrength.acceptable;
  return PasswordStrength.strong;
}
