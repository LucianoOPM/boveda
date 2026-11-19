import 'dart:ui';

/// Colores del sistema de diseño (`docs/DESIGN.md`, secciones 2.1, 2.2 y 2.4).
///
/// Los acentos viven en `BovedaAccent`. Un color nuevo se agrega aquí con su
/// contraste medido en la tabla 2.5 de `DESIGN.md`.
abstract final class BovedaColors {
  // 2.1 Superficies y bordes.
  static const bgCanvas = Color(0xFF070709);
  static const bgBase = Color(0xFF0A0A0C);
  static const surface1 = Color(0xFF121218);
  static const surface2 = Color(0xFF14141B);
  static const surfaceElevated = Color(0xFF1D1D27);
  static const iconBox = Color(0xFF0D0D12);
  static const borderSubtle = Color(0xFF22222C);
  static const borderCard = Color(0xFF242430);
  static const borderInteractive = Color(0xFF282836);
  static const outline = Color(0xFF71717A);
  static const lockScrim = Color(0xFF09090C);

  // 2.2 Texto.
  static const textPrimary = Color(0xFFF4F4F5);
  static const textBody = Color(0xFFD4D4D8);
  static const textSecondary = Color(0xFFA1A1AA);
  static const textDisabled = Color(0xFF71717A);

  // 2.4 Estados.
  static const error = Color(0xFFF87171);
  static const errorStrong = Color(0xFFEF4444);
  static const warning = Color(0xFFFBBF24);
  static const success = Color(0xFF4ADE80);
  static const lan = Color(0xFFF59E0B);

  /// `#450A0A` al 20 % sobre [surface2]: un color opaco.
  static final errorContainer = _onSurface2(const Color(0xFF450A0A), 0.20);

  /// Contenedores de estado: el tono al 14 % sobre [surface2].
  static final successContainer = _onSurface2(success, 0.14);
  static final warningContainer = _onSurface2(warning, 0.14);
  static final lanContainer = _onSurface2(lan, 0.14);

  // 2.4 Sintaxis de contraseñas.
  static const syntaxLetter = Color(0xFFF4F4F5);
  static const syntaxDigit = Color(0xFFFBBF24);
  static const syntaxSymbol = Color(0xFFEC4899);
  static const syntaxSpace = Color(0xFFA1A1AA);

  static Color _onSurface2(Color color, double opacity) =>
      Color.alphaBlend(color.withValues(alpha: opacity), surface2);
}
