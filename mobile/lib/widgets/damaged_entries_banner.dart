import 'package:flutter/material.dart';

/// Aviso de entradas que no se pudieron leer al desbloquear.
///
/// [count] es el total de dañadas del `UnlockReport` (con y sin id). Con 0,
/// o después de tocar "Entendido", no muestra nada.
///
/// Va dentro del árbol de la pantalla, no con
/// `ScaffoldMessenger.showMaterialBanner`: ese método lo muestra en todos los
/// `Scaffold` registrados y podría seguir visible en la pantalla de
/// desbloqueo después de bloquear.
///
/// El estado de oculto vive en este widget. Es correcto porque entre dos
/// desbloqueos siempre se pasa por el bloqueo y el aviso sale del árbol, así
/// que cada desbloqueo lo muestra una vez. La pantalla principal (#40) debe
/// mantenerlo en una posición que salga del árbol al bloquear; en un
/// ancestro que sobreviva al bloqueo, el estado de oculto se arrastraría al
/// siguiente desbloqueo.
class DamagedEntriesBanner extends StatefulWidget {
  const DamagedEntriesBanner({super.key, required this.count});

  final int count;

  @override
  State<DamagedEntriesBanner> createState() => _DamagedEntriesBannerState();
}

class _DamagedEntriesBannerState extends State<DamagedEntriesBanner> {
  bool _dismissed = false;

  @override
  Widget build(BuildContext context) {
    if (_dismissed || widget.count == 0) return const SizedBox.shrink();
    final message = widget.count == 1
        ? '1 entrada no se pudo leer'
        : '${widget.count} entradas no se pudieron leer';
    return MaterialBanner(
      content: Text(message),
      leading: const Icon(Icons.warning_amber),
      actions: [
        TextButton(
          onPressed: () => setState(() => _dismissed = true),
          child: const Text('Entendido'),
        ),
      ],
    );
  }
}
