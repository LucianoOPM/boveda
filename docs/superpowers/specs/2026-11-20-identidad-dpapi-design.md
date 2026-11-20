# Windows: identidad con DPAPI y lista de emparejados (#99)

Parte de la épica #92. Dependencias #94 y #97 (hechas). Tamaño: cambio mediano en una sola crate (`desktop/src-tauri`), sin cambios en `core/`. Este diseño recoge lo aprobado por el usuario el 2026-11-20 tras comparar las opciones de protección de la lista.

## Intención

La PC necesita una identidad X25519 propia y una lista de celulares emparejados que sobrevivan a un reinicio. Ninguna de las dos debe poder leerse ni alterarse copiando o editando los archivos. Esta issue entrega **solo la persistencia**: nadie la usa todavía salvo el arranque, que genera la identidad si no existe. El emparejamiento es #101; la interfaz de la lista y la revocación desde ella, #107.

## Metas

- Generar el par de llaves en el primer arranque y guardar la llave privada protegida con DPAPI; nunca en claro en disco.
- Guardar la lista (nombre, llave pública, fecha) con DPAPI, el archivo completo.
- Agregar, listar y quitar celulares (la API; sin interfaz).
- Escrituras atómicas: ante cualquier fallo se conserva el archivo anterior.

## Fuera de alcance y antimetas

- Interfaz, QR, emparejamiento, revocación desde la interfaz, servidor (#100), mDNS.
- Antimeta: **regenerar o vaciar en silencio.** Si un archivo existe y no se puede leer, el error se devuelve y no se toca nada (ver «Política de archivos dañados»).
- No se guarda otra llave ni se agrega criptografía propia: solo DPAPI.
- Fuera de la garantía (aceptado por el usuario): malware que corra bajo el mismo usuario (DPAPI se lo entrega) y restaurar una copia antigua válida de los archivos (no hay contador de versión). La entropía fija de la app **no es un secreto**.
- No hay migración ni versión de la estructura distinta de la 1.

## Aproximaciones consideradas

1. **Todo el archivo de la lista con DPAPI (elegida).** Una sola primitiva: un archivo alterado o copiado a otra cuenta no descifra y se rechaza; sin llave que guardar.
2. JSON legible más una firma: la llave de la firma tendría que vivir con DPAPI de todos modos; más complejo para el mismo resultado.
3. Para el crate de DPAPI: `windows-sys` directo (elegido; ya es dependencia directa y está en el lock, solo suma features) frente a un crate externo como `windows-dpapi` (otra dependencia criptográfica que habría que aprobar). Context7 no documenta ningún crate de DPAPI; `windows-sys` se verificó en su código (0.61.2).

## Restricciones

- Raíz y `desktop/CLAUDE.md`: llave privada con DPAPI y nunca en claro; sin secretos en logs, errores, panics ni `Debug`; sin dependencias criptográficas ni de red nuevas sin preguntar; la criptografía vive en `core/` (aquí solo se llama al sistema operativo, como ya prevé `ARCHITECTURE.md`).
- Verificación: `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all`, `cargo audit`, `cargo deny check`.

## Dependencias

| Crate | Uso | Notas |
| --- | --- | --- |
| `vault-core` (ruta) | `KeyPair`, `PublicKey` | Interna del workspace; `desktop/` ya está previsto que la use |
| `windows-sys` 0.61 (ya directa, `cfg(windows)`) | `CryptProtectData`, `CryptUnprotectData`, `LocalFree`, `MoveFileExW` | Se añaden las features `Win32_Security_Cryptography` y `Win32_Storage_FileSystem`; sin crates nuevos en el lock |
| `serde` y `postcard` (ya en el lock por el núcleo) | Serializar la lista | Mismas versiones y features que `core/`; **decisión mía**, no aprobada aparte: no son criptografía ni red |
| `tempfile` (ya en el lock; solo `dev-dependencies`) | Carpetas de prueba | Decisión mía; si `cargo deny` o el lock objetan, se usa `std::env::temp_dir` |

Se comprueba con `grep '^name = ' Cargo.lock` antes y después que no entra ningún crate nuevo; la lista exacta va en la PR.

## Diseño

### Módulos (`desktop/src-tauri/src/`, todos `pub`, sin `#[allow(dead_code)]`)

- `dpapi.rs` (`cfg(windows)`): el único sitio con `unsafe`.
  - `protect(plain: &[u8], entropy: &[u8]) -> Result<Vec<u8>, DpapiError>` y `unprotect(blob: &[u8], entropy: &[u8]) -> Result<Zeroizing<Vec<u8>>, DpapiError>`.
  - Alcance de **usuario** (sin `CRYPTPROTECT_LOCAL_MACHINE`) y `CRYPTPROTECT_UI_FORBIDDEN`. Descripción vacía. La salida de DPAPI se copia y el búfer de Windows se sobrescribe con ceros (`zeroize`) antes de `LocalFree` en el descifrado; si algo falla a medias, también se libera.
  - `DpapiError` no lleva datos; conserva el código de error de Windows.
- `atomic_file.rs`: `write_atomic(path, bytes)`.
  1. Crea un temporal en **el mismo directorio** (`<nombre>.tmp`, con `create_new`: si quedó uno de una ejecución anterior, se rechaza el temporal viejo borrándolo primero solo si es ese, nunca el destino).
  2. Escribe todo y llama a `File::sync_all`.
  3. Reemplaza con `MoveFileExW(tmp, destino, MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH)`: una sola operación que **no borra antes** el archivo anterior (los permisos de `std::fs::rename` son los mismos pero sin `WRITE_THROUGH`; se verificó en el código de `std`).
  4. Si falla cualquier paso, intenta borrar el temporal y deja el destino intacto; devuelve el error.
  - La promesa es la que da Windows (`sync_all` y `WRITE_THROUGH`); **no se promete durabilidad absoluta** ante un corte eléctrico. No se sincroniza el directorio.
- `identity.rs`: `IdentityStore` (ruta del archivo `identity.bin`).
  - `load_or_create(&self) -> Result<KeyPair, IdentityError>`: archivo ausente → genera con `KeyPair::generate`, guarda y devuelve; presente → lo lee y descifra. **Solo se genera cuando el archivo no existe.** Para distinguir «ausente» de «no se pudo leer» se usa `ErrorKind::NotFound` en la lectura; cualquier otro error de E/S se devuelve.
  - Contenido en claro antes de DPAPI: `boveda-pc-identity` (19 bytes) | versión `u16` little-endian (=1) | 32 bytes de llave privada. Exactamente esa longitud; bytes sobrantes o faltantes → `Corrupt`; versión distinta de 1 → `UnknownVersion`. La llave no se valida ni se transforma (como en el núcleo).
  - Entropía de DPAPI: `boveda-desktop identity v1`.
  - `IdentityError`: `Io`, `Protection` (DPAPI no pudo descifrar: archivo alterado, de otra cuenta o de otro equipo), `Corrupt`, `UnknownVersion`. Sin datos; `Debug` y `Display` no muestran nada del archivo ni de la llave.
  - Un temporal de prueba o una escritura que falla al crear la identidad **no deja una identidad a medias**: o existe el archivo completo o no existe.
- `paired_devices.rs`: `PairedDeviceStore` (archivo `paired-devices.bin`).
  - `PairedDevice { name: String, public_key: [u8; 32], paired_at: i64 }` (milisegundos Unix, que da el llamador). Nombre recortado, de 1 a 128 bytes UTF-8, igual que `vault::PairedPc`.
  - `open(path) -> Result<Self, PairedDevicesError>`: archivo ausente → lista vacía (sin crear nada en disco); presente → debe descifrarse y deserializarse **por completo** o se devuelve el error; **nunca** se reemplaza por una lista vacía.
  - `list(&self) -> &[PairedDevice]` (orden de inserción), `add(&mut self, device) -> Result<(), PairedDevicesError>` (rechaza llave repetida con `AlreadyPaired` y nombre inválido), `remove(&mut self, public_key: &[u8; 32]) -> Result<(), PairedDevicesError>` (`NotFound` si no está).
  - Cada cambio arma la lista nueva, la guarda con `write_atomic` y **solo si la escritura termina** actualiza la memoria: un fallo deja la lista en memoria y en disco como estaban.
  - Contenido en claro: `boveda-pc-devices` (17 bytes) | versión `u16` LE (=1) | cuerpo `postcard`. Se lee con `take_from_bytes` y se exige que no sobren bytes. Límite de 64 celulares y de tamaño del archivo (rechaza lo que lo exceda antes de deserializar) para que un archivo hostil no pida memoria sin tope.
  - Entropía distinta: `boveda-desktop paired-devices v1`. Así, intercambiar el archivo de la identidad por el de la lista (o al revés) no descifra.
  - `PairedDevicesError` análogo al de la identidad, más `AlreadyPaired`, `NotFound`, `InvalidName` y `TooMany`. Sin datos; las llaves públicas y los nombres no se incluyen en errores ni logs.
- `lib.rs`: en `setup`, se resuelve `app.path().app_data_dir()` (`%APPDATA%\<identificador>`), se crea la carpeta si falta y se llama a `IdentityStore::load_or_create` para que la identidad exista tras el primer arranque; el `KeyPair` se descarta. **Decisión mía:** si falla (archivo dañado, otra cuenta), se registra con `eprintln!` solo el tipo de error y la app sigue viva en la bandeja; el emparejamiento (#101) mostrará el error cuando exista una ventana. No se maneja estado de Tauri todavía: #100 y #101 abrirán los almacenes por su cuenta con la misma ruta. Sin IPC ni comandos nuevos.

### Política de archivos dañados

| Situación | Identidad | Lista |
| --- | --- | --- |
| Archivo ausente | se genera y guarda | lista vacía, no se escribe nada |
| Existe y no descifra (alterado, truncado, otra cuenta) | `Protection`; no se toca | `Protection`; no se toca |
| Descifra pero la estructura o la longitud no coinciden | `Corrupt`; no se toca | `Corrupt`; no se toca |
| Versión desconocida | `UnknownVersion`; no se toca | `UnknownVersion`; no se toca |
| Error de E/S distinto de «no existe» | `Io`; no se toca | `Io`; no se toca |

Reparar (restaurar una copia o borrar el archivo a mano) es una acción del usuario; esta issue no añade herramientas de reparación. Consecuencia aceptada: una identidad ilegible bloquea el emparejamiento hasta que el usuario borre el archivo, porque los celulares ya emparejados confían en esa llave.

## Pruebas (`cargo test --workspace`, en Windows con DPAPI real y carpetas temporales)

- `dpapi`: ida y vuelta; el blob no contiene el texto en claro; entropía distinta no descifra; un byte cambiado, vacío o truncado da error.
- `identity`: primer arranque crea el archivo y no contiene la llave privada (se compara con los bytes de la llave); segundo arranque devuelve la **misma** llave pública (persistencia tras «reiniciar», con un `IdentityStore` nuevo); un byte alterado, archivo truncado o vacío, versión desconocida (armada con DPAPI real) y longitud inválida devuelven error **sin regenerar** (el archivo queda byte por byte igual y no se crea otro); error de E/S distinto de «no existe» (la ruta es una carpeta) no regenera; el archivo de la lista no sirve como identidad ni al revés (entropía).
- `paired_devices`: agregar, listar y quitar; sobrevive a reabrir; no admite repetidos ni nombres vacíos o de más de 128 bytes ni más de 64; el archivo no contiene el nombre ni la llave pública en claro; alteración, truncado, versión desconocida, estructura inválida y bytes sobrantes devuelven error **sin vaciar** (archivo igual byte por byte); `add` y `remove` que fallan al escribir dejan igual la memoria y el disco.
- `atomic_file`: crea y reemplaza; el temporal queda en el mismo directorio y desaparece; **un fallo del reemplazo conserva el original** (se fuerza abriendo el destino sin `FILE_SHARE_DELETE`, así `MoveFileExW` falla) y no deja temporal; un temporal sobrante de una ejecución anterior no impide escribir.
- Sin regeneración silenciosa: una prueba que corrompe el archivo, llama dos veces a `load_or_create` y comprueba que ambas fallan y el archivo no cambió.
- Sin secretos: ninguna de las `Debug`/`Display` de los errores contiene la llave privada ni el nombre del celular (se compara contra valores conocidos).
- **Copia a otra cuenta de Windows:** no se puede automatizar sin crear usuarios en el equipo. Lo que sí se prueba automáticamente es que un blob protegido con otra entropía no descifra, y que DPAPI de usuario sin `LOCAL_MACHINE` se usó (revisando los flags en el código). **La comprobación real con otra cuenta queda como validación manual del usuario** y se anota en la PR (copiar `identity.bin` y `paired-devices.bin` a otra cuenta y comprobar que `load_or_create` y `open` fallan con `Protection`; se puede hacer con un ejemplo pequeño de `cargo run --example`, sin tocar su bóveda).

## Documentación

- `ARCHITECTURE.md` (sección de `desktop/`): módulos nuevos, ubicación de los archivos y la política de archivos dañados.
- `docs/especificacion.md`: párrafo «Decidido el 2026-11-20 (#99)» con la política, las limitaciones aceptadas (mismo usuario, copia antigua válida) y la entropía no secreta.
- `docs/roadmap.md`: marcar la llave de la PC con DPAPI como hecha; el flujo completo sigue en #100, #101 y #107.
- `deny.toml`: sin cambios previstos; se confirma con `cargo deny check`.

## Qué queda para quien construya

Nombres internos de funciones, el orden exacto de los commits (uno por módulo), y los mensajes de los errores (en español, sin datos). Si `windows-sys` pide otra feature para compilar, se agrega la mínima y se anota en la PR.
