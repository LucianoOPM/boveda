import 'dart:io';

import 'package:flutter/material.dart';

import 'app.dart';
import 'services/vault_location.dart';
import 'src/rust/api/password.dart';
import 'src/rust/api/session.dart';
import 'src/rust/frb_generated.dart';

/// Arma las dependencias reales y arranca la app.
Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await RustLib.init();
  final path = await vaultPath();
  runApp(
    BovedaApp(
      session: VaultSession(path: path),
      vaultExists: () => File(path).exists(),
      strengthOf: (password) => passwordStrength(password: password),
    ),
  );
}
