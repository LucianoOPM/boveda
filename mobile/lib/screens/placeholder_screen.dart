import 'package:flutter/material.dart';

/// Marcador de una pantalla que llega en otra issue. Solo muestra un texto.
class PlaceholderScreen extends StatelessWidget {
  const PlaceholderScreen({super.key, required this.text});

  final String text;

  @override
  Widget build(BuildContext context) {
    return Scaffold(body: Center(child: Text(text)));
  }
}
