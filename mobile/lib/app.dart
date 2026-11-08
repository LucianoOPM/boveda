import 'package:flutter/material.dart';

import 'screens/biometric_offer_screen.dart';
import 'screens/first_run_screen.dart';
import 'screens/placeholder_screen.dart';
import 'screens/unlock_screen.dart';
import 'services/biometric_service.dart';
import 'src/rust/api/session.dart';
import 'widgets/damaged_entries_banner.dart';
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
    required this.biometric,
  });

  final VaultSession session;
  final Future<bool> Function() vaultExists;
  final StrengthOf strengthOf;
  final BiometricService biometric;

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Bóveda',
      home: VaultGate(
        session: session,
        vaultExists: vaultExists,
        strengthOf: strengthOf,
        biometric: biometric,
      ),
    );
  }
}

/// Estado de la bóveda que decide la pantalla.
enum VaultGateState {
  checking,
  noVault,
  offerBiometric,
  locked,
  unlocked,
  openFailed,
}

/// Widget raíz: elige la pantalla según el estado de la sesión, sin pila de
/// rutas. Al cambiar de estado, la pantalla anterior sale del árbol y su
/// `dispose` limpia lo que tenga. Transiciones: crear (primer uso) lleva a
/// la oferta de la huella si el dispositivo la admite y no está activa, y
/// de ahí (o directo) a `unlocked`; desbloquear (#37, #38) lleva a
/// `unlocked`; bloquear llega en la #39.
///
/// Guarda la sesión, que vive mientras vive la app, y el total de entradas
/// dañadas del último desbloqueo; nunca una contraseña.
class VaultGate extends StatefulWidget {
  const VaultGate({
    super.key,
    required this.session,
    required this.vaultExists,
    required this.strengthOf,
    required this.biometric,
  });

  final VaultSession session;
  final Future<bool> Function() vaultExists;
  final StrengthOf strengthOf;
  final BiometricService biometric;

  @override
  State<VaultGate> createState() => _VaultGateState();
}

class _VaultGateState extends State<VaultGate> {
  VaultGateState _state = VaultGateState.checking;

  /// Entradas dañadas del último desbloqueo, para el aviso de la pantalla
  /// principal. 0 al crear la bóveda.
  int _damagedEntries = 0;

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

  /// Después de crear la bóveda: ofrecer la huella si el dispositivo la
  /// admite y no está activa. Si la comprobación falla, no se ofrece.
  Future<void> _afterCreate() async {
    bool offer;
    try {
      offer =
          await widget.biometric.isAvailable() &&
          !await widget.biometric.isEnabled();
    } catch (_) {
      offer = false;
    }
    if (!mounted) return;
    if (offer) {
      setState(() => _state = VaultGateState.offerBiometric);
    } else {
      _unlocked(0);
    }
  }

  void _unlocked(int damagedEntries) {
    setState(() {
      _damagedEntries = damagedEntries;
      _state = VaultGateState.unlocked;
    });
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
        biometric: widget.biometric,
        onCreated: _afterCreate,
        onAlreadyExists: _reevaluate,
      ),
      VaultGateState.offerBiometric => BiometricOfferScreen(
        session: widget.session,
        biometric: widget.biometric,
        onDone: () => _unlocked(0),
      ),
      VaultGateState.locked => UnlockScreen(
        session: widget.session,
        biometric: widget.biometric,
        onUnlocked: _unlocked,
      ),
      VaultGateState.unlocked => PlaceholderScreen(
        text: 'Pantalla principal: #40',
        banner: DamagedEntriesBanner(count: _damagedEntries),
      ),
      VaultGateState.openFailed => const PlaceholderScreen(
        text: 'No se pudo abrir la bóveda.',
      ),
    };
  }
}

/// Arranque: carga la librería de Rust, obtiene la ruta de la bóveda y arma
/// la app con [appFor]. Si cualquiera de los pasos falla, devuelve
/// [StartupErrorApp] en lugar de dejar la pantalla en blanco.
Future<Widget> bootstrap({
  required Future<void> Function() initRust,
  required Future<String> Function() vaultPath,
  required Widget Function(String path) appFor,
}) async {
  try {
    await initRust();
    return appFor(await vaultPath());
  } catch (_) {
    // Sin detalles: el error puede traer rutas o datos internos.
    return const StartupErrorApp();
  }
}

/// Pantalla mínima cuando la app no pudo iniciar.
class StartupErrorApp extends StatelessWidget {
  const StartupErrorApp({super.key});

  @override
  Widget build(BuildContext context) {
    return const MaterialApp(
      title: 'Bóveda',
      home: PlaceholderScreen(text: 'No se pudo iniciar la app'),
    );
  }
}
