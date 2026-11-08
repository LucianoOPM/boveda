// Pruebas de widgets. Corren en la PC sin la librería nativa de Rust:
// el puente se reemplaza con una implementación simulada de RustLibApi.
// Las pruebas que sí cargan Rust van en integration_test/ y corren en el dispositivo.

import 'package:flutter_test/flutter_test.dart';
import 'package:mocktail/mocktail.dart';

import 'package:boveda/main.dart';
import 'package:boveda/src/rust/frb_generated.dart';

class MockRustLibApi extends Mock implements RustLibApi {}

void main() {
  setUpAll(() => RustLib.initMock(api: MockRustLibApi()));

  testWidgets('muestra el marcador', (tester) async {
    await tester.pumpWidget(const BovedaApp());
    expect(find.text('Bóveda'), findsOneWidget);
  });
}
