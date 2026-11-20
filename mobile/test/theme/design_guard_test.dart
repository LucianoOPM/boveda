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

  test('migrated files use Lucide icons, not Material Icons', () {
    // Archivos ya restilizados; cada PR de pantalla agrega los suyos.
    const migrated = [
      'lib/screens/unlock_screen.dart',
      'lib/screens/first_run_screen.dart',
      'lib/screens/biometric_offer_screen.dart',
      'lib/widgets/password_field.dart',
      'lib/widgets/strength_indicator.dart',
      'lib/widgets/status_capsule.dart',
      'lib/screens/search_screen.dart',
      'lib/widgets/damaged_entries_banner.dart',
      'lib/widgets/vault_card.dart',
      'lib/widgets/vault_header.dart',
      'lib/widgets/tag_badge.dart',
      'lib/screens/entry_detail_screen.dart',
      'lib/widgets/secret_text.dart',
      'lib/screens/entry_form_screen.dart',
      'lib/widgets/tags_field.dart',
      'lib/widgets/labeled_field.dart',
    ];
    final offenders = [
      for (final path in migrated)
        if (RegExp(r'\bIcons\.').hasMatch(File(path).readAsStringSync())) path,
    ];

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
