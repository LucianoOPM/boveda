# Portapapeles de Windows: escritura excluida, aviso y borrado condicional a 30 s (#98)

Parte de la épica #92. Depende de #97 (hecha). Tamaño: cambio mediano, de una sola crate (`desktop/src-tauri`); este diseño recoge lo acordado en la conversación y la investigación de la carrera entre comprobar y borrar.

## Intención

Cuando la PC reciba una contraseña del celular (#100, #104), `desktop/` debe dejarla en el portapapeles de Windows **sin** que entre al historial de Win+V ni a la nube, avisar «Contraseña lista para pegar» y, a los 30 s, borrar el portapapeles **solo si todavía contiene esa contraseña**. Esta issue entrega ese módulo (`clipboard.rs`) probado de forma aislada; nadie lo llama todavía.

## Metas

- Escribir con `arboard` y `exclude_from_history()` + `exclude_from_cloud()`.
- Notificación del sistema «Contraseña lista para pegar».
- Borrado a los 30 s **condicional y atómico** respecto de otras aplicaciones (ver «La carrera»).
- Dos envíos seguidos, copia manual de la misma contraseña, contenido ajeno, vacío, imagen, portapapeles ocupado, salida de la app: todos con comportamiento definido y probado.
- La contraseña no cruza al webview y no queda en memoria más de lo necesario.

## Fuera de alcance y antimetas

- Recibir la contraseña por la red (#100) y el flujo de punta a punta (#104), incluido contestar `copied()` o `failed()` al celular.
- Antimeta: borrar a ciegas. Ante la duda (portapapeles ocupado o ilegible) se prefiere no limpiar a eliminar contenido ajeno.
- No se agrega `GetClipboardSequenceNumber`, BLAKE2 ni cambios a `core/`.

## Restricciones

- `desktop/CLAUDE.md`: las contraseñas nunca cruzan al webview (aquí no hay IPC ni comandos); escribir con `arboard`; guardar un hash, no la contraseña; sin ventanas nuevas.
- Raíz: sin secretos en logs, errores ni `Debug`; los tipos con secretos no derivan `Debug`.
- Verificación de cierre: `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all`, `cargo audit`, `cargo deny check`.

## La carrera entre comprobar y borrar

**Problema.** `get_text()` seguido de `clear()` no es atómico: otra aplicación puede copiar entre las dos llamadas y se borraría contenido nuevo.

**Verificado en el código** (`arboard` 3.7.0, `src/platform/windows.rs`; `clipboard-win` 5.4.3):

- `arboard` en Windows **abre y cierra el portapapeles en cada operación** (`Clipboard::open` dentro de cada `Get`, `Set` y `Clear`; el comentario del código lo dice: «opened for every operation and then closed afterwards»). Su API pública no permite mantenerlo abierto entre una lectura y un borrado. Con `arboard` solo, `get_text()` y `clear()` son dos aperturas distintas y entre ellas queda una ventana, corta pero real.
- `arboard` es una envoltura de `clipboard-win`, que sí expone una guarda: `clipboard_win::Clipboard::new_attempts(n)` abre (`OpenClipboard`) y su `Drop` cierra (`CloseClipboard`); con la guarda viva se pueden llamar `raw::is_format_avail`, `raw::get_string` y `raw::empty`. Según la documentación de Win32, mientras un proceso tiene el portapapeles abierto ningún otro puede abrirlo ni modificarlo, así que leer, comparar y vaciar dentro de **una sola apertura** es atómico frente a otras aplicaciones.
- `clipboard-win` es la dependencia de Windows de `arboard`, así que `arboard` ya la trae. **Corrección:** ni `arboard` ni `clipboard-win` están hoy en `Cargo.lock` (comprobado el 2026-11-20); entran las dos con esta issue. Declarar `clipboard-win` directamente **no suma crates a los que ya trae `arboard`**, porque la misma versión (5.4.x) cubre a ambos; la lista exacta de crates nuevos (`arboard`, `clipboard-win`, `error-code` y lo que traiga el plugin de notificaciones) se calcula comparando `Cargo.lock` antes y después, y va en la PR.

**Solución mínima (decisión).** `arboard` escribe (lo que piden la especificación y `desktop/CLAUDE.md`). El borrado condicional usa `clipboard-win` directamente, dentro de una función única `clear_if_matches` con **una sola guarda abierta** durante toda la operación:

1. Abrir con reintentos acotados (`new_attempts`; 5 reintentos de 5 ms). La guarda (`clipboard_win::Clipboard`) cierra el portapapeles en su `Drop`, así que se libera en todos los caminos, incluidos los errores y los `return` anticipados.
2. Si `CF_UNICODETEXT` no está disponible (vacío, imagen, otro formato): salir sin tocar nada → `NotOurs`.
3. Leer el texto a un `Zeroizing<Vec<u8>>`, calcular la huella y compararla con la guardada.
4. Si coincide, `raw::empty()` y soltar la guarda → `Cleared`. Si no, soltar la guarda sin vaciar → `NotOurs`.
5. Si no se pudo abrir, o hubo un error de lectura o de vaciado → `Unavailable` (nunca se borra a ciegas).

La lectura, la comparación y el vaciado ocurren con la misma guarda; no se vuelve a abrir entre ellos. Una prueba real (`#[ignore]`) lo comprueba. Costo: una dependencia directa más, solo en Windows. Preferida por el usuario a aceptar una carrera que podría borrar contenido ajeno.

**Alcance de la garantía.** Es atómica frente a otras aplicaciones y a nuestros propios hilos (el trabajador es el único que toca el portapapeles). Un propietario de portapapeles con renderizado diferido puede tardar en entregar el texto mientras lo mantenemos abierto; el plazo de apertura es corto y acotado, y el peor caso es `Unavailable` (no se limpia).

**Licencia.** `clipboard-win` es BSL-1.0 (Boost), permisiva y aprobada por la OSI, pero **no está** en `allow` de `deny.toml`. Hay que agregar `BSL-1.0`, que `arboard` ya exigiría de forma transitiva. Aprobado por el usuario. Se comprueba con `cargo deny check` que no entra ninguna otra licencia no autorizada.

## Dependencias (revisadas y aprobadas)

| Crate | Versión | Uso | Notas |
| --- | --- | --- | --- |
| `arboard` | 3.7.0, `default-features = false` | Escribir con exclusiones de Windows | Sin `image-data`: no entra la crate `image`. MIT OR Apache-2.0 |
| `clipboard-win` | 5.4.x (la misma que resuelve `arboard`), solo `cfg(windows)` | Borrado condicional bajo una sola apertura | BSL-1.0. No estaba en el lock; entra con `arboard` y declararla directo no agrega otra versión |
| `tauri-plugin-notification` | 2.8.1 (estable más reciente; la 3.0 es alpha) | Notificación | Pide Rust 1.90 (el repo usa 1.100) y `tauri` 2.12 (el lock tiene 2.12.7). Trae `notify-rust` 4.19. Solo API de Rust: sin permisos de capability. Licencias y avisos por confirmar con `cargo deny` y `cargo audit` |
| `secrecy`, `zeroize` | las del lock | `SecretString` y `Zeroizing` | Misma versión que el núcleo; sin crates nuevos |

No se agrega ninguna otra. En desarrollo el toast de Windows puede mostrarse con el nombre de PowerShell; con la app instalada usa el `identifier`. Se anota en `docs/diferencias-diseno.md`.

## Diseño

### `clipboard.rs` (módulo `pub` de la librería)

Para que Clippy no marque código muerto sin silenciarlo, el módulo y su API son `pub` (la crate es `rlib`). Si aun así hubiera advertencias se resuelve con el código, no con `#[allow(dead_code)]`.

- **Trait `ClipboardBackend`** (lo que se simula en las pruebas):
  - `write_sensitive(&mut self, text: &str) -> Result<(), WriteError>`
  - `clear_if_matches(&mut self, matches: &dyn Fn(&[u8]) -> bool) -> ClearOutcome`, con `ClearOutcome::{Cleared, NotOurs, Unavailable}`. El contrato es que comprobar y vaciar ocurren como una sola operación.
- **`WindowsBackend`** (`cfg(windows)`): `write_sensitive` con `arboard`; `clear_if_matches` con `clipboard-win` como se describió arriba.
- **`Fingerprint(u64)`**: `RandomState` creada una vez al construir el gestor y conservada toda su vida; cada huella sale de `build_hasher()` y `write(bytes)` sobre los bytes UTF-8 completos. Solo se guarda la huella de 64 bits. Riesgo residual de colisión aceptado por el usuario.
- **Contraseñas con NUL: se rechazan.** Windows corta el texto en el primer `\0` al leerlo y al pegarlo, así que una contraseña con NUL se pegaría truncada. `deliver` la rechaza **antes de tocar el portapapeles** con `ClipboardError::InvalidPassword` (sin datos del secreto), no modifica el secreto recibido, y #100 contestará `failed()`. Así lo que Bóveda confirma como copiado coincide con lo que el usuario puede pegar. La comprobación es lo primero que hace `deliver`, en el hilo que llama.
- **`ClipboardManager`**: arranca **un solo hilo trabajador** y un canal. Operaciones:
  - `deliver(&self, &SecretString) -> Result<(), ClipboardError>`: copia la contraseña a un `SecretString` propio (una asignación exacta, de vida corta, borrada al soltarse), la manda al trabajador y **espera su respuesta**; no devuelve hasta que el portapapeles quedó escrito (o falló). Es lo que #100 necesita para contestar `Copied` o `Failed`.
  - `shutdown(&self, timeout)`: pide al trabajador un último intento de limpieza y espera su respuesta hasta `timeout`.
  - `deliver_and_notify(&self, &SecretString, notify)`: `deliver` y, **después**, la notificación; el fallo del aviso se registra (sin datos sensibles) y no afecta al temporizador ni al resultado.
- **Trabajador**: bucle con `recv_timeout` hasta el vencimiento más próximo, medido con `Instant` (reloj monótono). Es el único que usa el backend. Su estado es una lista corta de `Pending { fingerprint, deadline, attempts }`.

### Reglas de comportamiento

1. **Escritura correcta:** el trabajador calcula la huella, **reemplaza** la lista por la entrada nueva (`deadline = ahora + 30 s`) y responde `Ok`. Esto cancela el temporizador anterior y ocurre en el mismo paso que registra el nuevo.
2. **Escritura fallida sin efecto:** se conserva el vencimiento original; responde `Err`.
3. **Escritura de resultado dudoso** (`arboard` puede fallar al agregar las exclusiones después de haber puesto el texto): el trabajador intenta `clear_if_matches` con la huella nueva de inmediato, con el mismo límite de reintentos que el vencimiento. Si queda `Cleared` o `NotOurs`, no agrega nada. Si queda `Unavailable`, **agrega** la huella nueva a la lista como entrada de limpieza con su propio límite de reintentos (sin quitar la anterior, que conserva su vencimiento original). Responde `Err` en todos los casos. Una entrada pendiente solo borra si el contenido coincide **con su propia huella**, así que la limpieza de una contraseña vieja nunca elimina una nueva ni contenido distinto.
4. **Vencimiento:** `clear_if_matches`. `Cleared` y `NotOurs` terminan la entrada. `Unavailable` reintenta hasta 3 veces separadas 250 ms (con `Instant`) y después se rinde y la descarta, registrando solo el hecho. Límites duros: como máximo 2 entradas en la lista (la vigente y, solo tras un resultado dudoso, la anterior) y cada una con un máximo de 3 reintentos; no hay vigilancia indefinida. Nunca se borra a ciegas.
5. **Misma contraseña copiada a mano antes del plazo:** el contenido coincide con la huella, así que se borra cuando vence el plazo del envío original. Es el comportamiento deseado y no se distingue de otro modo; se documenta y se prueba.
6. **Un envío nuevo mientras hay reintentos pendientes:** el `deliver` se atiende en cuanto llega (el `recv_timeout` espera al siguiente vencimiento, no bloquea).
7. **Salida desde la bandeja:** `lib.rs` llama a `shutdown` (espera acotada, 2 s) en `RunEvent::Exit`; el trabajador intenta `clear_if_matches` para cada entrada pendiente con un máximo de 3 intentos cortos (100 ms). Si Windows no deja comprobar, no se borra y el cierre sigue; nunca se bloquea indefinidamente.

### Notificación

`tauri-plugin-notification` registrado en el `Builder`; `deliver_and_notify` usa `app.notification().builder().title("Bóveda").body("Contraseña lista para pegar").show()`. El texto no incluye la contraseña ni el nombre del servicio.

### Memoria (verificado en el código)

- **Escritura (`arboard` 3.7.0):** convierte el UTF-8 directamente a UTF-16 dentro del bloque de memoria global que entrega a Windows (`MultiByteToWideChar`); **no** hay un búfer UTF-16 intermedio en nuestro proceso. La contraseña llega como `&str` prestado (`expose_secret()`), sin copia nuestra. Una vez en el portapapeles, la memoria es de Windows y no se puede borrar; lo único que se hace es vaciar el portapapeles. En el camino de fallo de `SetClipboardData`, `clipboard-win` libera el bloque sin borrarlo.
- **Lectura (`clipboard-win`):** `get_string` convierte con `WideCharToMultiByte` hacia nuestro `Vec<u8>`, que va en `Zeroizing` y con capacidad reservada de una vez (sin reasignaciones que dejen copias). Se compara por huella **sobre los bytes**, sin construir un `String`.
- **Copia del `deliver`:** la contraseña se copia una vez a un `SecretString` para cruzar al hilo; vive lo que dura la escritura.
- Se anota en `ARCHITECTURE.md` solo lo verificado: la copia en la memoria global de Windows no se puede borrar, la ruta de fallo de `SetClipboardData` libera sin borrar, y la copia momentánea del `deliver`. No se afirma que la memoria del secreto se borre por completo: la representación del portapapeles de Windows queda fuera de esa garantía. Sin primitivas propias.

## Pruebas

**Automatizadas (`cargo test --workspace`)** con un `FakeBackend` (portapapeles simulado con contenido texto, imagen, vacío y ocupado programables, y un registro de llamadas) y tiempos cortos inyectados:

- escritura correcta (y que use el camino sensible); escritura fallida conserva el vencimiento original; escritura dudosa con y sin `Unavailable`;
- el borrado ocurre a los 30 s (plazo corto en la prueba) solo si coincide; contenido distinto, vacío e imagen no se tocan;
- reemplazo del temporizador con dos envíos seguidos, y que el primero ya no borre;
- misma contraseña copiada a mano: se borra al vencer el envío original;
- ocupado: reintentos acotados (3), nunca borra a ciegas, y un `deliver` nuevo durante los reintentos se atiende;
- cierre: limpia lo pendiente, respeta el límite de espera y no borra si no puede comprobar;
- carrera: el trait solo ofrece `clear_if_matches` (no hay `get` y `clear` separados) y el `FakeBackend` registra que la lectura, la comparación y el vaciado ocurren dentro de **una sola apertura**; una prueba con un backend que «copia algo ajeno» en medio de la operación comprueba que lo ajeno no se pierde; otra, con contenido modificado por otra aplicación antes del vencimiento, comprueba que no se borra;
- contraseña con NUL: `deliver` devuelve `InvalidPassword`, el portapapeles y el temporizador anterior quedan intactos y no se llama al backend;
- fallo de apertura (`Unavailable`) en el vencimiento y en la salida: reintentos acotados, sin borrar;
- escritura parcialmente exitosa: limpieza inmediata si coincide; con `Unavailable`, la huella nueva queda con su límite y la anterior conserva el suyo;
- huella: misma contraseña da la misma huella en un gestor y distinta entre gestores (clave aleatoria);
- el fallo de la notificación no cancela el temporizador.

**Reales de Windows (`#[ignore]`, contraseñas ficticias):** escribir y comprobar el contenido; `clear_if_matches` con contenido distinto; un portapapeles mantenido abierto por la propia prueba hace que `clear_if_matches` devuelva `Unavailable` y que otro hilo no pueda escribir mientras tanto (evidencia de la exclusión); y que la guarda se libera tras un error (otra apertura posterior funciona). Se ejecutan con `cargo test -p boveda-desktop -- --ignored --nocapture`.

**Manuales (se documentan en la PR y en `docs/roadmap.md` cuando se corran):** con el compilado de `bun run tauri build --no-bundle` o la prueba `#[ignore]` que escribe una contraseña ficticia y espera: (1) Win+V no muestra la contraseña; (2) con la sincronización del portapapeles activada, no aparece en otro equipo; (3) a los 30 s el portapapeles queda vacío; (4) si antes se copia otra cosa, no se toca; (5) se ve el toast «Contraseña lista para pegar».

## Documentación

- `ARCHITECTURE.md`: `clipboard.rs` y sus dependencias en `desktop/`, la decisión de la carrera y el borrado de memoria (solo lo verificado).
- `docs/especificacion.md`: párrafo «Decidido el 2026-11-20 (#98)» con las reglas de arriba.
- `docs/roadmap.md`: marcar la tarea del portapapeles como hecha **a nivel de módulo**; el flujo completo sigue en #100 y #104.
- `docs/diferencias-diseno.md`: el toast en desarrollo; el aviso con cuenta regresiva queda como diferencia ya registrada.
- `deny.toml`: `BSL-1.0` en `allow`, con comentario.
- La PR lista los crates nuevos de `Cargo.lock` (comparando antes y después) y el resultado de `cargo deny` y `cargo audit`.

## Limitaciones conocidas

- **Escritura dudosa y el historial de Win+V** (hallazgo de la revisión final). `arboard` pone el texto y después agrega los formatos de exclusión; si agregarlos falla (o si Windows no registra un formato y `arboard` omite la exclusión sin avisar), la contraseña pudo quedar un instante sin excluir y llegar al historial de Win+V o a la nube antes de la limpieza inmediata, y vaciar el portapapeles no borra entradas del historial. Es muy poco probable y no se puede detectar desde fuera; cerrarlo del todo exigiría escribir también con `clipboard-win` bajo una sola guarda, lo que choca con «escribe con `arboard`» y queda como decisión pendiente.
- **`deliver` espera sin plazo** la respuesta del trabajador: si un intento de borrado anterior quedó dentro de la lectura esperando a una aplicación con renderizado diferido que no responde, `deliver` tarda lo que tarde Windows. #100 debe tenerlo en cuenta al fijar el plazo con el que contesta al celular.

## Qué queda al constructor

- Forma exacta de `ClipboardError` y `WriteError` (sin datos sensibles) y cómo se inyecta el reloj.
- Dónde se cablea `RunEvent::Exit` frente a `ExitRequested { code: Some(_) }`; se elige el que tenga garantía de ejecutarse una vez.
- Si `arboard::Error` distingue el fallo parcial de la escritura lo bastante bien como para no tratar todo error como dudoso.
