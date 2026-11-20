# Diseño: emparejamiento Noise con código de un solo uso (issue #95)

Fecha: 2026-11-20 · Etapa 3 (épica #92) · Rama `feat/95-emparejamiento` (desde `feat/94-protocolo-tipos`; el PR contra `dev` se abre cuando la #94 esté fusionada, para no repetir sus cambios)

## Objetivo

`vault-core` tiene el emparejamiento entre el celular y la PC: un intercambio Noise (`snow`) en el que el celular, que conoce la llave pública de la PC por el QR, y la PC, que conoce el código de un solo uso, se autentican y se pasan sus llaves públicas. Termina con una **confirmación explícita del usuario en la PC**: completar el intercambio no autoriza nada. Se prueba con los dos extremos en el mismo proceso, por `TcpStream` en `127.0.0.1`.

Incluye las máquinas de estado (sin sockets) y un conductor síncrono sobre `Read + Write` con tiempos de espera y cancelación. Sin tokio ni crates de red.

## Fuera de alcance

- Escuchar en la red real, redes privadas, mDNS y limitar las conexiones simultáneas sin autenticar: #100.
- El QR (conversión a texto, imagen, escaneo): #101 y #103.
- El envío de contraseñas y el rechazo de dispositivos no emparejados en sus conexiones: #96.
- Guardar nada: la lista de celulares de la PC es de la #99 y la lista de PCs del celular es de la #93 (`vault::PairedPc`). El núcleo **nunca persiste** una relación de confianza; entrega un resultado y la app lo guarda.
- Exponer algo al puente (#102). `vault::Error` no cambia.
- Que la PC envíe su nombre al celular: no en esta versión (ver "Datos que entrega al celular").
- Un acuse de la aceptación (un mensaje más): no en la #95 (ver "Autorización, entrega y persistencia").

## Decisiones

Aprobadas por el usuario el 2026-11-20:

1. **Patrón** `Noise_XKpsk0_25519_ChaChaPoly_BLAKE2s`, con prólogo `boveda/emparejamiento/v1` (24 bytes). El celular conoce la llave de la PC (QR) y usa el código de 32 bytes como PSK en la posición 0.
2. **Nombre del celular** en el mensaje 3, recortado, de 1 a 128 bytes UTF-8, sin caracteres de control ni de dirección de texto. Es solo informativo: la identidad es la llave pública. La PC muestra el nombre y la huella, y no se puede modificar la identidad durante el diálogo.
3. **Tres intentos fallidos** por código; al tercero queda invalidado. El código se consume al autenticarse el primer mensaje, aunque después se rechace, se cancele o caduque la confirmación. La caducidad (2 minutos) se mide desde que se crea la sesión, con reloj monótono. Un primer mensaje de longitud distinta de 48 bytes cuenta como intento fallido; una conexión que se cierra sin mandar un mensaje completo no cuenta, pero está limitada por tiempo.
4. **Confirmación de 60 s** en la PC; el celular espera hasta 65 s. La aceptación viaja por el canal Noise.
5. **Conductor** sobre `Read + Write` más una interfaz mínima para los tiempos de espera; tramas de largo delimitado, máximo 512 bytes.
6. **Excepción de borrado** de `snow` documentada, sin presentar la mitigación como borrado seguro.
7. **`PROTOCOL_VERSION` sigue en 1**: la #94 no está integrada ni usada, así que los tipos 4 y 5 son parte del protocolo inicial. Desde que se fije, un cambio incompatible sube la versión.
8. **Rama** `feat/95-emparejamiento` desde `feat/94-protocolo-tipos`.
9. **Aceptación perdida por un corte de red:** se acepta que la PC guarde a un celular que no recibió `Accepted`. No hay otro mensaje de confirmación; se documenta y se resuelve con la revocación (#107).
10. **Nombre de la PC:** no se envía en el apretón de manos; la #103 decide cómo lo obtiene el celular. Mientras tanto, `PairedPc` usa un nombre provisional derivado de datos reales (ver "Datos que entrega al celular").

Llamadas mías (el usuario puede cambiarlas):

- **`transport.rs` nace en la #95**, con lo mínimo que necesita (`TimedStream`, `CancelToken`, lectura y escritura de tramas). La #96 lo reutiliza y lo amplía; `ARCHITECTURE.md` hoy dice que llega con la #96 y se corrige.
- **Máquinas de estado por tipos**: cada paso consume el estado anterior y devuelve el siguiente. Pasos fuera de orden no compilan.
- **La sesión de la PC es dueña de su `KeyPair`** (la app arma uno desde la llave que guarda DPAPI al abrir la ventana del QR). Evita préstamos con tiempo de vida dentro de un `Mutex`.
- **Los 60 s se cuentan desde que se crea el `PendingPairing`**, que es cuando la app debe mostrar el diálogo. El núcleo los hace cumplir en `accept`; el temporizador del diálogo es de la app.
- **Los tiempos de espera** son constantes iniciales, ajustables en `PairingTimeouts`: 10 s por paso, 20 s desde que se acepta la conexión hasta tener la trama 3 completa.
- **La lectura es propia**, no `read_exact`: ver "Cancelación y tiempos de espera".

## Formato en el cable

### Tramas

Cada trama del emparejamiento es `longitud (u16, big-endian) | bytes`. La longitud no incluye sus 2 bytes. Máximo **512** (`MAX_FRAME_LEN`); 0 no se acepta. El lector lee los 2 bytes de la longitud, la comprueba **antes de reservar** el búfer y después lee exactamente esa cantidad. TCP no conserva los límites de los mensajes y el lector no depende de ellos: junta lecturas parciales hasta completar la trama.

### Intercambio

```
Celular (iniciador)                               PC (respondedor)
  trama 1:  e, es                (48 bytes)  ->   lee; sin respuesta si falla
            <-  trama 2:  e, ee   (48 bytes)
  trama 3:  s, se + PairingRequest (≤196)    ->   decodifica; PendingPairing
                                                  (el usuario decide, ≤60 s)
            <-  trama 4: PairingReply, cifrada con el canal Noise (19 bytes)
```

- La trama 1 trae `e` (32) y la etiqueta de 16 bytes de la carga vacía, ya cifrada con llaves que dependen del código y de la llave de la PC. Con un código incorrecto, la PC no la descifra y **no escribe nada**; cierra. (Verificado con `snow` 0.10.0: `read_message` da `Decrypt`.)
- La trama 2 la descifra el celular con una llave que depende de `es` y `ee`; si la PC no tiene la llave privada que corresponde a la llave pública del QR, falla aquí y el celular **no manda la trama 3** (ni su llave ni su nombre).
- La trama 3 lleva la llave pública del celular (`s`, 32 + 16) y su carga (`PairingRequest` + 16). La PC obtiene la llave con `get_remote_static()`.
- La trama 4 va ya en modo de transporte (`into_transport_mode`), con el contador 0 del sentido PC→celular.

### Mensajes nuevos (versión 1)

Se codifican con el mismo formato de la #94, `versión (u8) | tipo (u8) | cuerpo`, y con la decodificación estricta que ya existe (tamaño, versión, tipo, cuerpo, bytes sobrantes, forma canónica).

| Tipo | Mensaje | Cuerpo (postcard) | Bytes |
| --- | --- | --- | --- |
| 4 | `PairingRequest` | `name: String` (largo en varint + UTF-8) | 4 a 132 |
| 5 | `PairingReply` | `AckDecision`: `Accepted` = `0x00`, `Rejected` = `0x01` | 3 |

Ejemplos exactos: `PairingReply::Accepted` = `01 05 00`; `Rejected` = `01 05 01`. Cualquier otro valor del enumerado es `Malformed`.

**`DeviceName`** (tipo nuevo, `protocol::DeviceName`): `new(&str)` recorta con `str::trim` y falla con `InvalidField::DeviceName` si queda vacío, pasa de 128 bytes, contiene un carácter de control (`char::is_control`) o alguno de estos de dirección o separación de texto: U+061C, U+200E, U+200F, U+2028, U+2029, U+202A a U+202E y U+2066 a U+2069. Al decodificar, un nombre que no está ya recortado también es inválido (una sola codificación por mensaje). No se normaliza a NFC: es solo para mostrar.

### Tamaños

| Trama | Cálculo | Bytes |
| --- | --- | --- |
| 1 | `e` 32 + etiqueta 16 | 48 |
| 2 | `e` 32 + etiqueta 16 | 48 |
| 3 (máx.) | `s` 48 + (versión 1 + tipo 1 + varint 2 + nombre 128 + etiqueta 16) = 148 | 196 |
| 4 | versión 1 + tipo 1 + decisión 1 + etiqueta 16 | 19 |

Todas caben en los 512 bytes con holgura. Una prueba arma el peor caso (nombre de 128 bytes) y comprueba que la trama 3 mide 196.

## Constantes

| Constante | Valor |
| --- | --- |
| `PAIRING_PATTERN` | `Noise_XKpsk0_25519_ChaChaPoly_BLAKE2s` |
| `PAIRING_PROLOGUE` | `boveda/emparejamiento/v1` |
| `PAIRING_CODE_TTL` | 120 s |
| `MAX_FAILED_ATTEMPTS` | 3 |
| `FIRST_FRAME_LEN` | 48 |
| `MAX_FRAME_LEN` | 512 |
| `CANCEL_POLL` | 250 ms |
| `PairingTimeouts` por defecto | 10 s por paso · 20 s hasta la trama 3 · 60 s para confirmar · 65 s de espera del celular |

## Módulos

```
core/src/protocol/
├── mod.rs        # reexporta
├── error.rs      # ProtocolError, InvalidField (+ DeviceName)        [existe, se amplía]
├── message.rs    # + PairingRequest (4), PairingReply (5), DeviceName [existe, se amplía]
├── pairing.rs    # PairingSession, Initiator, Responder, estados y PairingError   (+ pairing/tests.rs)
└── transport.rs  # TimedStream, CancelToken, tramas y conductor      (+ transport/tests.rs)
core/tests/protocol.rs   # pruebas de la API pública, con TcpStream por 127.0.0.1
```

## Máquinas de estado (`protocol::pairing`, sin sockets)

### Código en la PC: `PairingSession`

```rust
PairingSession::new(identity: KeyPair, code: PairingCode, now: Instant) -> PairingSession
fn accept_first(&mut self, now: Instant, frame: &[u8]) -> Result<AwaitingThird, Refusal>
fn note_malformed(&mut self, now: Instant) -> Refusal   // la trama 1 no mide 48 bytes
fn cancel(&mut self)
AwaitingThird::second_message(&self) -> &[u8]            // la trama 2, sin prefijo de largo
```

La app crea la sesión en el mismo momento en que genera el código y muestra el QR: los 2 minutos cuentan desde `new`.

Estados del código y sus transiciones (todas bajo `&mut self`, es decir, atómicas):

```
                       ahora ≥ creado + 120 s            cancel()
        ┌──────────────────────────────► Expired      ┌──────────► Cancelled
        │                                              │
 Active { failed: 0..2 } ──trama 1 autentica──► Consumed   (final)
        │
        └─trama 1 no autentica, o no mide 48 bytes─► failed + 1 ──(3)──► Exhausted
```

- La caducidad se evalúa **primero**, en cada llamada, con `now - creado ≥ 120 s` (`Instant` monótono; las pruebas pasan `now` y no esperan).
- Solo `Active` intenta descifrar la trama 1 con el código. En `Expired`, `Consumed`, `Exhausted` y `Cancelled` **no se usa el código**: se devuelve la negativa sin tocar el estado.
- `Consumed` es final: el código ya no sirve aunque el usuario rechace, se cancele o caduque la confirmación. Para otro intento hay que crear otra sesión con otro código.
- Solo suman intento fallido (`failed + 1`) una trama 1 **completa** que no autentica, o un encabezado de longitud distinta de 48 (se cuenta sin leer el cuerpo). Una conexión sin trama completa (se corta, vence el tiempo o se cancela) **no cuenta**.
- Dos conexiones simultáneas con el código correcto: el conductor toma el `Mutex<PairingSession>` solo durante `accept_first` (un descifrado, sin entrada ni salida). La primera lo pasa a `Consumed`; la segunda encuentra `Consumed` y se rechaza. No hay ventana entre "autenticar" y "consumir".
- `Refusal` (`WrongCode`, `Malformed`, `Expired`, `AlreadyUsed`, `TooManyAttempts`, `Cancelled`) es **solo para la app local** (por ejemplo, ofrecer "Generar otro código"). No se envía ni cambia lo observable por la conexión: en todos los casos, la PC cierra sin escribir una sola trama.

### Celular: `Initiator` (por tipos)

```rust
Initiator::new(identity: &KeyPair, offer: &PairingOffer, name: DeviceName) -> Result<Initiator>
fn first_message(self) -> Result<(AwaitingSecond, Frame)>
AwaitingSecond::read_second(self, frame: &[u8]) -> Result<(AwaitingReply, Frame)>   // devuelve la trama 3
AwaitingReply::read_reply(self, frame: &[u8]) -> Result<PairedPc>
```

- El intercambio Noise termina en `AwaitingReply`, **no** en un resultado de éxito. `PairedPc` solo se construye en `read_reply`, con una decisión `Accepted` autenticada. `Rejected` da `PairingError::Rejected`.
- El celular obtiene la llave de la PC del QR (`offer.public_key()`); la prueba comprueba además que `snow` la confirma (`get_remote_static`).
- Una respuesta alterada, cifrada con otra llave, sin cifrar, de otra sesión o con otra decisión que `Accepted` no completa el emparejamiento.

### PC: del apretón de manos a la decisión

```rust
AwaitingThird::read_third(self, now: Instant, frame: &[u8]) -> Result<PendingPairing, PairingError>
PendingPairing: peer_public_key(), fingerprint(), peer_name(), deadline()   // solo lectura
PendingPairing::accept(self, now: Instant) -> Result<Decision, PairingError>
enum Decision { Accepted { device: PairedDevice, frame: Frame }, TimedOut { frame: Frame } }
PendingPairing::reject(self) -> Result<Frame, PairingError>   // la trama 4 con `Rejected`
```

- `PendingPairing` **no es** un emparejamiento: no tiene llaves ni nombre que se puedan cambiar, y la única forma de llegar a `PairedDevice` es `accept`. No existe conversión directa desde el apretón de manos.
- `accept` antes de `deadline` (creación + 60 s) cifra `Accepted`; pasado el plazo, devuelve `Decision::TimedOut` con la trama de `Rejected`: el conductor la escribe y da `ConfirmationTimeout`. El plazo de `read_third` es `CONFIRMATION_TIMEOUT` y el conductor lo ajusta a `PairingTimeouts::confirmation`.
- `PairedDevice { public_key, name }` es lo que la app guarda **solo si** el conductor pudo escribir la trama 4 completa (ver abajo). Su `Debug` muestra solo la huella.
- La app muestra el nombre y la huella (`Fingerprint`, que no autentica) y no puede editar la identidad durante el diálogo.

## Autorización, entrega y persistencia

Son tres cosas distintas, y el núcleo promete solo las dos primeras, y solo en su propio extremo:

1. **Autorización criptográfica.** Terminar el apretón de manos solo prueba que cada extremo tiene la llave privada de la que dice ser y que el celular conocía el código. **No autoriza nada.** La autorización es la decisión del usuario en la PC: `PendingPairing::accept` produce la trama `Accepted`, cifrada y autenticada con el canal Noise.
2. **Entrega de la respuesta.** `PendingConnection::confirm` escribe la trama 4 completa en su conexión **antes** de devolver `PairedDevice`. Eso garantiza que la PC la cifró y la escribió sin error en su lado; **no** que el celular la haya recibido (la escritura exitosa no es un acuse de TCP ni del celular). Soltar un `PendingConnection` sin decidir cierra la conexión.
3. **Persistencia de la confianza.** Es de cada app (#99 en la PC, #93 y #103 en el celular), después de recibir su resultado. El núcleo **no promete persistencia atómica entre dispositivos** ni que cualquiera de las apps consiga guardar la llave (disco lleno, bóveda bloqueada, error de DPAPI).

Casos que pueden quedar desparejados y cómo se resuelven (se documenta en la especificación):

| Caso | Quién queda con qué | Resolución |
| --- | --- | --- |
| Se corta la red tras escribir `Accepted` y el celular no lo lee | La PC guardó al celular; el celular no guardó a la PC y da `ConfirmationTimeout` o `Closed` | Inofensivo (el celular nunca obtuvo permiso en su lado; la PC solo tiene una llave autorizada sin uso). El usuario lo quita en "Dispositivos emparejados" (#107) o vuelve a emparejar; la PC debe tratar una llave ya existente como actualización (#99) |
| La app de la PC no logra guardar tras `confirm` | El celular recibió `Accepted` y guardó a la PC; la PC no tiene al celular | En el envío (#96) la PC rechaza esa llave; el celular avisa que falló y el usuario quita la PC y empareja de nuevo |
| La app del celular no logra guardar tras `read_reply` | La PC tiene al celular; el celular no tiene a la PC | Igual que el primero |

## Datos que entrega al celular

`protocol::PairedPc` es lo que devuelve `read_reply` (no se confunde con `vault::PairedPc`, que es un registro guardado). Se construye solo con datos reales del emparejamiento:

```rust
pub struct PairedPc { public_key: PublicKey, address: SocketAddr }  // del QR, ya autenticados por Noise
fn public_key(&self) -> PublicKey
fn address(&self) -> SocketAddr
fn provisional_name(&self) -> String        // "PC " + huella, por ejemplo "PC A1B2-C3D4-E5F6-0718" (22 bytes)
fn into_new_paired_pc(self, name: Option<String>) -> vault::NewPairedPc
```

- `into_new_paired_pc` llena `public_key` con la llave, `last_ip` con `Some(address.ip().to_canonical())` y `name` con el que dé la app o, si es `None`, con `provisional_name()`. El nombre provisional cumple el rango de `vault::PairedPc` (1 a 128 bytes, recortado) y no inventa información: sale de la llave.
- La PC no envía su nombre en esta versión. La #103 decide cómo lo obtiene el celular (lo escribe el usuario, un campo en el QR u otro) y lo aplica con `PairedPcUpdate`. Un nombre nuevo en el protocolo sería un cambio posterior del formato de los mensajes.

## Conductor síncrono (`protocol::transport` y funciones de `pairing`)

```rust
pub trait TimedStream: Read + Write {
    fn set_read_timeout(&mut self, d: Option<Duration>) -> io::Result<()>;
    fn set_write_timeout(&mut self, d: Option<Duration>) -> io::Result<()>;
}
impl TimedStream for std::net::TcpStream { /* ... */ }

pub struct CancelToken(Arc<AtomicBool>);   // Clone; cancel(); is_cancelled()

// PC
pub fn serve_pairing<S: TimedStream>(stream: S, session: &Mutex<PairingSession>,
    cancel: &CancelToken, t: &PairingTimeouts) -> Result<PendingConnection<S>, PairingError>
PendingConnection::{ pending() -> &PendingPairing, confirm(self) -> Result<PairedDevice>, reject(self) -> Result<()> }

// Celular
pub fn pair_with_pc<S: TimedStream>(stream: S, identity: &KeyPair, offer: &PairingOffer,
    name: DeviceName, cancel: &CancelToken, t: &PairingTimeouts) -> Result<PairedPc, PairingError>
```

Las dos funciones reciben la conexión **por valor**: se cierra al soltarse, en cualquier camino de salida.

### Cancelación y tiempos de espera

- **Ninguna lectura ni escritura bloquea sin límite.** Antes de cada lectura, el conductor fija `set_read_timeout(min(CANCEL_POLL, lo que falte del plazo))` y lee. Si vuelve `WouldBlock` o `TimedOut` (Unix y Windows usan uno u otro), revisa el `CancelToken` y los plazos y vuelve a intentar; con `Interrupted` reintenta sin más. Las escrituras hacen lo mismo con `set_write_timeout`. Así, una lectura bloqueada vuelve en **como máximo `CANCEL_POLL` (250 ms)** y la cancelación surte efecto dentro de ese plazo, no solo "antes de llamar a `read()`".
- **No se usa `read_exact`.** Ante un error de tiempo, `read_exact` descarta los bytes ya leídos; el conductor lleva su propia posición dentro de la trama, así que un tiempo agotado a la mitad no pierde ni mezcla bytes (y de todos modos el error cierra la conexión).
- **El encabezado se lee sin validar el largo.** `read_frame_header` devuelve el largo tal cual llegó, para que la PC cuente como intento cualquier primera trama de largo distinto de 48; el largo (1 a 512) lo valida `read_frame_body` **antes** de reservar memoria.
- **Contrato de `TimedStream`:** `read` y `write` deben volver con `WouldBlock` o `TimedOut` una vez cumplido el tiempo configurado. `TcpStream` lo cumple. Una implementación que lo ignore pierde la garantía, y esa condición se documenta en el trait.
- **Cancelar** un `CancelToken` solo afecta a las operaciones que lo usan; cancelar el **código** es `PairingSession::cancel`, que es otra cosa y solo la app decide.
- **Plazos:** 10 s por paso (lectura o escritura de una trama) y 20 s en total desde que la PC acepta la conexión hasta tener la trama 3. Los 65 s del celular para la respuesta empiezan al terminar de escribir la trama 3.

### Qué pasa en cada fallo

Toda salida con error **consume** el estado de la máquina (se pasa por valor), suelta el estado de `snow` y cierra la conexión; no queda nada reutilizable ni conexiones abiertas. Ninguna llave se entrega.

| Situación | Lado | Resultado | ¿Cuenta intento? | Estado del código |
| --- | --- | --- | --- | --- |
| Conexión cerrada antes de los 2 bytes de longitud, o con la trama 1 incompleta | PC | `Closed` | No | Sin cambios (`Active`) |
| Sin trama 1 completa a los 10 s, o a los 20 s en total | PC | `Timeout` | No | Sin cambios |
| Cancelación durante la lectura | PC | `Cancelled` (≤ 250 ms) | No | Sin cambios |
| Longitud de la trama 1 distinta de 48 (incluye 0 y > 512) | PC | `Refused(Malformed)`; cierra sin leer el cuerpo ni responder | Sí, si el código está `Active` | `failed + 1` |
| Trama 1 completa que no autentica | PC | `Refused(WrongCode)`; cierra sin responder | Sí | `failed + 1`; al tercero, `Exhausted` |
| Código caducado, consumido, agotado o cancelado | PC | `Refused(...)`; **lee la trama 1** y cierra sin responder (mismo comportamiento externo) | No | Sin cambios |
| Falla al escribir la trama 2, o la trama 3 llega incompleta, vence, se cancela o no descifra | PC | `Closed`, `Timeout`, `Cancelled`, `Io` o `Handshake` | No | `Consumed` (ya se gastó) |
| La app no decide a tiempo | PC | `accept` da `ConfirmationTimeout` y trama `Rejected`; si la app suelta el `PendingConnection`, se cierra | No | `Consumed` |
| El celular se desconecta durante el diálogo | PC | `confirm` falla al escribir (`Closed` o `Io`); no hay `PairedDevice` | No | `Consumed` |
| La trama 2 no se autentica (falsificada o alterada) | Celular | `PeerNotAuthentic`; **no manda la trama 3** | — | — |
| La PC tiene otra llave privada que la del QR | Ambos | No descifra la trama 1 (el `es` usa la llave del QR): la PC rechaza (`WrongCode`) y cierra sin responder; el celular ve `Closed` y no manda nada más | Sí | `failed + 1` |
| La PC cierra tras la trama 1 (código rechazado) o hay un corte de red | Celular | `Closed` (**mismo error en ambos casos**) | — | — |
| Vence un paso o los 65 s | Celular | `Timeout` o `ConfirmationTimeout` | — | — |
| Respuesta alterada, sin cifrar o de otro tipo | Celular | `Handshake` o `Protocol(...)`; no hay `PairedPc` | — | — |
| `Rejected` | Celular | `Rejected` | — | — |
| Cancelación | Celular | `Cancelled` (≤ 250 ms) | — | — |

- **El `PendingConnection` no tiene temporizador propio:** el núcleo no lanza hilos. `deadline()` le dice a la app cuándo debe llamar a `reject`; si no lo hace, la conexión se cierra al soltarse el valor y el celular se rinde a los 65 s. El diálogo con su temporizador y esa llamada son criterio de aceptación de la #101.
- **Un cliente que no se identifica** (conexión abierta sin mandar nada) solo ocupa el hilo que la app dedicó a esa conexión hasta los 10 s (o 20 s con goteo lento); cuántas conexiones sin autenticar se aceptan a la vez lo decide la #100.

### Errores

`PairingError` (`#[non_exhaustive]`, sin datos ni texto de `snow`): `Refused(Refusal)` (solo la PC, local), `PeerNotAuthentic` (celular), `Rejected`, `ConfirmationTimeout`, `Timeout`, `Cancelled`, `Closed`, `Io(ErrorKind)`, `FrameTooLarge`, `Handshake` y `Protocol(ProtocolError)`. Ninguno lleva bytes de mensaje ni secretos.

## Borrado de memoria

Excepción nueva, que se agrega a la lista de `ARCHITECTURE.md` (junto a las de #94):

- **`snow` 0.10.0 no depende de `zeroize` y no borra ningún secreto.** Lo que queda en su memoria, sin borrar, al soltarse el estado (revisado en el código fuente: `handshakestate.rs`, `resolvers/default.rs`, `symmetricstate.rs`, `cipherstate.rs`):
  - la llave privada estática (`Dh25519.privkey`, 32 bytes): es una copia de la del celular en el celular y de la de la PC en la PC;
  - la llave privada efímera (`Dh25519.privkey`);
  - el código de un solo uso (copia de 32 bytes en `psks`), además de los valores que se derivan al mezclarlo en el estado simétrico;
  - el estado simétrico: la llave de encadenamiento (`ck`) y el resumen (`h`), de hasta 64 bytes cada uno;
  - las llaves de cifrado de los dos sentidos (`CipherState` y, tras `into_transport_mode`, el `TransportState`), de 32 bytes cada una.
- Además, `curve25519-dalek` 4.1.3 (sin la feature `zeroize`) y `blake2` 0.10 (sin `zeroize`) dejan escalares y estados en pila y registros.
- **Por qué no se puede garantizar el borrado:** los campos viven dentro de tipos de `snow` y de sus dependencias que no ofrecen borrarlos ni tienen un `Drop` que lo haga. Solo se cerraría con una primitiva o un resolvedor criptográfico propios, que `core/CLAUDE.md` prohíbe.
- **Mitigación parcial, no es borrado seguro:** el núcleo pasa referencias (`&[u8]` desde el `SecretBox`) para no hacer una copia más por su cuenta, y suelta los estados de `snow` en cuanto termina el emparejamiento. Eso reduce las copias y su vida, pero no sobrescribe la memoria interna de `snow`. El secreto más delicado que queda es la llave privada de larga duración del dispositivo.
- El código es un secreto de 2 minutos y un uso, que además se muestra en el QR.

## Dependencias

Una sola, con visto bueno del usuario el 2026-11-20:

```toml
snow = { version = "0.10", default-features = false, features = [
    "use-curve25519", "use-chacha20poly1305", "use-blake2", "use-getrandom",
] }
```

Verificado con un proyecto aparte contra `snow` 0.10.0:

- **Sin la feature `std`** (en 0.10.0 activa también `ring`, `sha2` y `blake2/std`), compila y completa el intercambio. No entran `ring`, `sha2` ni AES-GCM.
- Árbol resultante: `snow`, `curve25519-dalek` 4.1.3, `chacha20poly1305` 0.10.1, `blake2` 0.10.6, `getrandom` 0.3.4, `subtle`, `zeroize`, `generic-array` y `cpufeatures`. Hay versiones duplicadas con las del núcleo (`curve25519-dalek` 5.0.0, `chacha20poly1305` 0.11, `blake2` 0.11, `getrandom` 0.4): `deny.toml` solo avisa (`multiple-versions = "warn"`). Se confirma con `cargo deny check` al agregarla.
- Sin otras dependencias de red ni criptográficas. `core/CLAUDE.md` ya incluye `snow` en su lista; se agregan sus features.

## Pruebas

Unitarias (`pairing/tests.rs`, `transport/tests.rs`, `message/tests.rs`), sin red:

- Máquinas de estado: el intercambio completo en memoria; los pasos fuera de orden no compilan (por tipos).
- `PairingSession`: correcto; incorrecto; caducado (con `now`, justo antes y justo en los 120 s); reutilizado; **el tercer intento fallido invalida** y un cuarto con el código correcto también falla; una trama 1 de 47 o 49 bytes cuenta como intento; los fallos en estado final no usan el código; `cancel`.
- Por la conexión, **igual comportamiento** en incorrecto, caducado, reutilizado y agotado: ni una trama de salida (el flujo de escritura queda vacío).
- `DeviceName`: recorte, 1 y 128 bytes, 129, vacío, control, cada carácter de dirección; y la trama 3 máxima mide 196.
- `PairingReply`: bytes exactos, un tercer valor, bytes sobrantes, forma canónica.
- Tramas: largo 0, largo mayor que 512 (**se rechaza antes de reservar**: un lector que falla si se le piden más de 2 bytes), lecturas de a un byte, cortes a la mitad.
- `PendingPairing`: `accept` después de los 60 s da `ConfirmationTimeout`; no hay forma de llegar a `PairedDevice` sin `accept`.
- `PairedPc`: el nombre provisional mide de 1 a 128 bytes; `into_new_paired_pc` con y sin nombre.

De la API pública (`core/tests/protocol.rs`, `TcpStream` en `127.0.0.1`):

- Código correcto: ambos extremos conocen la llave del otro; la PC conoce el nombre y la huella.
- Código incorrecto, caducado, reutilizado y agotado: el celular recibe el mismo `Closed`; la PC conoce el motivo.
- La PC rechaza (`reject`): el celular da `Rejected` y no hay `PairedPc`.
- La PC no responde en 60 s, y soltar el `PendingConnection`: sin `PairedDevice`; el celular da `ConfirmationTimeout` o `Closed`.
- **Una PC con otra llave privada** que la del QR: el celular da `PeerNotAuthentic` y **no manda la trama 3** (el servidor de prueba comprueba que no llegó nada más).
- **Una aceptación alterada** (un bit cambiado), **sin cifrar** o de otra sesión: no completa.
- **Dos conexiones simultáneas** con el código correcto (dos hilos): exactamente una llega a `PendingPairing`.
- **Cancelación efectiva:** con un cliente que conecta y no manda nada, la cancelación desde otro hilo devuelve `Cancelled` en menos de 1 s, y lo mismo en la espera de la respuesta del celular.
- **Tiempos:** una conexión que se cierra a medio mensaje da `Closed` y **no gasta intentos** (después, el código correcto aún funciona); goteo de un byte cada 100 ms hasta pasar los 20 s da `Timeout`; una conexión muda da `Timeout`.
- Un dispositivo sin código no obtiene ningún byte de la PC.

## Documentos a actualizar al implementar

`docs/especificacion.md` (sección "Emparejamiento y envío": decisiones, formato, estados, la separación autorización/entrega/persistencia, la limitación de la aceptación perdida y la excepción de borrado), `ARCHITECTURE.md` (módulos `pairing.rs` y `transport.rs`, tabla de decisiones), `docs/roadmap.md` (marcar la #95), `core/CLAUDE.md` (features de `snow`) y el comentario de la dependencia en `core/Cargo.toml`. El README de `core/tests/fixtures/` no cambia: no hay formatos de archivo nuevos.

## Criterios de aceptación (de la issue)

- [ ] El emparejamiento con el código correcto funciona y ambos guardan la llave del otro (la app guarda; el núcleo entrega `PairedPc` y `PairedDevice` solo tras la confirmación).
- [ ] Un código incorrecto, caducado o ya usado falla sin revelar el motivo exacto a quien intenta.
- [ ] Un dispositivo no emparejado no obtiene nada de la PC sin el código.
- [ ] `cargo test --workspace`, `clippy`, `fmt` y `deny` pasan.
