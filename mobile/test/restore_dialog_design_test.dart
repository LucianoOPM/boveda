// Pruebas de la presentación del diálogo de importar un respaldo
// (docs/DESIGN.md, 6.11 y 7.9). El comportamiento sigue en
// `backup_gate_test.dart`.

import 'dart:async';

import 'package:boveda/screens/restore_dialog.dart';
import 'package:boveda/src/rust/api/error.dart';
import 'package:boveda/src/rust/api/session.dart';
import 'package:boveda/theme/accents.dart';
import 'package:boveda/theme/colors.dart';
import 'package:boveda/theme/icons.dart';
import 'package:boveda/theme/spacing.dart';
import 'package:boveda/theme/theme.dart';
import 'package:boveda/widgets/activity_text_field.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'fakes.dart';

late FakeVaultSession session;

final warning = find.byKey(const Key('restore-warning'));
final password = find.byKey(const Key('restore-password'));
final submit = find.byKey(const Key('restore-submit'));

Future<void> pumpDialog(
  WidgetTester tester, {
  bool replacesVault = true,
  bool closeOnError = false,
  BovedaAccent accent = defaultAccent,
  double textScale = 1,
  Size size = const Size(360, 640),
  double keyboard = 0,
}) async {
  tester.view.physicalSize = size;
  tester.view.devicePixelRatio = 1;
  tester.view.viewInsets = FakeViewPadding(bottom: keyboard);
  addTearDown(tester.view.reset);
  await tester.pumpWidget(
    MaterialApp(
      theme: buildTheme(accent),
      builder: (context, child) => MediaQuery(
        data: MediaQuery.of(context)
            .copyWith(textScaler: TextScaler.linear(textScale)),
        child: child!,
      ),
      home: Builder(
        builder: (context) => Scaffold(
          body: Center(
            child: TextButton(
              onPressed: () => showDialog<RestoreResult>(
                context: context,
                barrierDismissible: false,
                builder: (_) => RestoreDialog(
                  session: session,
                  backupPath: '/tmp/backup-1.bak',
                  replacesVault: replacesVault,
                  closeOnError: closeOnError,
                ),
              ),
              child: const Text('abrir'),
            ),
          ),
        ),
      ),
    ),
  );
  await tester.tap(find.text('abrir'));
  await tester.pumpAndSettle();
}

void main() {
  setUp(() => session = FakeVaultSession()..locked = false);

  for (final accent in BovedaAccent.values) {
    testWidgets('con ${accent.name}: la advertencia es una tarjeta '
        'warningContainer con ícono', (tester) async {
      await pumpDialog(tester, accent: accent);

      expect(warning, findsOneWidget);
      final material = tester.widget<Material>(warning);
      expect(material.color, BovedaColors.warningContainer);
      final shape = material.shape! as RoundedRectangleBorder;
      expect(shape.borderRadius, BorderRadius.circular(Radii.control));
      expect(shape.side.color, BovedaColors.warning.withValues(alpha: 0.38));
      final icon = tester.widget<Icon>(
        find.descendant(
          of: warning,
          matching: find.byIcon(BovedaIcons.triangleAlert),
        ),
      );
      expect(icon.color, BovedaColors.warning);
      expect(
        find.descendant(
          of: warning,
          matching: find.text('Reemplaza la bóveda actual.'),
        ),
        findsOneWidget,
      );
    });
  }

  testWidgets('sin reemplazar (primer uso) no hay advertencia', (tester) async {
    await pumpDialog(tester, replacesVault: false);

    expect(warning, findsNothing);
    expect(find.text('Reemplaza la bóveda actual.'), findsNothing);
    expect(find.text('Restaurar respaldo'), findsOneWidget);
  });

  testWidgets('el título, el campo y los botones no cambian', (tester) async {
    await pumpDialog(tester);

    expect(find.text('Importar respaldo'), findsOneWidget);
    expect(find.text('CONTRASEÑA DEL RESPALDO'), findsOneWidget);
    expect(find.widgetWithText(TextButton, 'Cancelar'), findsOneWidget);
    expect(find.widgetWithText(FilledButton, 'Importar'), findsOneWidget);
    expect(tester.widget<FilledButton>(submit).onPressed, isNull);

    await tester.enterText(password, 'una-contraseña');
    await tester.pump();

    expect(tester.widget<FilledButton>(submit).onPressed, isNotNull);
  });

  testWidgets('la contraseña del respaldo sigue sin poder copiarse', (
    tester,
  ) async {
    await pumpDialog(tester);

    final field = tester.widget<ActivityTextField>(
      find.descendant(of: password, matching: find.byType(ActivityTextField)),
    );
    expect(field.copyable, isFalse);
    expect(field.obscureText, isTrue);
  });

  testWidgets('el error del campo se sigue mostrando', (tester) async {
    session.onImportBackup = (_, _) async => throw VaultError.wrongPassword;
    await pumpDialog(tester);

    await tester.enterText(password, 'mala');
    await tester.pump();
    await tester.tap(submit);
    await tester.pumpAndSettle();

    expect(find.text('Contraseña del respaldo incorrecta.'), findsOneWidget);
    // Sigue la advertencia y se puede reintentar.
    expect(warning, findsOneWidget);
  });

  testWidgets('ocupado: indicador en el botón y "Cancelar" desactivado', (
    tester,
  ) async {
    final pending = Completer<UnlockReport>();
    session.onImportBackup = (_, _) => pending.future;
    await pumpDialog(tester);

    await tester.enterText(password, 'una-contraseña');
    await tester.pump();
    await tester.tap(submit);
    await tester.pump();

    expect(
      find.descendant(
        of: submit,
        matching: find.byType(CircularProgressIndicator),
      ),
      findsOneWidget,
    );
    expect(
      tester
          .widget<TextButton>(find.widgetWithText(TextButton, 'Cancelar'))
          .onPressed,
      isNull,
    );
    pending.complete(emptyReport);
    await tester.pumpAndSettle();
  });

  testWidgets('a 200 % con la advertencia y el error no desborda', (
    tester,
  ) async {
    session.onImportBackup = (_, _) async => throw VaultError.wrongPassword;
    await pumpDialog(tester, textScale: 2);

    await tester.enterText(password, 'mala');
    await tester.pump();
    await tester.tap(submit);
    await tester.pumpAndSettle();

    expect(tester.takeException(), isNull);
    expect(find.text('Contraseña del respaldo incorrecta.'), findsOneWidget);
  });

  testWidgets('con el teclado abierto en una pantalla baja no desborda', (
    tester,
  ) async {
    await pumpDialog(tester, size: const Size(360, 400), keyboard: 200);

    expect(tester.takeException(), isNull);
    // Con el teclado abierto se sigue pudiendo escribir en el campo.
    await tester.enterText(password, 'una-contraseña');
    await tester.pump();
    expect(tester.takeException(), isNull);
    expect(tester.widget<FilledButton>(submit).onPressed, isNotNull);
  });

  testWidgets('a 200 % con el teclado abierto en una pantalla baja no '
      'desborda', (tester) async {
    await pumpDialog(
      tester,
      size: const Size(360, 400),
      keyboard: 200,
      textScale: 2,
    );

    expect(tester.takeException(), isNull);
  });
}
