// Pruebas del arranque: si la librería de Rust o la ruta de la bóveda
// fallan, la app muestra un mensaje genérico en lugar de quedar en blanco.

import 'package:boveda/app.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  const app = SizedBox(key: Key('app'));

  test('con todo bien arma la app con la ruta', () async {
    final paths = <String>[];
    final result = await bootstrap(
      initRust: () async {},
      vaultPath: () async => '/datos/no_backup/boveda.db',
      appFor: (path) {
        paths.add(path);
        return app;
      },
    );
    expect(result, same(app));
    expect(paths, ['/datos/no_backup/boveda.db']);
  });

  test('si falla la librería de Rust no pide la ruta ni arma la app', () async {
    var asked = false;
    var built = false;
    final result = await bootstrap(
      initRust: () async => throw StateError('sin librería'),
      vaultPath: () async {
        asked = true;
        return '/x';
      },
      appFor: (_) {
        built = true;
        return app;
      },
    );
    expect(result, isA<StartupErrorApp>());
    expect(asked, isFalse);
    expect(built, isFalse);
  });

  test('si falla la ruta de la bóveda muestra el error', () async {
    final result = await bootstrap(
      initRust: () async {},
      vaultPath: () async => throw StateError('sin directorio'),
      appFor: (_) => app,
    );
    expect(result, isA<StartupErrorApp>());
  });

  test('si falla al armar la app muestra el error', () async {
    final result = await bootstrap(
      initRust: () async {},
      vaultPath: () async => '/x',
      appFor: (_) => throw StateError('sesión'),
    );
    expect(result, isA<StartupErrorApp>());
  });

  testWidgets('StartupErrorApp muestra solo el mensaje genérico', (
    tester,
  ) async {
    await tester.pumpWidget(const StartupErrorApp());
    expect(find.text('No se pudo iniciar la app'), findsOneWidget);
    expect(find.textContaining('StateError'), findsNothing);
  });
}
