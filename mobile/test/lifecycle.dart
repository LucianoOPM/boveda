// Ayudantes para simular el ciclo de vida de la app en las pruebas de widgets.

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';

/// Estados en el orden en que Android los recorre al salir de la app.
const _lifecycleChain = [
  AppLifecycleState.resumed,
  AppLifecycleState.inactive,
  AppLifecycleState.hidden,
  AppLifecycleState.paused,
  AppLifecycleState.detached,
];

/// Lleva el ciclo de vida a [target] paso a paso: en las pruebas el binding
/// no sintetiza los estados intermedios, y `AppLifecycleListener` rechaza
/// las transiciones inválidas. De `detached` se vuelve directo a `resumed`.
void _walkLifecycle(WidgetTester tester, AppLifecycleState target) {
  var current = tester.binding.lifecycleState ?? AppLifecycleState.resumed;
  while (current != target) {
    if (current == AppLifecycleState.detached) {
      current = AppLifecycleState.resumed;
    } else {
      final from = _lifecycleChain.indexOf(current);
      final to = _lifecycleChain.indexOf(target);
      current = _lifecycleChain[from + (to > from ? 1 : -1)];
    }
    tester.binding.handleAppLifecycleStateChanged(current);
  }
}

/// Cambia el ciclo de vida y lo restaura a `resumed` al terminar la prueba:
/// el estado del binding sobrevive entre pruebas.
Future<void> setLifecycle(WidgetTester tester, AppLifecycleState state) async {
  addTearDown(() => _walkLifecycle(tester, AppLifecycleState.resumed));
  _walkLifecycle(tester, state);
  await tester.pump();
}
