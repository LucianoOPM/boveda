# Diseño: fachada del protocolo (issue #102)

Fecha: 2026-11-20 · Etapa 3 (épica #92) · Rama `feat/102-fachada-protocolo` (desde `dev`)

## Objetivo

`mobile/rust` expone a Dart lo que `vault-core` ya sabe hacer con el protocolo: la identidad del celular, emparejar con el QR de una PC, enviar una entrada a una PC y administrar la lista de PCs. Sigue siendo una fachada delgada: convierte tipos y llama al núcleo. Con esto, las pantallas (#103, #104, #107) quedan solo en Dart.

## Fuera de alcance

- Las pantallas (#103, #104, #105, #107) y descartar la identidad al importar (#106).
- **El descubrimiento por mDNS** y la política «última IP → mDNS → IP manual» (#104). Esta issue no agrega `mdns-sd` ni otras dependencias de red. Los permisos y requisitos de multidifusión de Android son de la #104.
- Guardar o actualizar la última IP tras un envío, y renombrar una PC: no están en el alcance de la issue.
- Cualquier lógica de protocolo o criptografía: vive en `core/`.

## Decisiones (aprobadas por el usuario el 2026-11-20)

1. mDNS queda fuera. `send_to_pc` recibe una dirección explícita (`ip:puerto`).
2. **El bloqueo tiene prioridad absoluta.** Ninguna operación de red mantiene tomado el `Mutex<Option<Vault>>`: se extrae lo necesario bajo el candado, se suelta y recién entonces se conecta. `lock()` bloquea la `Vault` (borra su llave), invalida la época vigente y cancela el `CancelToken` de la operación en curso; no espera a que termine la conexión.
3. **Sin resultados obsoletos.** Cada operación captura la época al empezar. Antes de persistir una PC emparejada vuelve a tomar el candado y comprueba que la bóveda siga desbloqueada y que la época no haya cambiado; si cambió, devuelve `Locked` y no guarda nada, aunque haya recibido un `Accepted` autenticado (es el caso (c) de «Autorización, entrega y persistencia» de la especificación). Un desbloqueo posterior no revive una operación anterior.
4. **Una sola operación de red a la vez**, compartida entre emparejar y enviar. Si hay una en curso, la segunda devuelve `Busy` al instante. Ninguna función lleva `#[frb(sync)]`. Cancelar libera el estado «ocupado» y una operación vieja nunca borra el de una más reciente.
5. **La contraseña no cruza a Dart al enviar.** `send_to_pc` recibe el id de la entrada; Rust revela el secreto y arma el `SendMessage`.
6. `android.permission.INTERNET` se agrega en esta issue.
7. `VaultError` sigue siendo un enum plano sin datos, con las variantes necesarias para distinguir `Locked`, `Busy`, cancelación y los fallos de emparejamiento y de envío.
8. Pruebas obligatorias (ver «Pruebas»).
9. **Texto del QR: función nueva en el núcleo.** `PairingOffer::decode_qr_text(&str)` decodifica el base64url sin relleno que escribe la PC; `pair` recibe el texto tal cual. Sin dependencias nuevas.
10. **PC repetida:** si la llave ya está en la lista, `pair` conserva su id, nombre y `paired_at`, actualiza `last_ip` con la dirección del QR y devuelve esa PC como éxito (solo con `update_paired_pc`).

## Llamadas mías (el usuario puede cambiarlas)

- **`pair` conecta a la dirección del QR** y no recibe una dirección aparte (decisión del usuario, 2026-11-20; la decisión 1 pedía una explícita en `pair` y `send`). **Esa dirección no autentica a la PC:** solo dice adónde conectar, y cualquiera puede mostrar un QR con la dirección que quiera. La PC se autentica con Noise: el patrón `XKpsk0` usa la llave pública de la oferta y el código de un solo uso, y una trama 2 que no se autentica es `PeerNotAuthentic` (el celular no manda su llave ni su nombre). La dirección tampoco se guarda como prueba de nada: es solo la última IP conocida.
- **Conexión.** `TcpStream::connect_timeout` de `std` con 5 s (`CONNECT_TIMEOUT`); no usa tokio ni crates. Un `connect` en curso no se puede interrumpir. **Al volver, antes de iniciar Noise, se comprueba el `CancelToken` y la época** (`OpGuard::check`): si cambiaron, la operación termina con `Cancelled` sin escribir un solo byte. Como `lock()` libera el estado «ocupado», una conexión colgada no bloquea la operación siguiente.
- **Sin éxitos tardíos.** Al terminar cualquier operación, antes de devolver, se vuelve a comprobar la época: si cambió, el resultado es `Cancelled` y no se presenta como vigente, aunque la red haya salido bien. En un envío, eso significa que la contraseña **pudo haber llegado** a la PC aunque Dart reciba `Cancelled`; es el mismo caso (c) de la especificación y la pantalla ya está bloqueada.
- **`import_backup` invalida la época** (ver «Estado de la sesión»): reemplaza la bóveda, y con ella la identidad, así que ninguna operación de red anterior puede actuar sobre la restaurada, aunque la importación falle.
- **La identidad cruza como huella** (`XXXX-XXXX-XXXX-XXXX`), no como llave: `ensure_identity() -> IdentityInfo { fingerprint }`. La llave privada nunca sale del núcleo.
- **La lista de PCs no lleva llaves:** `PairedPcSummary { id, name, fingerprint, last_ip, paired_at, last_seen_at }`, como pide la #107.
- **`send_to_pc` revela con `Vault::reveal`**, que actualiza `last_used_at` de la entrada (igual que revelar o copiar). La contraseña se copia a un `SecretString` de largo exacto; las notas se descartan.
- **Plazos:** los de `PairingTimeouts::default()` y `SendTimeouts::default()` del núcleo.

## Estado de la sesión

```rust
pub struct VaultSession {
    path: PathBuf,
    vault: Mutex<Option<Vault>>,
    net: Mutex<NetState>,        // nuevo
}

struct NetState {
    epoch: u64,                  // sube con cada lock() e import_backup()
    token: CancelToken,          // el de la época vigente
    active: Option<u64>,         // id de la operación de red en curso
    next_op: u64,
}
```

- `begin_op()` toma `net` un instante: si `active` es `Some`, devuelve `Busy`; si no, asigna un id y devuelve un `OpGuard { session, op, epoch, token }`. Al soltarse, el guard toma `net` y pone `active = None` **solo si** sigue siendo `Some(su id)`.
- **Orden de candados, siempre `vault` y después `net`.** Nadie toma `vault` mientras conserva `net`. `begin_op` y el `Drop` del guard solo toman `net`, un instante, sin tocar `vault`.
- **`lock()`** toma `vault` y, **sin soltarlo**, toma `net`. Con los dos tomados: bloquea la `Vault` (borra su llave), hace `epoch += 1`, `token.cancel()`, pone un token nuevo y `active = None`. Después suelta ambos. Así no existe un instante en que la bóveda esté desbloqueada con la época ya nueva: una operación que extrae datos bajo `vault` o bien los extrajo antes del bloqueo (con la época vieja, que `lock()` cancela) o bien ve la bóveda bloqueada (`Locked`). `lock()` solo espera a que termine una extracción o un guardado en curso (una operación breve sobre SQLite), nunca una conexión, porque la red no se ejecuta con ninguno de los dos candados. Sigue sin fallar con un candado envenenado (`unwrap_or_else(PoisonError::into_inner)` en los dos).
- **`import_backup`** ya conserva `vault` durante todo su ciclo (`take()`, importar, reabrir). Con `vault` tomado, y antes de soltar la `Vault`, invalida: `epoch += 1`, cancela el token, token nuevo y `active = None`. Vuelve a invalidar de la misma forma al terminar, con éxito o con error, ya con la bóveda final en su sitio, para que una operación que empezó durante la importación no actúe sobre la restaurada. Ningún camino sale sin haber invalidado.
- **Una extracción** (bajo `vault`) comprueba la época vigente con una lectura de `net` y devuelve los datos junto a esa época; **un guardado** (bajo `vault`) comprueba que la bóveda siga desbloqueada y que la época sea la capturada. Ambos son funciones de la fachada (`pub(crate)`) para probar las carreras sin depender de los tiempos.

### Flujo de `pair(offer_text, device_name)`

1. Decodifica la oferta y el nombre (`InvalidPairingOffer`, `InvalidDeviceName`), sin tocar la bóveda; los textos recibidos se borran.
2. `begin_op()` (`Busy`).
3. Bajo el candado: bóveda desbloqueada y época vigente; `ensure_identity()`; copia la llave privada a un `KeyPair` de vida corta (dentro del `SecretBox`, sin pasar por la pila). Suelta el candado.
4. Conecta a la dirección de la oferta (`connect_timeout`). **Antes de iniciar Noise comprueba el token y la época**: si cambiaron, `Cancelled`.
5. Llama a `pair_with_pc` con el token de la época.
6. Con un `PairedPc` autenticado, vuelve a tomar el candado: bóveda desbloqueada y misma época, si no `Locked` y no se guarda nada. Guarda con `into_new_paired_pc(None)` (nombre provisional) o, si la llave ya está, `update_paired_pc` (decisión 10).
7. Antes de devolver, comprueba la época otra vez: si cambió, `Cancelled`, no el resumen.
8. Devuelve el `PairedPcSummary`.

### Flujo de `send_to_pc(pc_id, entry_id, address)`

1. Valida los textos (`InvalidEntryId`, id de PC, `InvalidAddress`) y `begin_op()`.
2. Bajo el candado: desbloqueada y época vigente; busca la PC (`PairedPcNotFound`, `CorruptPairedPc` si su fila está dañada), copia su llave pública, `ensure_identity()` y revela la entrada. Arma el `SendMessage` y el `KeyPair` de vida corta. Suelta el candado.
3. Conecta. **Antes de iniciar Noise comprueba el token y la época**: si cambiaron, `Cancelled` y no sale ni un byte. Después llama a `send_to_pc` del núcleo con el token. `AckOutcome` cruza como `SendOutcome { Copied, Failed }` (`Failed`: la PC no pudo copiar; no es un error del protocolo).
4. Antes de devolver, comprueba la época: si cambió, `Cancelled`, no el `SendOutcome` (ver «Sin éxitos tardíos»).
5. No guarda nada: la contraseña se borra al soltarse el mensaje.

### Qué hace el bloqueo en cada caso

| Situación | Resultado |
| --- | --- |
| `lock()` durante una lectura o escritura de red | El conductor del núcleo ve el token en ≤ 250 ms y la operación devuelve `Cancelled`; `lock()` no espera |
| `lock()` durante el `connect` | La operación sigue hasta su plazo (≤ 5 s), vuelve, ve el token cancelado y devuelve `Cancelled` sin iniciar Noise; el estado «ocupado» ya estaba libre |
| `lock()` justo después de un `Accepted` y antes de guardar | `Locked`; no se guarda (la PC sí guardó al celular: caso (c)) |
| `lock()` y luego un desbloqueo mientras la operación seguía | Época distinta: `Locked` al guardar y `Cancelled` en los demás pasos; no continúa |
| `lock()` en medio de un envío que ya escribió todo | `Cancelled` aunque el `Ack` haya llegado; la contraseña ya salió de Rust y pudo copiarse en la PC |
| `import_backup` (con éxito o con error) con una operación de red en curso | Igual que un bloqueo: la operación termina con `Cancelled`/`Locked` y no toca la bóveda restaurada |
| Segunda operación de red | `Busy` al instante, sin tocar la bóveda |

## Funciones expuestas

| Método de `VaultSession` | Devuelve | Núcleo |
| --- | --- | --- |
| `ensure_identity()` | `IdentityInfo { fingerprint }` | `Vault::ensure_identity` |
| `pair(offer_text, device_name)` | `PairedPcSummary` | `PairingOffer::decode_qr_text`, `pair_with_pc`, `add_paired_pc` / `update_paired_pc` |
| `send_to_pc(pc_id, entry_id, address)` | `SendOutcome` | `Vault::reveal`, `send_to_pc` |
| `paired_pcs()` | `PairedPcList { pcs, corrupt, unidentified_corrupt }` | `Vault::paired_pcs` |
| `remove_paired_pc(id)` | `()` | `Vault::remove_paired_pc` |

Ninguna lleva `#[frb(sync)]`. Los ids cruzan en 32 caracteres hexadecimales en minúsculas (ayudante compartido con el de las entradas). `last_ip` cruza como texto. `corrupt` lleva ids en hexadecimal.

## Errores

`VaultError` agrega variantes sin datos (los nombres definitivos se fijan al implementar):

- **Estado:** `Busy`, `Cancelled`.
- **Entrada:** `InvalidPairingOffer`, `InvalidDeviceName`, `InvalidAddress`.
- **PCs:** `CorruptIdentity`, `CorruptPairedPc`, `PairedPcNotFound`, `PairedPcExists`, `InvalidPairedPcNameEmpty`, `InvalidPairedPcNameTooLong`, `InvalidPairedPcOther`. Reemplazan el mapeo provisional de la #93 (`StorageCorrupt`, `EntryNotFound`, `AlreadyExists`, `StorageOther`).
- **Red y protocolo:** `ConnectFailed` (no se pudo abrir la conexión), `NetworkTimeout`, `ConnectionClosed` (la PC cerró: no se distingue de un corte, ni de «la PC no reconoce al celular»), `NetworkIo`, `PeerNotAuthentic`, `PairingRejected`, `PairingConfirmationTimeout`, `SendExpired`, `ProtocolFailure`.

`PairingError` y `SendError` son `#[non_exhaustive]`: su mapeo lleva un brazo `_` hacia `ProtocolFailure`. Las variantes que el celular nunca ve (`Refused`, `NotPaired`) caen ahí. Ningún error lleva datos ni `Debug` con secretos.

## Núcleo: una función nueva

`PairingOffer::decode_qr_text(text: &str) -> Result<PairingOffer>` en `core/src/protocol/message.rs`: base64url sin relleno (alfabeto `A-Z a-z 0-9 - _`), sin espacios ni relleno `=`, con tope de largo comprobado **antes** de reservar y forma canónica (los bits sobrantes del último carácter son cero); decodifica con un búfer `Zeroizing` y delega en `PairingOffer::decode`. Sin dependencias nuevas; escrita a mano igual que el codificador de la PC. Tiene pruebas del núcleo, y la prueba de la fachada la cruza con un texto armado como lo hace `desktop/src-tauri/src/qr.rs`. Lo que no es ese formato es `ProtocolError`, que la fachada convierte en `InvalidPairingOffer`.

## Memoria y secretos

- **Los textos con secretos que llegan de Dart** (la oferta, que lleva el código de un solo uso) **ya existen como `String` de Dart y en los búferes de cruce de flutter_rust_bridge**, y Rust no puede borrar esas copias ni sabe cuántas conserva Dart. Lo único que hace la fachada es pasar el texto que recibe a un búfer de largo exacto, borrar el `String` que le entregó frb y no conservar nada más. **Ese borrado es de mejor esfuerzo** (excepción documentada en `ARCHITECTURE.md` para los búferes de cruce) y **no elimina las copias que Dart pueda conservar**. El código de la oferta es un secreto de 2 minutos y un uso.
- La llave privada del celular vive en `SecretBox` y solo se copia a un `KeyPair` de vida corta; nunca sale hacia Dart. `snow` no borra sus copias internas (excepción (5) de `ARCHITECTURE.md`, sin cambios).
- La contraseña enviada vive en `SendMessage` (`ZeroizeOnDrop`); ningún valor de retorno ni error la contiene. Una prueba lo comprueba con una contraseña distintiva.

## Pruebas

Unitarias en la fachada (`api/…/tests.rs`), con el servidor del lado de la PC armado con `serve_pairing` y `receive_send` del núcleo sobre `TcpListener` en `127.0.0.1`, y copias de las bóvedas de ejemplo (v3 para las PCs; nunca los originales):

- **Bloqueo durante un envío:** el servidor acepta y calla; `lock()` desde otro hilo; el envío devuelve `Cancelled` en menos de 1 s y la bóveda queda bloqueada con la llave borrada.
- **Bloqueo durante un emparejamiento:** igual, con `pair`.
- **Carrera entre `Accepted` y `lock()`:** el emparejamiento termina bien en la red, se bloquea antes de guardar (con la función de guardado por separado) y devuelve `Locked` sin guardar nada; al desbloquear, `paired_pcs()` sigue vacía.
- **Bloqueo y nuevo desbloqueo:** una operación capturada antes del bloqueo no guarda aunque la bóveda ya esté desbloqueada.
- **Orden de candados de `lock()` (determinista).** Una prueba toma `vault` con `slot()`, lanza `lock()` en otro hilo y comprueba, mientras `vault` sigue tomado, que la época no cambia, el token no se cancela y `active` sigue igual; al soltar `vault`, `lock()` termina y los tres cambios ocurren juntos. Otra comprueba que mientras se mantiene `vault` (una extracción) nadie observa la época nueva con la bóveda desbloqueada: la extracción hecha antes del bloqueo devuelve la época vieja, la hecha después da `Locked`.
- **Época y bloqueo real, sin tiempos.** Con una operación capturada antes (`begin_op` y su extracción): `lock()`, y comprobar que su token está cancelado, que `active` quedó libre y que el guardado da `Locked`; luego `unlock()` y comprobar que el guardado sigue dando `Locked` (época distinta) y que `paired_pcs()` está vacía.
- **`import_backup` invalida:** con una operación capturada antes, una importación con éxito y otra con contraseña incorrecta dejan su token cancelado, `active` libre y su guardado en `Locked`/`Cancelled`; una operación que empezó antes de la importación no actúa sobre la bóveda restaurada.
- **Después de `connect`:** si se cancela mientras se conecta (un servidor que acepta tarde, o `lock()` entre `begin_op` y la conexión), la operación termina con `Cancelled` y el servidor de prueba no recibe un solo byte de Noise.
- **Sin éxito tardío:** una operación que termina bien en la red pero cuya época cambió antes de devolver da `Cancelled`, no el resultado.
- **Dos operaciones simultáneas:** la segunda da `Busy` de inmediato (emparejar y enviar, en las dos combinaciones).
- **Liberación de `Busy`:** tras éxito, tras error (PC que cierra, plazo) y tras cancelar con `lock()`, la siguiente operación corre; un guard viejo que se suelta después no libera la operación nueva.
- **La contraseña no cruza:** con una contraseña distintiva, ni `SendOutcome` ni ningún error ni su `Debug` la contienen; el servidor sí la recibe.
- Extremo a extremo: `pair` con el texto de un QR real guarda la PC (nombre provisional) y `send_to_pc` llega con `Copied`; con `Failed`; a una PC que no reconoce al celular (`ConnectionClosed`); a una dirección sin servidor (`ConnectFailed`).
- `pair` sobre una PC ya emparejada: misma id, nombre y `paired_at`; `last_ip` nueva.
- Textos inválidos (oferta, nombre, dirección, ids) y bóveda bloqueada o sin bóveda (`Locked`, `NoVault`).
- `paired_pcs` y `remove_paired_pc` con la bóveda de ejemplo v3; la lista no contiene llaves.
- El mapeo de errores (cada variante del núcleo cae en la esperada, incluidas `PairingError` y `SendError`).
- Del núcleo: `decode_qr_text` válida, con relleno, con caracteres ajenos, demasiado larga, con bits sobrantes y cruzada con `encode`.

## Documentos y archivos a actualizar

- `mobile/rust/src/api/`: `protocol.rs` (métodos y tipos), `net.rs` (`NetState`, `OpGuard`), `session.rs` (el campo `net`, `lock()` e `import_backup`), `error.rs` y `entries.rs` (el ayudante de ids en hexadecimal pasa a ser común), con sus `tests.rs`.
- `mobile/rust/src/frb_generated.rs` y `mobile/lib/src/rust/` regenerados con `flutter_rust_bridge_codegen generate`.
- `mobile/android/app/src/main/AndroidManifest.xml`: `INTERNET`.
- `core/`: `decode_qr_text` y sus pruebas; `ARCHITECTURE.md` (módulos `api/` y nota del núcleo), `docs/especificacion.md` (decisión del 2026-11-20 sobre bloqueo y operaciones de red de la fachada) y `docs/roadmap.md`; `mobile/CLAUDE.md` (una operación de red nunca mantiene el candado de la sesión; el bloqueo la cancela).

## Notas para otras issues

- **#104:** `PairedPc` solo guarda la IP, no el puerto: la ruta «última IP» necesita el puerto (47615 por omisión o el que anuncie mDNS) y decidir dónde se guarda la IP manual. Tras un envío correcto, nada actualiza `last_ip`: no hay función de fachada para eso.
- **#103:** el nombre del celular lo decide la pantalla; la PC se guarda con nombre provisional. Si la PC ya estaba, `pair` conserva su nombre.
- **#105:** `ConnectionClosed` cubre un corte de red y «la PC ya no reconoce al celular» (por diseño, no se distinguen).

## Criterios de aceptación (de la issue)

- [ ] La fachada no tiene lógica propia: convierte tipos y llama a `vault-core`.
- [ ] Una contraseña enviada no aparece en ningún valor devuelto a Dart.
- [ ] El código generado está al día (la CI lo comprueba).
- [ ] Pruebas de la fachada con la bóveda de ejemplo.
- [ ] `cargo test --workspace`, `clippy` y `fmt` pasan.
