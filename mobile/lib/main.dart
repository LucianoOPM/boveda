import 'package:flutter/material.dart';
import 'package:boveda/src/rust/frb_generated.dart';

Future<void> main() async {
  await RustLib.init();
  runApp(const BovedaApp());
}

/// Marcador hasta que la #36 agregue el primer uso y la navegación.
class BovedaApp extends StatelessWidget {
  const BovedaApp({super.key});

  @override
  Widget build(BuildContext context) {
    return const MaterialApp(
      title: 'Bóveda',
      home: Scaffold(body: Center(child: Text('Bóveda'))),
    );
  }
}
