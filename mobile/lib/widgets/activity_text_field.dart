import 'package:flutter/material.dart';

import '../services/auto_lock.dart';

/// `TextField` que cuenta como actividad para el bloqueo automático.
///
/// El teclado en pantalla no genera eventos que Flutter vea de forma global:
/// sin este campo, escribir varios minutos sin tocar la pantalla bloquearía
/// la bóveda a mitad de la edición. En las pantallas de la bóveda, todo
/// campo de texto es este o `PasswordField`, que se arma sobre él.
///
/// Tocar fuera de cualquier campo cierra el teclado: en Android, Flutter no
/// quita el foco por omisión con un toque de dedo (#40). Los íconos de la
/// decoración (el ojo de `PasswordField`, el ✕ de la búsqueda) ya están
/// dentro de la región del campo; los controles del formulario que no deben
/// cerrarlo (botón de envío, casillas) se envuelven en `TextFieldTapRegion`,
/// que comparte el grupo de todos los campos.
///
/// Los demás parámetros se pasan tal cual al `TextField`, con sus mismos
/// valores por omisión. Fuera de un `AutoLockScope`, no reporta nada.
class ActivityTextField extends StatelessWidget {
  const ActivityTextField({
    super.key,
    required this.controller,
    this.enabled = true,
    this.onChanged,
    this.textInputAction,
    this.onSubmitted,
    this.onEditingComplete,
    this.focusNode,
    this.obscureText = false,
    this.enableSuggestions = true,
    this.autocorrect = true,
    this.enableIMEPersonalizedLearning = true,
    this.keyboardType,
    this.maxLines = 1,
    this.decoration = const InputDecoration(),
  });

  final TextEditingController controller;
  final bool enabled;
  final ValueChanged<String>? onChanged;
  final TextInputAction? textInputAction;
  final ValueChanged<String>? onSubmitted;
  final VoidCallback? onEditingComplete;
  final FocusNode? focusNode;
  final bool obscureText;
  final bool enableSuggestions;
  final bool autocorrect;
  final bool enableIMEPersonalizedLearning;
  final TextInputType? keyboardType;
  final int? maxLines;
  final InputDecoration? decoration;

  @override
  Widget build(BuildContext context) {
    return TextField(
      controller: controller,
      enabled: enabled,
      onChanged: (value) {
        AutoLockScope.maybeOf(context)?.reportActivity();
        onChanged?.call(value);
      },
      textInputAction: textInputAction,
      onSubmitted: onSubmitted,
      onEditingComplete: onEditingComplete,
      // Solo se llama con el campo enfocado: `primaryFocus` es este campo.
      onTapOutside: (_) => FocusManager.instance.primaryFocus?.unfocus(),
      focusNode: focusNode,
      obscureText: obscureText,
      enableSuggestions: enableSuggestions,
      autocorrect: autocorrect,
      enableIMEPersonalizedLearning: enableIMEPersonalizedLearning,
      keyboardType: keyboardType,
      maxLines: maxLines,
      decoration: decoration,
    );
  }
}
