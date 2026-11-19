// Compila la librería de Rust de la app con el toolchain de
// `rust/rust-toolchain.toml` (#48). Reemplaza a cargokit.
import 'dart:io';

import 'package:code_assets/code_assets.dart';
import 'package:flutter_rust_bridge_hooks/flutter_rust_bridge_hooks.dart';

void main(List<String> args) async {
  await build(args, (input, output) async {
    // La app es solo de Android. Flutter también ejecuta los hooks al correr
    // `flutter test` en la PC (destino Windows o Linux), y ahí no hace falta
    // la librería: las pruebas de widgets usan el puente simulado. Compilar el
    // crate en release para la PC en cada prueba sería lento y exigiría
    // listar los targets de la PC en rust/rust-toolchain.toml.
    //
    // Condición explícita: si el destino es Android, compila; si no, no hace
    // nada, y lo deja escrito en el log para que no parezca un fallo.
    if (!input.config.buildCodeAssets) {
      stdout.writeln(
        'hook/build.dart: no se piden code assets; no se compila Rust.',
      );
      return;
    }
    final targetOS = input.config.code.targetOS;
    if (targetOS != OS.android) {
      stdout.writeln(
        'hook/build.dart: el destino es $targetOS y no Android; '
        'no se compila Rust (las pruebas de widgets usan el puente simulado).',
      );
      return;
    }

    await const FlutterRustBridgeNativeAssetsBuilder(cratePath: 'rust')
        .run(input: input, output: output);
  });
}
