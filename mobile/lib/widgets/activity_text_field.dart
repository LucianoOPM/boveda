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
    this.textCapitalization = TextCapitalization.none,
    this.minLines,
    this.maxLines = 1,
    this.decoration = const InputDecoration(),
    this.copyable = true,
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
  final TextCapitalization textCapitalization;
  final int? minLines;
  final int? maxLines;
  final InputDecoration? decoration;

  /// Con `false`, lo escrito no se puede copiar ni cortar: ni con el menú
  /// de selección, que solo ofrece pegar y seleccionar todo (tampoco
  /// compartir ni las acciones de otras apps), ni con el atajo de un teclado
  /// físico. Es para los campos que muestran un secreto: copiarlo es una
  /// acción aparte, que marca el contenido como sensible. Pegar sigue
  /// funcionando.
  final bool copyable;

  /// Menú de selección sin ninguna acción que saque el texto del campo.
  static Widget _pasteOnlyMenu(BuildContext context, EditableTextState state) {
    return AdaptiveTextSelectionToolbar.buttonItems(
      anchors: state.contextMenuAnchors,
      buttonItems: [
        for (final item in state.contextMenuButtonItems)
          if (item.type == ContextMenuButtonType.paste ||
              item.type == ContextMenuButtonType.selectAll)
            item,
      ],
    );
  }

  /// El menú por omisión de `TextField`.
  static Widget _defaultMenu(BuildContext context, EditableTextState state) {
    return AdaptiveTextSelectionToolbar.editableText(editableTextState: state);
  }

  @override
  Widget build(BuildContext context) {
    final field = TextField(
      controller: controller,
      contextMenuBuilder: copyable ? _defaultMenu : _pasteOnlyMenu,
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
      textCapitalization: textCapitalization,
      minLines: minLines,
      maxLines: maxLines,
      decoration: decoration,
    );
    if (copyable) return field;
    // Copiar y cortar comparten esta intención: sin ella, Ctrl+C y Ctrl+X
    // de un teclado físico no hacen nada.
    return Actions(
      actions: {CopySelectionTextIntent: DoNothingAction(consumesKey: false)},
      child: field,
    );
  }
}
