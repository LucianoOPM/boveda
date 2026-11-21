# Diseño: ventana del QR y confirmación al emparejar (#101)

Fecha: 2026-11-20. Issue #101, parte de la épica #92. Depende de #95 (núcleo del emparejamiento) y #100 (servidor).

## Intención

«Emparejar celular» en la bandeja abre una ventana pequeña con el QR de la PC. Cuando un celular escanea el QR y completa el apretón de manos, la PC pregunta «¿Emparejar con [nombre]?». Solo si el usuario acepta, y el protocolo termina la confirmación autenticada, el celular se guarda en la lista de la #99.

Por qué: el emparejamiento es el único momento en que la PC y el celular intercambian sus llaves. Si se hace mal, un tercero en la misma red podría hacerse pasar por un celular de confianza. La seguridad la da el código de un solo uso (que viaja solo en el QR) y la decisión explícita del usuario en la PC.

## Alcance

- Ventana del QR con la oferta del núcleo: llave pública de la PC, IPv4:puerto y código de un solo uso de 2 minutos, con cuenta regresiva.
- Confirmación del usuario antes de guardar.
- Cancelar, Esc o la X invalidan la sesión y cierran la ventana. Caducar invalida el código y la ventana sigue abierta (ver «Caducidad»).
- Guardar el celular en la lista compartida (`PairedDeviceStore`).
- Enrutar en el servidor las conexiones de emparejamiento (toca código de #100).
- Habilitar «Emparejar celular» en la bandeja.
- Empaquetar las fuentes de `DESIGN.md` §3 con la primera ventana, como ya anotaba `docs/diferencias-diseno.md`.

## Fuera de alcance

Escanear el QR y la pantalla del celular (#103), la lista de dispositivos y la revocación (#107), el clic izquierdo de la bandeja, el ícono monocromo y IPv6 (queda fuera de la etapa, ver «Servidor de la PC»).

## Metas, no metas y anti-metas

- **Metas.** Un solo QR y una sola ventana, tal como lo describe `DESIGN.md` §8. Rust es la única fuente de verdad de la sesión, los plazos y la confirmación. Sin confirmación no se guarda nada.
- **No metas.** Selector de interfaz, varios QR, varias sesiones simultáneas.
- **Anti-metas (fracasos aunque «funcione»).** Que el código de un solo uso llegue al webview como dato, a un log, a un archivo o a un mensaje de error; que una interfaz retrasada pueda aceptar una sesión caducada; que un celular quede guardado sin confirmación.

## Restricciones

- Reglas de `desktop/CLAUDE.md`: nada de secretos por el IPC de Tauri; el canal es TCP ligado a las IPv4 privadas.
- Toda la criptografía vive en `vault-core`; aquí solo se llama a `serve_pairing`, `PendingConnection::confirm/reject` y `PairingSession`.
- Dependencia nueva autorizada por el usuario: `qrcode` 0.14 con `default-features = false` y solo `std` y `svg` (sin `image`). Ninguna dependencia nueva en `bun`. `cargo audit` y `cargo deny check` deben seguir pasando.
- `docs/especificacion.md` manda sobre `DESIGN.md`; ambas sobre la issue donde choquen.

## Enfoques considerados

**Cómo llega el QR al webview** (elegido el 2026-11-20):

1. Rust dibuja el QR en SVG y el webview solo muestra la imagen como `data:` URI. **Elegido.** El webview nunca recibe la oferta, el código ni llaves por separado, y no hay paquete de QR en JavaScript.
2. Rust envía la matriz de módulos y React la dibuja. Descartado: el webview recibiría una representación completa del código.
3. Biblioteca de QR en JavaScript. Descartado: el webview recibiría el texto de la oferta y habría que auditar otro paquete.

**Dirección del QR** (elegido): una automática, la IPv4 privada con puerta de enlace y menor métrica. Descartados el selector (añade un control que el diseño no tiene) y varios QR (no caben en 360 × 480). Si el celular está en otra subred, lo resuelve el flujo de error de #103, no esta ventana.

**Cómo distinguir emparejamiento de envío en el mismo puerto** (elegido): mirar los 2 bytes de largo de la primera trama con `peek`, sin consumirlos. Descartado un segundo puerto: el QR y mDNS solo publican uno, y el Firewall tendría otra regla.

## Decisiones

### Servidor: enrutar por la primera trama (toca #100)

El servidor de #100 asume que toda conexión empieza con la trama de **96 bytes** del envío: `run_connection` llama siempre a `receive_send`, que rechaza cualquier otro largo sin leer. La primera trama del emparejamiento mide **48 bytes**, así que #101 debe cambiar ese punto.

- Con la conexión ya aceptada y con ranura, se hace `peek` de los 2 bytes del encabezado de largo (u16 big-endian) con plazo (`step` de 10 s, en lecturas de 250 ms para respetar la cancelación). `peek` no consume: `receive_send` y `serve_pairing` leen el encabezado como siempre.
- `48` → emparejamiento; `96` → envío; cualquier otro largo, un cierre antes de tener los 2 bytes o un plazo vencido → se cierra sin leer más ni escribir. Un largo erróneo nunca llega a `PairingSession`, así que un atacante no puede gastar sus 3 intentos con tramas mal formadas.
- Las dos rutas comparten el límite de 4 conexiones. Una conexión de emparejamiento ocupa su ranura mientras espera la decisión del usuario (hasta 60 s).
- El servidor no sabe de ventanas: recibe un `PairingHandler` en `Deps` (trait, con un doble en las pruebas). Sin sesión activa, el manejador cierra sin leer ni escribir.
- Se actualizan `ARCHITECTURE.md` y «Servidor de la PC» para decir que el servidor atiende dos tipos de primera trama.

### Dirección del QR

- Solo se considera una dirección que el servidor **esté escuchando ahora**. El supervisor publica una instantánea `{generación, puerto, direcciones}`; el controlador elige de ahí, nunca de una consulta aparte.
- **Elegibilidad (aprobada el 2026-11-20).** Solo cuenta una IPv4 que el servidor esté escuchando y cuyo adaptador esté **activo** (`OperStatus` = Up), sea de **hardware** (no virtual: Hyper-V, WSL, VMware, VirtualBox, túneles, VPN; se comprueba con `HardwareInterface` de `MIB_IF_ROW2`, `GetIfEntry2`, y no por el nombre del adaptador, que es frágil; el plan lo verifica contra el código de `windows-sys` y, si no sirve, se documenta qué otra señal se usa) y de tipo Ethernet o Wi-Fi. Además la dirección pasa `is_usable` (sin loopback, link-local, no especificada, multidifusión ni difusión). Una interfaz que no cumple **nunca** se elige, tenga la métrica que tenga.
- Entre las elegibles: primero las que tienen puerta de enlace (`FirstGatewayAddress` de `GetAdaptersAddresses`, que `nlm.rs` ya consulta); entre ellas, la menor métrica IPv4; empate → la IPv4 numéricamente menor (determinista). Si ninguna elegible tiene puerta de enlace (red local sin router), se usa la menor métrica **solo entre las elegibles**. Si no queda ninguna elegible, no hay QR (ver `NoNetwork`).
- La elección es una función pura con pruebas (varias interfaces, empates, virtuales y caídas excluidas aunque tengan la métrica más baja, sin puerta de enlace, conjunto vacío).
- Sin dirección elegible (sin red privada) no se genera ni se muestra QR (ver «Estados»).

### Sesión y estado en Rust

Un `PairingController` (estado de Tauri) es el único dueño de la sesión. Ninguna operación cruza el IPC con identificadores: las órdenes del webview no llevan datos, y Rust decide si son válidas en el estado actual.

- **Una sola sesión.** Pedir «Emparejar celular» con la ventana abierta la enfoca; nunca crea una segunda sesión. «Generar otro código» cancela la sesión anterior (su `CancelToken` y `PairingSession::cancel`) antes de crear otra: **no se reutiliza ninguna sesión.**
- **Por sesión:** una `PairingSession` nueva con un `KeyPair` leído del `IdentityStore` (`load()`, copia propia; la lectura con DPAPI es barata y no comparte el `Arc` del servidor), un `PairingCode::generate()` y un `CancelToken` propio. Si la identidad no se puede leer (archivo dañado o de otra cuenta), la ventana lo explica y no se crea sesión; nunca se regenera.
- **Plazos.** Los 2 minutos los cuenta `PairingSession` con `Instant` (ya en el núcleo). El controlador guarda además `expires_at: Instant` para decidir en Rust, no en el webview. Un hilo del controlador revisa cada 250 ms: al vencer, cancela la sesión, descarta la imagen y emite el nuevo estado. La confirmación tiene 60 s (`PairingTimeouts::confirmation`, `PendingPairing::deadline`).
- **Conexión pendiente.** El hilo trabajador del servidor corre `serve_pairing` y, al volver con un `PendingConnection`, lo entrega al controlador y **espera la decisión** (un canal con `recv_timeout` hasta `deadline`). El controlador no toca sockets.
- **Aceptar.** `accept` solo hace algo si la fase es «confirmando» y `Instant::now() < deadline` según Rust; si no, se ignora. Ejecuta `confirm()`. Solo si devuelve `Ok(PairedDevice)` se guarda en `PairedDeviceStore` (`paired_at` en ms Unix). Rechazar, cancelar, agotar el plazo, un error de `confirm` o un celular que se desconecta no guardan nada.
- **Si guardar falla tras `confirm`.** Es el caso (b) ya aceptado en «Emparejamiento con Noise»: el celular guardó a la PC y la PC rechazará esa llave en el envío. La ventana lo dice («No se pudo guardar el celular») y ofrece generar otro código; no se promete atomicidad entre dispositivos.
- **Llave ya emparejada** (`AlreadyPaired`): se trata como éxito sin cambios, como pide la especificación (la PC debe tratar una llave existente como actualización). **Mi decisión:** se conserva el nombre guardado.
- **Candado envenenado** en la lista: error, no se guarda.
- **Cambio de red.** Si cambia la generación del servidor, o la dirección o el puerto de la instantánea dejan de ser los de la oferta, el QR quedó obsoleto: se cancela la sesión y se muestra «La red cambió». El criterio por generación es más estricto que el de dirección y puerto y los incluye; el controlador lo revisa en cada vuelta de su hilo y también al entregarle una conexión, así que un QR anterior no sigue siendo utilizable.
- **Carreras (aprobado el 2026-11-20).** Cada sesión lleva un `SessionId` interno, creciente y solo de Rust. La `PendingConnection` que entrega un hilo trabajador, la decisión del usuario y el resultado de `confirm` viajan etiquetados con el `SessionId` de la sesión que los originó, y el controlador **descarta** cualquiera cuyo id no sea el vigente: una respuesta tardía de una sesión anterior nunca modifica ni autoriza la nueva (la conexión vieja se rechaza o se cierra y no se guarda nada). Las órdenes del webview siempre actúan sobre la sesión vigente y no pueden nombrar otra. Casos cubiertos con pruebas: caducidad mientras llega una conexión (el servidor entrega la conexión justo cuando vence; gana el plazo de Rust y no hay diálogo), cancelar durante la confirmación (no se guarda nada y el celular recibe cierre o rechazo), cambio de red durante la espera y durante la confirmación, generar una sesión nueva con una conexión pendiente de la anterior, y aceptar con la orden llegando después de la regeneración.
- **Cerrar o salir.** `CloseRequested` de la ventana equivale a cancelar. «Salir» llama al controlador para cancelar antes de parar el servidor.

### Estados y textos

Un DTO serializable, `PairingView`, es todo lo que recibe el webview, por un evento `pairing://state` y una orden `pairing_snapshot` al abrirse. Sus fases:

| Fase | Qué muestra | Datos que cruzan |
| --- | --- | --- |
| `NoNetwork` | «No hay una red privada» y por qué | ninguno |
| `Waiting` | QR, «CADUCA EN m:ss», Cancelar | imagen (`data:` URI) y milisegundos restantes |
| `Confirming` | «¿Emparejar con [nombre]?», huella, Emparejar, Rechazar | nombre, huella, milisegundos restantes |
| `Saving` / `Done` | «Emparejado con [nombre]» y Listo | nombre |
| `Ended(motivo)` | motivo y «Generar otro código» | solo el motivo |

Motivos de `Ended`: caducó, rechazado, agotó el plazo de la confirmación, la red cambió, el celular se desconectó, no se pudo guardar, identidad no disponible. Los textos fijos de `DESIGN.md` §8 se respetan; el resto sigue §10 (español neutro, tuteo, el botón dice la acción). **Mi decisión** sobre los textos que el diseño no da: «Escanea este código con Bóveda en tu celular», «CADUCA EN 1:45», «No se encontró una red privada. Conecta la PC a una red que Windows marque como privada.», «Esta ventana caducó», «Emparejado con [nombre]».

- La cuenta regresiva del webview es solo visual: se calcula localmente desde los milisegundos que manda Rust y se corrige con cada evento. Si el webview se atrasa o se suspende, no puede aceptar nada: Rust revisa su propio plazo. Pasa a `warning` en los últimos 20 s (`DESIGN.md` §8).
- El nombre del celular lo da un tercero (aunque ya viene validado por el núcleo: de 1 a 128 bytes, sin caracteres de control). Se muestra como texto (React lo escapa) y **siempre junto a la huella corta de la llave**, en `secret`, como pide §8. El nombre es solo informativo: la identidad es la llave. La huella ayuda a comparar con la pantalla del celular, pero no autentica por sí sola.

### Caducidad (resuelve la contradicción con la issue)

La issue dice que caducar «cierra la ventana»; `DESIGN.md` §8 dice que el QR se atenúa al 20 % y aparece «Generar otro código». **Decidido por el usuario el 2026-11-20: manda el diseño.** Al caducar: el código queda invalidado, la ventana permanece abierta y ofrece «Generar otro código», que crea una sesión nueva. Cancelar, Esc o la X invalidan la sesión y cierran la ventana.

**Cómo se concilia con tratar la imagen como sensible.** **Diferencia con `DESIGN.md` §8, a documentar en `docs/diferencias-diseno.md`:** el diseño atenúa el QR real; aquí se atenúa un **marcador fijo** (una cuadrícula genérica al 20 %), porque no se conserva un QR que contiene un código ya invalidado (decidido por el usuario el 2026-11-20). Al caducar, cancelar, completar, rechazar o cambiar de red, Rust descarta el SVG de su estado y el webview lo borra de su estado y de la pantalla. El marcador no contiene ningún dato.

### Imagen sensible

La imagen del QR contiene el código de emparejamiento y se trata como información sensible:

- Vive solo en memoria: no se escribe a disco, no hay archivo temporal ni caché propia, y no se guarda en `localStorage`. Llega al `<img>` como `data:` URI, nunca como navegación a una URL.
- No aparece en logs, `Debug` ni mensajes de error: el DTO no implementa `Debug` con la imagen (muestra solo la fase), y los errores usan los tipos del núcleo, que no llevan datos. Hay una prueba que lo comprueba para cada fase.
- El SVG se genera una vez por sesión y se reemplaza por nada al terminar la sesión (el `String` va en `Zeroizing` en el controlador; en el webview el estado se pone en `null`).
- **A verificar con Context7 y el código de Tauri en uso:** si `content_protected` existe y qué hace en Windows, y si WebView2 deja alguna copia de la imagen en su perfil (`%LOCALAPPDATA%\com.jesus.boveda\EBWebView`). Ninguna protección contra capturas ni contra la persistencia se presenta como garantía si no se comprobó: lo que no se pueda confirmar se documenta como límite conocido, sin prometerlo.

### Formato del QR

La especificación de #94 dejó abierto el texto exacto y el nivel de corrección. **No se fija por intuición:** la primera tarea del plan es una prueba que codifica una `PairingOffer` real (con IPv4 y con IPv6) con `qrcode` y registra versión, módulos y tamaño del SVG en estas variantes: bytes crudos, base64url sin relleno y base32 en mayúsculas (modo alfanumérico), con niveles de corrección L, M y Q. Criterios, fijados antes de medir y con prioridad en **escanear de forma confiable dentro de 240 × 240**, no en minimizar la versión: nivel **al menos M**; píxeles por módulo = 240 / (módulos + 8) (incluye la zona de silencio de 4 módulos), lo más altos posible y **nunca menos de 4 px**; y un texto que cualquier lector entregue sin pérdida (por eso base64url de texto es la opción por defecto y solo se cambia si otra variante gana claramente en píxeles por módulo sin bajar el nivel de corrección). Entre dos variantes aceptables gana la de mayor margen (más px por módulo y mayor nivel de corrección). La medición no sustituye a escanear: la prueba manual con un celular real (a cargo del usuario) confirma que la ventana de verdad se lee. El resultado y la codificación exacta se escriben en `docs/especificacion.md` como contrato con #103.

### Interfaz (frontend)

- Una ventana `pairing` creada desde Rust con `WebviewWindowBuilder` (`tauri.conf.json` mantiene `windows: []`), no redimensionable, centrada; carga `index.html?view=pairing` y `App.tsx` elige la vista por ese parámetro. Cambia entre 360 × 480 y 360 × 200 según la fase. En `Confirming` se enfoca la ventana y se pide atención del usuario.
- Estilos solo con variables de `tokens.css` (sin colores ni tamaños literales; la única excepción, `#0F1720` sobre blanco del QR, va dentro del SVG que dibuja Rust, como pide §8). Tarjeta blanca de radio 16, QR de 240 × 240.
- Fuentes Plus Jakarta Sans y JetBrains Mono en `desktop/src/assets/fonts/`, copiadas de `mobile/assets/fonts/` con sus licencias OFL (sin descargar nada). El QR usa `metric` en JetBrains Mono. Sin íconos nuevos.
- Teclado: Esc cancela; Tab recorre los botones; en la confirmación, «Rechazar» tiene el foco por defecto; foco visible con anillo de 2 px en `accent`; objetivos de 32 × 32 o más; respeta `prefers-reduced-motion` y `forced-colors`.
- Órdenes del webview (sin datos): `pairing_snapshot`, `pairing_cancel`, `pairing_regenerate`, `pairing_accept`, `pairing_reject`.
- Sin dependencias nuevas de `bun`.

### Bandeja

`ID_PAIR` pasa a habilitada y abre la ventana. Se actualiza la prueba de `menu_spec`. «Dispositivos emparejados» sigue deshabilitada hasta #107.

## Pruebas

- **Controlador (sin red, con reloj inyectado):** una sola sesión; caducar invalida y descarta la imagen; regenerar no reutiliza; aceptar tras `deadline` se ignora aunque el webview lo pida; rechazar, cancelar, agotar el plazo y cerrar no guardan nada; guardar solo tras `confirm` correcto; `AlreadyPaired`; fallo al guardar; candado envenenado; cambio de generación.
- **Servidor:** enrutado 96 → envío, 48 → emparejamiento, otro largo → cierre sin leer, menos de 2 bytes y plazo; que `peek` no consuma; el límite de 4 se comparte; sin sesión activa se cierra sin escribir.
- **De punta a punta por `127.0.0.1`:** el servidor real con el controlador y `pair_with_pc` como celular: código correcto + aceptar guarda; sin QR (código ajeno) no se empareja y no se guarda nada; rechazar no guarda; código reutilizado.
- **Selección de dirección** y **formato del QR** (la medición de arriba).
- **Seguridad:** ningún `Debug`, error ni log del DTO contiene la imagen ni el código, en cada fase.
- **Frontend:** `bun run tauri build` debe pasar (incluye `tsc`).
- Mutaciones: desactivar cada protección (plazo en Rust, guardar solo tras `confirm`, enrutado por largo) y comprobar que la prueba correspondiente falla.
- **Manual, a cargo del usuario** (no se automatiza): ver el QR en pantalla, la cuenta regresiva y el cambio de color; Esc y la X; que la ventana no se abre dos veces; el comportamiento con el celular real queda para #103 y el cierre (#108).

## Documentación que cambia

- `docs/especificacion.md`: nueva subsección «Ventana del QR y confirmación (#101)» (estados, caducidad, imagen sensible, formato del QR, dirección elegida, enrutado por la primera trama) y ajuste de «Servidor de la PC» y del paso 1 de «Emparejamiento».
- `ARCHITECTURE.md`: módulo nuevo del controlador, ajuste del servidor, frontend y fuentes.
- `docs/diferencias-diseno.md`: se quita la fila de las fuentes, se actualiza «Emparejar celular» y se anotan las diferencias de esta ventana: el estado «Emparejado con [nombre]», que el diseño no tiene, y el marcador fijo en lugar del QR atenuado.
- `desktop/CLAUDE.md`: la interfaz solo muestra la imagen del QR.
- `desktop/src-tauri/Cargo.toml`: `qrcode`.

## Qué queda para quien construya

Nombres de módulos y tipos; cómo se leen la puerta de enlace y la métrica en `GetAdaptersAddresses`; la forma exacta del canal entre el hilo trabajador y el controlador; el contenido del marcador del QR; los textos menores. La codificación del QR se decide con la medición, con los criterios de arriba.
