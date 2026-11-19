import 'package:flutter/material.dart';

import '../theme/extensions.dart';
import '../theme/icons.dart';
import '../theme/spacing.dart';
import 'activity_text_field.dart';
import 'field_label.dart';

/// Campo para escribir una contraseña: la maestra o la de una entrada.
///
/// No aplica ninguna regla sobre lo escrito: el mínimo y el indicador de
/// fortaleza son solo de la contraseña maestra y los pone la pantalla que
/// los necesita.
///
/// Oculta el texto por omisión, con un botón para mostrarlo. Lo escrito no
/// se puede copiar ni cortar, tampoco cuando está a la vista; pegar sí. El teclado no
/// sugiere, no corrige ni aprende lo escrito, y el campo no declara pistas de
/// autocompletado para que el gestor de contraseñas del sistema no ofrezca
/// guardarla.
///
/// El controlador es de quien usa el campo: lo crea, lo limpia y lo libera.
/// Lo mismo vale para [focusNode], si se pasa.
///
/// [textInputAction], [onSubmitted] y [focusNode] son opcionales y se pasan
/// tal cual al `TextField`; sin ellos, el campo se comporta como el
/// `TextField` por omisión.
///
/// Se arma sobre [ActivityTextField]: escribir cuenta como actividad para el
/// bloqueo automático.
class PasswordField extends StatefulWidget {
  const PasswordField({
    super.key,
    required this.controller,
    required this.label,
    this.enabled = true,
    this.errorText,
    this.onChanged,
    this.textInputAction,
    this.onSubmitted,
    this.focusNode,
  });

  final TextEditingController controller;
  final String label;
  final bool enabled;
  final String? errorText;
  final ValueChanged<String>? onChanged;
  final TextInputAction? textInputAction;
  final ValueChanged<String>? onSubmitted;
  final FocusNode? focusNode;

  @override
  State<PasswordField> createState() => _PasswordFieldState();
}

class _PasswordFieldState extends State<PasswordField> {
  bool _obscured = true;

  @override
  Widget build(BuildContext context) {
    final secret =
        (Theme.of(context).extension<BovedaText>() ?? BovedaText.standard)
            .secret;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        // La etiqueta visible va en mayúsculas; el lector de pantalla lee la
        // original desde el campo.
        ExcludeSemantics(child: FieldLabel(widget.label)),
        const SizedBox(height: Space.sm),
        Semantics(
          label: widget.label,
          child: ActivityTextField(
            controller: widget.controller,
            enabled: widget.enabled,
            onChanged: widget.onChanged,
            textInputAction: widget.textInputAction,
            onSubmitted: widget.onSubmitted,
            focusNode: widget.focusNode,
            obscureText: _obscured,
            enableSuggestions: false,
            autocorrect: false,
            enableIMEPersonalizedLearning: false,
            keyboardType: TextInputType.visiblePassword,
            maxLines: 1,
            style: secret,
            // Con el ojo, la contraseña queda a la vista: no se copia desde
            // aquí.
            copyable: false,
            decoration: InputDecoration(
              errorText: widget.errorText,
              suffixIcon: IconButton(
                tooltip: _obscured
                    ? 'Mostrar contraseña'
                    : 'Ocultar contraseña',
                icon: Icon(
                  _obscured ? BovedaIcons.eye : BovedaIcons.eyeOff,
                  size: 20,
                ),
                onPressed: () => setState(() => _obscured = !_obscured),
              ),
            ),
          ),
        ),
      ],
    );
  }
}
