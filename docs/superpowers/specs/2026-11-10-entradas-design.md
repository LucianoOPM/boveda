# Diseño: detalle, creación, edición y eliminación de entradas (issue #41)

Fecha: 2026-11-10 · Etapa 2 · Rama `feat/41-entradas`

## Objetivo

Que la app sirva para guardar contraseñas: desde la pantalla principal se crea una entrada, se abre su detalle, se muestra u oculta la contraseña, se edita y se elimina (`docs/roadmap.md`, etapa 2: "Detalle, creación, edición y eliminación de entradas; mostrar u ocultar la contraseña").

La issue toca tres capas:

- `core/`: dos variantes nuevas de `SecretChange` y `Vault::entry`. Es un cambio de API, **no de formato**: no sube la versión, no hay migración ni bóveda de ejemplo nueva.
- `mobile/rust/src/api/`: `edit` con firma nueva y `entry`. Se regenera el puente.
- `mobile/lib/`: detalle, formulario, campo de etiquetas y la conexión con la pantalla principal.

Sin dependencias nuevas.

### Fuera de alcance

- Copiar (#42). Los textos revelados no son seleccionables, para que no se copien sin la marca de sensible.
- Capturas de pantalla y miniatura de recientes (#43).
- "Enviar a PC" (etapa 3) y generador de contraseñas (etapa 4). El formulario no deja un botón vacío para él.
- Pantalla de entradas dañadas que no aparecen en la lista: #65, de la etapa 4 (ver "Entradas dañadas").
- Fechas en el detalle (`modified_at`, `last_used_at`): si hacen falta, van en su propia issue con la decisión sobre `intl`.
- Abrir la URL: pediría `url_launcher`.

### Criterios de aceptación (de la issue)

- Crear, ver, editar y eliminar funcionan de punta a punta en el emulador.
- Editar solo una etiqueta conserva la contraseña sin que cruce a Dart (`Keep`).
- La contraseña está oculta por defecto y se descarta al salir de la pantalla.
- Un campo que excede el límite muestra el error del núcleo en ese campo.
- Eliminar pide confirmación y la entrada desaparece de la lista.
- Pruebas de widgets con el puente simulado.
- `flutter analyze` y `flutter test` pasan; CI en verde.

## Lo que confirman la documentación y el código

Verificado con Context7 (flutter_rust_bridge v2.13.0, API de Flutter) y con el código del repositorio y del SDK local (Flutter 3.47.9):

- **Enums con datos en frb.** Un `enum` de Rust con campos se traduce a una `@freezed sealed class` y exige `freezed`, `freezed_annotation` y `build_runner`. Por eso los tipos de la fachada son structs, enums planos u `Option`.
- **`SecretChange` hoy** solo tiene `Keep` y `Replace(Secret)` (contraseña y notas juntas). Cambiar solo las notas obligaría a que la contraseña cruzara a Dart para devolverla, contra la especificación ("una contraseña solo cruza a Flutter al mostrarla o copiarla localmente").
- **`edit` con `Keep` descifra el secreto**, porque siempre vuelve a cifrar las dos partes: sobre un secreto dañado da `CorruptEntry` aunque solo cambie una etiqueta.
- **`edit` devuelve `()`** y el núcleo normaliza (recorta, quita etiquetas duplicadas y en blanco): Dart no conoce los valores guardados, y no hay forma de leer una entrada por id.
- **Entradas dañadas.** Con solo el secreto dañado, la entrada está en la caché y en la lista, y se descubre al revelar. Con los datos de búsqueda o la fila dañados, nunca entra en la caché: solo figura en el `UnlockReport`, del que `VaultGate` guarda el total.
- **Límites.** Están en bytes UTF-8 sobre el valor normalizado y son `pub(crate)`. `maxLength` de `TextField` cuenta grafemas: no sirve para validar antes.
- **Revelar.** `reveal_password` y `reveal_notes` llaman cada una a `Vault::reveal`, que actualiza `last_used_at`. Dart no puede saber si hay notas sin revelarlas: son parte del secreto.
- **`PopScope`.** `canPop: false` impide el gesto de atrás del sistema y `Navigator.maybePop` (la flecha del `AppBar`); `onPopInvokedWithResult` recibe `didPop == false`. `Navigator.pop` programático no consulta `PopScope`. El bloqueo quita las rutas con `removeRoute`, que tampoco lo consulta (probado en la #39).
- **`pushReplacement`** (`packages/flutter/lib/src/widgets/navigator.dart`, 3.47.9): la ruta reemplazada se completa (`didComplete`, que completa `popped`) en el reemplazo, no cuando el usuario vuelve. El futuro que devuelve `pushReplacement` es el de la ruta nueva y se completa cuando esa sale.
- **`enableSuggestions: false` en Android** (hallazgo de la #40, `TextInputPlugin.java`): Flutter lo traduce a `TYPE_TEXT_FLAG_NO_SUGGESTIONS` más `TYPE_TEXT_VARIATION_VISIBLE_PASSWORD`, y Gboard muestra el teclado de contraseña, con la fila de números. Es una razón de comportamiento del teclado, no de privacidad: lo que evita que el teclado aprenda lo escrito es `enableIMEPersonalizedLearning: false`.
- **`PasswordField`** no incluye el indicador de fortaleza (`StrengthIndicator` es un widget aparte) ni conoce el mínimo de 12 caracteres, que el núcleo aplica solo en `create` y `change_master_password`.
- **`ActivityTextField`** ya acepta `keyboardType` y `maxLines`.
- **Etiquetas en el núcleo** (`validate_search_v1`): cada una se recorta; las que quedan en blanco se descartan; las duplicadas (sin distinguir mayúsculas) también, conservando la primera.

## Decisiones

1. **Cuándo revelar.** Abrir el detalle no descifra nada ni cambia el orden de recientes. "Mostrar" (contraseña) y "Mostrar notas" son acciones separadas, cada una con su `revealX`. "Mostrar notas" aparece siempre; sin notas, muestra "Sin notas". Cada revelado actualiza `last_used_at`: la especificación cuenta mostrar como uso.
2. **Editar el secreto.** Se amplía el núcleo con `ReplacePassword` y `ReplaceNotes`: la parte no tocada nunca sale de Rust. En el formulario de edición el secreto empieza cerrado.
3. **Refrescar.** `Vault::entry(id)` y `VaultSession.entry(id)`, desde la caché. Lo usarán también la #42 y la #44.
4. **Validación.** Solo el núcleo, al guardar, con el error en su campo. Dart no recorta, no valida vacíos y no conoce los límites: el `trim` de Dart y el de Rust no definen igual los espacios Unicode. Los mensajes no llevan número: en bytes confundiría y en caracteres sería falso con acentos.
5. **Etiquetas.** Chips; Enter o coma agregan. La coma no puede formar parte de una etiqueta: es una limitación de la interfaz, no del formato.
6. **Entradas dañadas.** Con el secreto dañado: "La contraseña y las notas de esta entrada están dañadas", con "Reparar" (`Replace`) y "Eliminar". Las que no aparecen en la lista quedan para su issue; las que no tienen id solo se cuentan, como hoy.
7. **Navegación.** Botón "+"; tras crear, el detalle de la entrada nueva en lugar del formulario; tras editar, el detalle refrescado; tras eliminar, la lista. "¿Descartar cambios?" al salir del formulario con cambios. Al volver a la principal se repite la búsqueda.
8. **Bloqueo con un formulario a medias: descarta los cambios sin preguntar.** Es intencional: seguridad antes que comodidad.
9. **Contraseña y notas mostradas.** No seleccionables; la contraseña, monoespaciada; visibles hasta ocultarlas o salir de la pantalla, sin ocultado por tiempo. La copia en Dart es de mejor esfuerzo (`ARCHITECTURE.md`, "Decisiones clave").

## Núcleo (`core/`)

### `SecretChange`

```rust
pub enum SecretChange {
    /// Conservar el secreto actual. No sale del núcleo.
    Keep,
    /// Reemplazar contraseña y notas juntas. Repara una entrada dañada.
    Replace(Secret),
    /// Contraseña nueva; las notas actuales no salen del núcleo.
    ReplacePassword(SecretString),
    /// Notas nuevas (`None` o en blanco las quita); la contraseña actual no
    /// sale del núcleo.
    ReplaceNotes(Option<SecretString>),
}
```

Sigue derivando `Debug`: el de `SecretString` no muestra el valor.

### Reglas del secreto en un solo lugar (`vault/entry.rs`)

`build_secret_v1` se separa en dos comprobaciones que también usan los caminos nuevos:

- `check_password_v1(&SecretString) -> Result<(), InvalidEntryField>`: vacía o demasiado larga.
- `normalize_notes_v1(Option<SecretString>) -> Result<Option<SecretString>, InvalidEntryField>`: en blanco cuenta como ausente; demasiado larga es un error.

`build_secret_v1` pasa a llamar a las dos. Sobre ellas, en `Secret`, cuatro funciones `pub(crate)` que devuelven `Error::InvalidEntry`:

- `Secret::check_password(&SecretString) -> Result<()>` y `Secret::normalize_notes(Option<SecretString>) -> Result<Option<SecretString>>`, para validar antes de descifrar.
- `with_password(self, SecretString) -> Result<Secret>` y `with_notes(self, Option<SecretString>) -> Result<Secret>`: consumen el secreto y arman el nuevo con `build_secret_v1`, moviendo los `SecretString` sin copiarlos. La parte reemplazada se suelta ahí, y `SecretString` la borra al soltarse.

### `Vault::edit`

Orden fijo para `ReplacePassword` y `ReplaceNotes`:

1. `Locked` y el reloj, como hoy. Los datos de búsqueda ya llegan validados: `SearchData` solo se construye con sus reglas.
2. **Validar el valor nuevo** (`check_password` o `normalize_notes`). Un valor no válido no pone en memoria la contraseña ni las notas vigentes.
3. Leer la fila y descifrar el secreto actual.
4. Armar el secreto nuevo con `with_password` o `with_notes`.
5. Cifrar las dos partes con nonces nuevos y escribir, como hoy: actualiza `modified_at`, no `last_used_at`.

| Situación | Resultado |
| --- | --- |
| Contraseña nueva vacía o demasiado larga | `InvalidEntry(PasswordEmpty / PasswordTooLong)`, sin tocar el archivo |
| Notas nuevas demasiado largas | `InvalidEntry(NotesTooLong)`, sin tocar el archivo |
| Notas nuevas `None` o en blanco | Se guardan como ausentes |
| Secreto dañado o fila ilegible | `CorruptEntry`, sin tocar el archivo: solo `Replace` repara |
| Secreto dañado **y** valor no válido | `InvalidEntry` (paso 2 antes que el 3) |
| Id inexistente o lápida, con valor válido | `EntryNotFound`, como hoy |

`Keep` y `Replace` no cambian.

### `Vault::entry`

```rust
/// La entrada con ese id, como en [`Vault::entries`]. Desde la caché: no
/// toca el disco ni el secreto.
pub fn entry(&self, id: EntryId) -> Result<&EntrySummary>
```

- Bloqueada: `Locked`.
- Id inexistente, eliminado o con los datos de búsqueda dañados: `EntryNotFound`. Desde la caché los tres casos son indistinguibles (una dañada nunca entra en ella) y distinguirlos exigiría leer la fila. Desde la interfaz solo se llega con ids que vinieron de la lista o de `add`, así que una dañada solo aparece por una carrera, y "no encontrada" es una respuesta correcta. La documentación de `Error::EntryNotFound` se amplía para decirlo.
- Una entrada con solo el secreto dañado está en la caché y se devuelve normal.

### Pruebas (primero)

`core/src/vault/tests.rs` y `core/src/vault/entry/tests.rs`:

- `ReplacePassword`: después, `reveal` da la contraseña nueva y las notas de antes, sin cambios. También sobre una entrada sin notas.
- `ReplaceNotes`: después, `reveal` da la contraseña de antes y las notas nuevas; `None` y un texto en blanco las quitan.
- Ambas: `modified_at` cambia y `last_used_at` no; los datos de búsqueda nuevos quedan en la caché.
- Ambas, sobre un secreto dañado y sobre una fila ilegible: `CorruptEntry` y el archivo intacto (se comparan los bytes).
- Ambas, con un valor no válido: `InvalidEntry` y el archivo intacto.
- Orden: secreto dañado y valor no válido a la vez → `InvalidEntry`.
- Ambas, bloqueada: `Locked`; con un id inexistente: `EntryNotFound`.
- `check_password`, `normalize_notes`, `with_password` y `with_notes`: mismas reglas que `Secret::new` (vacía, límite exacto, límite más uno, notas en blanco).
- `entry`: igual al elemento de `entries()`; refleja un `edit` y el `last_used_at` de un `reveal`; `EntryNotFound` para inexistente, eliminada y con datos de búsqueda dañados; `Locked` bloqueada; con solo el secreto dañado, la devuelve.

`core/tests/vault.rs` (API pública, sobre una copia de `vault-v2.db`): `ReplacePassword`, `ReplaceNotes` y `entry` de punta a punta, cerrando y volviendo a abrir la bóveda.

Cada prueba de un arreglo se comprueba desactivándolo: sin el paso 2, la prueba de orden debe fallar.

## Fachada (`mobile/rust/src/api/entries.rs`)

```rust
/// Reemplaza los datos de búsqueda. Con `new_password` y `new_notes` en
/// `None` conserva el secreto, que no sale del núcleo. Con uno solo,
/// reemplaza esa parte y la otra no sale del núcleo. Con los dos, reemplaza
/// el secreto completo (repara una entrada dañada). Unas notas en blanco
/// las quitan.
pub fn edit(
    &self,
    id: String,
    input: EntryInput,
    new_password: Option<String>,
    new_notes: Option<String>,
) -> FacadeResult<()>

/// Datos de búsqueda y fechas de una entrada, desde la caché.
pub fn entry(&self, id: String) -> FacadeResult<EntrySummary>
```

| `new_password` | `new_notes` | `SecretChange` |
| --- | --- | --- |
| `None` | `None` | `Keep` |
| `Some(p)` | `None` | `ReplacePassword(p)` |
| `None` | `Some(n)` | `ReplaceNotes(Some(n))` |
| `Some(p)` | `Some(n)` | `Replace(Secret::new(p, Some(n)))` |

- Los dos `String` pasan por `secret_from` al principio, antes de cualquier salida con error (id mal formado, datos de búsqueda no válidos), para que siempre se borren.
- `NewSecret` queda solo para `add`.
- `VaultError` no cambia: el núcleo no agrega variantes.
- Ninguna de las dos lleva `#[frb(sync)]`.
- Se regenera el puente (`flutter_rust_bridge_codegen generate`) antes de `cargo test`. En el mismo commit, `FakeVaultSession` se adapta a la firma nueva de `edit` y gana `entry` (sin implementar todavía), para que `flutter test` pase.

Pruebas (`entries/tests.rs`): las cuatro combinaciones, comprobadas con `reveal_password` y `reveal_notes`; `Some("")` quita las notas; `entry` tras `add`, `edit` y `delete` (`EntryNotFound`); `entry` y `edit` con un id mal formado dan `InvalidEntryId`; `entry` bloqueada da `Locked`.

## Detalle (`lib/screens/entry_detail_screen.dart`, nuevo)

```dart
class EntryDetailScreen extends StatefulWidget {
  const EntryDetailScreen({super.key, required this.session, required this.entry});

  final VaultSession session;

  /// El resumen que ya tiene quien la abre: la lista, o `entry(id)` tras crear.
  final EntrySummary entry;
}
```

Abrirla no llama a Rust.

### Contenido

`AppBar` con el servicio como título y dos acciones con `tooltip`: "Editar" y "Eliminar". Debajo, un `ListView` con `keyboardDismissBehavior: onDrag`:

| Fila | Se muestra |
| --- | --- |
| Usuario | Solo si existe |
| URL | Solo si existe, como texto |
| Etiquetas | Solo si hay, como chips de solo lectura |
| Contraseña | Oculta: ocho puntos fijos (no delatan el largo) y "Mostrar". Visible: el texto y "Ocultar" |
| Notas | "Mostrar notas", siempre. Visibles: el texto, o "Sin notas", y "Ocultar notas" |

- La contraseña visible es un `Text` (no `SelectableText`) con `fontFamily: 'monospace'`, sin fuente nueva. Las notas visibles también son `Text`.
- Los ocho puntos van en `Semantics(label: 'Contraseña oculta')` con `ExcludeSemantics` sobre el texto, para que el lector de pantalla no lea "punto" ocho veces.

### Estado y secretos

- La contraseña y las notas viven solo en el `State` de esta pantalla, como `String?`; nunca en un controlador, servicio ni caché.
- Se descartan (`null`) al tocar "Ocultar", al abrir el formulario (la pantalla cubierta no retiene nada, y tras editar lo mostrado podría estar viejo) y en `dispose`. El bloqueo quita la ruta: pasa por `dispose`.
- Durante un revelado, su botón se desactiva. Cada revelado lleva una generación (una para la contraseña y otra para las notas) que sube al lanzarlo, al ocultar y al abrir el formulario: una respuesta de una generación vieja, o con la pantalla desmontada, se descarta.

### Errores al revelar

| Error | Qué pasa |
| --- | --- |
| `corruptEntry` | El secreto es un solo bloque cifrado: afecta a las dos partes. Las filas de contraseña y notas se reemplazan por "La contraseña y las notas de esta entrada están dañadas", con "Reparar" y "Eliminar" |
| `locked` | Silencio: carrera con el bloqueo |
| `entryNotFound` | Vuelve a la lista |
| Otro | "No se pudo mostrar. Intenta de nuevo." en esa fila, sin detalles; el botón queda disponible |

### Editar y reparar

"Editar" hace `push` del formulario en modo editar; "Reparar", en modo reparar. Antes del `push` se descarta lo revelado. Si el formulario devuelve `true` (guardó), el detalle llama a `session.entry(id)`, muestra lo que el núcleo guardó y vuelve las filas del secreto al estado oculto (también tras reparar). Con `entryNotFound`, vuelve a la lista; con `locked`, nada; con otro error, conserva lo que mostraba.

### Eliminar

`showDialog` (en el `Navigator` raíz, que el bloqueo vacía): "¿Eliminar «servicio»?", "Esta acción no se puede deshacer.", con "Cancelar" y "Eliminar". No hay "deshacer": la lápida no conserva el secreto.

- Éxito, o `entryNotFound` (ya no estaba): `pop` a la lista.
- `locked`: silencio.
- Otro error: `SnackBar` "No se pudo eliminar. Intenta de nuevo." y se queda.
- Mientras elimina, las acciones y los botones se desactivan.

## Formulario (`lib/screens/entry_form_screen.dart`, nuevo)

```dart
EntryFormScreen.create({required VaultSession session, required ValueChanged<EntrySummary> onCreated})
EntryFormScreen.edit({required VaultSession session, required EntrySummary entry, bool repair = false})
```

| Modo | Título | Secreto |
| --- | --- | --- |
| Crear | "Nueva entrada" | Contraseña y notas abiertas y vacías |
| Editar | "Editar entrada" | Cerrado: "Contraseña: sin cambios · Cambiar" y "Notas: sin cambios · Editar notas" |
| Reparar | "Reparar entrada" | Abierto y vacío, con el aviso arriba; las secciones no se pueden cerrar |

En editar y reparar, los datos de búsqueda llegan llenos desde el `EntrySummary`, sin llamar a Rust. El modo reparar es un estado del formulario: se entra con `repair: true` o desde el modo editar (ver "Paso a reparación").

### Campos

En este orden, dentro de un `ListView` con `keyboardDismissBehavior: onDrag`: servicio, usuario, URL, etiquetas, contraseña, notas y, al final, "Guardar".

| Campo | Widget | Propiedades |
| --- | --- | --- |
| Servicio | `ActivityTextField` | `textCapitalization: sentences`, acción "siguiente" |
| Usuario | `ActivityTextField` | `keyboardType: emailAddress`, acción "siguiente" |
| URL | `ActivityTextField` | `keyboardType: url`, acción "siguiente" |
| Etiquetas | `TagsField` | ver abajo |
| Contraseña | `PasswordField` | oculta, con el ojo |
| Notas | `ActivityTextField` | `keyboardType: multiline`, `minLines: 3`, `maxLines: 8` (después desplaza) |

- Todos los campos de texto llevan `autocorrect: false` y `enableIMEPersonalizedLearning: false`: el teclado no corrige ni aprende servicios, usuarios, etiquetas ni notas. `PasswordField` ya lo hace.
- `enableSuggestions` queda en `true` en los que no son la contraseña, solo por el comportamiento del teclado (ver "Lo que confirman la documentación y el código"); la privacidad la da `enableIMEPersonalizedLearning: false`.
- **Sin regla de la contraseña maestra:** el formulario no muestra `StrengthIndicator` ni aplica un mínimo. Una entrada guarda la contraseña que el servicio exija, aunque sea "1234".
- El comentario de `PasswordField` deja de decir "contraseña maestra": sirve para cualquier contraseña.

### `ActivityTextField`

Se agregan exactamente dos parámetros, pasados tal cual al `TextField` con su valor por omisión: `textCapitalization` (`TextCapitalization.none`) y `minLines` (`null`). `keyboardType` y `maxLines` ya existen. `onTapOutside` sigue fijo, como en la #40.

### `TagsField` (`lib/widgets/tags_field.dart`, nuevo)

```dart
class TagsField extends StatelessWidget {
  const TagsField({
    super.key,
    required this.tags,          // la lista es del formulario
    required this.onChanged,     // ValueChanged<List<String>>
    required this.controller,    // texto pendiente; del formulario
    this.focusNode,
    this.enabled = true,
    this.errorText,
  });
}
```

Un `Wrap` de `InputChip` sobre un `ActivityTextField` "Agregar etiqueta".

- Enter agrega lo escrito y deja el foco en el campo (`onEditingComplete` propio). Una coma también: en cuanto el texto contiene una coma, se parte por comas, se agregan todos los trozos y el campo queda vacío. Pegar "a,b,c" agrega tres; una coma insertada en medio de un texto lo parte en dos etiquetas.
- Un trozo que queda en blanco (`trim().isEmpty`) no crea chip. Es solo para no dibujar un chip vacío: el núcleo descarta las etiquetas en blanco, así que lo guardado es lo mismo. El contenido de cada chip no se recorta en Dart.
- Dart no quita duplicados: lo hace el núcleo al guardar, y el detalle refrescado muestra el resultado.
- Cada chip tiene su ✕ (`onDeleted`, con `deleteButtonTooltipMessage: 'Quitar etiqueta'`); el texto largo se corta con elipsis.
- El `Wrap` de chips va en un `TextFieldTapRegion`: tocar un chip o su ✕ no cierra el teclado.
- **Limitación de la interfaz:** una etiqueta no puede contener una coma. El formato sí lo permite.

### Secciones del secreto (modo editar)

- **"Cambiar"** abre un `PasswordField` vacío para la contraseña nueva, sin revelar la actual. "Cancelar" lo cierra y lo vacía.
- **"Editar notas"** llama a `revealNotes` (cuenta como uso) y abre el campo con ellas, o vacío si no hay. Durante la carga, el botón se desactiva. "Cancelar" lo cierra y lo vacía.
- **Una sección abierta se envía siempre; una cerrada, nunca.** No se compara con las notas originales, para no guardar una segunda copia en el `State`; volver a cifrar las mismas notas es inofensivo.
- Error de `revealNotes`: `corruptEntry` → paso a reparación; `locked` → silencio; otro → "No se pudieron cargar las notas. Intenta de nuevo." junto al botón.

### Guardar

1. El texto pendiente del campo de etiquetas se agrega como etiqueta (salvo en blanco), para no perderlo en silencio.
2. Los textos se envían tal cual: sin recortar y sin validar vacíos. Usuario y URL van siempre como texto; el núcleo trata el blanco como ausente.
3. Crear: `add(input, NewSecret(password, notes))`. Editar: `edit(id, input, newPassword, newNotes)` con cada valor solo si su sección está abierta. Reparar: `edit` con los dos (`newNotes` es `''` si el campo está vacío).
4. Mientras guarda: campos y botones desactivados, el botón dice "Guardando…" y "atrás" no hace nada.

| Resultado | Qué pasa |
| --- | --- |
| Éxito al crear | `entry(id)` y `onCreated(resumen)`. Si `entry(id)` falla, `pop` |
| Éxito al editar o reparar | `pop(true)` |
| `invalidEntryServiceEmpty` | "El servicio es obligatorio", en servicio |
| `invalidEntryPasswordEmpty` | "La contraseña es obligatoria", en contraseña |
| `invalidEntryServiceTooLong`, `…UsernameTooLong`, `…UrlTooLong`, `…PasswordTooLong`, `…NotesTooLong` | "Este campo es demasiado largo", en su campo |
| `invalidEntryTagTooLong` | "Este campo es demasiado largo", en etiquetas |
| `invalidEntryTooManyTags` | "Demasiadas etiquetas", en etiquetas |
| `corruptEntry` | Paso a reparación |
| `locked` | Silencio |
| Otro (incluidos `invalidEntryOther` y `entryNotFound`) | "No se pudo guardar. Intenta de nuevo." sobre el botón |

El campo con error recibe el foco y se desplaza a la vista (`Scrollable.ensureVisible`). El error de un campo se borra al cambiar ese campo; el mensaje general, al cambiar cualquiera. El núcleo informa un error a la vez.

### Paso a reparación

`edit` con `Keep`, `ReplacePassword` o `ReplaceNotes` sobre un secreto dañado da `corruptEntry`, aunque el usuario solo haya cambiado una etiqueta. Al recibirlo (al guardar o en "Editar notas"), el formulario:

- Muestra arriba "La contraseña y las notas de esta entrada están dañadas. Escribe una contraseña nueva para repararla."
- Pasa al modo reparar: título "Reparar entrada", las dos secciones abiertas y sin "Cancelar".
- Conserva todo lo escrito: los datos de búsqueda, los chips y una contraseña o unas notas nuevas ya escritas.
- Lleva el foco a la contraseña.

El siguiente "Guardar" envía los dos valores (`Replace`).

### Descartar cambios

`PopScope` con `canPop: false` cuando hay cambios (un campo distinto del valor inicial, chips distintos, texto pendiente o una sección abierta en modo editar) o mientras guarda. Con el gesto de atrás o la flecha, si no está guardando: "¿Descartar cambios?", con "Seguir editando" y "Descartar". El `pop` tras guardar es programático y no pregunta.

**El bloqueo automático quita la ruta sin preguntar y los cambios se pierden.** Es intencional: seguridad antes que comodidad.

### Secretos

La contraseña y las notas viven solo en sus controladores, que se vacían (`clear()`) y se liberan en `dispose`. Una respuesta de `revealNotes` o de guardar que llega con la pantalla desmontada se descarta. El borrado en Dart es de mejor esfuerzo.

### Controles en `TextFieldTapRegion`

"Guardar", los chips, "Cambiar", "Editar notas" y los "Cancelar" de las secciones. Tocar un espacio vacío o desplazar la lista cierra el teclado.

## Navegación y pantalla principal

Todas las rutas son `MaterialPageRoute` en el `Navigator` raíz (`Navigator.of(context, rootNavigator: true)`), que el bloqueo vacía.

| Acción | Navegación |
| --- | --- |
| Tocar una entrada | `push` del detalle con el resumen de la lista |
| "+" | `push` del formulario en modo crear |
| Crear con éxito | `onCreated`: la principal hace `pushReplacement` al detalle de la entrada nueva; "atrás" vuelve a la lista |
| "Editar" o "Reparar" | `push` del formulario sobre el detalle; al guardar, `pop(true)` y el detalle se refresca |
| Eliminar | `pop` a la lista |

**Por qué `onCreated`.** Con `pushReplacement`, el futuro del `push` del formulario se completa en el reemplazo, con el detalle todavía encima (`navigator.dart`, 3.47.9: la ruta reemplazada recibe `didComplete`). Si la principal repitiera la búsqueda al completarse ese futuro, lo haría antes de tiempo y se perdería los revelados hechos en el detalle. Por eso el formulario no navega tras crear: entrega el resumen a quien lo abrió. La principal hace el `pushReplacement` y espera el futuro que este devuelve, el del detalle, que se completa al volver. El futuro del formulario solo dispara la búsqueda si el formulario salió sin crear.

Tras crear se muestra el detalle y no la lista: enseña lo que el núcleo guardó (normalizado) y, si el texto de búsqueda no coincide con la entrada nueva, el usuario no se queda sin verla.

### `SearchScreen`

- `_EntryTile` recibe `onTap`.
- `FloatingActionButton` con `tooltip: 'Agregar contraseña'` e ícono `Icons.add`.
- Estado vacío: "Todavía no hay contraseñas guardadas. Toca + para agregar la primera."
- Al volver de cualquier ruta, `_search(_query.text)`. Como el texto no cambió, la lista no salta al principio (regla de la #40), aunque sin texto cambie el orden de recientes.
- Se actualiza el comentario de la clase.

`VaultGate` no cambia: sigue guardando solo el total de dañadas.

## Entradas dañadas

| Caso | En esta issue |
| --- | --- |
| Solo el secreto dañado | Aparece en la lista. Al revelar (o al guardar una edición), el aviso con "Reparar" (`Replace`) y "Eliminar" |
| Datos de búsqueda o fila dañados, con id | No aparece en la lista; solo la cuenta el aviso del desbloqueo. La pantalla para verlas, volver a escribirlas y eliminarlas es de la etapa 4, en su propia issue (#65) |
| Sin id | Solo se cuentan, como hoy |

## Pruebas

### Doble de sesión (`test/fakes.dart`)

`add`, `edit`, `delete`, `entry`, `revealPassword` y `revealNotes` dejan de lanzar `UnimplementedError`: cada uno registra sus llamadas (`addCalls`, `editCalls`, `deleteCalls`, `entryCalls`, `revealPasswordCalls`, `revealNotesCalls`) y tiene un `on…` configurable, como `search`. Por omisión: `add` devuelve un id fijo, `entry` un resumen armado con el id, y los revelados, textos de prueba.

### `test/entry_detail_screen_test.dart` (nuevo)

- Al abrir: ninguna llamada a `reveal*` ni a `entry`; las filas opcionales aparecen solo con dato.
- Contraseña: "Mostrar" llama a `revealPassword` una vez y muestra el texto; no hay `SelectableText`; "Ocultar" lo quita del árbol.
- Notas: con notas y con "Sin notas"; mostrar la contraseña no llama a `revealNotes`, ni al revés.
- Semántica: oculta, la fila expone "Contraseña oculta" y no los puntos.
- Con un `Completer`: el botón está desactivado durante la carga; ocultar o abrir el formulario antes de la respuesta la descarta; una respuesta tras desmontar no lanza nada.
- Errores: `corruptEntry` (por la contraseña y por las notas) → el aviso con "Reparar" y "Eliminar"; genérico → mensaje y reintento; `locked` → nada; `entryNotFound` → `pop`.
- Eliminar: cancelar no llama a `delete`; confirmar llama y hace `pop`; `entryNotFound` hace `pop`; un fallo muestra el `SnackBar` y se queda.
- Editar: abrir el formulario descarta lo revelado; al volver con `true` se llama a `entry(id)` y se ven los datos nuevos; al volver sin guardar no se llama.
- Reparar: abre el formulario en modo reparar; al volver con `true`, las filas del secreto vuelven a estar ocultas.

### `test/entry_form_screen_test.dart` (nuevo)

- Crear: `add` recibe los textos tal cual (con espacios, sin recortar) y `onCreated` recibe el resumen de `entry(id)`.
- Crear con contraseña "1234" funciona; no hay `StrengthIndicator` en el árbol.
- Editar sin tocar el secreto: `edit` recibe `newPassword` y `newNotes` en `null`, y ni `revealPassword` ni `revealNotes` se llaman (criterio de `Keep`).
- "Cambiar" envía solo `newPassword`; "Editar notas" llama a `revealNotes`, y vaciar el campo envía `''`; "Cancelar" vuelve a `null`.
- Errores: cada variante `invalidEntry*` aparece en su campo con el texto acordado, le da el foco y se borra al escribir en él; un error genérico muestra el mensaje general.
- Paso a reparación: por `corruptEntry` al guardar y por `corruptEntry` en "Editar notas"; aparece el aviso, se conservan los demás campos y los chips, y el siguiente guardado envía los dos valores.
- Modo reparar directo (`repair: true`): secciones abiertas, sin "Cancelar".
- Guardando, con un `Completer`: todo desactivado y "atrás" sin efecto; una respuesta tras desmontar no lanza nada.
- Descartar, con el formulario como segunda ruta (lección de la #36): sin cambios sale directo; con cambios pregunta, por la flecha y por el atrás del sistema; "Seguir editando" conserva lo escrito; "Descartar" sale.
- Teclado: tocar "Guardar", un chip o "Cambiar" no quita el foco; un espacio vacío sí; las propiedades de cada campo (`autocorrect`, `enableIMEPersonalizedLearning`, `enableSuggestions`, `keyboardType`, `minLines`, `maxLines`).

### `test/widgets_test.dart`

- `TagsField`: Enter, coma, pegado con comas, ✕, trozo en blanco, elipsis, `errorText`, desactivado.
- `ActivityTextField`: `textCapitalization` y `minLines` llegan al `TextField`.

### `test/search_screen_test.dart`

- Tocar una entrada abre el detalle; "+" abre el formulario; el estado vacío muestra el texto nuevo.
- Al volver del detalle se repite la búsqueda con el texto actual y la lista no salta.
- Tras crear: se ve el detalle de la nueva, sin ninguna búsqueda mientras sigue encima; "atrás" lleva a la lista con una búsqueda nueva.
- Salir del formulario sin crear repite la búsqueda.

### `test/app_test.dart` y `test/auto_lock_test.dart`

- Flujo con el doble: desbloquear → crear → detalle → editar → eliminar → lista.
- Escribir en cada campo del formulario reinicia el temporizador de inactividad.
- Bloqueo (`hidden`) con el detalle abierto y la contraseña visible, con el formulario a medias, con el diálogo de eliminar y con el de descartar: tras volver a `resumed` solo queda el desbloqueo y ninguno de esos textos está en el árbol.

### En dispositivo (`integration_test/vault_session_test.dart`)

Con la librería de Rust real: las cuatro combinaciones de `edit`, `''` quita las notas, y `entry` tras `add`, `edit` y `delete`.

### A mano, en el emulador

1. Bóveda vacía: el texto nuevo y "+".
2. Crear con todos los campos → detalle con lo normalizado (espacios recortados, etiqueta duplicada quitada).
3. Crear con contraseña "1234".
4. Mostrar y ocultar la contraseña y las notas; "Sin notas"; la entrada sube en recientes al volver.
5. Editar solo una etiqueta; cambiar solo la contraseña; solo las notas; quitar las notas.
6. Un servicio de más de 256 bytes → error en su campo.
7. Descartar cambios: flecha y gesto.
8. Eliminar: cancelar y confirmar; la entrada desaparece de la lista.
9. Teclado: "Guardar", chips y "Cambiar" no lo cierran; fuera y al desplazar, sí.
10. Bloqueo con el detalle visible y con el formulario a medias: al volver, desbloqueo; después, la lista, sin restos.
11. TalkBack: "Contraseña oculta".
12. Secreto dañado. **Solo sobre una bóveda de prueba creada en el emulador para esto, nunca con datos reales.** Con la app cerrada, se saca una copia con `run-as`, se altera un byte dentro del blob `secret` de una fila (no del blob `search`: la entrada desaparecería de la lista y no se probaría "Reparar") y se devuelve al mismo lugar. Se comprueba el aviso, "Reparar", "Eliminar" y que las demás entradas siguen abriendo.

### Antes del PR

`cargo fmt --all`, `cargo test --workspace` y `cargo clippy --workspace --all-targets -- -D warnings` (desde la raíz); `flutter analyze` y `flutter test` (desde `mobile/`); el puente regenerado al día (lo que revisa `mobile.yml`: regenerar y que `git diff` quede vacío); `integration_test/vault_session_test.dart` en el emulador; los doce casos a mano; la revisión final.

## Archivos

```
core/src/vault/mod.rs                  # SecretChange, edit, entry
core/src/vault/entry.rs                # comprobaciones separadas, with_password, with_notes
core/src/error.rs                      # documentación de EntryNotFound
core/src/vault/tests.rs, core/src/vault/entry/tests.rs, core/tests/vault.rs
mobile/rust/src/api/entries.rs         # edit, entry
mobile/rust/src/api/entries/tests.rs
mobile/rust/src/frb_generated.rs, mobile/lib/src/rust/   # regenerados
mobile/lib/screens/entry_detail_screen.dart   # nuevo
mobile/lib/screens/entry_form_screen.dart     # nuevo
mobile/lib/widgets/tags_field.dart            # nuevo
mobile/lib/screens/search_screen.dart
mobile/lib/widgets/activity_text_field.dart
mobile/lib/widgets/password_field.dart        # solo el comentario
mobile/test/fakes.dart
mobile/test/entry_detail_screen_test.dart     # nuevo
mobile/test/entry_form_screen_test.dart       # nuevo
mobile/test/search_screen_test.dart, widgets_test.dart, app_test.dart, auto_lock_test.dart
mobile/integration_test/vault_session_test.dart
```

## Documentación

- `docs/especificacion.md`:
  - "Formato de la bóveda → API": párrafo "Decidido el 2026-11-10 (#41, detalle en `docs/superpowers/specs/2026-11-10-entradas-design.md`)": las cuatro variantes de `SecretChange`, el orden de validación, que solo `Replace` repara, y `entry(id)` desde la caché con `EntryNotFound`.
  - "Experiencia de uso → Acciones": párrafo "Decidido el 2026-11-10 (#41, …)": cuándo se revela; el formulario con el secreto cerrado; la validación solo en el núcleo, con mensajes sin número; las etiquetas sin coma; la reparación del secreto dañado y la issue de las demás dañadas; la navegación; el bloqueo que descarta sin preguntar; los textos revelados no seleccionables.
- `ARCHITECTURE.md`: `screens/` (detalle y formulario; "futuro: ajustes"), `widgets/` (`tags_field.dart`), `api/entries.rs` con `entry`, y en la sección `mobile/` el flujo detalle → formulario.
- `mobile/CLAUDE.md`: la regla de la contraseña se extiende a las notas; los textos revelados no son seleccionables; el formulario no valida ni recorta, lo hace el núcleo.
- `docs/roadmap.md`: marcar el punto en un commit propio, **solo después** de la prueba a mano.

## Hallazgos de la implementación

1. **El formulario no es un `ListView`.** Es un `SingleChildScrollView` con una `Column`, también con `keyboardDismissBehavior: onDrag`. Un `ListView` no construye los campos que quedan fuera de la pantalla, y tras un error de validación no se le podría dar el foco al campo señalado.
2. **`Semantics` necesita `container: true`.** Sin él, "Contraseña oculta" no forma un nodo propio y el lector de pantalla no lo expone.
3. **El texto pendiente de etiquetas no pasa por `onChanged` de `TagsField`**, que avisa de los chips. El formulario escucha su controlador para contarlo como cambio (para "¿Descartar cambios?") y para borrar el error de las etiquetas.
4. **"Guardar" también se desactiva mientras cargan las notas**, para no guardar con la sección a medio abrir.
5. **El campo de etiquetas lleva la ayuda "Enter o coma para agregar"**: sin ella no hay forma de descubrir el gesto.
6. **La lista de la pantalla principal lleva un margen inferior** para que el botón "+" no tape la última entrada.
7. **Búsqueda al bloquear con una ruta apilada.** Al quitar el detalle o el formulario, su futuro se completa y la pantalla principal, que con la app oculta sigue montada hasta el siguiente cuadro, repite la búsqueda. El núcleo ya está bloqueado, responde `locked` y la pantalla lo ignora, como en la carrera que ya cubría la #40. No cruza ningún dato y no se evita.
8. **Precedencia en `edit`.** Como el valor nuevo se valida antes de leer la fila, un valor no válido sobre un id inexistente da `InvalidEntry`, no `EntryNotFound`.
9. **Las pruebas de `TagsField` viven en `test/tags_field_test.dart`**, no en `widgets_test.dart`.
10. **`entry` en el doble de sesión.** Dentro de `FakeVaultSession`, el método `entry` oculta la función `entry(...)` que arma resúmenes de prueba; el doble la usa con otro nombre (`_summary`).
11. **`mounted` no basta tras un `await`** (hallazgo de la revisión final). El bloqueo quita la ruta en el acto, pero el `State` sigue montado hasta el siguiente cuadro, y con la app oculta no hay cuadros. Una respuesta de `add`, `edit`, `delete` o de un revelado que llegara en esa ventana hacía `pop` sobre la ruta de debajo (la raíz, con `VaultGate`) y dejaba la app en blanco al volver. El detalle y el formulario guardan su `ModalRoute` y, tras cada `await`, exigen `mounted` y `route.isActive`. Las pruebas de "respuesta tras quitar la ruta" completan la operación sin ningún cuadro en medio.
12. **El diálogo de eliminar es desplazable** (`scrollable: true`, hallazgo de la revisión final): con un servicio muy largo, o con la fuente del sistema ampliada, el título dejaba "Eliminar" fuera de la pantalla.

## PR

Con el formato habitual (Resumen, Decisiones de implementación, Criterios de aceptación con casillas, Verificación local, `Closes #41`), e incluye los doce casos de la prueba a mano marcados, el emulador y la versión de Android.

## Comentarios en issues

Al abrir el PR, y solo con la confirmación del usuario, un comentario en la #42 con lo que hereda: dónde va "Copiar" en el detalle, `entry(id)`, y que los textos revelados no son seleccionables.

## Commits previstos

1. `docs: diseño de las entradas`
2. `docs: plan de las entradas`
3. `feat: ReplacePassword y ReplaceNotes en el núcleo`
4. `feat: Vault::entry`
5. `feat: edit y entry en la fachada, con el puente regenerado`
6. `test: entradas en la sesión simulada`
7. `feat: campo de etiquetas y parámetros de ActivityTextField`
8. `feat: detalle de la entrada`
9. `feat: formulario de la entrada`
10. `feat: conectar la pantalla principal`
11. `docs: especificación, arquitectura y reglas de mobile`
12. `docs: marcar las entradas en el roadmap` (solo tras la prueba a mano)

Cada commit compila y pasa las pruebas: el 5 adapta `FakeVaultSession` a la firma nueva de `edit`, aunque sus llamadas sigan sin implementar hasta el 6.
