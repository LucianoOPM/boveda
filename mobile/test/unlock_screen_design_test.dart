// Pruebas de la presentación de la pantalla de desbloqueo (docs/DESIGN.md, 7.3
// y el board "Desbloqueo" de Penpot). El comportamiento sigue en
// `unlock_screen_test.dart`.

import 'dart:async';
import 'dart:typed_data';
import 'dart:ui' show Tristate;

import 'package:boveda/screens/unlock_screen.dart';
import 'package:boveda/services/biometric_service.dart';
import 'package:boveda/src/rust/api/error.dart';
import 'package:boveda/src/rust/api/session.dart';
import 'package:boveda/theme/accents.dart';
import 'package:boveda/theme/colors.dart';
import 'package:boveda/theme/icons.dart';
import 'package:boveda/theme/theme.dart';
import 'package:boveda/widgets/primary_glow.dart';
import 'package:boveda/widgets/status_capsule.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'fakes.dart';
import 'theme/contrast.dart';

const passwordKey = Key('unlock-password');
const submitKey = Key('unlock-submit');
const biometricKey = Key('unlock-biometric');
const enableKey = Key('unlock-enable-biometric');
const noticeKey = Key('unlock-notice');
const errorKey = Key('unlock-error');

late FakeVaultSession session;
late FakeBiometricService biometric;

Future<void> pumpScreen(
  WidgetTester tester, {
  BovedaAccent accent = defaultAccent,
  UnlockNotice? notice,
  double textScale = 1,
}) async {
  tester.view.physicalSize = const Size(360, 640);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(
    MaterialApp(
      theme: buildTheme(accent),
      builder: (context, child) => MediaQuery(
        data: MediaQuery.of(context)
            .copyWith(textScaler: TextScaler.linear(textScale)),
        child: child!,
      ),
      home: UnlockScreen(
        session: session,
        biometric: biometric,
        notice: notice,
        onUnlocked: (_) {},
      ),
    ),
  );
  await tester.pump();
}

Future<void> drain(WidgetTester tester) async {
  for (var i = 0; i < 10; i++) {
    await tester.pump();
  }
}

Future<void> typePassword(WidgetTester tester) async {
  await tester.enterText(find.byKey(passwordKey), 'prueba-bóveda-v2');
  await tester.pump();
}

/// Color de fondo de un `Container` con [BoxDecoration].
Color containerColor(WidgetTester tester, Key key) {
  final container = tester.widget<Container>(find.byKey(key));
  return (container.decoration! as BoxDecoration).color!;
}

void main() {
  setUp(() {
    session = FakeVaultSession();
    biometric = FakeBiometricService()..available = false;
  });

  for (final accent in BovedaAccent.values) {
    group('con ${accent.name}', () {
      testWidgets('muestra el candado, el título y la cápsula', (tester) async {
        await pumpScreen(tester, accent: accent);

        expect(
          tester.widget<Scaffold>(find.byType(Scaffold)).backgroundColor,
          BovedaColors.lockScrim,
        );
        final box = find.byKey(const Key('unlock-lock-box'));
        expect(tester.getSize(box), const Size(72, 72));
        expect(
          find.descendant(of: box, matching: find.byIcon(BovedaIcons.lock)),
          findsOneWidget,
        );
        expect(find.text('Bóveda bloqueada'), findsOneWidget);
        final capsule = find.byType(StatusCapsule);
        expect(
          find.descendant(of: capsule, matching: find.text('BLOQUEADA')),
          findsOneWidget,
        );
        expect(find.text('CONTRASEÑA MAESTRA'), findsOneWidget);
      });

      testWidgets('el spinner del botón se ve sobre el fondo deshabilitado', (
        tester,
      ) async {
        final pending = Completer<UnlockReport>();
        session.onUnlock = (_) => pending.future;
        await pumpScreen(tester, accent: accent);
        await typePassword(tester);
        await tester.tap(find.byKey(submitKey));
        await tester.pump();

        expect(find.text('Desbloqueando…'), findsOneWidget);
        final spinner = find.descendant(
          of: find.byKey(submitKey),
          matching: find.byType(CircularProgressIndicator),
        );
        expect(spinner, findsOneWidget);
        final color =
            tester.widget<CircularProgressIndicator>(spinner).color ??
            buildTheme(accent).progressIndicatorTheme.color!;
        // El botón deshabilitado es el acento tenue sobre el fondo.
        final background = Color.alphaBlend(
          accent.accentDim,
          BovedaColors.lockScrim,
        );
        expect(contrast(color, background), greaterThanOrEqualTo(3));
        pending.complete(emptyReport);
        // La pantalla sigue ocupada (el spinner no termina): no `pumpAndSettle`.
        await drain(tester);
      });
    });
  }

  testWidgets('la cápsula anuncia "Bóveda bloqueada"', (tester) async {
    final handle = tester.ensureSemantics();
    await pumpScreen(tester);

    expect(find.bySemanticsLabel('Bóveda bloqueada'), findsWidgets);
    expect(find.bySemanticsLabel('BLOQUEADA'), findsNothing);
    handle.dispose();
  });

  testWidgets('el resplandor sigue al botón principal', (tester) async {
    await pumpScreen(tester);
    expect(tester.widget<FilledButton>(find.byKey(submitKey)), isNotNull);
    expect(tester.widget<PrimaryGlow>(find.byType(PrimaryGlow)).enabled, false);

    await typePassword(tester);

    expect(tester.widget<PrimaryGlow>(find.byType(PrimaryGlow)).enabled, true);
  });

  testWidgets('el error de contraseña lleva el ícono de alerta', (
    tester,
  ) async {
    session.onUnlock = (_) async => throw VaultError.wrongPassword;
    await pumpScreen(tester);
    await typePassword(tester);
    await tester.tap(find.byKey(submitKey));
    await drain(tester);

    final error = find.byKey(errorKey);
    expect(
      find.descendant(of: error, matching: find.text('Contraseña incorrecta')),
      findsOneWidget,
    );
    expect(
      find.descendant(
        of: error,
        matching: find.byIcon(BovedaIcons.triangleAlert),
      ),
      findsOneWidget,
    );
  });

  testWidgets('un aviso de error va en errorContainer', (tester) async {
    await pumpScreen(tester, notice: const UnlockNotice('No se pudo importar'));

    expect(find.text('No se pudo importar'), findsOneWidget);
    expect(containerColor(tester, noticeKey), BovedaColors.errorContainer);
  });

  testWidgets('un aviso que no es de error va en warningContainer', (
    tester,
  ) async {
    await pumpScreen(
      tester,
      notice: const UnlockNotice('Importado', isError: false),
    );

    expect(find.text('Importado'), findsOneWidget);
    expect(containerColor(tester, noticeKey), BovedaColors.warningContainer);
  });

  testWidgets('el aviso de huella desactivada va en warningContainer', (
    tester,
  ) async {
    biometric
      ..enabled = true
      ..available = true
      ..onUnlock = () async =>
          throw const BiometricException(BiometricFailure.invalidated);
    await pumpScreen(tester);
    await drain(tester);

    expect(
      find.text(
        'La huella se desactivó. Desbloquea con tu contraseña maestra.',
      ),
      findsOneWidget,
    );
    expect(
      containerColor(tester, const Key('unlock-biometric-notice')),
      BovedaColors.warningContainer,
    );
  });

  testWidgets('el botón de huella es un círculo de 64 con tooltip', (
    tester,
  ) async {
    biometric
      ..enabled = true
      ..available = true
      ..onUnlock = () async =>
          throw const BiometricException(BiometricFailure.canceled);
    await pumpScreen(tester);
    await drain(tester);

    expect(tester.getSize(find.byKey(biometricKey)), const Size(64, 64));
    expect(
      find.descendant(
        of: find.byKey(biometricKey),
        matching: find.byIcon(BovedaIcons.fingerprint),
      ),
      findsOneWidget,
    );
    expect(find.byTooltip('Usar huella'), findsOneWidget);
    // El texto de siempre, debajo del círculo.
    expect(find.text('Usar huella'), findsOneWidget);
  });

  testWidgets('el botón de huella se anuncia como botón y sigue al estado', (
    tester,
  ) async {
    final handle = tester.ensureSemantics();
    final pending = Completer<Uint8List>();
    biometric
      ..enabled = true
      ..available = true
      ..onUnlock = () async =>
          throw const BiometricException(BiometricFailure.canceled);
    await pumpScreen(tester);
    await drain(tester);

    var flags = tester.getSemantics(find.byKey(biometricKey)).flagsCollection;
    expect(flags.isButton, isTrue);
    expect(flags.isEnabled, Tristate.isTrue);

    // Con una huella en curso el botón sigue siendo botón, pero desactivado.
    biometric.onUnlock = () => pending.future;
    await tester.tap(find.byKey(biometricKey));
    await tester.pump();

    flags = tester.getSemantics(find.byKey(biometricKey)).flagsCollection;
    expect(flags.isButton, isTrue);
    expect(flags.isEnabled, Tristate.isFalse);
    pending.completeError(const BiometricException(BiometricFailure.canceled));
    await drain(tester);
    handle.dispose();
  });

  testWidgets('con texto al 200 % a 360 dp no hay desbordes', (tester) async {
    biometric
      ..enabled = true
      ..available = true
      ..onUnlock = () async =>
          throw const BiometricException(BiometricFailure.invalidated);
    session.onUnlock = (_) async => throw VaultError.wrongPassword;
    await pumpScreen(
      tester,
      textScale: 2,
      notice: const UnlockNotice('No se pudo importar el respaldo'),
    );
    await drain(tester);
    await typePassword(tester);
    await tester.ensureVisible(find.byKey(submitKey));
    await tester.tap(find.byKey(submitKey));
    await drain(tester);

    expect(find.byKey(errorKey), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets('el botón ocupado a 200 % a 360 dp no desborda', (tester) async {
    final pending = Completer<UnlockReport>();
    session.onUnlock = (_) => pending.future;
    await pumpScreen(tester, textScale: 2);
    await typePassword(tester);
    await tester.ensureVisible(find.byKey(submitKey));
    await tester.tap(find.byKey(submitKey));
    await tester.pump();

    expect(find.text('Desbloqueando…'), findsOneWidget);
    expect(tester.takeException(), isNull);
    pending.complete(emptyReport);
    await drain(tester);
  });

  testWidgets('con la casilla de huella a 200 % no hay desbordes', (
    tester,
  ) async {
    biometric.available = true;
    await pumpScreen(tester, textScale: 2);
    await drain(tester);

    expect(find.byKey(enableKey), findsOneWidget);
    expect(tester.takeException(), isNull);
  });
}
