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

  test('entry y las cuatro combinaciones de edit (#41)', () async {
    final dir = await Directory.systemTemp.createTemp('boveda-');
    final session = VaultSession(path: p.join(dir.path, 'boveda.db'));
    try {
      await session.create(password: 'contraseña de prueba');
      final id = await session.add(
        input: const EntryInput(
          service: '  Gmail  ',
          username: 'ana@example.com',
          tags: ['correo', 'Correo', ' '],
        ),
        secret: const NewSecret(password: '1234', notes: 'notas viejas'),
      );

      // Lo que el núcleo guardó, normalizado.
      final added = await session.entry(id: id);
      expect(added.id, id);
      expect(added.service, 'Gmail');
      expect(added.tags, ['correo']);
      expect(added.lastUsedAt, isNull);

      const input = EntryInput(service: 'Gmail', tags: ['personal']);

      // Ninguno: conserva el secreto.
      await session.edit(id: id, input: input);
      expect((await session.entry(id: id)).tags, ['personal']);
      expect(await session.revealPassword(id: id), '1234');
      expect(await session.revealNotes(id: id), 'notas viejas');
      expect((await session.entry(id: id)).lastUsedAt, isNotNull);

      // Solo la contraseña.
      await session.edit(id: id, input: input, newPassword: 'nueva');
      expect(await session.revealPassword(id: id), 'nueva');
      expect(await session.revealNotes(id: id), 'notas viejas');

      // Solo las notas.
      await session.edit(id: id, input: input, newNotes: 'notas nuevas');
      expect(await session.revealPassword(id: id), 'nueva');
      expect(await session.revealNotes(id: id), 'notas nuevas');

      // Unas notas vacías las quitan.
      await session.edit(id: id, input: input, newNotes: '');
      expect(await session.revealPassword(id: id), 'nueva');
      expect(await session.revealNotes(id: id), isNull);

      // Los dos: reemplaza el secreto completo.
      await session.edit(
        id: id,
        input: input,
        newPassword: 'otra',
        newNotes: 'otras',
      );
      expect(await session.revealPassword(id: id), 'otra');
      expect(await session.revealNotes(id: id), 'otras');

      // Un valor no válido llega como error del campo.
      await expectLater(
        session.edit(id: id, input: input, newPassword: ''),
        throwsA(VaultError.invalidEntryPasswordEmpty),
      );

      await session.delete(id: id);
      await expectLater(
        session.entry(id: id),
        throwsA(VaultError.entryNotFound),
      );
    } finally {
      session.dispose();
      await dir.delete(recursive: true);
    }
  });
}
