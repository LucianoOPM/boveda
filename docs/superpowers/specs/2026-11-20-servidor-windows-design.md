# Windows: servidor (redes privadas, mDNS, rechazo de no emparejados) (#100)

Parte de la épica #92. Dependencias #96 y #99 (hechas). Tamaño: proyecto mediano en `desktop/src-tauri`, sin cambios en `core/`. Este diseño recoge lo aprobado por el usuario el 2026-11-20 y deja marcado lo que es decisión mía.

## Intención

La PC debe recibir envíos del celular por la red local sin exponerse en redes que no sean de confianza. La garantía que se quiere, en palabras del usuario: **si Windows no considera privada ninguna red apta, la app no escucha en ningún socket ni publica mDNS**. La regla del Firewall es una segunda capa, no el único mecanismo. Esta issue entrega el servidor; el QR y la confirmación son #101, el portapapeles ya está (#98) y la búsqueda desde el celular es #104.

## Metas

- Servidor TCP que escucha **solo** en las direcciones IPv4 de redes cuyo perfil de Windows es `Private`. Nunca `0.0.0.0`.
- Anuncio mDNS únicamente mientras exista un listener válido, y solo por esas mismas direcciones.
- Rechazo de dispositivos no emparejados durante el intercambio Noise, antes de leer contenido. Lo hace `receive_send` del núcleo; `server.rs` no autoriza nada por su cuenta.
- Reevaluación periódica de la red y arranque y parada completos del servidor según el conjunto de direcciones privadas.
- Regla de Firewall limitada al perfil `Private`, instalada con privilegios.

## Fuera de alcance y antimetas

- QR y confirmación (#101), portapapeles (#98), búsqueda desde el celular (#104), interfaz de dispositivos (#107).
- **IPv6, deliberadamente fuera.** Solo se escucha y se anuncia sobre IPv4.
- Eventos COM de cambio de red: no se usan. Si más adelante se quiere reacción inmediata, se sustituye el backend de detección sin tocar el servidor.
- Antimetas: que una IP RFC1918 baste para considerar privada una red (una red pública puede usar `192.168.x.x`); que un fallo de mDNS reinicie el listener; que se anuncie por una interfaz que no pasó la comprobación de red.

## Aproximaciones consideradas

1. **Revisión periódica del perfil de red (elegida).** Cada 5 s se calcula el conjunto de IPv4 privadas; solo se actúa si cambia. Se prueba con un detector falso y sin esperas.
2. Eventos COM de cambio de red: reacción inmediata, pero más código COM y más difícil de probar. Descartada para esta issue.
3. Parar y no reintentar hasta reiniciar: la más simple, pero la función queda muerta tras cualquier cambio de red. Descartada.

Para el acceso a la red de Windows: la crate `windows` 0.62 (ya en el lock por Tauri) frente a vtables manuales con `windows-sys`. Se elige `windows` para reducir `unsafe`. La Regla de Firewall se trata en su apartado.

## Restricciones

- `desktop/CLAUDE.md`: el servidor escucha solo en redes privadas y rechaza a no emparejados en Noise antes de leer contenido; las contraseñas no cruzan al webview.
- Raíz: sin secretos en logs, errores ni `Debug`; sin lógica de plataforma en `vault-core`; sin dependencias de red nuevas salvo `mdns-sd`; pruebas primero; `cargo test --workspace`, `clippy -D warnings`, `fmt`, `audit` y `deny` pasan antes de dar la tarea por terminada.
- Los errores se registran solo con su tipo, nunca con contenido de la conexión.

## Dependencias

| Crate | Uso | Notas |
| --- | --- | --- |
| `mdns-sd` 0.22 | Anuncio mDNS | Aprobada en el épico. Licencia Apache-2.0 OR MIT. Trae `fastrand`, `flume`, `if-addrs`, `mio`, `socket-pktinfo` y `socket2`; la lista exacta de crates nuevos en el lock va en la PR y se pasa por `cargo deny` |
| `windows` 0.62 (solo `desktop/`, `cfg(windows)`) | `INetworkListManager` y COM | **Aprobada por el usuario.** Ya está en el lock (la usa `tao`). Mínimo de features: `Win32_Networking_NetworkListManager`; si no alcanza, solo las auxiliares de COM o Foundation que hagan falta (por ejemplo `Win32_System_Com`), cada una justificada en la PR. **Nunca en `core/`** |
| `windows-sys` (ya directa) | `GetAdaptersAddresses` | Se agrega `Win32_NetworkManagement_IpHelper` y las que pida; sin crates nuevos |

## Diseño

### Detección de red (`network.rs`)

```rust
trait NetworkProbe: Send + 'static {
    /// IPv4 de las interfaces cuyo perfil de Windows es Private, sin
    /// duplicados y ordenadas. Cualquier error o duda devuelve un error:
    /// quien llama lo trata como conjunto vacío (fail closed).
    fn private_addresses(&mut self) -> Result<BTreeSet<Ipv4Addr>, ProbeError>;
}
```

- Backend real: `INetworkListManager::GetNetworks` con las redes conectadas; solo `NLM_NETWORK_CATEGORY_PRIVATE`. `DomainAuthenticated` y `Public` no cuentan. Cada red se enlaza a sus conexiones, y de cada conexión se toma el id del adaptador; ese id se cruza con `GetAdaptersAddresses` para obtener sus IPv4.
- Se descartan loopback, link-local (`169.254.0.0/16`), no especificadas y multicast: aunque el adaptador sea privado, no son direcciones donde el celular pueda alcanzar a la PC. **Decisión mía.**
- COM se inicializa (MTA) en el hilo del supervisor y se libera al terminar. Cualquier fallo de COM, de la API o de la correspondencia adaptador-IP da error y, por tanto, conjunto vacío.
- El estado supervisado es el **conjunto normalizado** (`BTreeSet`), no un booleano. El mismo conjunto en otro orden es igual y no hace nada.

| Cambio | Acción |
| --- | --- |
| `{A}` → `{}` | Bajar la generación |
| `{}` → `{A}` | Subir una generación |
| `{A}` → `{A, B}` | Bajar y subir una nueva |
| `{A}` → `{C}` | Bajar y subir una nueva |
| Mismo conjunto, otro orden | Nada |

### Supervisor (`server.rs`)

- `ServerSupervisor` guarda el estado de Tauri y un hilo propio. Cada 5 s (`Duration` inyectable) llama a un `tick()` también público dentro del crate, para probar `privada → pública → privada` sin esperas.
- Si el conjunto cambió, baja la generación vigente (si hay) y sube otra con el conjunto nuevo (si no está vacío).
- `stop()` (al salir) baja la generación y termina el hilo. Se llama en `RunEvent::Exit`, **antes** de apagar el portapapeles.
- Un fallo real del listener (el hilo de `accept` murió o un `accept` devuelve un error no recuperable) marca la generación como caída. En la siguiente revisión se sube una nueva con el mismo conjunto; con un reintento escalonado acotado para no girar en vacío. **Un fallo de mDNS no cuenta como fallo del listener.**

### Generación (`server.rs`)

Una generación es: un `CancelToken` propio, uno o más `TcpListener` (uno por IPv4 privada), el hilo de `accept` y sus trabajadores, y el anuncio mDNS.

**Orden de arranque:** crear los `TcpListener` y comprobar que todos se enlazaron → solo entonces anunciar mDNS. **Orden de parada:** retirar el anuncio mDNS (`unregister`, esperando su estado un instante, y `shutdown` del daemon) → cancelar el token → cerrar los listeners → esperar a los trabajadores con un plazo acotado.

- **Puerto.** Fijo preferido, 47615 (rango dinámico; **decisión mía**). Si está ocupado en alguna IP, se pide un puerto al sistema y ese es el de la generación. Un puerto fijo permite escribir solo la IP en el modo manual. El puerto real es el que leerán mDNS y el QR (#101). Se expone con una función `local_port()` en el estado.
- **Un único puerto por generación (precisión del usuario).** Con el fallback, todos los listeners de una generación usan el **mismo** puerto; nunca se hace `bind(ip:0)` por separado en cada IP. Algoritmo: (1) intentar enlazar todas las IPs a 47615; (2) si alguna falla por «dirección en uso», cerrar todo, obtener un puerto candidato (enlazar una vez a `ip0:0` y leer el puerto asignado) e intentar enlazar **todas** las IPs a ese puerto, soltando el sondeo justo antes; (3) si hay una carrera y alguna falla, cerrar todo y repetir con un candidato nuevo, con un número de intentos acotado; (4) agotados los intentos, la generación no sube y se reintenta en la siguiente revisión. `local_port()`, mDNS y el QR exponen un solo puerto por generación.
- **Bind.** Cada IPv4 privada tiene su propio `TcpListener`. Si falla alguno, se cierran los demás de la generación y se reintenta en el siguiente ciclo: nunca se queda escuchando en un subconjunto sin avisar. **Decisión mía.**
- **Aceptación.** Sockets no bloqueantes con sondeo de 100 ms para observar la cancelación; un único hilo de `accept` para todos los listeners.
- **Conexiones.** Un trabajador por conexión que llama a `receive_send(stream, &identity, is_paired, &cancel, &SendTimeouts::default())`. Plazos del núcleo: 10 s por trama y 30 s en total. Límite de **4 conexiones simultáneas** (contador compartido con guarda `Drop`); la excedente se cierra al instante, sin leer ni escribir. Los trabajadores guardan un clon del token de su generación: al cancelar, `receive_send` termina en menos de `CANCEL_POLL` (250 ms).
- **Autorización.** `is_paired` consulta la lista compartida en cada conexión. `receive_send` la invoca después de autenticar la primera trama y cierra sin responder cuando la llave no está. `server.rs` no vuelve a comprobar nada.
- **Entrega.** Con un `IncomingSend`, el trabajador llama a `ClipboardManager::deliver_and_notify` y responde `copied()` o `failed()` según el resultado. La contraseña solo sale de `vault-core` hacia el código Rust nativo que la entrega al `ClipboardManager`; nunca cruza al webview, no se persiste y no se registra. El trabajador responde `copied()` únicamente después de que `deliver_and_notify` haya escrito correctamente el portapapeles; ante fallo responde `failed()`.
- **Errores.** Un error de una conexión se registra con su tipo (`SendError` no tiene secretos) y no afecta a las demás.

### mDNS (`announce.rs`)

- Un `ServiceDaemon` **por generación**, creado y apagado con ella. Servicio `_boveda._tcp.local.`.
- Nombre de instancia y `host_name` aleatorios por generación (`boveda-<8 hex>` y `boveda-<8 hex>.local.`); **nunca** el nombre de la PC.
- TXT: `v=1` e `id=` con la huella corta de la llave pública (`Fingerprint`). Es un identificador estable visible en la red local; la llave pública ya viaja en el QR. Se acepta como coste y se documenta. **Decisión mía.**
- Las direcciones se pasan **explícitas** (las IPv4 privadas de la generación), sin `enable_addr_auto`, de modo que no se publica ninguna otra.
- **Condición de seguridad sobre IPv6 (del usuario).** La documentación de `mdns-sd` no garantiza que `disable_interface(IfKind::IPv6)` ni `Addr(ip)` eviten abrir o anunciar por IPv6: `Addr` identifica una interfaz, no una dirección, `new()` abre sockets IPv4 e IPv6 y el loopback está habilitado por defecto. Por eso **la propiedad de seguridad no depende de esas llamadas**. Se resuelve así, en este orden:
  1. Al empezar la implementación, una tarea de verificación lee el código fuente de `mdns-sd` 0.22.0 y comprueba, con un daemon real, qué sockets abre y por qué interfaces responde y anuncia, con una combinación de `disable_interface(IfKind::All)` y `enable_interface` por interfaz privada (o equivalente).
  2. Si se puede garantizar que solo se anuncia sobre las IPv4 privadas, se usa esa configuración y una prueba la fija.
  3. Si **no** se puede garantizar, **no se continúa en silencio**: se documenta el bloqueo técnico y se le consulta al usuario antes de seguir (alternativas a evaluar entonces: otra versión, abrir los sockets mDNS propios, o anunciar en la capa de red con otro mecanismo).
- Un fallo de mDNS (crear el daemon, registrar, un `DaemonEvent::Error`) **no baja el listener**. Se registra su tipo y se reintenta de forma independiente en las siguientes revisiones, con el mismo acotamiento que el listener. Mientras no haya listener válido no hay anuncio.
- **Reintento independiente (precisión del usuario).** La generación guarda un estado de anuncio propio (`Announced`, o `Retrying { next_attempt: Instant, failures: u32 }`) y el supervisor lo revisa en cada `tick()`, aparte de la comparación del conjunto de IPs. Si `register()` falla con el listener sano y el conjunto sin cambios, solo se vuelve a intentar mDNS cuando llega `next_attempt` (espera creciente y acotada); el listener, sus trabajadores y el token no se tocan. Un fallo temporal de mDNS nunca baja ni reinicia el listener; al bajar la generación, el anuncio pendiente se descarta.

### Lista de emparejados compartida

`PairedDeviceStore` se envuelve en `Arc<Mutex<…>>` y es estado de Tauri. `is_paired` toma el candado, busca la llave y lo suelta. #101 reutilizará esa misma lista al emparejar. **Decisión mía.**

### Ciclo de vida con Tauri (`lib.rs`)

- `setup`: tras `ensure_identity`, se abren los almacenes y, si la identidad se leyó bien, se arranca el supervisor. Si la identidad no se puede leer, el servidor **no arranca** y no se regenera (se hereda la política de #99).
- `RunEvent::Exit`: `supervisor.stop()` y después `clipboard.shutdown(...)`.
- La identidad se carga una vez al arrancar el supervisor y se mantiene en memoria dentro de un `KeyPair` (el tipo ya oculta el secreto en `Debug`).

### Regla de Firewall

Instaladas por el instalador NSIS con `installerHooks` (`NSIS_HOOK_POSTINSTALL` y `NSIS_HOOK_POSTUNINSTALL`):

- Entrada TCP para el programa y entrada UDP 5353 (mDNS) para el programa, ambas con `profile=private`. Nombres de regla fijos para poder borrarlas.
- **Elevación (precisión del usuario).** La instalación se mantiene **por usuario**. Se elevará solo la operación del Firewall con `ExecShellWait "runas"`, que muestra un UAC únicamente para ese comando. Si Tauri/NSIS no lo permite de forma soportada y mantenible, se cambia a `perMachine`, que el usuario ya aceptó, y se registra el motivo. No hay comandos silenciosos lanzados desde la app en cada inicio.
- Si el usuario rechaza el UAC, la instalación continúa sin regla y se avisa. Lo que Windows haga después (mostrar o no su propio aviso) se valida a mano; **la seguridad del servidor no depende de ello**, solo de la comprobación de red de la app.
- **`tauri dev` no crea la regla.** El comando manual para desarrollo se deja en `desktop/README.md`.
- WiX/MSI no documenta una vía equivalente; se mantiene solo NSIS para este punto. Si `bundle.targets` es `all`, se ajusta para no producir un MSI sin la regla.

## Pruebas (primero)

Todas sin red real, salvo donde se indica.

- `network`: el filtro de direcciones (loopback, link-local, no especificadas, multicast) y la normalización del conjunto, con el detector falso. La prueba del backend real, contra la red de la máquina, va como `#[ignore]` (patrón de #98).
- Supervisor con detector falso y `tick()` directo: `{A}→{}`, `{}→{A}`, `{A}→{A,B}`, `{A}→{C}`, mismo conjunto en otro orden; **un error del detector baja el servidor** (fail closed); no se reconstruye si nada cambió (contador de generaciones).
- Generación contra `127.0.0.1` con el detector falso: cancelar corta a los trabajadores activos; una generación nueva usa un token nuevo y no hereda el cancelado.
- Servidor por localhost con el núcleo: `send_to_pc` de un celular emparejado recibe confirmación; un no emparejado queda en `Closed` y el gestor del portapapeles falso **no recibe nada**; la quinta conexión simultánea se cierra sin leer; un cliente que no envía nada se cierra por plazo.
- mDNS detrás de un trait `Announcer` con un doble: se anuncia solo con listener válido, se retira antes de cerrar, el fallo del anuncio no baja el listener y se reintenta. La prueba con el `ServiceDaemon` real, de la condición IPv6, va aparte y `#[ignore]` si necesita multicast.
- Manual (criterio de la issue): Firewall solo en redes privadas; cambio de red con la app abierta; mDNS con el celular real queda para #104/#108.

## Orden de trabajo

1. Verificación de `mdns-sd` (IPv6, interfaces): si falla, parar y consultar.
2. `network.rs` con el detector falso y el filtro; después el backend real.
3. Generación: bind por IP, `accept`, trabajadores y límite.
4. Supervisor y su ciclo de vida en `lib.rs`.
5. `announce.rs` con el `Announcer` y el doble; luego el daemon real.
6. Hook NSIS, README de desarrollo y documentación (`ARCHITECTURE.md`, `docs/especificacion.md`, `docs/roadmap.md`).

## Lo que queda al implementador

- Los intervalos exactos del reintento acotado y el plazo de espera a los trabajadores al bajar.
- El nombre exacto de las reglas del Firewall y de sus archivos del hook.
- Si `GetNetworks` o `GetNetworkConnections` es el camino más corto para obtener el adaptador de cada red, y las features mínimas de `windows` que haga falta sumar.
