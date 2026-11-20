// Pruebas de los íconos de Lucide empaquetados como fuente.

import 'package:boveda/theme/icons.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  final icons = {
    'lock': BovedaIcons.lock,
    'eye': BovedaIcons.eye,
    'eyeOff': BovedaIcons.eyeOff,
    'fingerprint': BovedaIcons.fingerprint,
    'triangleAlert': BovedaIcons.triangleAlert,
    'search': BovedaIcons.search,
    'x': BovedaIcons.x,
    'settings': BovedaIcons.settings,
    'plus': BovedaIcons.plus,
    'vault': BovedaIcons.vault,
    'pencil': BovedaIcons.pencil,
    'trash': BovedaIcons.trash,
    'chevronLeft': BovedaIcons.chevronLeft,
  };

  test('every icon comes from the Lucide font', () {
    for (final MapEntry(key: name, value: icon) in icons.entries) {
      expect(icon.fontFamily, 'Lucide', reason: name);
      expect(icon.codePoint, greaterThan(0), reason: name);
    }
  });

  test('icons are all different', () {
    final codePoints = {for (final icon in icons.values) icon.codePoint};

    expect(codePoints, hasLength(icons.length));
  });

  test('new icons use the codes of the bundled font version', () {
    // Códigos de `lucide.ttf` 1.76.0, comprobados contra su tabla `cmap`.
    expect(BovedaIcons.search.codePoint, 0xe151);
    expect(BovedaIcons.x.codePoint, 0xe1b2);
    expect(BovedaIcons.settings.codePoint, 0xe154);
    expect(BovedaIcons.plus.codePoint, 0xe13d);
    expect(BovedaIcons.vault.codePoint, 0xe58f);
    expect(BovedaIcons.pencil.codePoint, 0xe1f9);
    // `trash-2` de Lucide.
    expect(BovedaIcons.trash.codePoint, 0xe18e);
    expect(BovedaIcons.chevronLeft.codePoint, 0xe06e);
  });
}
