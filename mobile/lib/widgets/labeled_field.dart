import 'package:flutter/material.dart';

import '../theme/spacing.dart';
import 'field_label.dart';

/// Un campo con su etiqueta encima (`docs/DESIGN.md`, 6.4 y 7.6): [label] en
/// MAYÚSCULAS, en `meta`, y [child] debajo.
///
/// - **Lector de pantalla:** la etiqueta visible va en mayúsculas y se excluye
///   de la semántica; el campo anuncia [label] con su texto original.
/// - **Foco:** tocar la etiqueta enfoca el campo, como hacía `labelText`. No
///   obliga a pasarle un `FocusNode` a [child]: un `Focus` transparente
///   alrededor (sin tomar el foco ni entrar al recorrido) llega al nodo del
///   `TextField`. Con [enabled] en `false` la etiqueta se atenúa y no enfoca.
///
/// Es solo presentación: no cambia lo que hace [child].
class LabeledField extends StatefulWidget {
  const LabeledField({
    super.key,
    required this.label,
    this.enabled = true,
    required this.child,
  });

  final String label;
  final bool enabled;
  final Widget child;

  @override
  State<LabeledField> createState() => _LabeledFieldState();
}

class _LabeledFieldState extends State<LabeledField> {
  /// Nodo transparente alrededor del campo: no toma el foco ni entra al
  /// recorrido, solo deja a la etiqueta llegar al nodo del `TextField` sin
  /// obligar a pasarle uno (el campo conserva su `focusNode` por omisión).
  final _wrapper = FocusNode(
    debugLabel: 'LabeledField',
    canRequestFocus: false,
    skipTraversal: true,
  );

  /// Enfoca el campo. Con el campo deshabilitado no hay nodo que pueda tomar
  /// el foco: no hace nada.
  void _focusField() =>
      _wrapper.traversalDescendants.firstOrNull?.requestFocus();

  @override
  void dispose() {
    _wrapper.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        ExcludeSemantics(
          // La etiqueta es parte del grupo del campo: tocarla no cuenta como
          // "tocar fuera" (`onTapOutside` quita el foco y cierra el teclado),
          // así que pasar de un campo a otro por su etiqueta no hace parpadear
          // el teclado.
          child: TextFieldTapRegion(
            child: GestureDetector(
              behavior: HitTestBehavior.opaque,
              onTap: widget.enabled ? _focusField : null,
              child: FieldLabel(widget.label, enabled: widget.enabled),
            ),
          ),
        ),
        const SizedBox(height: Space.sm),
        Semantics(
          label: widget.label,
          child: Focus(focusNode: _wrapper, child: widget.child),
        ),
      ],
    );
  }
}
