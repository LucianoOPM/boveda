// Pruebas de los íconos de Lucide empaquetados como fuente.

import 'package:boveda/theme/icons.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('every icon comes from the Lucide font', () {
    final icons = [
      BovedaIcons.lock,
      BovedaIcons.eye,
      BovedaIcons.eyeOff,
      BovedaIcons.fingerprint,
      BovedaIcons.triangleAlert,
    ];

    for (final icon in icons) {
      expect(icon.fontFamily, 'Lucide');
      expect(icon.codePoint, greaterThan(0));
    }
  });

  test('icons are all different', () {
    final codePoints = {
      BovedaIcons.lock.codePoint,
      BovedaIcons.eye.codePoint,
      BovedaIcons.eyeOff.codePoint,
      BovedaIcons.fingerprint.codePoint,
      BovedaIcons.triangleAlert.codePoint,
    };

    expect(codePoints, hasLength(5));
  });
}
