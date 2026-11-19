// Pruebas de las fuentes empaquetadas y de sus licencias.

import 'dart:io';

import 'package:boveda/theme/licenses.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter_test/flutter_test.dart';

/// Texto de `pubspec.yaml`: `flutter test` corre con `mobile/` como directorio.
String _pubspec() => File('pubspec.yaml').readAsStringSync();

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('declared font assets exist', () {
    final assets = RegExp(
      r'^\s*-?\s*asset:\s*(\S+)',
      multiLine: true,
    ).allMatches(_pubspec()).map((m) => m.group(1)!).toList();

    expect(assets, hasLength(6));
    for (final path in assets) {
      final file = File(path);
      expect(file.existsSync(), isTrue, reason: '$path no existe');
      expect(file.lengthSync(), greaterThan(0), reason: '$path está vacío');
    }
  });

  test('font families are the two documented', () {
    final families = RegExp(
      r'^\s*-\s*family:\s*(\S+)',
      multiLine: true,
    ).allMatches(_pubspec()).map((m) => m.group(1)!).toSet();

    expect(families, {'PlusJakartaSans', 'JetBrainsMono'});
  });

  test('font licenses are registered', () async {
    registerFontLicenses();

    final entries = await LicenseRegistry.licenses.toList();
    final byPackage = <String, String>{};
    for (final entry in entries) {
      final text = entry.paragraphs.map((p) => p.text).join('\n');
      for (final package in entry.packages) {
        byPackage[package] = text;
      }
    }

    expect(
      byPackage.keys,
      containsAll(['Plus Jakarta Sans', 'JetBrains Mono']),
    );
    expect(byPackage['Plus Jakarta Sans'], isNotEmpty);
    expect(byPackage['JetBrains Mono'], isNotEmpty);
  });
}
