import 'package:flutter/material.dart';

import '../src/rust/api/password.dart';

/// Calcula la fortaleza de una contraseña. En la app es `passwordStrength`
/// del núcleo; las pruebas la reemplazan.
typedef StrengthOf = PasswordStrength Function(String password);

/// Barra de tres pasos y texto con la fortaleza de la contraseña maestra.
class StrengthIndicator extends StatelessWidget {
  const StrengthIndicator({super.key, required this.strength});

  final PasswordStrength strength;

  @override
  Widget build(BuildContext context) {
    final colors = Theme.of(context).colorScheme;
    final (value, color, label) = switch (strength) {
      PasswordStrength.tooShort => (1 / 3, colors.error, 'Demasiado corta'),
      PasswordStrength.acceptable => (2 / 3, colors.tertiary, 'Aceptable'),
      PasswordStrength.strong => (1.0, colors.primary, 'Fuerte'),
    };
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        LinearProgressIndicator(value: value, color: color),
        const SizedBox(height: 4),
        Text(label, style: TextStyle(color: color)),
      ],
    );
  }
}
