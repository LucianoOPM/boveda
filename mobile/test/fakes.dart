// Dobles escritos a mano para las pruebas de widgets: reemplazan la sesión
// de Rust y la regla de fortaleza sin cargar la librería nativa.

import 'package:boveda/services/biometric_service.dart';
import 'package:boveda/src/rust/api/error.dart';
import 'package:boveda/src/rust/api/entries.dart';
import 'package:boveda/src/rust/api/password.dart';
import 'package:boveda/src/rust/api/session.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';

/// Reporte de desbloqueo sin entradas dañadas.
const emptyReport = UnlockReport(corrupt: [], unidentifiedCorrupt: 0);

/// Id que devuelve `add` por omisión en la sesión simulada.
const fakeNewId = '000000000000000000000000000000aa';

/// Contraseña y notas de prueba que revela la sesión simulada por omisión.
const fakePassword = 'secreta-de-prueba';
const fakeNotes = 'notas de prueba';

/// Sesión simulada. Los respaldos (`exportBackup`, `importBackup`) lanzan
/// `UnimplementedError` para que una llamada inesperada falle la prueba; el
/// resto registra sus llamadas y hace lo que diga su `on…`.
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

  /// Llamadas a `add`, en orden.
  final List<({EntryInput input, NewSecret secret})> addCalls = [];

  /// Lo que hace `add` después de registrar la llamada. Por omisión,
  /// devuelve [fakeNewId] enseguida.
  Future<String> Function(EntryInput input, NewSecret secret) onAdd = (
    _,
    _,
  ) async => fakeNewId;

  @override
  Future<String> add({required EntryInput input, required NewSecret secret}) {
    addCalls.add((input: input, secret: secret));
    return onAdd(input, secret);
  }

  /// Ids recibidos por `delete`, en orden.
  final List<String> deleteCalls = [];

  /// Lo que hace `delete` después de registrar la llamada.
  Future<void> Function(String id) onDelete = (_) async {};

  @override
  Future<void> delete({required String id}) {
    deleteCalls.add(id);
    return onDelete(id);
  }

  /// Llamadas a `edit`, en orden.
  final List<
    ({String id, EntryInput input, String? newPassword, String? newNotes})
  >
  editCalls = [];

  /// Lo que hace `edit` después de registrar la llamada.
  Future<void> Function(
    String id,
    EntryInput input,
    String? newPassword,
    String? newNotes,
  )
  onEdit = (_, _, _, _) async {};

  @override
  Future<void> edit({
    required String id,
    required EntryInput input,
    String? newPassword,
    String? newNotes,
  }) {
    editCalls.add((
      id: id,
      input: input,
      newPassword: newPassword,
      newNotes: newNotes,
    ));
    return onEdit(id, input, newPassword, newNotes);
  }

  /// Ids recibidos por `entry`, en orden.
  final List<String> entryCalls = [];

  /// Lo que hace `entry` después de registrar la llamada. Por omisión, un
  /// resumen de "Servicio" con ese id.
  Future<EntrySummary> Function(String id) onEntry = (id) async =>
      _summary('Servicio', id: id);

  @override
  Future<EntrySummary> entry({required String id}) {
    entryCalls.add(id);
    return onEntry(id);
  }

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

  /// Ids recibidos por `revealNotes`, en orden.
  final List<String> revealNotesCalls = [];

  /// Lo que hace `revealNotes`. Por omisión, [fakeNotes].
  Future<String?> Function(String id) onRevealNotes = (_) async => fakeNotes;

  @override
  Future<String?> revealNotes({required String id}) {
    revealNotesCalls.add(id);
    return onRevealNotes(id);
  }

  /// Ids recibidos por `revealPassword`, en orden.
  final List<String> revealPasswordCalls = [];

  /// Lo que hace `revealPassword`. Por omisión, [fakePassword].
  Future<String> Function(String id) onRevealPassword = (_) async =>
      fakePassword;

  @override
  Future<String> revealPassword({required String id}) {
    revealPasswordCalls.add(id);
    return onRevealPassword(id);
  }

  /// Textos recibidos por `search`, en orden.
  final List<String> searchCalls = [];

  /// Lo que hace `search` después de registrar la llamada. Por omisión,
  /// ninguna entrada, enseguida.
  Future<List<EntrySummary>> Function(String query) onSearch = (_) async => [];

  @override
  Future<List<EntrySummary>> search({required String query}) {
    searchCalls.add(query);
    return onSearch(query);
  }

  @override
  Future<UnlockReport> unlock({required String password}) async {
    unlockCalls.add(password);
    final report = await onUnlock(password);
    locked = false;
    return report;
  }
}

/// [entry] con otro nombre: dentro de [FakeVaultSession], `entry` es el
/// método de la sesión y oculta la función.
const _summary = entry;

/// `EntrySummary` de prueba. Sin [id], uno de 32 caracteres hexadecimales
/// derivado del servicio: único mientras los servicios de la prueba lo sean.
EntrySummary entry(
  String service, {
  String? id,
  String? username,
  String? url,
  List<String> tags = const [],
  int? lastUsedAt,
}) {
  return EntrySummary(
    id: id ?? service.hashCode.toRadixString(16).padLeft(32, '0'),
    service: service,
    url: url,
    username: username,
    tags: tags,
    modifiedAt: 0,
    lastUsedAt: lastUsedAt,
  );
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

/// Si el campo de texto dentro de [field] tiene el foco.
bool fieldHasFocus(WidgetTester tester, Finder field) => tester
    .widget<EditableText>(
      find.descendant(of: field, matching: find.byType(EditableText)),
    )
    .focusNode
    .hasFocus;

/// Simula el portapapeles del sistema y devuelve la lista de textos que la
/// app copie durante la prueba. Pegar entrega [pasteText].
List<String> mockClipboard(WidgetTester tester, {String pasteText = 'pegado'}) {
  final copied = <String>[];
  tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
    SystemChannels.platform,
    (call) async {
      switch (call.method) {
        case 'Clipboard.setData':
          copied.add((call.arguments as Map)['text'] as String);
          return null;
        case 'Clipboard.getData':
          return {'text': pasteText};
        case 'Clipboard.hasStrings':
          return {'value': true};
        default:
          return null;
      }
    },
  );
  addTearDown(
    () => tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      SystemChannels.platform,
      null,
    ),
  );
  return copied;
}

/// Selecciona todo el texto del campo dentro de [field] y envía el atajo
/// Ctrl+[key] de un teclado físico.
Future<void> selectAllAndPress(
  WidgetTester tester,
  Finder field,
  LogicalKeyboardKey key,
) async {
  final editable = tester.widget<EditableText>(
    find.descendant(of: field, matching: find.byType(EditableText)),
  );
  editable.focusNode.requestFocus();
  await tester.pump();
  editable.controller.selection = TextSelection(
    baseOffset: 0,
    extentOffset: editable.controller.text.length,
  );
  await tester.pump();
  await tester.sendKeyDownEvent(LogicalKeyboardKey.control);
  await tester.sendKeyEvent(key);
  await tester.sendKeyUpEvent(LogicalKeyboardKey.control);
  await tester.pump();
}
