import 'dart:ui';

/// Los cuatro acentos intercambiables (`docs/DESIGN.md`, 2.3). Cada uno
/// define el mismo juego de tokens; el resto de la paleta no cambia.
///
/// La app usa [cyberLime] fijo; elegir otro es el selector de Ajustes (D2),
/// que todavía no existe.
enum BovedaAccent {
  cyberLime(
    accent: Color(0xFFB5FF00),
    accentDim: Color.fromRGBO(181, 255, 0, 0.14),
    accentBorder: Color.fromRGBO(181, 255, 0, 0.38),
  ),
  electricEmerald(
    accent: Color(0xFF00F59B),
    accentDim: Color.fromRGBO(0, 245, 155, 0.14),
    accentBorder: Color.fromRGBO(0, 245, 155, 0.38),
  ),
  solarAmber(
    accent: Color(0xFFFF9F1C),
    accentDim: Color.fromRGBO(255, 159, 28, 0.14),
    accentBorder: Color.fromRGBO(255, 159, 28, 0.38),
  ),
  neonCrimson(
    accent: Color(0xFFFF2A85),
    accentDim: Color.fromRGBO(255, 42, 133, 0.15),
    accentBorder: Color.fromRGBO(255, 42, 133, 0.42),
  );

  const BovedaAccent({
    required this.accent,
    required this.accentDim,
    required this.accentBorder,
  });

  /// Acción principal y estado seguro.
  final Color accent;

  /// Fondo tenue del acento (14 %, 15 % en Neon Crimson).
  final Color accentDim;

  /// Borde del acento (38 %, 42 % en Neon Crimson).
  final Color accentBorder;

  /// Texto e íconos sobre el acento. Siempre `#08080A`: el blanco sobre Neon
  /// Crimson da 3.55:1 y no pasa ni AA.
  Color get onAccent => const Color(0xFF08080A);

  /// El acento al 28 %: solo para el resplandor de la acción principal.
  Color get accentGlow => accent.withValues(alpha: 0.28);
}
