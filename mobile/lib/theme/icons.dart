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
  static const search = IconData(0xe151, fontFamily: _family);
  static const x = IconData(0xe1b2, fontFamily: _family);
  static const settings = IconData(0xe154, fontFamily: _family);
  static const plus = IconData(0xe13d, fontFamily: _family);
  static const vault = IconData(0xe58f, fontFamily: _family);
  static const pencil = IconData(0xe1f9, fontFamily: _family);

  /// `trash-2` de Lucide.
  static const trash = IconData(0xe18e, fontFamily: _family);
  static const chevronLeft = IconData(0xe06e, fontFamily: _family);
}
