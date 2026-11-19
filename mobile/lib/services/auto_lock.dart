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
  /// [now] es el reloj con el que `VaultGate` mide el tope de
  /// [maxAwayDuringOperation]; las pruebas lo reemplazan.
  AutoLockController({this.now = DateTime.now});

  /// Cuánto puede estar la app fuera mientras corre una operación sin
  /// bloqueo (#44). Superado ese tiempo, bloquea al volver.
  static const maxAwayDuringOperation = Duration(minutes: 2);

  final DateTime Function() now;

  /// Lo que hace un reporte de actividad; `null` si nadie escucha.
  VoidCallback? onActivity;

  bool _suspended = false;

  /// Hay una operación en curso que suspende el bloqueo automático.
  bool get isSuspended => _suspended;

  /// Reinicia la cuenta de inactividad, si `VaultGate` la lleva.
  void reportActivity() => onActivity?.call();

  /// Ejecuta [operation] sin que ocultar la app bloquee la bóveda ni corra
  /// el temporizador de inactividad: para el selector de archivos y otras
  /// actividades externas (#44). Al terminar, también con una excepción, la
  /// cuenta de inactividad se reinicia.
  ///
  /// No se anida: con una operación en curso lanza [StateError] de
  /// inmediato.
  Future<T> runWithoutAutoLock<T>(Future<T> Function() operation) {
    if (_suspended) {
      throw StateError('Ya hay una operación sin bloqueo automático en curso');
    }
    _suspended = true;
    // `VaultGate` cancela el temporizador al ver la marca.
    reportActivity();
    return _run(operation);
  }

  Future<T> _run<T>(Future<T> Function() operation) async {
    try {
      return await operation();
    } finally {
      _suspended = false;
      reportActivity();
    }
  }
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

  /// Ver [AutoLockController.runWithoutAutoLock].
  Future<T> runWithoutAutoLock<T>(Future<T> Function() operation) =>
      controller.runWithoutAutoLock(operation);

  /// Ejecuta [operation] sin bloqueo automático si hay un alcance; sin él
  /// (pruebas de pantallas sueltas), la ejecuta tal cual.
  static Future<T> run<T>(
    BuildContext context,
    Future<T> Function() operation,
  ) {
    final scope = maybeOf(context);
    return scope == null ? operation() : scope.runWithoutAutoLock(operation);
  }

  /// Nunca notifica: reportar actividad no reconstruye nada.
  @override
  bool updateShouldNotify(AutoLockScope oldWidget) => false;
}
