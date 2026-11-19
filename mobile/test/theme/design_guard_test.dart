// Guardias del diseño: el tema entra a la app y los colores y tamaños de texto
// salen solo de `lib/theme/` (docs/DESIGN.md, 5.1).

import 'dart:io';

import 'package:boveda/app.dart';
import 'package:boveda/theme/accents.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import '../fakes.dart';

void main() {
  test('no literal colors or font sizes outside lib/theme', () {
    final forbidden = [
      RegExp(r'Color\(0x'),
      RegExp(r'\bColors\.'),
      RegExp(r'\bfontSize:'),
    ];
    final offenders = <String>[];
    for (final entity in Directory('lib').listSync(recursive: true)) {
      if (entity is! File || !entity.path.endsWith('.dart')) continue;
      final path = entity.path.replaceAll('\\', '/');
      // `lib/src/rust/` es código generado.
      if (path.startsWith('lib/theme/') || path.startsWith('lib/src/rust/')) {
        continue;
      }
      final text = entity.readAsStringSync();
      for (final pattern in forbidden) {
        if (pattern.hasMatch(text)) offenders.add('$path: ${pattern.pattern}');
      }
    }

    expect(offenders, isEmpty);
  });

  testWidgets('app uses the dark theme', (tester) async {
    await tester.pumpWidget(
      BovedaApp(
        session: FakeVaultSession(),
        vaultExists: () async => true,
        strengthOf: fakeStrength,
        biometric: FakeBiometricService()..available = false,
        files: FakeBackupFiles(),
      ),
    );
    await tester.pumpAndSettle();

    final app = tester.widget<MaterialApp>(find.byType(MaterialApp));
    expect(app.themeMode, ThemeMode.dark);
    expect(app.darkTheme!.colorScheme.primary, BovedaAccent.cyberLime.accent);
    expect(app.theme!.colorScheme.primary, BovedaAccent.cyberLime.accent);
  });
}
