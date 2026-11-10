# Pantalla principal (issue #40): plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reemplazar el marcador de la pantalla principal por `SearchScreen`: barra de búsqueda enfocada al llegar, lista filtrada por el núcleo en cada cambio y cierre del teclado al tocar fuera en toda la app.

**Architecture:** Solo Dart, en `mobile/`. `SearchScreen` llama a `VaultSession.search` en cada cambio y descarta las respuestas viejas con un contador de generación; `VaultGate` la monta en el estado `unlocked` bajo la `key` `_epoch`. `ActivityTextField` gana un `onTapOutside` fijo que quita el foco, y los controles de los formularios de contraseña se agrupan con el campo mediante `TextFieldTapRegion`.

**Tech Stack:** Flutter 3.47.9, flutter_rust_bridge 2.13.0 (sin regenerar), `flutter_test`, `integration_test`.

**Spec:** `docs/superpowers/specs/2026-11-10-pantalla-principal-design.md`

## Global Constraints

- Sin cambios en `core/` ni en `mobile/rust/src/api/`; sin regenerar el puente. Si algo lo exige, detenerse y preguntar.
- Sin dependencias nuevas (`characters` llega por `package:flutter/widgets.dart`).
- Todo campo de texto es `ActivityTextField` o `PasswordField`.
- Textos exactos: `'Buscar'`, `'Borrar búsqueda'`, `'Todavía no hay contraseñas guardadas'`, `'Sin resultados para «$texto»'`, `'No se pudo buscar'`, `'Reintentar'`.
- Ningún log con el texto buscado ni con mensajes de error: solo `if (kDebugMode) debugPrint('search: ${e.runtimeType}')`.
- Rendimiento: cada `search` de ida y vuelta < 50 ms (sin contar la primera llamada); percentil 90 de construcción y de rasterizado < 16 ms; 500 entradas; medido en profile en el emulador.
- Comandos desde `mobile/`: `flutter analyze`, `flutter test`. Desde la raíz: `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`.
- Comentarios y mensajes de commit en español; identificadores en inglés; Conventional Commits.
- El roadmap se marca solo después de que pasen la prueba de rendimiento y los ocho casos a mano.

## Review Focus

1. **Respuesta inicial tardía:** la búsqueda `''` del arranque responde después de la primera letra escrita → se ven los resultados de la letra, no la lista completa. Prueba en la Task 4 (`'la búsqueda inicial tardía no pisa la primera letra'`).
2. **Pantalla baja con aviso y teclado:** aviso de dañadas + campo + teclado abierto en una pantalla baja (horizontal) → sin desbordamiento (*overflow*). Prueba en la Task 4 (`'sin desbordamiento en una pantalla baja con aviso y teclado'`).
3. **Grafemas compuestos:** un servicio que empieza con una secuencia ZWJ ("👩‍💻 Trabajo") → la inicial es el grupo completo. Se agrega al caso de inicial de la Task 4.
4. **Lista larga desplazada y texto nuevo:** el usuario desplazó una lista larga, vuelve al campo y escribe; si los resultados nuevos siguen siendo largos, el mejor resultado debe verse arriba. `SearchScreen` usa un `ScrollController` y, al mostrar resultados de un texto distinto de `_shownQuery`, salta al principio (`jumpTo(0)` si `hasClients`). Prueba en la Task 4 (`'resultados de otro texto vuelven al principio de la lista'`). Precisión que el diseño no fijaba; se informa al usuario con el plan.
5. **Error que se recupera:** después de "No se pudo buscar", la siguiente letra con éxito quita el mensaje y muestra la lista. Aserción agregada al caso de fallo de la Task 4.

---

### Task 1: `ActivityTextField` cierra el teclado al tocar fuera

**Files:**
- Modify: `mobile/lib/widgets/activity_text_field.dart`
- Test: `mobile/test/widgets_test.dart` (grupo `'bloqueo automático'` existente o un grupo nuevo `'teclado'`)

**Interfaces:**
- Produces: `ActivityTextField({…, bool autofocus = false, VoidCallback? onEditingComplete})`, ambos pasados tal cual al `TextField`. `onTapOutside` fijo: `(_) => FocusManager.instance.primaryFocus?.unfocus()`, no configurable.

- [ ] **Step 1: Write the failing tests** en `widgets_test.dart` (que pasa a importar `fakes.dart`), grupo `'teclado'`. Ayudante en `test/fakes.dart`, que usan también las Tasks 2, 4 y 5:

```dart
bool fieldHasFocus(WidgetTester tester, Finder field) => tester
    .widget<EditableText>(find.descendant(of: field, matching: find.byType(EditableText)))
    .focusNode
    .hasFocus;
```

Casos (montados en `MaterialApp(home: Scaffold(body: Column(...)))` con un `SizedBox(height: 200)` vacío debajo para tocar fuera):
- `'ActivityTextField: tocar fuera quita el foco'`: `tester.tap(campo)`, `fieldHasFocus` es `true`; `tester.tapAt(punto del SizedBox vacío)`, `pump`, `fieldHasFocus` es `false`.
- `'ActivityTextField: tocar un TextFieldTapRegion vecino no quita el foco'`: un `TextFieldTapRegion(child: TextButton(key: Key('vecino'), onPressed: () {}, …))` debajo del campo; tocarlo deja `fieldHasFocus` en `true`.
- `'ActivityTextField pasa autofocus y onEditingComplete'`: con `autofocus: true` el `TextField` lo tiene en `true` y el campo tiene el foco tras `pump`; `testTextInput.receiveAction(TextInputAction.search)` con `textInputAction: TextInputAction.search` y `onEditingComplete` que cuenta llamadas → 1 llamada y el campo sigue con el foco.
- `'PasswordField: el ojo no quita el foco y tocar fuera sí'`: enfocar, tocar `find.byTooltip('Mostrar contraseña')` → foco sigue; `tapAt` vacío → sin foco.

- [ ] **Step 2: Run to verify they fail**

Run: `flutter test test/widgets_test.dart`
Expected: FAIL (no compila: `autofocus`/`onEditingComplete` no existen; tocar fuera no quita el foco).

- [ ] **Step 3: Implement** los dos parámetros y el `onTapOutside` fijo en `ActivityTextField`. Actualizar el comentario de la clase: tocar fuera de cualquier campo cierra el teclado (en Android Flutter no lo hace por omisión con el dedo); los controles del formulario que no deben cerrarlo se envuelven en `TextFieldTapRegion`; los íconos de la decoración ya están dentro de la región del campo.

- [ ] **Step 4: Run to verify they pass**

Run: `flutter test` y `flutter analyze`
Expected: todo pasa (ninguna prueba existente depende de que tocar fuera conserve el foco; si alguna falla, revisar si tocaba un control que ahora pertenece a la Task 2 y anotarlo).

- [ ] **Step 5: Commit**

```bash
git add mobile/lib/widgets/activity_text_field.dart mobile/test/widgets_test.dart mobile/test/fakes.dart
git commit -m "feat: cerrar el teclado al tocar fuera de un campo"
```

### Task 2: controles de los formularios de contraseña en el grupo del campo

**Files:**
- Modify: `mobile/lib/screens/first_run_screen.dart:175-200` (casilla `first-run-accept`, botón `first-run-create`)
- Modify: `mobile/lib/screens/unlock_screen.dart:374-395` (casilla `unlock-enable-biometric`, botón `unlock-submit`)
- Test: `mobile/test/unlock_screen_test.dart`, `mobile/test/first_run_screen_test.dart`

**Interfaces:**
- Consumes: Task 1 (`onTapOutside` en `ActivityTextField`).
- Produces: nada nuevo; las `Key` existentes siguen en los mismos widgets (la `Key` queda en el control, no en el `TextFieldTapRegion`).

- [ ] **Step 1: Write the failing tests**, con `fieldHasFocus` de `fakes.dart` (Task 1).

`unlock_screen_test.dart`:
- `'la casilla de la huella no quita el foco del campo'`: `biometric.available = true`; `pumpScreen`; `tester.tap(find.byKey(passwordKey))`; tocar `enableKey` → `fieldHasFocus` sigue `true` y la casilla quedó marcada.
- `'Desbloquear desactivado no quita el foco'`: campo vacío (botón desactivado); enfocar; tocar `submitKey` → foco sigue.
- `'un espacio vacío quita el foco'`: enfocar; `tester.tapAt(const Offset(1, 1))` → sin foco.
- `'Usar huella quita el foco'`: `biometric..available = true..enabled = true`, con `onUnlock` pendiente (un `Completer` que no se completa) para que la pantalla quede en la contraseña tras cancelar/fallar según el flujo existente; enfocar el campo y tocar `Key('unlock-biometric')` cuando esté activo → sin foco. Si el botón está desactivado durante la huella inicial, completar primero esa huella con `BiometricException(BiometricFailure.canceled)` como hacen las pruebas existentes de cancelación.

`first_run_screen_test.dart`:
- `'la casilla no quita el foco del campo'`: enfocar `first-run-password`; `ensureVisible` y tocar `first-run-accept` → foco sigue.
- `'Crear bóveda desactivado no quita el foco'`: campos vacíos; enfocar; tocar `first-run-create` → foco sigue.
- `'un espacio vacío quita el foco'`: `tapAt(const Offset(1, 1))` → sin foco.

- [ ] **Step 2: Run to verify they fail**

Run: `flutter test test/unlock_screen_test.dart test/first_run_screen_test.dart`
Expected: FAIL en los casos de casilla y botón desactivado (pierden el foco).

- [ ] **Step 3: Implement**: envolver cada uno de los cuatro controles en `TextFieldTapRegion(child: …)`. "Usar huella" y `BiometricOfferScreen` no cambian. Un comentario corto en cada pantalla: los controles del formulario comparten el grupo del campo para no cerrar el teclado.

- [ ] **Step 4: Run to verify they pass**

Run: `flutter test` y `flutter analyze`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add mobile/lib/screens/first_run_screen.dart mobile/lib/screens/unlock_screen.dart mobile/test/unlock_screen_test.dart mobile/test/first_run_screen_test.dart
git commit -m "feat: mantener el teclado con los controles de los formularios de contraseña"
```

### Task 3: búsqueda en la sesión simulada

**Files:**
- Modify: `mobile/test/fakes.dart`

**Interfaces:**
- Produces, en `FakeVaultSession`:
  - `final List<String> searchCalls = [];`
  - `Future<List<EntrySummary>> Function(String query) onSearch = (_) async => [];`
  - `search({required String query})` registra en `searchCalls` y devuelve `onSearch(query)`.
- Produces, función de nivel superior: `EntrySummary entry(String service, {String? id, String? username, String? url, List<String> tags = const [], int? lastUsedAt})`. `id` por omisión: `service` codificado de forma estable a 32 caracteres hexadecimales (por ejemplo, `service.hashCode.toRadixString(16).padLeft(32, '0')`; basta con que sea único dentro de cada prueba). `modifiedAt: 0`.
- Actualizar el comentario de la clase (qué métodos hacen algo).

- [ ] **Step 1: Implement** lo anterior.
- [ ] **Step 2: Run** `flutter test` → todo pasa (las pruebas que llegan a la pantalla principal aún ven el marcador; nada llama a `search` todavía).
- [ ] **Step 3: Commit**

```bash
git add mobile/test/fakes.dart
git commit -m "test: búsqueda en la sesión simulada"
```

### Task 4: `SearchScreen`

**Files:**
- Create: `mobile/lib/screens/search_screen.dart`
- Test: `mobile/test/search_screen_test.dart`

**Interfaces:**
- Consumes: Task 1 (`ActivityTextField.autofocus`, `onEditingComplete`), Task 3 (`searchCalls`, `onSearch`, `entry(...)`).
- Produces: `class SearchScreen extends StatefulWidget { const SearchScreen({super.key, required VaultSession session, required int damagedEntries}); }`.

Comportamiento completo en el diseño, sección "`SearchScreen`". Fijado aquí:
- Estado: `_query`, `_focus`, `_scroll` (`ScrollController`, Review Focus 4), `_generation`, `List<EntrySummary>? _results`, `String _shownQuery = ''`, `bool _failed = false`.
- `_search` como el bloque del diseño, más: si `query != _shownQuery` y `_scroll.hasClients`, `_scroll.jumpTo(0)` después del `setState` (en un `addPostFrameCallback` si la lista todavía no tiene la nueva longitud).
- `_onChanged(String text)`: `setState(() {}); _search(text);`. `_clear()`: `_query.clear(); setState(() {}); _focus.requestFocus(); _search('');`.
- Inicial: `service.isEmpty ? '?' : service.characters.first.toUpperCase()`.

Ayudante de las pruebas:

```dart
Future<void> pumpSearch(WidgetTester tester, FakeVaultSession session,
    {int damaged = 0}) async {
  await tester.pumpWidget(MaterialApp(
      home: SearchScreen(session: session, damagedEntries: damaged)));
  await tester.pump();
}
```

- [ ] **Step 1: Write the failing tests** (`search_screen_test.dart`). Nombres y aserciones:
  - `'primera carga: indicador y luego la lista en el orden recibido'`: `onSearch` con un `Completer`; tras `pumpSearch`, `CircularProgressIndicator` presente; completar con `[entry('Zeta'), entry('Alfa')]`; `pump`; `tester.getTopLeft(find.text('Zeta')).dy < tester.getTopLeft(find.text('Alfa')).dy`.
  - `'subtítulo: usuario, si no URL, si no nada'`: `entry('A', username: 'ana', url: 'a.com')` muestra `'ana'` y no `'a.com'`; `entry('B', url: 'https://muy.larga/…')` muestra la URL con un `Text` de `maxLines: 1` y `overflow: TextOverflow.ellipsis`; `entry('C')` → su `ListTile.subtitle` es `null`.
  - `'inicial: primer grafema en mayúscula'`: servicios `'gmail'`, `'Ébano'`, `'🔑 Llaves'`, `'👩‍💻 Trabajo'` → `find.text('G')`, `'É'`, `'🔑'`, `'👩‍💻'` dentro de `CircleAvatar`.
  - `'la inicial no se lee con el lector de pantalla'`: `final handle = tester.ensureSemantics();` el nodo semántico del `ListTile` de `'gmail'` no contiene `'G'` en su `label` (usar `tester.getSemantics(find.byType(ListTile))`); `handle.dispose()`.
  - `'busca en cada cambio'`: `enterText(campo, 'g')`, `pump`, `enterText(campo, 'gm')`, `pump` → `session.searchCalls == ['', 'g', 'gm']`.
  - `'una respuesta vieja no pisa la nueva'`: `onSearch` con un `Completer` por texto; escribir `'g'` y `'gm'`; completar `'gm'` con `[entry('Gmail')]`, luego `'g'` con `[entry('Google')]` → se ve `'Gmail'` y no `'Google'`.
  - `'la búsqueda inicial tardía no pisa la primera letra'` (Review Focus 1): `''` pendiente; escribir `'g'`; completar `'g'` con `[entry('Gmail')]`, luego `''` con 3 entradas → solo `'Gmail'`.
  - `'un fallo viejo no muestra el error'`: `'g'` falla (`StateError`) después de que `'gm'` respondió → no aparece `'No se pudo buscar'`.
  - `'bóveda vacía'`: `[]` con `''` → `'Todavía no hay contraseñas guardadas'`; escribir `'   '` con `[]` → el mismo texto.
  - `'sin resultados'`: escribir `'xyz'` con `[]` → `'Sin resultados para «xyz»'`.
  - `'fallo: mensaje, Reintentar y recuperación'`: `''` lanza `StateError` → `'No se pudo buscar'` y `'Reintentar'`; cambiar `onSearch` a éxito y tocar `'Reintentar'` → `searchCalls.last == ''` y la lista visible; otro fallo con `'g'` y luego éxito con `'gm'` → el mensaje desaparece (Review Focus 5).
  - `'Locked se ignora en silencio'`: primero lista con `[entry('Gmail')]`; luego `'g'` lanza `VaultError.locked` → sin `'No se pudo buscar'` y `'Gmail'` sigue visible.
  - `'una respuesta tras desmontar no lanza'`: `''` pendiente; `pumpWidget(const SizedBox())`; completar → `tester.takeException()` es `null`.
  - `'el ✕ sigue al texto aunque la búsqueda esté pendiente'`: sin texto, `find.byTooltip('Borrar búsqueda')` no existe; `onSearch` que nunca completa; escribir `'g'`; `pump` → existe.
  - `'el ✕ borra, deja el foco y busca vacío'`: escribir `'gm'`; tocar el ✕ → campo vacío, `fieldHasFocus` `true`, `searchCalls.last == ''`.
  - `'el ✕ devuelve el foco tras perderlo'`: escribir `'g'`; `tapAt` vacío (foco fuera); tocar ✕ → foco de vuelta.
  - `'la tecla buscar no quita el foco'`: `testTextInput.receiveAction(TextInputAction.search)` → foco sigue.
  - `'configuración del teclado'`: el `TextField` tiene `enableSuggestions == false`, `autocorrect == false`, `enableIMEPersonalizedLearning == false`, `textInputAction == TextInputAction.search`.
  - `'el campo tiene el foco al montar'`.
  - `'desplazar la lista, tocar una entrada o un espacio vacío cierran el teclado'`: 40 entradas; enfocar; `tester.drag(find.byType(ListView), const Offset(0, -300))` → sin foco; volver a enfocar y tocar una entrada → sin foco; volver a enfocar y `tapAt` vacío (por ejemplo, el margen a la izquierda del campo) → sin foco.
  - `'resultados de otro texto vuelven al principio de la lista'` (Review Focus 4): 40 entradas; arrastrar hacia abajo; escribir `'e'` con otras 40 entradas → `ScrollController` de la lista en `offset == 0` (leer `tester.widget<ListView>(…).controller!.offset`).
  - `'aviso de dañadas arriba de la lista'`: `damaged: 2` → `'2 entradas no se pudieron leer'` arriba del campo (`dy` menor); `damaged: 0` → sin `MaterialBanner`.
  - `'sin desbordamiento en una pantalla baja con aviso y teclado'` (Review Focus 2): `tester.view.physicalSize = const Size(800, 400); tester.view.devicePixelRatio = 1;` y `tester.view.viewInsets = const FakeViewPadding(bottom: 200);` con `addTearDown(tester.view.reset)`; `damaged: 3`, 10 entradas → `tester.takeException()` es `null`.
- [ ] **Step 2: Run to verify they fail**

Run: `flutter test test/search_screen_test.dart`
Expected: FAIL (no existe `search_screen.dart`).

- [ ] **Step 3: Implement `SearchScreen`** según el diseño y los puntos fijados arriba. Comentario de la clase: qué muestra, búsqueda por cambio con generación, foco al llegar (por qué `autofocus` basta aquí y no en el desbloqueo, #37), dañadas solo en el aviso (corrección a la #37) y la nota de que tocar una entrada se conecta en la #41.
- [ ] **Step 4: Run to verify they pass**

Run: `flutter test` y `flutter analyze`
Expected: PASS. Si el caso de pantalla baja desborda, envolver el cuerpo en lo necesario (el `Expanded` ya cede; el aviso + campo deben caber o el `Column` pasa a permitir desplazamiento) y anotar la decisión en el diseño, sección "Hallazgos".
- [ ] **Step 5: Commit**

```bash
git add mobile/lib/screens/search_screen.dart mobile/test/search_screen_test.dart
git commit -m "feat: pantalla principal con búsqueda y lista filtrada"
```

### Task 5: `VaultGate` muestra `SearchScreen`

**Files:**
- Modify: `mobile/lib/app.dart` (estado `unlocked` en `build`, comentario de `VaultGate`)
- Modify: `mobile/lib/screens/placeholder_screen.dart` (quitar el parámetro `banner`, que ya nadie usa)
- Test: `mobile/test/app_test.dart`, `mobile/test/auto_lock_test.dart`, `mobile/test/widgets_test.dart:171-193`

**Interfaces:**
- Consumes: Task 4 (`SearchScreen(session:, damagedEntries:)`), Task 3.

- [ ] **Step 1: Update and add tests.**
  - `app_test.dart`: las 9 comprobaciones de `'Pantalla principal: #40'` pasan a `find.byType(SearchScreen)` (las de `findsNothing` también).
  - `auto_lock_test.dart`: quitar `mainText`; `expect(find.text(mainText), …)` → `find.byType(SearchScreen)`; `mainContext` → `tester.element(find.byType(SearchScreen))`.
  - `widgets_test.dart`: las dos pruebas de `PlaceholderScreen` se reducen a una, `'PlaceholderScreen muestra el texto'`, con `'No se pudo abrir la bóveda.'`.
  - Nuevas en `app_test.dart` (ayudante `bool searchHasFocus(tester)` = `fieldHasFocus(tester, find.byType(SearchScreen))`):
    - `'tras desbloquear con contraseña, la búsqueda tiene el foco'` (con `unlockFromScreen`).
    - `'tras desbloquear con huella, la búsqueda tiene el foco'`: `biometric..available = true..enabled = true`; `pumpApp` con bóveda; `pumpAndSettle` → `SearchScreen` y foco. (El doble de huella devuelve la llave enseguida y `unlockWithKey` un reporte vacío.)
    - `'tras crear sin huella, la búsqueda tiene el foco'` (con `createFromFirstRun`).
    - `'tras la oferta de la huella, la búsqueda tiene el foco'`: oferta → `offer-skip` → foco.
    - `'la pantalla principal pasa el total de dañadas'`: la prueba existente de 3 dañadas ya lo cubre; solo cambia el finder.
  - Nuevas en `auto_lock_test.dart`:
    - `'bloquear descarta el texto de búsqueda'`: `pumpUnlocked`; `enterText(find.byType(SearchScreen) > TextField, 'gm')`; `setLifecycle(hidden)`; `expectLocked`; `submitPassword`; `drain` → el `TextField` de la búsqueda tiene texto vacío y `session.searchCalls.last == ''`.
    - `'escribir en la búsqueda reinicia la inactividad'`: con `timeout` corto (como las pruebas de inactividad existentes), escribir en la búsqueda antes de que venza → sigue en `SearchScreen` pasado el tiempo original; sin escribir, se bloquea.
- [ ] **Step 2: Run to verify the new ones fail**

Run: `flutter test test/app_test.dart test/auto_lock_test.dart`
Expected: FAIL (se ve el marcador, no `SearchScreen`).

- [ ] **Step 3: Implement**: `VaultGateState.unlocked => SearchScreen(session: widget.session, damagedEntries: _damagedEntries)`; quitar el import de `damaged_entries_banner.dart` de `app.dart` si queda sin uso; quitar `banner` de `PlaceholderScreen` y su comentario. Actualizar el comentario de `VaultGate`: el estado desbloqueado es `SearchScreen`, y el remontaje por `_epoch` descarta el texto de búsqueda.
- [ ] **Step 4: Run**

Run: `flutter test` y `flutter analyze`
Expected: PASS; `grep -rn "Pantalla principal: #40" lib test` sin resultados.
- [ ] **Step 5: Commit**

```bash
git add mobile/lib/app.dart mobile/lib/screens/placeholder_screen.dart mobile/test/app_test.dart mobile/test/auto_lock_test.dart mobile/test/widgets_test.dart
git commit -m "feat: mostrar la pantalla principal al desbloquear"
```

### Task 6: prueba de rendimiento con 500 entradas

**Files:**
- Create: `mobile/integration_test/search_performance_test.dart`

**Interfaces:**
- Consumes: `VaultSession` real (`create`, `add`, `search`, `revealPassword`), `EntryInput`, `NewSecret`, `SearchScreen` (Task 4).

Estructura fijada:
- Al inicio de `main`: `final binding = IntegrationTestWidgetsFlutterBinding.ensureInitialized(); binding.framePolicy = LiveTestWidgetsFlutterBindingFramePolicy.benchmarkLive;` (referencia: `dev/benchmarks/macrobenchmarks/test/util.dart` del SDK; el *cookbook* no lo fija). `setUpAll(() async => await RustLib.init());`
- Datos deterministas: 50 servicios base (incluye `'Gmail'`, `'Google'`, `'Teléfono'`, `'Ñandú Bank'`, `'Banco Ávila'`…) × 10 variantes (`'$base'` para la primera, `'$base $i'` para las demás) = 500; usuario `'usuario$i@correo.com'` en las pares; URL `'https://$dominio/login'` en múltiplos de 3 y `'$dominio'` sin esquema en múltiplos de 5; etiquetas de `['trabajo', 'personal', 'teléfono', 'compras']` rotando. Contraseña `'secreto-$i'`.
- Un solo `testWidgets` (la sesión se crea una vez) o `setUpAll` que crea la bóveda en `Directory.systemTemp.createTemp('boveda-perf-')` y la borra en `tearDownAll`.
- `pumpUntil(WidgetTester tester, Finder expected, {Duration timeout = const Duration(seconds: 5)})`: bombea `tester.pump(const Duration(milliseconds: 16))` hasta que `expected` encuentre algo; si vence, `fail('…')` con el texto esperado.

- [ ] **Step 1: Write the test** con estos pasos y aserciones:
  1. Funcionales: `search('gm')` contiene `'Gmail'`; `search('gmial')` contiene `'Gmail'`; `revealPassword` de la entrada `'Ñandú Bank 3'` → `search('')` tiene a `'Ñandú Bank 3'` primero.
  2. Ida y vuelta: llamada de calentamiento `search('')` descartada; luego `''` y cada prefijo de `'gmial'`, `'teléfono trabajo'`, `'nandu'`, medidos con `Stopwatch`. `debugPrint` del máximo y la mediana en ms (`'search ida y vuelta: mediana X ms, máximo Y ms, n=N'`). `expect(cada, lessThan(50))` en ms.
  3. Interfaz: `pumpWidget(MaterialApp(home: SearchScreen(session: session, damagedEntries: 0)))`; `pumpUntil` del primer elemento de `''`. Dentro de `binding.watchPerformance(() async { … }, reportKey: 'performance')`: para cada prefijo creciente de `'gmial'` y de `'teléfono trabajo'` → `enterText` + `pumpUntil` del primer resultado esperado de ese prefijo (obtenido antes con `session.search` fuera de la medición) o de `'Sin resultados para «…»'`; después, una pasada borrando letra por letra hasta `''` con el mismo `pumpUntil`.
  4. Leer `final perf = binding.reportData!['performance'] as Map<String, dynamic>;` `debugPrint` de `90th_percentile_frame_build_time_millis` y `90th_percentile_frame_rasterizer_time_millis`; `expect(…, lessThan(16))` en ambos.
- [ ] **Step 2: Comprobar el emulador**: `adb` en `$LOCALAPPDATA/Android/Sdk/platform-tools/adb.exe`; emulador encendido; `rustup run stable rustc --version` igual al `channel` de `rust-toolchain.toml`. Anotar modelo del AVD y versión de Android (`adb shell getprop ro.build.version.release` y `ro.product.model`).
- [ ] **Step 3: Run** (visto bueno del usuario para descargar los artefactos de profile)

Run (desde `mobile/`): `flutter drive --profile --no-dds --driver=test_driver/integration_test.dart --target=integration_test/search_performance_test.dart -d <emulador>`
Expected: `All tests passed!` con las líneas de mediana, máximo y percentiles. **Si falla por algo distinto de los umbrales (por ejemplo, profile rechazado en el emulador), detenerse y mostrar el error al usuario.** Si fallan los umbrales, detenerse, mostrar la medición y proponer el retardo de la decisión 1.
- [ ] **Step 4: `flutter analyze`** → sin problemas.
- [ ] **Step 5: Commit** (las mediciones se guardan para el PR, no en el repositorio)

```bash
git add mobile/integration_test/search_performance_test.dart
git commit -m "test: rendimiento de la búsqueda con 500 entradas"
```

### Task 7: documentación

**Files:**
- Modify: `docs/especificacion.md` (sección "Experiencia de uso", después de "**Pantalla principal.** …")
- Modify: `ARCHITECTURE.md` (mapa: `app.dart`, `screens/`, `widgets/`; sección `mobile/`, viñeta de `lib/app.dart`)
- Modify: `mobile/CLAUDE.md` (regla de `ActivityTextField`)
- Modify: `docs/superpowers/specs/2026-11-10-pantalla-principal-design.md` (sección "Hallazgos de la implementación" si hubo alguno, y la precisión del Review Focus 4)

- [ ] **Step 1: Write** los textos que fija el diseño, sección "Documentación" (párrafo "Decidido el 2026-11-10 (#40, …)" con el formato de los de la #37 y la #38). En el diseño, agregar a `SearchScreen` el `ScrollController` que vuelve al principio con resultados de otro texto.
- [ ] **Step 2: Run** `flutter analyze` y `flutter test` (sin cambios de código, deben seguir pasando).
- [ ] **Step 3: Commit**

```bash
git add docs/especificacion.md ARCHITECTURE.md mobile/CLAUDE.md docs/superpowers/specs/2026-11-10-pantalla-principal-design.md
git commit -m "docs: especificación, arquitectura y reglas de mobile"
```

### Task 8: verificación completa, prueba a mano y roadmap

**Files:**
- Modify: `docs/roadmap.md` (solo al final)

- [ ] **Step 1: Suite completa**: desde `mobile/`, `flutter analyze` y `flutter test`; desde la raíz, `cargo test --workspace` y `cargo clippy --workspace --all-targets -- -D warnings`. Todo pasa. `git diff dev --stat -- core mobile/rust` vacío.
- [ ] **Step 2: Prueba a mano en el emulador** (`flutter run` en debug), los ocho casos del diseño, "A mano, en el emulador". Para el caso 8, bajar `defaultAutoLockTimeout` en `lib/services/auto_lock.dart` solo en la copia local (por ejemplo, 20 s), probar y restaurarlo con `git checkout -- mobile/lib/services/auto_lock.dart` **después de confirmar que no hay otros cambios sin commit en ese archivo** (`git diff mobile/lib/services/auto_lock.dart` muestra solo el valor). Si `autofocus` no abre el teclado en alguno de los tres caminos, detenerse con la evidencia (logcat) antes de probar otra vía.
- [ ] **Step 3: Roadmap**, solo si pasaron la prueba de rendimiento y los ocho casos: marcar "Pantalla principal con la barra de búsqueda enfocada y la lista filtrándose al escribir" en `docs/roadmap.md`.

```bash
git add docs/roadmap.md
git commit -m "docs: marcar la pantalla principal en el roadmap"
```

- [ ] **Step 4:** revisión final en opus de toda la rama y PR contra `dev` según el diseño, sección "PR" (mediciones, emulador y versión de Android, ocho casos marcados). El comentario en la #41, solo con la confirmación del usuario.
