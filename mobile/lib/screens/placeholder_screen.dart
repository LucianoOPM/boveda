import 'package:flutter/material.dart';

/// Marcador de una pantalla que llega en otra issue. Muestra un texto y,
/// opcionalmente, un [banner] arriba (el aviso de entradas dañadas en el
/// marcador de la pantalla principal).
class PlaceholderScreen extends StatelessWidget {
  const PlaceholderScreen({super.key, required this.text, this.banner});

  final String text;
  final Widget? banner;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: SafeArea(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            ?banner,
            Expanded(child: Center(child: Text(text))),
          ],
        ),
      ),
    );
  }
}
