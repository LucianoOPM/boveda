// Pruebas en el dispositivo: cargan la librería de Rust de verdad.
// Se ejecutan a mano en el emulador y en el celular:
//   flutter test integration_test/vault_session_test.dart -d <dispositivo>

import 'dart:io';

import 'package:boveda/services/vault_location.dart';
import 'package:boveda/src/rust/api/entries.dart';
import 'package:boveda/src/rust/api/error.dart';
import 'package:boveda/src/rust/api/session.dart';
import 'package:boveda/src/rust/frb_generated.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:path/path.dart' as p;

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(() async => await RustLib.init());

  test('la ruta de la bóveda es absoluta y está en no_backup', () async {
    final path = await vaultPath();
    expect(p.isAbsolute(path), isTrue);
    expect(p.basename(p.dirname(path)), 'no_backup');
    expect(p.basename(path), 'boveda.db');
  });

  test('crear, bloquear, desbloquear, agregar, revelar y buscar', () async {
    final dir = await Directory.systemTemp.createTemp('boveda-');
    final session = VaultSession(path: p.join(dir.path, 'boveda.db'));
    try {
      await session.create(password: 'contraseña de prueba');
      final id = await session.add(
        input: const EntryInput(service: 'Gmail', tags: []),
        secret: const NewSecret(password: 'clave-gmail'),
      );

      await session.lock();
      expect(await session.isLocked(), isTrue);
      await expectLater(
        session.unlock(password: 'incorrecta'),
        throwsA(VaultError.wrongPassword),
      );
      await session.unlock(password: 'contraseña de prueba');

      final found = await session.search(query: 'gm');
      expect(found.first.service, 'Gmail');
      expect(found.first.id, id);
      expect(await session.revealPassword(id: id), 'clave-gmail');
    } finally {
      session.dispose();
      await dir.delete(recursive: true);
    }
  });
}
