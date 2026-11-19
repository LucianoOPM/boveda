import 'package:flutter/material.dart';

import 'accents.dart';
import 'colors.dart';

/// Colores de estado que Material no cubre (`docs/DESIGN.md`, 2.4).
@immutable
class StatusColors extends ThemeExtension<StatusColors> {
  const StatusColors({
    required this.success,
    required this.successContainer,
    required this.warning,
    required this.warningContainer,
    required this.lan,
    required this.lanContainer,
    required this.errorStrong,
  });

  static final standard = StatusColors(
    success: BovedaColors.success,
    successContainer: BovedaColors.successContainer,
    warning: BovedaColors.warning,
    warningContainer: BovedaColors.warningContainer,
    lan: BovedaColors.lan,
    lanContainer: BovedaColors.lanContainer,
    errorStrong: BovedaColors.errorStrong,
  );

  final Color success;
  final Color successContainer;
  final Color warning;
  final Color warningContainer;
  final Color lan;
  final Color lanContainer;

  /// Rellenos de error: barra del temporizador, cápsula "BLOQUEADA".
  final Color errorStrong;

  @override
  StatusColors copyWith({
    Color? success,
    Color? successContainer,
    Color? warning,
    Color? warningContainer,
    Color? lan,
    Color? lanContainer,
    Color? errorStrong,
  }) => StatusColors(
    success: success ?? this.success,
    successContainer: successContainer ?? this.successContainer,
    warning: warning ?? this.warning,
    warningContainer: warningContainer ?? this.warningContainer,
    lan: lan ?? this.lan,
    lanContainer: lanContainer ?? this.lanContainer,
    errorStrong: errorStrong ?? this.errorStrong,
  );

  @override
  StatusColors lerp(StatusColors? other, double t) {
    if (other == null) return this;
    return StatusColors(
      success: Color.lerp(success, other.success, t)!,
      successContainer: Color.lerp(
        successContainer,
        other.successContainer,
        t,
      )!,
      warning: Color.lerp(warning, other.warning, t)!,
      warningContainer: Color.lerp(
        warningContainer,
        other.warningContainer,
        t,
      )!,
      lan: Color.lerp(lan, other.lan, t)!,
      lanContainer: Color.lerp(lanContainer, other.lanContainer, t)!,
      errorStrong: Color.lerp(errorStrong, other.errorStrong, t)!,
    );
  }
}

/// Colores por tipo de carácter de una contraseña revelada (2.4).
@immutable
class SyntaxColors extends ThemeExtension<SyntaxColors> {
  const SyntaxColors({
    required this.letter,
    required this.digit,
    required this.symbol,
    required this.space,
  });

  static const standard = SyntaxColors(
    letter: BovedaColors.syntaxLetter,
    digit: BovedaColors.syntaxDigit,
    symbol: BovedaColors.syntaxSymbol,
    space: BovedaColors.syntaxSpace,
  );

  final Color letter;
  final Color digit;
  final Color symbol;
  final Color space;

  @override
  SyntaxColors copyWith({
    Color? letter,
    Color? digit,
    Color? symbol,
    Color? space,
  }) => SyntaxColors(
    letter: letter ?? this.letter,
    digit: digit ?? this.digit,
    symbol: symbol ?? this.symbol,
    space: space ?? this.space,
  );

  @override
  SyntaxColors lerp(SyntaxColors? other, double t) {
    if (other == null) return this;
    return SyntaxColors(
      letter: Color.lerp(letter, other.letter, t)!,
      digit: Color.lerp(digit, other.digit, t)!,
      symbol: Color.lerp(symbol, other.symbol, t)!,
      space: Color.lerp(space, other.space, t)!,
    );
  }
}

/// Variantes del acento elegido: fondo tenue, borde y resplandor (2.3).
@immutable
class AccentColors extends ThemeExtension<AccentColors> {
  const AccentColors({
    required this.dim,
    required this.border,
    required this.glow,
  });

  factory AccentColors.of(BovedaAccent accent) => AccentColors(
    dim: accent.accentDim,
    border: accent.accentBorder,
    glow: accent.accentGlow,
  );

  final Color dim;
  final Color border;
  final Color glow;

  @override
  AccentColors copyWith({Color? dim, Color? border, Color? glow}) =>
      AccentColors(
        dim: dim ?? this.dim,
        border: border ?? this.border,
        glow: glow ?? this.glow,
      );

  @override
  AccentColors lerp(AccentColors? other, double t) {
    if (other == null) return this;
    return AccentColors(
      dim: Color.lerp(dim, other.dim, t)!,
      border: Color.lerp(border, other.border, t)!,
      glow: Color.lerp(glow, other.glow, t)!,
    );
  }
}

/// Estilos de texto que el `TextTheme` de Material no tiene (`docs/DESIGN.md`,
/// sección 3). Sin color: lo pone quien los usa. `meta` va en MAYÚSCULAS, cosa
/// que también hace quien lo usa.
@immutable
class BovedaText extends ThemeExtension<BovedaText> {
  const BovedaText({
    required this.brand,
    required this.secret,
    required this.metric,
    required this.meta,
  });

  static const sans = 'PlusJakartaSans';
  static const mono = 'JetBrainsMono';

  static const standard = BovedaText(
    // Marca en el encabezado: 16 / 800, -0.02em.
    brand: TextStyle(
      fontFamily: sans,
      fontSize: 16,
      fontWeight: FontWeight.w800,
      letterSpacing: -0.32,
    ),
    // Contraseña revelada o generada: 17 / 700, 0.05em.
    secret: TextStyle(
      fontFamily: mono,
      fontSize: 17,
      fontWeight: FontWeight.w700,
      letterSpacing: 0.85,
      fontFeatures: [FontFeature.tabularFigures()],
    ),
    // Valores grandes (bits de entropía, temporizador del QR): 15 / 800, 0.15em.
    metric: TextStyle(
      fontFamily: mono,
      fontSize: 15,
      fontWeight: FontWeight.w800,
      letterSpacing: 2.25,
      fontFeatures: [FontFeature.tabularFigures()],
    ),
    // Insignias, etiquetas y temporizadores: 11 / 500, 0.08em. Las insignias
    // que pidan más peso usan `copyWith(fontWeight: FontWeight.w700)`.
    meta: TextStyle(
      fontFamily: mono,
      fontSize: 11,
      fontWeight: FontWeight.w500,
      letterSpacing: 0.88,
      fontFeatures: [FontFeature.tabularFigures()],
    ),
  );

  final TextStyle brand;
  final TextStyle secret;
  final TextStyle metric;
  final TextStyle meta;

  @override
  BovedaText copyWith({
    TextStyle? brand,
    TextStyle? secret,
    TextStyle? metric,
    TextStyle? meta,
  }) => BovedaText(
    brand: brand ?? this.brand,
    secret: secret ?? this.secret,
    metric: metric ?? this.metric,
    meta: meta ?? this.meta,
  );

  @override
  BovedaText lerp(BovedaText? other, double t) {
    if (other == null) return this;
    return BovedaText(
      brand: TextStyle.lerp(brand, other.brand, t)!,
      secret: TextStyle.lerp(secret, other.secret, t)!,
      metric: TextStyle.lerp(metric, other.metric, t)!,
      meta: TextStyle.lerp(meta, other.meta, t)!,
    );
  }
}
