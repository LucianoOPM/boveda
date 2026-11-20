import 'package:flutter/material.dart';

import '../services/biometric_service.dart';
import '../services/biometric_unlock.dart';
import '../src/rust/api/session.dart';
import '../theme/extensions.dart';
import '../theme/icons.dart';
import '../theme/spacing.dart';
import '../theme/theme.dart';
import '../widgets/primary_glow.dart';

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
    final accentColors =
        theme.extension<AccentColors>() ?? AccentColors.of(defaultAccent);
    // Contenido centrado y botones al pie; con texto grande la pantalla se
    // desplaza en vez de desbordar.
    return Scaffold(
      body: SafeArea(
        child: CustomScrollView(
          slivers: [
            SliverFillRemaining(
              hasScrollBody: false,
              child: Padding(
                padding: const EdgeInsets.all(Space.lg),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    const Spacer(),
                    Center(
                      child: ExcludeSemantics(
                        child: Container(
                          key: const Key('offer-icon'),
                          width: 64,
                          height: 64,
                          decoration: BoxDecoration(
                            color: accentColors.dim,
                            shape: BoxShape.circle,
                          ),
                          child: Icon(
                            BovedaIcons.fingerprint,
                            size: 32,
                            color: theme.colorScheme.primary,
                          ),
                        ),
                      ),
                    ),
                    const SizedBox(height: Space.lg),
                    Text(
                      '¿Activar huella?',
                      textAlign: TextAlign.center,
                      style: theme.textTheme.headlineSmall,
                    ),
                    const SizedBox(height: Space.sm),
                    Text(
                      'Desbloquea la bóveda con tu huella. La contraseña '
                      'maestra seguirá funcionando y la necesitarás si '
                      'registras una huella nueva.',
                      textAlign: TextAlign.center,
                      style: theme.textTheme.bodyMedium?.copyWith(
                        color: theme.colorScheme.onSurfaceVariant,
                      ),
                    ),
                    if (_error case final error?) ...[
                      const SizedBox(height: Space.lg),
                      Row(
                        key: const Key('offer-error'),
                        mainAxisAlignment: MainAxisAlignment.center,
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Icon(
                            BovedaIcons.triangleAlert,
                            size: 16,
                            color: theme.colorScheme.error,
                          ),
                          const SizedBox(width: Space.sm),
                          Flexible(
                            child: Text(
                              error,
                              style: theme.textTheme.bodyMedium?.copyWith(
                                color: theme.colorScheme.error,
                              ),
                            ),
                          ),
                        ],
                      ),
                    ],
                    const Spacer(),
                    const SizedBox(height: Space.lg),
                    PrimaryGlow(
                      enabled: !_busy,
                      child: FilledButton(
                        key: const Key('offer-enable'),
                        onPressed: _busy ? null : _enable,
                        child: const Text('Activar'),
                      ),
                    ),
                    const SizedBox(height: Space.sm),
                    TextButton(
                      key: const Key('offer-skip'),
                      style: TextButton.styleFrom(
                        foregroundColor: theme.colorScheme.onSurfaceVariant,
                      ),
                      onPressed: _busy ? null : widget.onDone,
                      child: const Text('Ahora no'),
                    ),
                  ],
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
