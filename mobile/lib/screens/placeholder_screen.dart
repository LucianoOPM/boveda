import 'package:flutter/material.dart';

/// Pantalla mínima con un texto centrado: el error al abrir la bóveda y el
/// de arranque.
class PlaceholderScreen extends StatelessWidget {
  const PlaceholderScreen({super.key, required this.text});

  final String text;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: SafeArea(child: Center(child: Text(text))),
    );
  }
}
