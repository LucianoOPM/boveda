import 'package:flutter/material.dart';

import '../services/biometric_service.dart';
import '../services/biometric_unlock.dart';
import '../src/rust/api/session.dart';

/// Oferta de activar la huella al terminar el primer uso, con la bóveda
/// recién creada y desbloqueada.
class BiometricOfferScreen extends StatefulWidget {
  const BiometricOfferScreen({
    super.key,
    required this.session,
    required this.biometric,
    required this.onDone,
  });

  final VaultSession session;
  final BiometricService biometric;

  /// Se llama al activar la huella o al tocar "Ahora no".
  final VoidCallback onDone;

  @override
  State<BiometricOfferScreen> createState() => _BiometricOfferScreenState();
}

class _BiometricOfferScreenState extends State<BiometricOfferScreen> {
  bool _busy = false;
  String? _error;

  Future<void> _enable() async {
    if (_busy) return;
    setState(() {
      _busy = true;
      _error = null;
    });
    final outcome = await enableBiometric(widget.session, widget.biometric);
    if (!mounted) return;
    switch (outcome) {
      case EnableOutcome.enabled:
        widget.onDone();
      case EnableOutcome.canceled:
        // Cancelar no es un error: se queda en la oferta, sin mensaje.
        setState(() => _busy = false);
      case EnableOutcome.failed:
        setState(() {
          _busy = false;
          _error = 'No se pudo activar la huella.';
        });
    }
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Scaffold(
      body: SafeArea(
        child: SingleChildScrollView(
          padding: const EdgeInsets.all(24),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Text('¿Activar huella?', style: theme.textTheme.headlineSmall),
              const SizedBox(height: 8),
              const Text(
                'Desbloquea la bóveda con tu huella. La contraseña maestra '
                'seguirá funcionando y la necesitarás si registras una huella '
                'nueva.',
              ),
              if (_error case final error?) ...[
                const SizedBox(height: 16),
                Text(error, style: TextStyle(color: theme.colorScheme.error)),
              ],
              const SizedBox(height: 24),
              FilledButton(
                key: const Key('offer-enable'),
                onPressed: _busy ? null : _enable,
                child: const Text('Activar'),
              ),
              const SizedBox(height: 8),
              TextButton(
                key: const Key('offer-skip'),
                onPressed: _busy ? null : widget.onDone,
                child: const Text('Ahora no'),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
