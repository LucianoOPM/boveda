import 'package:flutter/widgets.dart';

/// Escala del texto del sistema por encima de la cual una fila se apila.
///
/// Hasta un 110 % el encabezado y el aviso de entradas dañadas caben en una
/// fila; con más, sus elementos se colocan en líneas propias para que ningún
/// texto se parta en una columna estrecha (`docs/DESIGN.md`, 12).
const largeTextScale = 1.1;

/// Si el texto del sistema es más grande que [largeTextScale].
bool isLargeText(BuildContext context) =>
    MediaQuery.textScalerOf(context).scale(10) / 10 > largeTextScale;
