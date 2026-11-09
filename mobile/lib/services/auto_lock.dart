import 'package:flutter/widgets.dart';

/// Tiempo de inactividad tras el que la bóveda se bloquea sola.
///
/// Es el valor por omisión hasta la etapa 4, que agrega el ajuste del tiempo
/// de bloqueo: este es el único lugar del valor, para que el ajuste solo
/// tenga que reemplazarlo.
const defaultAutoLockTimeout = Duration(minutes: 5);

/// Conecta los reportes de actividad de la interfaz con `VaultGate`.
///
/// `BovedaApp` lo crea y lo comparte entre `MaterialApp.builder` (que está
/// por encima del `Navigator`) y `VaultGate` (que está dentro), sin
/// `GlobalKey`. `VaultGate` asigna [onActivity] al montarse y lo borra al
/// desmontarse.
class AutoLockController {
  /// Lo que hace un reporte de actividad; `null` si nadie escucha.
  VoidCallback? onActivity;

  /// Reinicia la cuenta de inactividad, si `VaultGate` la lleva.
  void reportActivity() => onActivity?.call();
}

/// Da acceso al [AutoLockController] a cualquier widget bajo `MaterialApp`.
///
/// Los campos de texto de las pantallas de la bóveda lo usan para reportar
/// lo que se escribe con el teclado en pantalla, que no genera eventos
/// globales en Flutter (ver `ActivityTextField`).
class AutoLockScope extends InheritedWidget {
  const AutoLockScope({
    super.key,
    required this.controller,
    required super.child,
  });

  final AutoLockController controller;

  /// El alcance más cercano, o `null` fuera de él (pruebas de pantallas
  /// sueltas, la pantalla de error de arranque). No registra dependencia:
  /// el alcance nunca cambia de forma observable.
  static AutoLockScope? maybeOf(BuildContext context) =>
      context.getInheritedWidgetOfExactType<AutoLockScope>();

  void reportActivity() => controller.reportActivity();

  /// Nunca notifica: reportar actividad no reconstruye nada.
  @override
  bool updateShouldNotify(AutoLockScope oldWidget) => false;
}
