import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/scheduler.dart';
import 'package:flutter/services.dart';

import 'screens/biometric_offer_screen.dart';
import 'screens/first_run_screen.dart';
import 'screens/placeholder_screen.dart';
import 'screens/search_screen.dart';
import 'screens/unlock_screen.dart';
import 'services/auto_lock.dart';
import 'services/biometric_service.dart';
import 'src/rust/api/session.dart';
import 'widgets/strength_indicator.dart';

/// La app: un `MaterialApp` cuyo inicio es [VaultGate].
///
/// Recibe sus dependencias para que las pruebas de widgets las reemplacen
/// sin cargar la librería de Rust. Crea el [AutoLockController] que
/// comparten `MaterialApp.builder` (por encima del `Navigator`) y
/// [VaultGate] (dentro de él).
class BovedaApp extends StatefulWidget {
  const BovedaApp({
    super.key,
    required this.session,
    required this.vaultExists,
    required this.strengthOf,
    required this.biometric,
    this.autoLockTimeout = defaultAutoLockTimeout,
  });

  final VaultSession session;
  final Future<bool> Function() vaultExists;
  final StrengthOf strengthOf;
  final BiometricService biometric;

  /// Inactividad tras la que la bóveda se bloquea sola.
  final Duration autoLockTimeout;

  @override
  State<BovedaApp> createState() => _BovedaAppState();
}

class _BovedaAppState extends State<BovedaApp> {
  final _autoLock = AutoLockController();

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Bóveda',
      // Por encima del `Navigator`: cubre rutas, diálogos y hojas. Cualquier
      // toque cuenta como actividad; el `Listener` translúcido no lo consume.
      builder: (context, child) => AutoLockScope(
        controller: _autoLock,
        child: Listener(
          behavior: HitTestBehavior.translucent,
          onPointerDown: (_) => _autoLock.reportActivity(),
          child: child,
        ),
      ),
      home: VaultGate(
        session: widget.session,
        vaultExists: widget.vaultExists,
        strengthOf: widget.strengthOf,
        biometric: widget.biometric,
        autoLock: _autoLock,
        autoLockTimeout: widget.autoLockTimeout,
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
/// `unlocked`; bloquear (#39) lleva a `locked`.
///
/// Bloqueo automático (#39, detalle en
/// `docs/superpowers/specs/2026-11-08-bloqueo-automatico-design.md`):
///
/// - Al ocultarse la app (`hidden`) con la bóveda en pantalla, o durante la
///   activación de la huella con la casilla, bloquea. Con un desbloqueo en
///   curso no llama a `lock()`: esperaría el `Mutex` durante Argon2id y
///   desfasaría Rust y la interfaz.
/// - Toda transición a un estado con Rust desbloqueado pasa por
///   `_enterUnlocked`, que bloquea si la app está oculta.
/// - Con la bóveda en pantalla (`unlocked`, `offerBiometric`), bloquea tras
///   `autoLockTimeout` sin actividad: toques, teclas físicas y lo que se
///   escribe en un `ActivityTextField`.
/// - Cada bloqueo sube `_epoch`, la `key` de la pantalla: toda pantalla
///   montada sale del árbol y sus `await` pendientes encuentran
///   `mounted == false`. Así, el texto de búsqueda de [SearchScreen] (la
///   pantalla del estado `unlocked`, #40) no pasa al siguiente desbloqueo.
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
    required this.autoLock,
    required this.autoLockTimeout,
  });

  final VaultSession session;
  final Future<bool> Function() vaultExists;
  final StrengthOf strengthOf;
  final BiometricService biometric;
  final AutoLockController autoLock;
  final Duration autoLockTimeout;

  @override
  State<VaultGate> createState() => _VaultGateState();
}

class _VaultGateState extends State<VaultGate> {
  VaultGateState _state = VaultGateState.checking;

  /// Entradas dañadas del último desbloqueo, para el aviso de la pantalla
  /// principal. 0 al crear la bóveda.
  int _damagedEntries = 0;

  /// `key` de la pantalla de cada estado; sube con cada bloqueo.
  int _epoch = 0;

  /// Rust ya está desbloqueado aunque la interfaz siga en `locked`: la
  /// pantalla de desbloqueo está activando la huella con la casilla.
  bool _openWhileLocked = false;

  late final AppLifecycleListener _lifecycle;

  /// Temporizador de inactividad; solo corre con la bóveda en pantalla.
  Timer? _idle;

  @override
  void initState() {
    super.initState();
    _lifecycle = AppLifecycleListener(onHide: _onHidden);
    widget.autoLock.onActivity = _restartIdle;
    HardwareKeyboard.instance.addHandler(_onKey);
    _start();
  }

  @override
  void dispose() {
    _idle?.cancel();
    HardwareKeyboard.instance.removeHandler(_onKey);
    if (widget.autoLock.onActivity == _restartIdle) {
      widget.autoLock.onActivity = null;
    }
    _lifecycle.dispose();
    super.dispose();
  }

  /// Una tecla física cuenta como actividad. No la consume.
  bool _onKey(KeyEvent event) {
    _restartIdle();
    return false;
  }

  /// Reinicia la cuenta de inactividad con la bóveda en pantalla; en otro
  /// estado solo cancela el temporizador.
  void _restartIdle() {
    _idle?.cancel();
    _idle = null;
    if (_state == VaultGateState.unlocked ||
        _state == VaultGateState.offerBiometric) {
      _idle = Timer(widget.autoLockTimeout, _lock);
    }
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
    _enterUnlocked(
      offer ? VaultGateState.offerBiometric : VaultGateState.unlocked,
    );
  }

  /// La app no está a la vista. `null` (sin estado todavía) e `inactive`
  /// (diálogos del sistema, cortina, recientes) cuentan como presentes.
  bool get _isAway => switch (SchedulerBinding.instance.lifecycleState) {
    AppLifecycleState.hidden ||
    AppLifecycleState.paused ||
    AppLifecycleState.detached => true,
    _ => false,
  };

  /// Única entrada a un estado con Rust desbloqueado. Si la operación que
  /// desbloqueó terminó con la app oculta, bloquea en lugar de mostrar la
  /// bóveda.
  void _enterUnlocked(VaultGateState next, {int damagedEntries = 0}) {
    _openWhileLocked = false;
    if (_isAway) {
      _lock();
      return;
    }
    setState(() {
      _damagedEntries = damagedEntries;
      _state = next;
    });
    _restartIdle();
  }

  void _onHidden() {
    final shouldLock = switch (_state) {
      VaultGateState.unlocked || VaultGateState.offerBiometric => true,
      VaultGateState.locked => _openWhileLocked,
      _ => false,
    };
    if (shouldLock) _lock();
  }

  /// Bloquea: borra la llave en Rust, cierra todo lo apilado sobre la raíz
  /// (rutas, diálogos, hojas) y los `SnackBar`, y monta un desbloqueo nuevo.
  ///
  /// Si `lock()` falla, igual se muestra el desbloqueo; la fachada bloquea
  /// aunque el `Mutex` esté envenenado, así que en la práctica no falla. El
  /// error se descarta sin detalles.
  void _lock() {
    if (!mounted) return;
    _idle?.cancel();
    _idle = null;
    _openWhileLocked = false;
    unawaited(
      Future.sync(widget.session.lock).then((_) {}, onError: (Object _) {}),
    );
    _removeStackedRoutes(Navigator.of(context));
    ScaffoldMessenger.of(context)
      ..clearSnackBars()
      ..removeCurrentSnackBar();
    setState(() {
      _epoch++;
      _state = VaultGateState.locked;
    });
  }

  /// Quita, sin animación, todo lo apilado sobre la primera ruta. `popUntil`
  /// animaría la salida: con la app oculta no hay cuadros, así que al volver
  /// la pantalla de detalle (o un diálogo) se vería unos 300 ms sobre el
  /// desbloqueo. `removeRoute` la quita y la desecha en el acto.
  void _removeStackedRoutes(NavigatorState navigator) {
    while (true) {
      Route<dynamic>? top;
      // Con un predicado que acepta la primera ruta que ve, `popUntil` no
      // quita nada: solo da la ruta de arriba.
      navigator.popUntil((route) {
        top = route;
        return true;
      });
      final route = top;
      if (route == null || route.isFirst) return;
      navigator.removeRoute(route);
    }
  }

  @override
  Widget build(BuildContext context) {
    final Widget screen = switch (_state) {
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
        onDone: () => _enterUnlocked(VaultGateState.unlocked),
      ),
      VaultGateState.locked => UnlockScreen(
        session: widget.session,
        biometric: widget.biometric,
        onVaultOpened: () => _openWhileLocked = true,
        onUnlocked: (damaged) =>
            _enterUnlocked(VaultGateState.unlocked, damagedEntries: damaged),
      ),
      VaultGateState.unlocked => SearchScreen(
        session: widget.session,
        damagedEntries: _damagedEntries,
      ),
      VaultGateState.openFailed => const PlaceholderScreen(
        text: 'No se pudo abrir la bóveda.',
      ),
    };
    return KeyedSubtree(key: ValueKey(_epoch), child: screen);
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
