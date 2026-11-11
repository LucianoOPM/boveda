# Diseño: pantalla principal con búsqueda enfocada y lista filtrada (issue #40)

Fecha: 2026-11-10 · Etapa 2 · Rama `feat/40-pantalla-principal`

## Objetivo

Reemplazar el marcador de la pantalla principal ("Pantalla principal: #40") por la pantalla real: al desbloquear, el usuario llega a una barra de búsqueda con el teclado abierto, escribe "gm" y la lista muestra Gmail, aunque haya un error de una letra. Sin texto, la lista muestra primero las entradas usadas recientemente (`docs/especificacion.md`, "Experiencia de uso → Pantalla principal").

Toda la búsqueda vive en el núcleo (`Vault::search`, issue #8) y la fachada ya la expone (`VaultSession.search`, issue #35). Esta issue es solo Dart: **sin cambios en `core/` ni en `mobile/rust/src/api/`, y sin regenerar el puente**.

De paso se resuelve el pendiente registrado en la #41 desde la #37: cerrar el teclado al tocar fuera de un campo, en toda la app.

### Fuera de alcance

- Deslizar para enviar a la PC (etapa 4).
- Lógica de búsqueda en Dart: Dart no filtra, no reordena ni extrae el dominio de la URL.
- Botón para crear (#41), detalle de la entrada (#41) y acceso a ajustes (#44): no se muestran hasta su issue. Tocar una entrada no hace nada por ahora.
- Repetir la búsqueda al volver de otra ruta (#41, ver "Nota para la #41").
- Marcar en la lista las entradas dañadas (ver "Entradas dañadas").

### Criterios de aceptación (de la issue)

- Al desbloquear, el teclado aparece con el cursor en la búsqueda. **Cambiado por decisión del usuario tras la prueba a mano:** la búsqueda no se enfoca al llegar (ver "Sin foco inicial").
- Escribir "gm" encuentra "Gmail"; un error de una letra también la encuentra.
- Sin texto, las usadas recientemente aparecen primero.
- La interfaz no se traba al escribir con unos cientos de entradas (probado en el emulador).
- Pruebas de widgets con el puente simulado.
- `flutter analyze` y `flutter test` pasan; CI en verde.

## Lo que confirma la documentación

Verificado con Context7 (flutter_rust_bridge v2.13.0, API de Flutter, sitio de Flutter) y con el código del SDK local (Flutter 3.47.9):

- **Costo de llamar al núcleo.** Una función de Rust sin `#[frb(sync)]` se ejecuta en un *thread pool* y la llamada asíncrona cuesta del orden de 0.1 ms ("even if your Rust code computes for 100ms, the async call will only take, say, 0.1ms, and will not block your UI"). `search` recorre la caché en memoria (sin disco ni Argon2). Varias búsquedas en curso esperan el `Mutex` de la sesión y pueden responder en otro orden.
- **`onTapOutside` por omisión en Android.** `_EditableTextTapOutsideAction` (`widgets/editable_text.dart`) solo quita el foco con un toque de dedo si es web; con mouse, lápiz u otros dispositivos sí lo quita. La documentación de `EditableTextTapUpOutsideIntent` lo confirma: "unfocus on a touch event on web and do nothing on other platforms".
- **`onTapOutside` solo se conecta con el campo enfocado.** `EditableText` pasa `onTapOutside: _hasFocus ? … : null` a su `TextFieldTapRegion`: un campo sin foco nunca reacciona.
- **Región del campo.** `TextField` envuelve todo su contenido, decoración incluida (`prefixIcon`, `suffixIcon`), en un `TextFieldTapRegion`. Su `groupId` por omisión es `EditableText`, el mismo que usan todos los campos. Por eso tocar el ✕ de la búsqueda, el ojo de `PasswordField` o un campo vecino no dispara `onTapOutside`. Un `TapRegion` agrupa con `groupId`: si se toca cualquier miembro del grupo, ninguno recibe `onTapOutside`.
- **La tecla de acción quita el foco por omisión.** Sin `onEditingComplete`, `EditableText._finalizeEditing` hace `unfocus()` con `TextInputAction.search` (y `done`, `go`, `send`…). Para que la tecla "buscar" no haga nada hay que pasar un `onEditingComplete` vacío.
- **Desactivar un campo le quita el foco.** `TextField` pone `canRequestFocus = enabled`, y `FocusNode.canRequestFocus = false` hace `unfocus()` si tenía el foco. Durante "Desbloqueando…" y "Creando…" el teclado ya se cierra hoy porque el campo se desactiva; agrupar el botón no cambia eso.
- **`autofocus`.** Un `TextField` con `autofocus` toma el foco "if nothing else is already focused" en su `FocusScope`.
- **`keyboardDismissBehavior: onDrag`.** El `ScrollView` cierra el teclado en pantalla cuando empieza un arrastre.
- **`characters`.** `package:flutter/widgets.dart` exporta `package:characters`: `texto.characters.first` da el primer grafema sin dependencia nueva.
- **Medir cuadros.** `IntegrationTestWidgetsFlutterBinding.watchPerformance` guarda en `reportData[reportKey]` el resumen de `FrameTimingSummarizer`, con `90th_percentile_frame_build_time_millis` y `90th_percentile_frame_rasterizer_time_millis`. En modo debug imprime una advertencia: los tiempos no son representativos. Hay que medir en profile (`flutter drive --profile`).
- **Profile en el emulador.** La documentación se contradice: la página de modos de compilación dice que profile está desactivado en emuladores; el *cookbook* de *profiling* indica agregar `--no-dds` "on a mobile device or emulator". Manda el código: en `flutter_tools` 3.47.9, `AndroidDevice.supportsRuntimeMode` acepta profile en `android_x64` y `android_arm64` (solo rechaza `jitRelease`). Se confirma al ejecutar en el emulador.
- **`framePolicy` para medir.** El *cookbook* no lo fija (usa `traceAction`). El código de Flutter que mide con `watchPerformance` (`dev/benchmarks/macrobenchmarks/test/util.dart`) fija `LiveTestWidgetsFlutterBindingFramePolicy.benchmarkLive`: el motor programa los cuadros que pide la app y los `pump` de la prueba no agregan cuadros artificiales.

## Decisiones

1. **Cuándo buscar.** Una llamada por cada cambio del texto, sin retardo, con un contador de generación que descarta las respuestas viejas. Solo se agrega un retardo si la prueba de rendimiento muestra trabas, y en ese caso se documenta aquí con la medición.
2. **Cada elemento.**
   - Título: el servicio.
   - Subtítulo: el usuario. Sin usuario, la URL tal como está guardada, en una línea y recortada con elipsis. Sin ninguno de los dos, sin subtítulo.
   - A la izquierda, un círculo con la inicial del servicio: el primer grafema (`characters`), no el primer código, para que funcione con acentos y emoji. Va en `ExcludeSemantics`, para que el lector de pantalla no lea la letra antes del servicio.
   - Las etiquetas no se muestran.
   - La lista respeta el orden de `search`: Dart no reordena ni filtra.
   - No se extrae el dominio en Dart: `Uri.tryParse("gmail.com").host` da vacío sin esquema, justo el caso que `domain()` del núcleo resuelve a mano, y duplicaría una regla de la especificación con otro comportamiento. Si el dominio hace falta en la interfaz, se expone desde la fachada en su propia issue.
3. **Teclado del campo de búsqueda.** `enableSuggestions`, `autocorrect` y `enableIMEPersonalizedLearning` en `false`, para que el teclado no aprenda qué servicios se buscan. Acción "buscar" sin efecto. El ✕ aparece solo con texto, borra el texto y deja el foco en el campo.
4. **Crear y ajustes.** No se muestran hasta la #41 y la #44; tocar una entrada no hace nada por ahora. El texto del estado vacío no menciona ningún botón.
5. **Cerrar el teclado** (pendiente de la #41, se resuelve aquí):
   - En la lista, `keyboardDismissBehavior: onDrag`.
   - `onTapOutside` quita el foco en `ActivityTextField`, y por lo tanto en `PasswordField`: aplica en toda la app.
   - Los controles que forman parte del formulario no lo quitan: los íconos dentro del campo (ya están en su región), y los botones de envío y las casillas de las pantallas de contraseña, envueltos en `TextFieldTapRegion`.
   - Tocar una entrada de la lista sí cierra el teclado.
6. **Estados.** Bóveda sin entradas: "Todavía no hay contraseñas guardadas". Sin resultados: "Sin resultados para «texto»". Primera carga: un indicador. Fallo de `search`: "No se pudo buscar", sin detalles, con un botón "Reintentar". Un `Locked` por la carrera con el bloqueo se ignora en silencio. El aviso de dañadas queda arriba de la lista, dentro de la pantalla, una vez por desbloqueo (#37, #39).
7. **Repetir la búsqueda al volver** de otra ruta: no se prepara. La #41 decide cómo vuelve de su ruta y agrega ahí la nueva búsqueda.
8. **Rendimiento.** Prueba en `integration_test/` con 500 entradas reales, medida en profile en el emulador (ver "Pruebas"). Criterio: cada `search` de ida y vuelta por debajo de 50 ms; percentil 90 de construcción y de rasterizado de cuadros por debajo de 16 ms. La prueba a mano se hace mirando el emulador mientras corre esa misma prueba, sin código de depuración en `lib/`.
9. **Nombre.** `lib/screens/search_screen.dart` con `SearchScreen`. `PlaceholderScreen` queda solo para el error al abrir (`openFailed`). El texto de búsqueda no se conserva entre bloqueos: el remontaje por `_epoch` ya lo descarta.

### Entradas dañadas: corrección a una nota de la #37

El diseño de la #37 dice que "la #40 marcará cada entrada en la lista". No se puede cumplir: las entradas dañadas nunca llegan a la caché de búsqueda del núcleo (`Unlocked.entries`, #7 y #8), así que `search` no las devuelve, y no tienen un servicio legible que mostrar; del `UnlockReport` solo se conocen sus ids (o ni eso). El criterio "una entrada corrupta se marca como dañada sin bloquear el resto" queda cubierto por el aviso con el total (`DamagedEntriesBanner`) arriba de la lista. El diseño de la #37 no se modifica; la corrección vive aquí.

## Componentes

### `SearchScreen` (`lib/screens/search_screen.dart`, nuevo)

```dart
class SearchScreen extends StatefulWidget {
  const SearchScreen({
    super.key,
    required this.session,
    required this.damagedEntries,
  });

  final VaultSession session;

  /// Total de entradas dañadas del último desbloqueo (0 al crear).
  final int damagedEntries;
}
```

**Estado:**

- `TextEditingController _query`, `FocusNode _focus` y `ScrollController _scroll`, de la pantalla; se liberan en `dispose`.
- `int _generation`: sube con cada búsqueda lanzada.
- `List<EntrySummary>? _results`: `null` mientras no llega la primera respuesta.
- `String _shownQuery`: el texto al que corresponden los resultados en pantalla.
- `bool _failed`.

**Búsqueda.** `initState` lanza `_search('')`. Cada cambio del texto hace `setState(() {})` (para que el ✕ siga al texto, no a la respuesta) y lanza `_search(texto)`:

```dart
Future<void> _search(String query) async {
  final generation = ++_generation;
  try {
    final results = await widget.session.search(query: query);
    if (!mounted || generation != _generation) return; // respuesta vieja
    setState(() {
      _results = results;
      _shownQuery = query;
      _failed = false;
    });
  } on VaultError catch (e) {
    if (e == VaultError.locked) return; // carrera con el bloqueo: silencio
    _fail(generation);
  } catch (e) {
    if (kDebugMode) debugPrint('search: ${e.runtimeType}');
    _fail(generation);
  }
}
```

- `_fail(generation)` marca `_failed = true` solo si la pantalla sigue montada y la generación sigue vigente. La siguiente letra, o "Reintentar", vuelve a intentar.
- El `debugPrint` solo existe en modo debug y solo lleva el tipo del error: ni el mensaje ni el texto buscado.
- Un `VaultError` distinto de `locked` no se registra: es un error esperado del núcleo, no de programación.

**Cuerpo, según el estado:**

| Estado | Se muestra |
| --- | --- |
| `_failed` | "No se pudo buscar" y debajo un botón "Reintentar", que llama a `_search(_query.text)` |
| `_results == null` | Indicador de progreso (solo en la primera carga) |
| Vacío y `_shownQuery.trim()` vacío | "Todavía no hay contraseñas guardadas" (el núcleo trata un texto de solo espacios como vacío) |
| Vacío con texto | "Sin resultados para «`_shownQuery.trim()`»" |
| Con resultados | La lista |

Mientras llega una búsqueda nueva se siguen viendo los resultados anteriores, sin parpadeo. Cuando llegan resultados de un texto distinto del que se mostraba, la lista vuelve al principio (`_scroll.jumpTo(0)`): si el usuario la había desplazado y los resultados nuevos siguen siendo largos, el mejor resultado tiene que verse arriba (precisión agregada en el plan, Review Focus 4).

**Estructura:** `Scaffold` sin `AppBar` → `SafeArea` → `Column`:

1. `DamagedEntriesBanner(count: widget.damagedEntries)`: dentro de la pantalla, así sale del árbol al bloquear y su estado de oculto no se arrastra.
2. El campo de búsqueda (con un margen).
3. `Expanded` con el cuerpo.

**Lista:** `ListView.builder` con `keyboardDismissBehavior: ScrollViewKeyboardDismissBehavior.onDrag`. Cada elemento es un `ListTile` con `key: ValueKey(entry.id)` (el id cruza como `String` hexadecimal, que compara por valor):

- `leading`: `ExcludeSemantics(child: CircleAvatar(child: Text(inicial)))`, con `inicial = service.characters.first.toUpperCase()`; `'?'` si el servicio llegara vacío (el núcleo no lo permite; es solo defensa).
- `title`: `Text(service)`.
- `subtitle`: `Text(username ?? url, maxLines: 1, overflow: TextOverflow.ellipsis)` si alguno existe; si no, `null`.
- `onTap`: `null` hasta la #41. Tocar el elemento igual cierra el teclado, porque queda fuera de la región del campo.

**Campo de búsqueda:**

```dart
ActivityTextField(
  controller: _query,
  focusNode: _focus,
  textInputAction: TextInputAction.search,
  onEditingComplete: () {}, // la tecla "buscar" no quita el foco
  onChanged: _onChanged,
  enableSuggestions: false,
  autocorrect: false,
  enableIMEPersonalizedLearning: false,
  decoration: InputDecoration(
    hintText: 'Buscar',
    prefixIcon: const Icon(Icons.search),
    suffixIcon: _query.text.isEmpty
        ? null
        : IconButton(
            tooltip: 'Borrar búsqueda',
            icon: const Icon(Icons.close),
            onPressed: _clear,
          ),
  ),
)
```

- `_onChanged(texto)`: `setState(() {})` y `_search(texto)`.
- `_clear()`: `_query.clear()`, `setState(() {})`, `_focus.requestFocus()` y `_search('')`. `clear()` no dispara `onChanged`, por eso se busca directo. `requestFocus` cubre el caso en que el teclado se cerró al desplazar y el usuario toca ✕: el foco vuelve al campo. Tocar ✕ cuenta como actividad por el `Listener` global de `BovedaApp`.
- Escribir cuenta como actividad por `ActivityTextField`.

**Sin foco inicial.** La búsqueda no se enfoca al llegar: el teclado sale cuando el usuario toca el campo. Decisión del usuario tras la prueba a mano, que reemplaza el `autofocus: true` diseñado al principio.

El diseño original usaba `autofocus`, porque en ningún camino de llegada corre la animación de arranque en frío que en la #37 impidió abrir el teclado. En el emulador (Pixel 10, Android 17, API 37) funcionó tras la contraseña y tras el primer uso con la oferta de la huella, pero **no tras desbloquear con huella**, ni en frío ni al volver a la app (tres repeticiones): el campo quedaba enfocado y sin teclado. Logcat, en el momento de montar la pantalla:

```
W InputMethodManager: Ignoring showSoftInput() as view=...FlutterView... is not served.
I ImeTracker: onRequestHide at ORIGIN_SERVER reason HIDE_UNSPECIFIED_WINDOW
```

El desbloqueo termina mientras la ventana del `BiometricPrompt` todavía tiene el foco (la app está en `inactive`): Android descarta la petición del teclado y, cuando la app recupera la ventana, lo oculta. Se propuso volver a pedir el teclado al llegar `resumed`; el usuario prefirió quitar el foco inicial en todos los caminos, para que la app se comporte igual con contraseña y con huella, como el desbloqueo de la #37. El `autofocus` de `ActivityTextField` se quitó con él, porque nada más lo usaba.

### `VaultGate` (`lib/app.dart`)

El estado `unlocked` muestra `SearchScreen(session: widget.session, damagedEntries: _damagedEntries)` en lugar de `PlaceholderScreen`, dentro del mismo `KeyedSubtree(key: ValueKey(_epoch))`. Cada bloqueo sube `_epoch` y desmonta la pantalla: el texto de búsqueda, los resultados y el aviso oculto se descartan. Una búsqueda pendiente encuentra `mounted == false` y no hace nada. Se actualiza el comentario de la clase.

### `ActivityTextField` (`lib/widgets/activity_text_field.dart`)

- Parámetro nuevo, pasado tal cual al `TextField`: `onEditingComplete`. (Llevó también `autofocus` hasta que se quitó el foco inicial.)
- `onTapOutside` fijo, no configurable: `(_) => FocusManager.instance.primaryFocus?.unfocus()`. Solo se llama con el campo enfocado, así que `primaryFocus` es ese campo; no depende de que quien lo use pase un `focusNode`.
- El comentario de la clase explica la regla: tocar fuera de cualquier campo cierra el teclado (en Android, Flutter no lo hace por omisión con el dedo); los controles del formulario que no deben cerrarlo se envuelven en `TextFieldTapRegion`.

`PasswordField` se arma sobre él: el cambio llega al primer uso, al desbloqueo, a la búsqueda y a los campos de la #41 sin cambiar la API de `PasswordField`. Su ojo de mostrar u ocultar ya está dentro de la región del campo.

### Pantallas de contraseña

Envueltos en `TextFieldTapRegion` (grupo `EditableText`, el de todos los campos):

- `FirstRunScreen`: la casilla "Entiendo que si olvido…" y el botón "Crear bóveda".
- `UnlockScreen`: la casilla "Usar huella para desbloquear" y el botón "Desbloquear".

Las casillas son parte del formulario: el usuario escribe, marca y pulsa; si la casilla cerrara el teclado, tendría que volver a tocar el campo para corregir lo escrito.

Quedan fuera, y cierran el teclado: "Usar huella" (abre el diálogo del sistema) y los botones de `BiometricOfferScreen` (no tiene campos).

Sin cambios: durante "Desbloqueando…" y "Creando…" el campo se desactiva y suelta el foco (ver "Lo que confirma la documentación"); tras una contraseña incorrecta se reenfoca, como fijó la #37.

### Cierre del pendiente de la #41

El comentario de la #41 (registrado desde la #37) queda resuelto aquí:

- Tocar fuera de un campo cierra el teclado en todos los campos de la app (`ActivityTextField`, y por lo tanto `PasswordField`).
- Desplazar una lista lo cierra (`onDrag`).
- Los íconos dentro del campo, los botones de envío y las casillas de los formularios no lo cierran.
- La #41 lo hereda usando `ActivityTextField` y `PasswordField`; solo tiene que envolver su botón "Guardar" (y las casillas o controles de su formulario) en `TextFieldTapRegion`.

### Nota para la #41

- Revelar una contraseña o unas notas actualiza `last_used_at` y cambia el orden de recientes. La #41 decide cómo vuelve de su ruta (resultado del `pop`, `await` del `push`) y repite ahí la búsqueda con el texto actual.
- Tocar una entrada hoy no hace nada (`onTap: null`); la #41 lo conecta al detalle.
- La #41 agrega el botón para crear; el estado vacío no lo menciona y puede quedarse así o ajustarse en la #41.

## Pruebas

### Doble de sesión (`test/fakes.dart`)

`FakeVaultSession.search` deja de lanzar `UnimplementedError`:

- `searchCalls`: los textos recibidos, en orden.
- `onSearch`: `Future<List<EntrySummary>> Function(String query)`; por omisión devuelve `[]` enseguida, para que las pruebas que llegan a la pantalla principal no fallen.
- Un ayudante para armar `EntrySummary` de prueba (`summary(service, {username, url, id, …})`).

Las 14 comprobaciones de `'Pantalla principal: #40'` (`app_test`, `auto_lock_test`, `widgets_test`) pasan a `find.byType(SearchScreen)`.

### Widgets: `SearchScreen` (`test/search_screen_test.dart`, nuevo)

La pantalla montada sola, con el doble:

| Caso | Comprobación |
| --- | --- |
| Primera carga | Con un `Completer` pendiente, indicador; al completarse, la lista en el orden recibido, sin reordenar |
| Elemento | Título = servicio; subtítulo = usuario; sin usuario, la URL en una línea con elipsis; sin ninguno, sin subtítulo |
| Inicial | "gmail" → "G", "Ébano" → "É", "🔑 Llaves" → "🔑" (grafema completo); la semántica del elemento no incluye la letra |
| Por cada cambio | Escribir "g" y luego "gm" → `searchCalls == ['', 'g', 'gm']` |
| Respuesta vieja | La de "g" llega después de la de "gm" → se ven los resultados de "gm"; un fallo de una generación vieja no muestra el error |
| Vacía | "Todavía no hay contraseñas guardadas", también con texto de solo espacios |
| Sin resultados | "Sin resultados para «xyz»" |
| Fallo | "No se pudo buscar" + "Reintentar" → repite con el texto actual y muestra la lista |
| `Locked` | Sin mensaje; se conserva lo que había |
| Tras `dispose` | Una respuesta que llega con la pantalla desmontada no lanza nada |
| ✕ | No aparece sin texto; aparece al escribir con la búsqueda pendiente; tocarlo borra, deja el foco y busca `''`; tras perder el foco al desplazar, tocarlo lo devuelve |
| Tecla "buscar" | `testTextInput.receiveAction(TextInputAction.search)` → el campo sigue con el foco |
| Teclado | El `TextField` tiene `enableSuggestions`, `autocorrect` y `enableIMEPersonalizedLearning` en `false`, y `TextInputAction.search` |
| Foco | Al montar, el campo no tiene el foco |
| Cerrar teclado | Desplazar la lista (con entradas suficientes para desplazarla), tocar una entrada y tocar un espacio vacío quitan el foco |
| Aviso | Con `damagedEntries: 2`, el aviso arriba de la lista; con 0, nada |

### Widgets: componentes (`test/widgets_test.dart`)

- `ActivityTextField`: un toque fuera (tipo *touch*, el de `tester.tap`, con la plataforma Android de las pruebas) quita el foco; un toque en un `TextFieldTapRegion` vecino no; `autofocus` y `onEditingComplete` llegan al `TextField`.
- `PasswordField`: tocar el ojo no quita el foco; un toque fuera sí.

### Widgets: pantallas de contraseña

- `UnlockScreen` (`test/unlock_screen_test.dart`): la casilla no quita el foco del campo; "Desbloquear", tocado mientras está desactivado (campo vacío), tampoco, lo que prueba que pertenece al grupo sin que el campo se desactive; un espacio vacío sí lo quita; "Usar huella" sí lo quita.
- `FirstRunScreen` (`test/first_run_screen_test.dart`): la casilla y "Crear bóveda" desactivado no quitan el foco; un espacio vacío sí.

### Widgets: flujo de la app (`test/app_test.dart`, `test/auto_lock_test.dart`)

- Al llegar a la pantalla principal, el campo de búsqueda no tiene el foco por los tres caminos: contraseña, huella y crear (directo y por la oferta de la huella).
- Escribir, bloquear (`hidden`) y desbloquear deja el campo vacío y vuelve a buscar `''`.
- Escribir en la búsqueda reinicia el temporizador de inactividad.

### Dispositivo: rendimiento (`integration_test/search_performance_test.dart`, nuevo)

Con la librería de Rust real, en el emulador y en profile:

1. **Preparación.** `binding.framePolicy = LiveTestWidgetsFlutterBindingFramePolicy.benchmarkLive` (como `dev/benchmarks/macrobenchmarks/test/util.dart`). Crea una bóveda en un directorio temporal absoluto (calibra Argon2id) y agrega 500 entradas por la fachada, deterministas: unos 50 servicios base con variantes ("Gmail", "Teléfono", "Ñandú Bank"…), usuarios, URLs con y sin esquema, y etiquetas con acentos.
2. **Criterios funcionales con el núcleo real.** "gm" encuentra Gmail; "gmial" también; tras `revealPassword` de una entrada, `search('')` la devuelve primero.
3. **Ida y vuelta.** Un `Stopwatch` alrededor de `session.search` para `''` (la respuesta más grande: 500 entradas cruzando) y para cada prefijo de unas frases ("gmial", "teléfono trabajo"…). La primera llamada es de calentamiento y no cuenta. La prueba imprime el máximo y la mediana, y exige menos de 50 ms en cada llamada.
4. **Interfaz.** Monta `SearchScreen` sola sobre esa sesión. Dentro de `binding.watchPerformance`, escribe letra por letra (`enterText` con prefijos crecientes) y, después de cada prefijo, un ayudante `pumpUntil` bombea cuadros hasta que la pantalla muestre los resultados de ese prefijo (el primer elemento esperado, o "Sin resultados para «…»"), con un tiempo límite que hace fallar la prueba. Así se mide la reconstrucción de la lista con cada respuesta, no cuadros vacíos. Después, una segunda pasada borrando letra por letra, que vuelve a listas grandes. Al terminar, lee `binding.reportData['performance']` y exige `90th_percentile_frame_build_time_millis` y `90th_percentile_frame_rasterizer_time_millis` por debajo de 16 ms; también los imprime.
5. Borra el directorio temporal.

Se ejecuta con:

```
flutter drive --profile --no-dds \
  --driver=test_driver/integration_test.dart \
  --target=integration_test/search_performance_test.dart \
  -d <emulador>
```

La primera compilación en profile descarga los artefactos `android-*-profile` del motor en la caché de Flutter (aprobado por el usuario). Si la ejecución en profile falla en el emulador por algo distinto de lo visto en `flutter_tools`, se detiene el trabajo y se muestra el error antes de buscar otra vía. La CI no ejecuta `integration_test/`.

Si la prueba muestra trabas, se agrega un retardo a la búsqueda y se documenta aquí con la medición (decisión 1).

### A mano, en el emulador

1. Desbloqueo con contraseña → la búsqueda sin foco y el teclado cerrado; tocar el campo lo abre.
2. Arranque en frío con huella → lo mismo.
3. Instalación limpia → primer uso → oferta de la huella → lo mismo.
4. Bóveda vacía: "Todavía no hay contraseñas guardadas".
5. Teclado: tocar fuera lo cierra; la tecla "buscar" no; el ✕ borra y lo deja abierto; en el desbloqueo y el primer uso, las casillas no lo cierran y un espacio vacío sí.
6. Inicio → volver → desbloquear: el campo de búsqueda vacío.
7. Mirar el emulador mientras corre la prueba de rendimiento: la lista se filtra sin trabas a cada letra (sin el teclado real: la prueba usa el simulado, ver "Hallazgos"). (La app real todavía no puede agregar entradas, #41.)
8. Con el teclado abierto y texto en la búsqueda, esperar la inactividad sin tocar nada: se bloquea, y al desbloquear el campo está vacío y sin foco. Para no esperar 5 minutos se baja `defaultAutoLockTimeout` solo en la copia local, sin commit, y se restaura después.

### Antes de dar la issue por terminada

`flutter analyze` y `flutter test` (desde `mobile/`), `cargo test --workspace` y `cargo clippy --workspace --all-targets -- -D warnings` (desde la raíz), aunque Rust no cambie. Después, la prueba de rendimiento, los ocho casos a mano y la revisión final.

## Archivos

```
mobile/lib/screens/search_screen.dart          # nuevo
mobile/lib/app.dart                            # unlocked → SearchScreen
mobile/lib/widgets/activity_text_field.dart    # onTapOutside, onEditingComplete
mobile/lib/screens/first_run_screen.dart       # TextFieldTapRegion en casilla y botón
mobile/lib/screens/unlock_screen.dart          # TextFieldTapRegion en casilla y botón
mobile/test/fakes.dart                         # search en el doble
mobile/test/search_screen_test.dart            # nuevo
mobile/test/widgets_test.dart
mobile/test/unlock_screen_test.dart
mobile/test/first_run_screen_test.dart
mobile/test/app_test.dart
mobile/test/auto_lock_test.dart
mobile/integration_test/search_performance_test.dart  # nuevo
```

## Documentación

- `docs/especificacion.md`, "Pantalla principal": párrafo "Decidido el 2026-11-10 (#40, detalle en `docs/superpowers/specs/2026-11-10-pantalla-principal-design.md`)": búsqueda en cada cambio, sin retardo, descartando respuestas viejas; cada elemento muestra el servicio y el usuario (o la URL), en el orden del núcleo; las dañadas no aparecen en la lista, solo en el aviso con el total; en toda la app, tocar fuera de un campo o desplazar una lista cierra el teclado, salvo los controles del mismo formulario.
- `ARCHITECTURE.md`:
  - `screens/`: agrega `search_screen.dart` (pantalla principal: barra de búsqueda y lista filtrada) y quita "búsqueda" de "futuro"; `placeholder_screen.dart` queda solo para el error al abrir.
  - `app.dart` (mapa y sección `mobile/`): en el estado desbloqueado muestra `SearchScreen` con el total de dañadas; al bloquear, el remontaje por `_epoch` descarta el texto de búsqueda.
  - `widgets/`: `activity_text_field.dart` cierra el teclado al tocar fuera; los controles del formulario van en `TextFieldTapRegion`.
- `mobile/CLAUDE.md`: se amplía la regla de `ActivityTextField`: tocar fuera de un campo cierra el teclado; los controles del formulario (botón de envío, casillas, íconos fuera del campo) se envuelven en `TextFieldTapRegion` para no cerrarlo; toda lista desplazable de las pantallas de la bóveda usa `keyboardDismissBehavior: onDrag`.
- `docs/roadmap.md`: marcar "Pantalla principal con la barra de búsqueda enfocada y la lista filtrándose al escribir", en un commit propio, **solo después** de que pasen la prueba de rendimiento y los ocho casos a mano. Si algo falla ahí, se corrige antes y el roadmap espera.

## Hallazgos de la implementación

1. **`onDrag` y `onTapOutside` se solapan.** El toque con el que empieza un arrastre sobre la lista ya dispara `onTapOutside` y quita el foco, así que la prueba de desplazar comprueba también la propiedad `keyboardDismissBehavior`. `onDrag` queda como respaldo.
2. **"Usar huella" no tiene prueba de grupo.** Al pulsarlo, el campo se desactiva y suelta el foco por eso; una prueba pasaría aunque el botón estuviera en el grupo.
3. **`PlaceholderScreen` pierde `banner`**, que solo usaba el marcador de la pantalla principal; queda para los mensajes de error.
4. **Prueba de rendimiento sin ejecutar en la implementación.** La escribe la implementación y la ejecuta el usuario en el emulador y en el celular, junto con la prueba a mano. Espera cada respuesta contando las búsquedas respondidas (una sesión que delega en la real), no un elemento visible, que podría venir del prefijo anterior.
5. **`enterText` en profile necesita el teclado simulado** (hallazgo de la revisión final). `IntegrationTestWidgetsFlutterBinding` no lo registra (`registerTestTextInput` es `false`), así que `enterText` manda el texto con el cliente `-1`, que `TextInput` solo acepta dentro de un `assert`: en profile lo escrito se descarta. La prueba registra `binding.testTextInput` antes de montar la pantalla. Consecuencia para el caso 7 de la prueba a mano: el teclado real no aparece durante la medición; lo que se mira es la lista filtrándose.

6. **Rendimiento: el celular es la medición válida** (decisión del usuario). En el emulador (GPU emulada, `ro.hardware.egl=emulation`) pasó 1 de 3 corridas; las otras dos fallaron por un pico aislado de unos 60 ms en una de 27 búsquedas (mediana de unos 3 ms) y por el percentil 90 de rasterizado en 16.3 y 17.2 ms. En el celular (Xiaomi 2511FPC34G, Android 16, API 36, arm64) pasaron las 3: ida y vuelta con mediana de 1.07 a 1.67 ms y máximo de 3.35 a 9.93 ms; percentil 90 de construcción de 4.8 a 5.3 ms y de rasterizado de 3.4 a 3.6 ms. No se agregó retardo: no acelera una llamada aislada ni el costo de cada cuadro. El emulador queda como prueba de humo.

## PR

Con el formato habitual (Resumen, Decisiones de implementación, Criterios de aceptación con casillas, Verificación local, `Closes #40`), e incluye además:

- Las mediciones de la prueba de rendimiento: mediana y máximo de la ida y vuelta, y los percentiles 90 de construcción y de rasterizado.
- El emulador y la versión de Android donde se midió.
- Los ocho casos de la prueba a mano, marcados.

## Comentarios en issues

Al abrir el PR, y solo después de la confirmación del usuario, un comentario en la #41 con lo de "Cierre del pendiente de la #41" y "Nota para la #41".

## Commits previstos

1. `docs: diseño de la pantalla principal`
2. `docs: plan de la pantalla principal`
3. `feat: cerrar el teclado al tocar fuera de un campo` (`ActivityTextField` y pruebas de componentes)
4. `feat: mantener el teclado con los controles de los formularios de contraseña` (`TextFieldTapRegion` y pruebas)
5. `test: búsqueda en la sesión simulada`
6. `feat: pantalla principal con búsqueda y lista filtrada` (`SearchScreen` y `search_screen_test`)
7. `feat: mostrar la pantalla principal al desbloquear` (`VaultGate`, las 14 comprobaciones y los flujos)
8. `test: rendimiento de la búsqueda con 500 entradas`
9. `docs: especificación, arquitectura y reglas de mobile`
10. `docs: marcar la pantalla principal en el roadmap` (después de la prueba de rendimiento y la prueba a mano)
