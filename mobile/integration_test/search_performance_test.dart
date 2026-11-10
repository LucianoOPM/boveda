// Rendimiento de la búsqueda con 500 entradas reales (#40). Carga la
// librería de Rust y mide en profile; en debug los tiempos de cuadro no son
// representativos. Se ejecuta a mano, desde mobile/:
//
//   flutter drive --profile --no-dds \
//     --driver=test_driver/integration_test.dart \
//     --target=integration_test/search_performance_test.dart \
//     -d <dispositivo>
//
// Criterios (diseño de la #40): cada `search` de ida y vuelta por debajo de
// 50 ms, sin contar la primera; percentil 90 de construcción y de
// rasterizado de cuadros por debajo de 16 ms.

import 'dart:io';

import 'package:boveda/screens/search_screen.dart';
import 'package:boveda/src/rust/api/entries.dart';
import 'package:boveda/src/rust/api/session.dart';
import 'package:boveda/src/rust/frb_generated.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:path/path.dart' as p;

/// Servicios base; cada uno se repite con 10 variantes (500 entradas).
const _bases = [
  ('Gmail', 'mail.google.com'),
  ('Google Drive', 'drive.google.com'),
  ('Teléfono', 'telefono.example.com'),
  ('Ñandú Bank', 'nandu.example.com'),
  ('Banco Ávila', 'bancoavila.example.com'),
  ('Facebook', 'facebook.com'),
  ('Instagram', 'instagram.com'),
  ('Twitter', 'twitter.com'),
  ('LinkedIn', 'linkedin.com'),
  ('GitHub', 'github.com'),
  ('GitLab', 'gitlab.com'),
  ('Amazon', 'amazon.com'),
  ('Mercado Libre', 'mercadolibre.com'),
  ('Netflix', 'netflix.com'),
  ('Spotify', 'spotify.com'),
  ('Steam', 'store.steampowered.com'),
  ('Discord', 'discord.com'),
  ('Slack', 'slack.com'),
  ('Zoom', 'zoom.us'),
  ('Dropbox', 'dropbox.com'),
  ('Outlook', 'outlook.live.com'),
  ('Yahoo', 'yahoo.com'),
  ('Apple ID', 'appleid.apple.com'),
  ('Microsoft', 'account.microsoft.com'),
  ('Uber', 'uber.com'),
  ('Didi', 'didiglobal.com'),
  ('Rappi', 'rappi.com'),
  ('Airbnb', 'airbnb.com'),
  ('Booking', 'booking.com'),
  ('Aeroméxico', 'aeromexico.com'),
  ('Volaris', 'volaris.com'),
  ('SAT', 'sat.gob.mx'),
  ('IMSS', 'imss.gob.mx'),
  ('Infonavit', 'infonavit.org.mx'),
  ('Telcel', 'telcel.com'),
  ('Izzi', 'izzi.mx'),
  ('Totalplay', 'totalplay.com.mx'),
  ('CFE', 'cfe.mx'),
  ('Wi-Fi casa', '192.168.1.1'),
  ('Router oficina', '10.0.0.1:8080'),
  ('Correo trabajo', 'correo.empresa.example'),
  ('VPN empresa', 'vpn.empresa.example'),
  ('Jira', 'empresa.atlassian.net'),
  ('Notion', 'notion.so'),
  ('Figma', 'figma.com'),
  ('Canva', 'canva.com'),
  ('Duolingo', 'duolingo.com'),
  ('Coursera', 'coursera.org'),
  ('Médico', 'clinica.example.com'),
  ('Escuela', 'escuela.example.edu'),
];

const _tags = ['trabajo', 'personal', 'teléfono', 'compras'];

/// Frases que se escriben letra por letra.
const _phrases = ['gmial', 'teléfono trabajo', 'nandu'];

/// Sesión que delega en la real y cuenta las búsquedas respondidas, para
/// esperar a que la pantalla reciba cada respuesta. Solo implementa
/// `search`, lo único que usa `SearchScreen`.
class _CountingSession implements VaultSession {
  _CountingSession(this._inner);

  final VaultSession _inner;
  int completed = 0;

  @override
  Future<List<EntrySummary>> search({required String query}) async {
    final results = await _inner.search(query: query);
    completed++;
    return results;
  }

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

List<String> _prefixes(String phrase) => [
  for (var i = 1; i <= phrase.length; i++) phrase.substring(0, i),
];

double _median(List<double> values) {
  final sorted = [...values]..sort();
  final mid = sorted.length ~/ 2;
  return sorted.length.isOdd
      ? sorted[mid]
      : (sorted[mid - 1] + sorted[mid]) / 2;
}

/// Espera, sin pedir cuadros (el motor los programa con `benchmarkLive`),
/// hasta que [done] se cumpla o venza [timeout].
Future<void> _waitUntil(
  WidgetTester tester,
  bool Function() done,
  String what, {
  Duration timeout = const Duration(seconds: 5),
}) async {
  final watch = Stopwatch()..start();
  while (!done()) {
    if (watch.elapsed > timeout) fail('No llegó a tiempo: $what');
    await tester.pump(const Duration(milliseconds: 16));
  }
}

void main() {
  final binding = IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  // Como las pruebas de rendimiento de Flutter con `watchPerformance`
  // (dev/benchmarks/macrobenchmarks/test/util.dart): el motor programa los
  // cuadros que pide la app y los `pump` de la prueba no agregan cuadros.
  binding.framePolicy = LiveTestWidgetsFlutterBindingFramePolicy.benchmarkLive;

  late Directory dir;
  late VaultSession session;

  setUpAll(() async {
    await RustLib.init();
    dir = await Directory.systemTemp.createTemp('boveda-perf-');
    session = VaultSession(path: p.join(dir.path, 'boveda.db'));
    await session.create(password: 'contraseña de prueba de rendimiento');
    var i = 0;
    for (final (base, domain) in _bases) {
      for (var variant = 0; variant < 10; variant++, i++) {
        await session.add(
          input: EntryInput(
            service: variant == 0 ? base : '$base $variant',
            username: i.isEven ? 'usuario$i@correo.com' : null,
            url: i % 3 == 0
                ? 'https://$domain/login'
                : (i % 5 == 0 ? domain : null),
            tags: [_tags[i % _tags.length]],
          ),
          secret: NewSecret(password: 'secreto-$i'),
        );
      }
    }
  });

  tearDownAll(() async {
    session.dispose();
    await dir.delete(recursive: true);
  });

  testWidgets('criterios funcionales con el núcleo real', (tester) async {
    expect((await session.search(query: '')).length, 500);

    final gm = await session.search(query: 'gm');
    expect(gm.map((e) => e.service), contains('Gmail'));
    final typo = await session.search(query: 'gmial');
    expect(typo.map((e) => e.service), contains('Gmail'));

    final nandu = (await session.search(query: 'Ñandú Bank 3'))
        .firstWhere((e) => e.service == 'Ñandú Bank 3');
    await session.revealPassword(id: nandu.id);
    expect((await session.search(query: '')).first.service, 'Ñandú Bank 3');
  });

  testWidgets('cada búsqueda de ida y vuelta tarda menos de 50 ms', (
    tester,
  ) async {
    final queries = ['', for (final phrase in _phrases) ..._prefixes(phrase)];
    // Calentamiento: la primera llamada no cuenta.
    await session.search(query: '');

    final millis = <double>[];
    for (final query in queries) {
      final watch = Stopwatch()..start();
      await session.search(query: query);
      watch.stop();
      millis.add(watch.elapsedMicroseconds / 1000);
    }
    final max = millis.reduce((a, b) => a > b ? a : b);
    debugPrint(
      'search ida y vuelta: mediana ${_median(millis).toStringAsFixed(2)} ms, '
      'máximo ${max.toStringAsFixed(2)} ms, n=${millis.length}',
    );
    for (var i = 0; i < queries.length; i++) {
      expect(millis[i], lessThan(50), reason: '«${queries[i]}»');
    }
  });

  testWidgets('escribir letra por letra no traba la interfaz', (tester) async {
    final counting = _CountingSession(session);
    await tester.pumpWidget(
      MaterialApp(home: SearchScreen(session: counting, damagedEntries: 0)),
    );
    await _waitUntil(tester, () => counting.completed == 1, 'la lista inicial');
    await tester.pump(const Duration(milliseconds: 100));

    final field = find.byType(TextField);
    var expected = 1;
    Future<void> typeAndWait(String text) async {
      await tester.enterText(field, text);
      expected++;
      final target = expected;
      await _waitUntil(
        tester,
        () => counting.completed >= target,
        'la respuesta de «$text»',
      );
      // La respuesta llegó: dos cuadros para reconstruir la lista.
      await tester.pump(const Duration(milliseconds: 32));
    }

    await binding.watchPerformance(() async {
      for (final phrase in _phrases) {
        for (final prefix in _prefixes(phrase)) {
          await typeAndWait(prefix);
        }
        // Borrar letra por letra vuelve a listas grandes.
        for (final prefix in _prefixes(phrase).reversed.skip(1)) {
          await typeAndWait(prefix);
        }
        await typeAndWait('');
      }
    }, reportKey: 'performance');

    final perf = binding.reportData!['performance'] as Map<String, dynamic>;
    final build = perf['90th_percentile_frame_build_time_millis'] as num;
    final raster = perf['90th_percentile_frame_rasterizer_time_millis'] as num;
    debugPrint(
      'cuadros: p90 construcción $build ms, p90 rasterizado $raster ms, '
      'cuadros ${perf['frame_count']}',
    );
    expect(build, lessThan(16));
    expect(raster, lessThan(16));
  });
}
