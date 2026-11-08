import 'package:flutter/material.dart';

import 'screens/first_run_screen.dart';
import 'screens/placeholder_screen.dart';
import 'src/rust/api/session.dart';
import 'widgets/strength_indicator.dart';

/// La app: un `MaterialApp` cuyo inicio es [VaultGate].
///
/// Recibe sus dependencias para que las pruebas de widgets las reemplacen
/// sin cargar la librería de Rust.
class BovedaApp extends StatelessWidget {
  const BovedaApp({
    super.key,
    required this.session,
    required this.vaultExists,
    required this.strengthOf,
  });

  final VaultSession session;
  final Future<bool> Function() vaultExists;
  final StrengthOf strengthOf;

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Bóveda',
      home: VaultGate(
        session: session,
        vaultExists: vaultExists,
        strengthOf: strengthOf,
      ),
    );
  }
}

/// Estado de la bóveda que decide la pantalla.
enum VaultGateState { checking, noVault, locked, unlocked, openFailed }

/// Widget raíz: elige la pantalla según el estado de la sesión, sin pila de
/// rutas. Al cambiar de estado, la pantalla anterior sale del árbol y su
/// `dispose` limpia lo que tenga. Las issues siguientes agregan transiciones
/// (desbloquear en la #37, bloquear en la #39).
///
/// Guarda la sesión, que vive mientras vive la app; nunca una contraseña.
class VaultGate extends StatefulWidget {
  const VaultGate({
    super.key,
    required this.session,
    required this.vaultExists,
    required this.strengthOf,
  });

  final VaultSession session;
  final Future<bool> Function() vaultExists;
  final StrengthOf strengthOf;

  @override
  State<VaultGate> createState() => _VaultGateState();
}

class _VaultGateState extends State<VaultGate> {
  VaultGateState _state = VaultGateState.checking;

  @override
  void initState() {
    super.initState();
    _start();
  }

  /// Sin archivo, primer uso. Con archivo, se abre (queda bloqueada). Si
  /// algo falla, un mensaje genérico y nunca el primer uso: crear encima de
  /// un archivo existente daría `AlreadyExists`.
  Future<void> _start() async {
    VaultGateState next;
    try {
      if (await widget.vaultExists()) {
        await widget.session.open();
        next = VaultGateState.locked;
      } else {
        next = VaultGateState.noVault;
      }
    } catch (_) {
      next = VaultGateState.openFailed;
    }
    if (!mounted) return;
    setState(() => _state = next);
  }

  /// `create` encontró el archivo ya creado: se vuelve a comprobar desde
  /// el principio, que lo abre (desbloqueo) o muestra el error genérico.
  void _reevaluate() {
    setState(() => _state = VaultGateState.checking);
    _start();
  }

  @override
  Widget build(BuildContext context) {
    return switch (_state) {
      VaultGateState.checking => const Scaffold(
        body: Center(child: CircularProgressIndicator()),
      ),
      VaultGateState.noVault => FirstRunScreen(
        session: widget.session,
        strengthOf: widget.strengthOf,
        onCreated: () => setState(() => _state = VaultGateState.unlocked),
        onAlreadyExists: _reevaluate,
      ),
      VaultGateState.locked => const PlaceholderScreen(text: 'Desbloqueo: #37'),
      VaultGateState.unlocked => const PlaceholderScreen(
        text: 'Pantalla principal: #40',
      ),
      VaultGateState.openFailed => const PlaceholderScreen(
        text: 'No se pudo abrir la bóveda.',
      ),
    };
  }
}
