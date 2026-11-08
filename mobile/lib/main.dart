import 'dart:io';

import 'package:flutter/material.dart';

import 'app.dart';
import 'services/vault_location.dart';
import 'src/rust/api/password.dart';
import 'src/rust/api/session.dart';
import 'src/rust/frb_generated.dart';

/// Arma las dependencias reales y arranca la app. Si la librería de Rust o
/// la ruta de la bóveda fallan, muestra un mensaje genérico.
Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  runApp(
    await bootstrap(
      initRust: RustLib.init,
      vaultPath: vaultPath,
      appFor: (path) => BovedaApp(
        session: VaultSession(path: path),
        vaultExists: () => File(path).exists(),
        strengthOf: (password) => passwordStrength(password: password),
      ),
    ),
  );
}
