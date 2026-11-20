# Diseño: envío con Noise IK (issue #96)

Fecha: 2026-11-20 · Etapa 3 (épica #92) · Rama `feat/96-envio-noise` (desde `feat/95-emparejamiento`, porque reutiliza su transporte; el PR contra `dev` se abre cuando la #95 esté fusionada, sin repetir sus cambios)

## Objetivo

Con las llaves ya conocidas por el emparejamiento (#95), `vault-core` permite que el celular envíe una contraseña a la PC por un canal Noise (`snow`), con un plazo de 30 s y una confirmación de vuelta, y que la PC rechace a cualquier dispositivo no emparejado **durante el apretón de manos, sin escribirle nada y antes de leer contenido**. Se prueba con los dos extremos en el mismo proceso, por `TcpStream` en `127.0.0.1`.

Incluye máquinas de estado por tipos (sin sockets) y conductores síncronos sobre `Read + Write` con plazos y cancelación, igual que el emparejamiento. Sin tokio ni crates de red.

## Fuera de alcance

- Descubrimiento, escucha en la red real y límite de conexiones sin autenticar: #100 y #104.
- El portapapeles de Windows, su exclusión del historial y el borrado a los 30 s: #98. Los 30 s del portapapeles son **independientes** de los de este diseño.
- La huella digital del celular y la lista de dispositivos de la app: #104 y #107.
- Guardar nada: el núcleo no persiste la lista de celulares emparejados; la app se la pasa en cada conexión.
- Mensajes nuevos o cambio de `PROTOCOL_VERSION` (sigue en 1): `SendMessage` y `Ack` ya existen desde la #94.
- Mecanismos criptográficos adicionales (identificadores de mensaje, cachés de repeticiones, marcas de hora, PSK).

## Decisiones

Aprobadas por el usuario el 2026-11-20 (el mensaje de aprobación dice "#95" por error de tipeo; es la #96):

1. **Patrón** `Noise_IK_25519_ChaChaPoly_BLAKE2s`, prólogo `boveda/envio/v1`, sin PSK. El celular (iniciador) conoce la llave de la PC; la PC aprende la del celular en la trama 1.
2. **Protección contra repeticiones.** La trama 1 de IK **sí puede reenviarse**: la PC contestará con una trama 2 (con una llave efímera nueva), pero quien la reenvía no tiene la llave privada del celular ni su efímera, así que no deriva las llaves de transporte y no puede completar un envío válido. El `SendMessage` viaja **solo después** de completar el apretón de manos; las tramas 1 y 2 no llevan datos de aplicación. Una conexión acepta **un solo envío** y se cierra. Sin identificadores de mensaje ni caché.
3. **Plazo de 30 s.** La PC lo cuenta con `Instant` desde que acepta la conexión. Incluye el apretón de manos y la recepción **completa y autenticada** del `SendMessage`. Si vence antes, se rechaza sin entregar el secreto a la app (`Expired`). Esto **solo limita la recepción**; no demuestra que la contraseña se haya generado hace menos de 30 s (no hay marcas de hora: los relojes no coinciden). El campo `ttl_secs` del mensaje sigue siendo exactamente 30 y solo lo declara.
4. **Autorización.** La app pasa un `Fn(&PublicKey) -> bool`. El núcleo lo consulta **después** de autenticar la trama 1, con la llave estática que `snow` recupera (`get_remote_static`), nunca con una identidad que el cliente declare. Si no está emparejada: se cierra sin escribir la trama 2 y sin procesar contenido. La revocación aplica a conexiones nuevas; una conexión ya autorizada puede terminar su envío.
5. **Confirmación.** `Copied` o `Failed`. El núcleo entrega el `SendMessage` a la app y solo escribe `Copied` cuando la app lo pide tras escribir en el portapapeles (`IncomingSend::copied`); `failed` escribe `Failed`. El portapapeles no se implementa aquí.
6. **Tramas.** `transport.rs` parametriza el máximo: 512 bytes para el emparejamiento (sin cambios) y **2048** para el envío (`MAX_SEND_FRAME_LEN`).

Llamadas mías (el usuario puede cambiarlas):

- **Nuevo archivo `protocol/send.rs`** con las máquinas de estado y los conductores del envío, igual que `pairing.rs` para el emparejamiento; `transport.rs` conserva solo tramas, plazos y cancelación. (La issue nombra `transport.rs` para el intercambio; se cumple en lo que ese archivo ya hace: el transporte, ahora con máximo configurable.)
- **`SendError` propio**, con `From<PairingError>` para los errores de tramas, porque el envío necesita `NotPaired` y `Expired`, que no tienen sentido en el emparejamiento.
- **El celular devuelve `Result<AckOutcome, SendError>`**: `Failed` de la PC es un `Ok(Failed)`, no un error del protocolo; la app decide qué mostrar.
- **Una trama de más en la misma conexión** no se lee: tras escribir el `Ack`, la PC cierra. No se sondea antes de escribirlo.
- **Plazos por defecto** (`SendTimeouts`): 10 s por trama, 30 s en total (`SEND_TTL`) y 10 s del celular para esperar el `Ack`.

## Formato en el cable

Cada trama es `longitud (u16 big-endian) | bytes`, de 1 a `max` bytes (la comprobación es **antes de reservar**).

```
Celular (iniciador)                               PC (respondedor)
  trama 1:  e, es, s, ss          (96 bytes)  ->   descifra; lee la llave estática;
                                                   ¿emparejada? no → cierra sin escribir
            <-  trama 2:  e, ee, se            (48 bytes)
  trama 3:  SendMessage cifrado (transporte)  ->   decodifica; IncomingSend
                                                   (la app escribe en el portapapeles)
            <-  trama 4:  Ack cifrado                 (19 bytes)
                                                   cierra
```

### Tamaños

| Trama | Cálculo | Bytes |
| --- | --- | --- |
| 1 | `e` 32 + `s` cifrada 48 + etiqueta de la carga vacía 16 | 96 |
| 2 | `e` 32 + etiqueta de la carga vacía 16 | 48 |
| 3 (máx.) | versión 1 + tipo 1 + `ttl` 1 + largo varint 2 + contraseña 1024 + etiqueta 16 | 1045 |
| 4 | versión 1 + tipo 1 + resultado 1 + etiqueta 16 | 19 |

Todas caben en 2048 con holgura. Una prueba arma el peor caso (contraseña de 1024 bytes) y comprueba que la trama 3 mide 1045.

- La **trama 1** de largo distinto de 96 se rechaza leyendo solo el encabezado, sin reservar ni leer el cuerpo.
- Las tramas 1 y 2 llevan carga vacía: una carga no vacía es `Handshake`.
- Los límites de `snow` (65 535 bytes por mensaje) no se acercan.

## Máquinas de estado (`protocol::send`, sin sockets)

```rust
// PC
Responder::new(identity: &KeyPair) -> Responder
Responder::read_first(self, now: Instant, frame: &[u8], is_paired: impl Fn(&PublicKey) -> bool)
    -> Result<(AwaitingSend, Frame), SendError>      // trama 2 solo si está emparejada
AwaitingSend::read_send(self, now: Instant, frame: &[u8]) -> Result<ReceivedSend, SendError>
ReceivedSend::{ peer(), message() -> &SendMessage, ack(self, outcome) -> Result<Frame, SendError> }

// Celular
Sender::new(identity: &KeyPair, pc_key: &PublicKey) -> Result<Sender, SendError>
Sender::first_message(self) -> Result<(AwaitingSecond, Frame), SendError>
AwaitingSecond::read_second(self, frame: &[u8], message: &SendMessage)
    -> Result<(AwaitingAck, Frame), SendError>       // la trama 3; PeerNotAuthentic si la 2 no se autentica
AwaitingAck::read_ack(self, frame: &[u8]) -> Result<AckOutcome, SendError>
```

- `read_first` guarda `deadline = now + SEND_TTL`. `read_send` revisa `now >= deadline` **después** de descifrar y decodificar y, si venció, suelta el mensaje (se borra) y da `Expired`: el secreto no llega a la app.
- Consultar `is_paired` es lo primero que hace `read_first` tras autenticar la trama 1; la trama 2 no se escribe antes. Una llave no emparejada es `NotPaired` (**solo para la app local**: por la conexión se ve igual que un fallo).
- `PeerNotAuthentic` en el celular: la trama 2 no se autentica (otra llave privada que la conocida o trama falsificada) y **no se manda la trama 3**: la contraseña no sale.
- Los pasos fuera de orden no compilan; todo error consume el estado.
- Con una trama 3 de otra sesión (mismas llaves, otra efímera) o con un bit cambiado, el descifrado falla: `Handshake`, sin mensaje.

## Conductores síncronos

```rust
pub fn receive_send<S: TimedStream>(
    stream: S, identity: &KeyPair, is_paired: impl Fn(&PublicKey) -> bool,
    cancel: &CancelToken, timeouts: &SendTimeouts,
) -> Result<IncomingSend<S>, SendError>

IncomingSend::{ peer() -> PublicKey, message() -> &SendMessage, copied(self) -> Result<(), SendError>, failed(self) -> Result<(), SendError> }

pub fn send_to_pc<S: TimedStream>(
    stream: S, identity: &KeyPair, pc_key: &PublicKey, message: &SendMessage,
    cancel: &CancelToken, timeouts: &SendTimeouts,
) -> Result<AckOutcome, SendError>
```

- Las dos reciben la conexión **por valor**: se cierra al soltarse. Soltar un `IncomingSend` sin `copied` ni `failed` cierra sin `Ack`, y el celular ve `Closed`.
- `receive_send` empieza a contar los 30 s al **entrar** (la app la llama justo al aceptar). Si un paso agota su plazo y el total ya venció, el error es `Expired`; si no, `Timeout`.
- La app llama a `copied`/`failed` y descarta el `IncomingSend`: el `SendMessage` se borra al soltarse (`SecretString`).
- `IncomingSend` no tiene temporizador propio: el núcleo no lanza hilos.
- Los conductores usan `read_frame_header`, `read_frame_body` y `write_frame` de `transport.rs` con `MAX_SEND_FRAME_LEN`.

### Qué pasa en cada fallo

| Situación | Lado | Resultado |
| --- | --- | --- |
| Llave estática no emparejada | PC | `NotPaired`; **no escribe nada** y cierra |
| Trama 1 que no descifra, de otro largo, o con carga no vacía | PC | `Handshake`; cierra sin escribir |
| Trama 1 reenviada de una captura | PC | contesta la trama 2; sin la llave privada nadie manda una trama 3 válida: `Handshake`, `Timeout` o `Expired`; no hay mensaje |
| Trama 3 de otra sesión o alterada | PC | `Handshake`; no hay mensaje |
| Trama 3 que llega después de los 30 s | PC | `Expired`; no hay mensaje |
| Trama más grande que 2048 o de largo 0 | PC y celular | `FrameTooLarge` / `Protocol(Malformed)` antes de reservar |
| Conexión que se corta a medias | Ambos | `Closed` |
| Un segundo envío en la misma conexión | PC | no se lee; tras el `Ack` se cierra. El celular ve `Closed` si lo intenta |
| La PC no tiene la llave del QR (otra llave privada) o la trama 2 es falsa | Celular | `Closed` / `PeerNotAuthentic`; **no manda la contraseña** |
| PC no emparejada con ese celular | Celular | `Closed` (igual que un corte de red) |
| La app responde `Failed` | Celular | `Ok(AckOutcome::Failed)` |

## Borrado de memoria

- `SendMessage` se borra al soltarse. Los búferes de trabajo que reciben la contraseña en claro (el descifrado de la trama 3 en la PC y la codificación en el celular) van en `Zeroizing`.
- `snow` 0.10.0 sigue sin borrar sus secretos: las llaves privadas, el estado simétrico y las llaves de transporte de IK quedan en su memoria al soltarse, igual que en el emparejamiento (excepción (5) de `ARCHITECTURE.md`, que se amplía a "#95 y #96"). **A verificar al implementar** leyendo `resolvers/default.rs` de `snow` 0.10.0: si el cifrado de transporte deja una copia de la contraseña en claro dentro de su estado, se agrega a esa excepción; no se promete más borrado del que se pueda mostrar.
- La contraseña nunca aparece en `Debug`, errores ni `Display`: `SendError` no lleva datos.

## Pruebas

Unitarias (`send/tests.rs`, `transport/tests.rs`), sin red, con `now` inyectado:

- Intercambio completo en memoria, tamaños 96 / 48 / 1045 (contraseña de 1024 bytes) / 19.
- `is_paired` falso: no hay trama 2; `is_paired` recibe la llave que `snow` recuperó (no la que diga el cliente).
- Caducidad: `read_send` con `now` justo antes y justo en el plazo.
- Trama 1 reenviada: la PC contesta, y una trama 3 de otra sesión da `Handshake`.
- Trama 3 con un bit cambiado, trama 3 sin cifrar, trama 3 vacía.
- Tramas: máximo 2048 configurable, 512 sin cambios en el emparejamiento, 2049 rechazada antes de reservar.
- `Ack`: `Copied` y `Failed` ida y vuelta; un `Ack` de otra sesión o alterado no se acepta.

De la API pública (`core/tests/protocol.rs`, `TcpStream` en `127.0.0.1`):

- Envío exitoso y confirmación `Copied`; también con una contraseña de 1024 bytes.
- No emparejado: la PC no escribe un solo byte y el celular ve `Closed`.
- Trama 1 reenviada de una captura y trama 3 de otra sesión: no se entrega nada.
- Caducidad real con plazo corto (`SendTimeouts` pequeño): `Expired`, sin mensaje.
- Trama de 2049 bytes y de largo 0.
- Datos cifrados alterados.
- Conexión interrumpida (celular y PC).
- `Ack::Failed`.
- Un segundo envío en la misma conexión no se procesa.
- Una PC con otra llave privada: el celular no manda la contraseña.
- Cancelación efectiva en ambos extremos.
- La contraseña no aparece en `Debug` ni en el `Display` de ningún error.

## Documentos a actualizar al implementar

`docs/especificacion.md` (nueva subsección "Envío con Noise" en "Emparejamiento y envío"), `ARCHITECTURE.md` (módulo `send.rs`, tabla de decisiones y la excepción (5)) y `docs/roadmap.md` (marcar la #96). `core/CLAUDE.md` no cambia: ya incluye `snow`. No hay formatos de archivo nuevos: las bóvedas de ejemplo no cambian.

## Criterios de aceptación (de la issue)

- [ ] Un celular emparejado envía y recibe confirmación.
- [ ] Un no emparejado es rechazado en el intercambio, sin que la PC lea contenido.
- [ ] Un mensaje con más de 30 s, o repetido, se rechaza (se entiende como en las decisiones 2 y 3: la recepción vence a los 30 s, y una captura no se puede reproducir).
- [ ] La contraseña en claro se borra de memoria tras usarse y nunca aparece en errores ni `Debug`.
- [ ] `cargo test --workspace`, `clippy`, `fmt` y `deny` pasan.
