import 'package:flutter/services.dart';

/// Error del portapapeles local.
///
/// No conserva el error de plataforma: podría incluir el contenido que se
/// intentó copiar.
class LocalClipboardException implements Exception {
  const LocalClipboardException();

  @override
  String toString() => 'LocalClipboardException';
}

/// Portapapeles local de Android.
///
/// El valor devuelto indica si Flutter debe mostrar el aviso de copia. En
/// Android 13 o posterior el sistema ya lo muestra y devuelve `false`.
abstract interface class LocalClipboard {
  Future<bool> copySensitive(String text);

  Future<bool> copyPlain(String text);
}

/// [LocalClipboard] sobre el canal `boveda/clipboard` de `MainActivity`.
class PlatformLocalClipboard implements LocalClipboard {
  const PlatformLocalClipboard();

  static const _channel = MethodChannel('boveda/clipboard');

  @override
  Future<bool> copySensitive(String text) => _copy('copySensitive', text);

  @override
  Future<bool> copyPlain(String text) => _copy('copyPlain', text);

  Future<bool> _copy(String method, String text) async {
    try {
      final showFeedback = await _channel.invokeMethod<bool>(method, text);
      if (showFeedback == null) throw const LocalClipboardException();
      return showFeedback;
    } on LocalClipboardException {
      rethrow;
    } catch (_) {
      throw const LocalClipboardException();
    }
  }
}
