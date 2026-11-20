// Pruebas del umbral de "letra grande" que usan el encabezado y el aviso de
// entradas dañadas para apilar sus elementos.

import 'package:boveda/theme/text_scale.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

Future<bool> largeAt(WidgetTester tester, double scale) async {
  late bool result;
  await tester.pumpWidget(
    MediaQuery(
      data: MediaQueryData(textScaler: TextScaler.linear(scale)),
      child: Builder(
        builder: (context) {
          result = isLargeText(context);
          return const SizedBox();
        },
      ),
    ),
  );
  return result;
}

void main() {
  test('el umbral es 110 %', () {
    expect(largeTextScale, 1.1);
  });

  testWidgets('hasta el 110 % no es letra grande', (tester) async {
    expect(await largeAt(tester, 1), isFalse);
    expect(await largeAt(tester, 1.1), isFalse);
  });

  testWidgets('por encima del 110 % es letra grande', (tester) async {
    expect(await largeAt(tester, 1.11), isTrue);
    expect(await largeAt(tester, 2), isTrue);
  });
}
