import 'package:flutter/widgets.dart';

/// Íconos de [Lucide](https://lucide.com) (`docs/DESIGN.md`, 5.1), empaquetados
/// como fuente (`assets/fonts/lucide.ttf`, versión 1.76.0, licencia ISC).
///
/// Solo están los que usan las pantallas ya migradas; un ícono nuevo se agrega
/// con su código del `info.json` de la misma versión de la fuente.
abstract final class BovedaIcons {
  static const _family = 'Lucide';

  static const lock = IconData(0xe10b, fontFamily: _family);
  static const eye = IconData(0xe0ba, fontFamily: _family);
  static const eyeOff = IconData(0xe0bb, fontFamily: _family);
  static const fingerprint = IconData(0xe2cb, fontFamily: _family);
  static const triangleAlert = IconData(0xe193, fontFamily: _family);
}
