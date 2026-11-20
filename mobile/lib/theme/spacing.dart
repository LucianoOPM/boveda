/// Escala de espaciado base 4 (`docs/DESIGN.md`, 4.1).
abstract final class Space {
  static const double xs = 4;
  static const double sm = 8;

  /// Separación entre tarjetas.
  static const double s10 = 10;
  static const double md = 12;

  /// Relleno interno de las tarjetas.
  static const double s14 = 14;

  /// Margen lateral de pantalla.
  static const double lg = 16;
  static const double xl = 24;
  static const double xxl = 32;
  static const double huge = 48;
}

/// Tamaños de los íconos de Lucide que usan la pantalla de la bóveda y sus
/// widgets (`docs/DESIGN.md`, 5.1: trazo de 2 px, el mismo ícono en toda la
/// app).
abstract final class IconSizes {
  /// Dentro de una insignia (la ✕ de una etiqueta).
  static const double inline = 14;

  /// Dentro de un campo, un botón o un aviso.
  static const double control = 20;

  /// Ícono de un estado (vacío o error).
  static const double feature = 32;

  /// Ícono grande de un estado sin resultados.
  static const double large = 40;
}

/// Radios (`docs/DESIGN.md`, 4.2).
abstract final class Radii {
  /// Tarjetas, diálogos y hojas inferiores.
  static const double card = 16;

  /// Botones, campos de texto y contenedores internos.
  static const double control = 12;

  /// Insignias y chips de etiqueta (6 a 8).
  static const double badge = 8;

  /// Insignia de etiqueta de la tarjeta de entrada (`DESIGN.md` 6.2).
  static const double tag = 6;

  /// Cápsulas, barras de progreso e indicadores.
  static const double full = 999;
}
