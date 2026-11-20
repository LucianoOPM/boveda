# Arquitectura

Gestor de contraseñas personal. La bóveda vive en el celular Android; una app de Windows recibe contraseñas por la red local y las deja en el portapapeles. Todo lo crítico (cifrado, formato de la bóveda, búsqueda y protocolo) está escrito una sola vez en Rust y lo usan ambas apps.

Documentos relacionados: `docs/especificacion.md` (qué y por qué) y `docs/roadmap.md` (en qué orden); el diseño visual está en `docs/DESIGN.md`.

## Mapa del repositorio

La estructura de la raíz, de `core/` y de `mobile/` es la definitiva y debe seguirse desde ahora; en `mobile/`, las carpetas marcadas como futuras se crean con las pantallas de la etapa 2. La de `desktop/` es la esperada: la definitiva la genera el creador de Tauri al iniciar la etapa 3. Si lo generado difiere de este mapa, manda lo generado y se actualiza este archivo.

```
boveda/
├── Cargo.toml              # workspace: core, mobile/rust, desktop/src-tauri
├── Cargo.lock
├── rust-toolchain.toml     # versión fija de Rust; subirla es un commit propio
├── deny.toml               # reglas de cargo deny
├── .gitignore
├── .github/workflows/
│   ├── ci.yml              # núcleo (Linux), workspace completo (Windows) y dependencias
│   └── mobile.yml          # Flutter: analyze, test, puente al día y APK debug
├── .claude/                # configuración de Claude Code (settings, skills, rules)
├── CLAUDE.md
├── ARCHITECTURE.md
├── docs/
│   ├── especificacion.md
│   ├── roadmap.md
│   ├── DESIGN.md           # sistema de diseño de las dos apps (referencia)
│   └── superpowers/
│       ├── specs/          # diseños por issue (AAAA-MM-DD-tema-design.md)
│       └── plans/          # planes de implementación por issue
│
├── core/                   # crate vault-core (definitivo)
│   ├── Cargo.toml
│   ├── CLAUDE.md
│   ├── src/
│   │   ├── lib.rs
│   │   ├── error.rs        # tipo Error (thiserror), sin secretos
│   │   ├── crypto/         # kdf.rs (Argon2), keys.rs, aead.rs, normalize.rs (NFC)
│   │   ├── vault/          # mod.rs (API), header.rs, entry.rs, storage.rs, backup.rs, password.rs, identity.rs, paired_pc.rs
│   │   ├── search.rs
│   │   └── protocol/       # mod.rs, error.rs, identity.rs, message.rs (#94); pairing.rs (emparejamiento Noise) y transport.rs (tramas, plazos y cancelación) (#95); send.rs (envío Noise IK: máquinas de estado y conductores) (#96)
│   └── tests/
│       ├── vault.rs
│       ├── protocol.rs
│       └── fixtures/       # bóvedas de ejemplo fijas (vault-vN.db, ver su README)
│
├── mobile/                 # app Flutter (etapa 2)
│   ├── CLAUDE.md
│   ├── pubspec.yaml
│   ├── analysis_options.yaml
│   ├── flutter_rust_bridge.yaml
│   ├── rust/               # crate puente rust_lib_boveda: fachada delgada sobre vault-core
│   │   ├── Cargo.toml      # edición 2021 por el código generado
│   │   ├── rust-toolchain.toml  # mismo canal y componentes que el de la raíz, más los 3 targets de Android; lo lee el build hook
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── api/        # expuesto a Dart, un archivo por área:
│   │       │               #   mod.rs, error.rs (VaultError), session.rs (VaultSession),
│   │       │               #   entries.rs (buscar, leer por id, agregar, editar,
│   │       │               #   eliminar y revelar), backup.rs y password.rs (fortaleza
│   │       │               #   de la contraseña maestra); pruebas en <área>/tests.rs
│   │       ├── test_support.rs   # ayudantes de pruebas (copia de vault-v2.db)
│   │       └── frb_generated.rs  # generado; no se edita
│   ├── hook/
│   │   └── build.dart      # build hook (flutter_rust_bridge_hooks y native_toolchain_rust): compila la librería de Rust de Android con rust/rust-toolchain.toml; reemplaza a cargokit
│   ├── test/               # pruebas de widgets en la PC, con el puente simulado
│   ├── integration_test/   # pruebas en dispositivo que cargan la librería de Rust
│   ├── test_driver/        # punto de entrada para correr integration_test con flutter drive
│   ├── lib/
│   │   ├── main.dart       # arma las dependencias reales y arranca la app
│   │   ├── app.dart        # BovedaApp y VaultGate (la pantalla según el estado de la sesión, con la oferta de la huella tras el primer uso; guarda el total de dañadas del desbloqueo para el aviso; bloqueo automático al ocultarse y por inactividad; desbloqueada, muestra la búsqueda, y cada bloqueo descarta su texto); bootstrap: si el arranque falla, "No se pudo iniciar la app"
│   │   ├── src/rust/       # código generado por flutter_rust_bridge; no se edita
│   │   ├── screens/        # first_run_screen.dart, unlock_screen.dart (contraseña y huella), biometric_offer_screen.dart, search_screen.dart (pantalla principal: barra de búsqueda, lista filtrada por el núcleo y botón para crear), entry_detail_screen.dart (detalle: copiar, mostrar u ocultar la contraseña y las notas, eliminar), entry_form_screen.dart (crear, editar y reparar una entrada), placeholder_screen.dart (mensajes de error), settings_screen.dart (ajustes: exportar e importar el respaldo), restore_dialog.dart (importar un respaldo: contraseña del respaldo y resultado; lo comparten Ajustes y el primer uso)
│   │   ├── widgets/        # activity_text_field.dart (campo que cuenta como actividad, cierra el teclado al tocar fuera y, con `copyable: false`, no deja copiar ni cortar; los controles del formulario van en `TextFieldTapRegion`), password_field.dart, tags_field.dart (etiquetas como chips), strength_indicator.dart, damaged_entries_banner.dart
│   │   └── services/       # vault_location.dart (ruta en noBackupFilesDir), auto_lock.dart (tiempo de bloqueo, alcance de la actividad y `runWithoutAutoLock`), backup_files.dart (selector de archivos y temporales del respaldo), biometric_service.dart (canal de la huella), biometric_unlock.dart (activar y desbloquear con huella), local_clipboard.dart (copia local)
│   ├── android/            # app/src/main/kotlin/.../MainActivity.kt (FLAG_SECURE, canales; cancela la huella en onStop), BiometricVault.kt (Keystore y huella), LocalClipboard.kt (marca sensible y borrado condicional) y BackupFiles.kt (selector del sistema y temporales en `noBackupFilesDir/backup-tmp/`; `BackupPaths.kt` comprueba que un temporal cuelgue de esa carpeta); AndroidManifest.xml y res/xml/ (respaldo automático desactivado)
│   └── build/              # ignorado; el build hook compila Rust en .dart_tool/hooks_runner (en Windows, en un directorio corto del temporal)
│
└── desktop/                # app Tauri 2 (esperado, etapa 3)
    ├── CLAUDE.md
    ├── package.json
    ├── src/                # interfaz: esqueleto de React sin pantallas (main.tsx, App.tsx) y styles/tokens.css; la ventana del QR y la lista de dispositivos llegan con #101 y #107
    └── src-tauri/
        ├── Cargo.toml      # sin sección [workspace]: pertenece al de la raíz
        ├── tauri.conf.json
        └── src/
            ├── main.rs
            ├── lib.rs        # Builder: single-instance, autostart y bandeja; no termina al cerrar la última ventana
            ├── tray.rs       # ícono y menú de la bandeja (menu_spec)
            ├── autostart.rs  # lógica de la casilla Iniciar con Windows
            ├── server.rs     # escucha, mDNS, acepta solo emparejados
            ├── clipboard.rs  # API del portapapeles (#98): ClipboardManager con un hilo trabajador, Config, ClipboardError y el aviso «Contraseña lista para pegar»
            ├── clipboard/    # backend.rs (trait), fingerprint.rs (huella), worker.rs (vencimientos y reintentos), windows.rs (arboard y clipboard-win); fake.rs y tests.rs solo en pruebas
            │                 # (pruebas con el portapapeles real, #[ignore], en src-tauri/tests/clipboard_windows.rs)
            └── identity.rs   # llave privada protegida con DPAPI
```

Los miembros del workspace se agregan conforme existen: `core` en la etapa 1, `mobile/rust` en la etapa 2 y `desktop/src-tauri` en la etapa 3.

## Qué va dónde

- Si toca criptografía, el formato de la bóveda o el protocolo, o si ambas apps lo necesitan → `core/`.
- Si es interfaz o algo propio de una plataforma (huella, Keystore, DPAPI, bandeja de Windows) → la app correspondiente.
- `mobile/rust/` es solo una fachada: convierte tipos y llama a `vault-core`, sin lógica propia.

## Componentes

### core/ (vault-core)

| Módulo | Responsabilidad |
| --- | --- |
| `crypto/` | Argon2id (`kdf.rs`, sobre la contraseña maestra en NFC); normalizar la contraseña maestra a NFC con una copia que se borra (`normalize.rs`); generar la llave de bóveda, derivar la llave maestra y cifrar o descifrar con ella la llave de bóveda, y crear y comprobar el verificador `key_check` (`keys.rs`); cifrar y descifrar cada parte de una entrada con XChaCha20-Poly1305 y su dato autenticado (`aead.rs`) |
| `vault/` | API de la bóveda (`mod.rs`, estructura `Vault`); cabecera (formato 3; con `key_check` desde la v2): codificarla como dato autenticado con la versión de la cabecera (`HEADER_VERSION`, independiente de `format_version`), leerla y validarla, y armar la v2 al desbloquear una v1 (`header.rs`); identidad del celular, la llave privada X25519 cifrada con la llave de bóveda (`identity.rs`); PCs emparejadas, cifradas con su propio dato autenticado (`paired_pc.rs`); modelo de entrada (`entry.rs`) y entrada cifrada (`entry/sealed.rs`), almacenamiento SQLite con lápidas para las eliminadas, las tablas `identity` y `paired_pcs` (v3) y las migraciones v1→v2 y v2→v3 (`storage.rs`), respaldos: exportar con `VACUUM INTO` e importar validando una copia temporal, que se migra a v3 y se deja sin identidad ni PCs antes del renombrado (`backup.rs`), regla de la contraseña maestra: longitud mínima y fortaleza (`password.rs`) |
| `search.rs` | Búsqueda tolerante a errores sobre los datos de búsqueda |
| `protocol/` | Tipos y codificación (#94, sin red): par de llaves X25519 y llave pública con su huella corta (`identity.rs`, `x25519-dalek`), mensajes con versión y tipo (`message.rs`: oferta del QR con el código de un solo uso, envío con TTL de 30 s y confirmación; decodificación estricta con `postcard`, máximo 4 KiB) y `ProtocolError` (`error.rs`); mensajes del emparejamiento (#95): `PairingRequest` con el nombre del celular (`DeviceName`) y `PairingReply`. Emparejamiento con Noise (#95, sin tokio): máquinas de estado por tipos sin sockets (`pairing.rs`: `PairingSession` con los estados del código de un solo uso, `Initiator`, `PendingPairing`) y conductor síncrono sobre `Read + Write` (`transport.rs`: `TimedStream`, `CancelToken`, tramas de máximo 512 bytes con plazos y cancelación). Envío con Noise IK (#96, sin tokio): máquinas de estado (`send.rs`: `Responder`, `AwaitingSend` y `ReceivedSend` en la PC; `Sender`, `AwaitingResponse` y `AwaitingAck` en el celular) y conductores síncronos `receive_send` y `send_to_pc` con tramas de máximo 2048 bytes (`transport.rs`, que ahora recibe el máximo como parámetro); la PC consulta la lista de emparejados que le pasa la app antes de contestar y cierra sin escribir nada si no está. El descubrimiento mDNS llega después |

Sin dependencias de interfaz ni de plataforma.

### mobile/ (Android, Flutter)

- `rust/`: crate puente expuesto con flutter_rust_bridge. Dart guarda un objeto opaco `VaultSession` (la ruta y un `Mutex<Option<Vault>>`) y le pide todas las operaciones; los errores cruzan como un enum plano `VaultError`, sin datos.
- `lib/src/rust/`: código generado; no se edita a mano.
- `lib/app.dart`: widget raíz que elige la pantalla según el estado de la sesión (sin bóveda, oferta de la huella, bloqueada, desbloqueada, error al abrir), sin go_router. Al desbloquear, muestra `SearchScreen` (`lib/screens/search_screen.dart`) con el total de entradas dañadas del `UnlockReport` para su aviso; la pantalla llama a `search` en cada cambio del texto y descarta las respuestas viejas. Bloquea al pasar la app a `hidden` y tras 5 minutos sin actividad (`defaultAutoLockTimeout`, en `lib/services/auto_lock.dart`): cierra todo lo apilado sobre la raíz y monta un desbloqueo nuevo, porque cada bloqueo cambia la `key` de la pantalla (`_epoch`). Toda transición a un estado con Rust desbloqueado pasa por `_enterUnlocked`, que bloquea si la operación terminó con la app oculta. Los toques llegan por un `Listener` en `MaterialApp.builder` y lo escrito por `ActivityTextField`, a través de `AutoLockScope`. Recibe la sesión, la regla de fortaleza y el servicio de huella inyectados, para que las pruebas de widgets usen dobles escritos a mano.
- `lib/screens/`, `lib/widgets/`: interfaz. Desde `SearchScreen` se apilan en el `Navigator` raíz el detalle (`EntryDetailScreen`) y el formulario (`EntryFormScreen`); al volver, la búsqueda se repite. El detalle no llama a Rust al abrirse: la contraseña y las notas se piden por separado, solo al tocar "mostrar", viven en el estado de esa pantalla y se descartan al ocultarlas, al abrir el formulario y al salir. "Copiar" pide la contraseña por separado y la mantiene solo en una variable local mientras cruza el canal sensible. El formulario envía los textos tal cual (el núcleo normaliza y valida) y solo manda la parte del secreto que el usuario abrió, así que la otra no sale de Rust. Tras crear o editar, la app lee lo guardado con `entry(id)`.
- `lib/services/`: ruta de la bóveda (`noBackupFilesDir`, por un canal de Kotlin), huella (canal `boveda/biometric` y los flujos de activar y desbloquear), portapapeles local (canal `boveda/clipboard`) y selector de archivos del respaldo (canal `boveda/backup`: Dart solo maneja rutas de temporales; la copia con la URI `content://` la hace Kotlin). Los respaldos se exportan e importan desde `SettingsScreen` (engranaje de la pantalla principal) y se restauran desde el primer uso; el selector corre dentro de `runWithoutAutoLock`, y un error al importar desde Ajustes bloquea la interfaz y muestra el motivo en el desbloqueo (aviso de un solo uso en `VaultGate`).
- `android/app/src/main/kotlin/com/jesus/boveda/`: `MainActivity.kt` marca la ventana con `FLAG_SECURE` en `onCreate` (sin capturas y sin contenido en recientes) y registra los canales; `BiometricVault.kt` guarda la copia de la llave de bóveda cifrada con una llave del Keystore que solo se usa con huella (`noBackupFilesDir/biometric/vault_key.bin`) y muestra `BiometricPrompt`; `cancelPrompt()`, llamado desde `MainActivity.onStop`, cancela el diálogo abierto y responde `canceled` de inmediato. `LocalClipboard.kt` marca contraseñas como sensibles y, a los 30 segundos, borra solo el clip que todavía conserve su token.

### desktop/ (Windows, Tauri 2)

- `src-tauri/src/`: bandeja con su menú de cuatro opciones (`tray.rs`; «Emparejar celular» y «Dispositivos emparejados» siguen deshabilitadas hasta #101 y #107) y la casilla «Iniciar con Windows» (`autostart.rs`). Faltan el servidor de red local y la identidad protegida con DPAPI. Sin ventana al arrancar; instancia única con `tauri-plugin-single-instance` 2.5.5. «Iniciar con Windows» usa `tauri-plugin-autostart` 2.7.0 y no tiene archivo de ajustes: la casilla lee y reconcilia `is_enabled()`, que consulta la clave `Run` del usuario en el registro. Ambos plugins piden `tauri` 2.12 o posterior.
- `src-tauri/src/clipboard/` (#98): `ClipboardManager` (estado de la app, arrancado con `start()`) recibe una contraseña (`&SecretString`) por `deliver`, la escribe con `arboard` excluida del historial de Win+V y de la nube, y a los 30 s la borra solo si el portapapeles todavía la contiene. Un único hilo trabajador, con un canal, `recv_timeout` e `Instant`, es el único que toca el portapapeles. Solo guarda una huella de 64 bits (`RandomState` creada con el gestor), nunca la contraseña. Rechaza contraseñas vacías o con `\0` (`InvalidPassword`) antes de tocar nada, porque Windows las pegaría truncadas. Un envío nuevo reemplaza el temporizador solo si su escritura salió bien; una escritura de resultado dudoso intenta limpiarse al instante y, si no puede comprobarlo, deja una entrada de limpieza con reintentos acotados (como máximo dos entradas, tres reintentos cada una). Limitación conocida: `arboard` pone el texto y después agrega los formatos de exclusión; si agregarlos falla (o si Windows no registra un formato y `arboard` omite la exclusión sin avisar), la contraseña pudo quedar un instante sin excluir y llegar al historial de Win+V o a la nube antes de la limpieza inmediata, y vaciar el portapapeles no borra entradas del historial. Es muy poco probable y no se puede detectar desde fuera; cerrarlo del todo exigiría escribir también con `clipboard-win` bajo una sola guarda, lo que choca con «escribe con `arboard`». El usuario aceptó provisionalmente este riesgo residual y mantiene `arboard` (2026-11-20). La exclusión del historial y de la nube **no es una garantía absoluta** en ese escenario: si el contenido ya entró al historial, vaciar el portapapeles **no lo elimina de Win+V**. «Salir» pide un último intento de limpieza (`RunEvent::Exit`, espera máxima de 2 s). El aviso lo muestra `tauri-plugin-notification` desde Rust (sin permisos de capability). Nadie lo llama todavía: lo usarán el servidor de red (#100) y el flujo de punta a punta (#104).
- `src/`: interfaz mínima (esqueleto de React y `styles/tokens.css`); la ventana del QR y la lista de dispositivos llegan con #101 y #107.

## Flujos principales

**Desbloqueo.** La contraseña maestra pasa por Argon2id y descifra la llave de bóveda; con huella, el Keystore libera una copia cifrada de esa misma llave. La app ofrece activar la huella y la desactiva sola, con un aviso, si la copia deja de servir (por ejemplo, al registrar una huella nueva).

**Búsqueda.** Al desbloquear se descifran solo los datos de búsqueda de cada entrada. El secreto (contraseña y notas) se descifra únicamente al copiar, mostrar o enviar.

**Envío a la PC.**
1. El celular confirma con huella y busca la PC (última IP, luego mDNS, luego IP manual).
2. Abre una conexión corta con Noise usando las llaves de ambos dispositivos.
3. Envía la contraseña con un tiempo de vida de 30 s y espera confirmación.
4. La PC la escribe en el portapapeles, excluida del historial y la nube, y la borra a los 30 s si sigue ahí.

## Decisiones clave

| Decisión | Por qué |
| --- | --- |
| Un solo repositorio | Ambas apps deben usar exactamente la misma versión del formato y del protocolo |
| Núcleo en Rust compartido | La parte delicada se escribe y prueba una sola vez |
| Crate puente separado | Los detalles de flutter_rust_bridge no contaminan el núcleo |
| Dos llaves (maestra y de bóveda) | Cambiar la contraseña maestra y usar huella sin recifrar entradas |
| Borrado de memoria garantizado solo dentro de `vault-core` | El núcleo guarda secretos y texto plano en `SecretString`/`Zeroizing` y evita reasignaciones que dejen copias; fuera del núcleo es de mejor esfuerzo, porque Dart no permite borrar sus cadenas. Lo mismo vale para los búferes de cruce de flutter_rust_bridge y de `StandardMessageCodec` (canales de Kotlin), para la copia que la fachada entrega al revelar una contraseña o unas notas, y para la copia de la llave de bóveda que entrega para el Keystore. Excepciones documentadas: (1) las copias internas de `nucleo-matcher` y `strsim` durante la búsqueda (datos de búsqueda, no secretos); (2) los búferes internos del iterador `nfc()` de `unicode-normalization` (de 4 elementos en la pila, o en el heap si un grupo de marcas combinantes es más largo): `tinyvec` no los borra, así que pueden quedar unos pocos caracteres de varios grupos anteriores, nunca la contraseña entera, solo con contraseñas maestras que no están en NFC o que contienen marcas combinantes o jamo (ASCII y casi todas las letras precompuestas no pasan por ellos). Se acepta porque las otras opciones eran una primitiva propia con tablas Unicode o dejar la #57 sin resolver; si la crate ofrece borrar ese búfer, se revisa; (3) las copias internas de las llaves y de la contraseña maestra en las dependencias de la derivación y el cifrado: `full_out`, un arreglo de 64 bytes en la pila de `blake2b_long` de `argon2` 0.6.0 con el hash completo (sus 32 primeros bytes son la llave derivada, y no se borra); la llave que `ChaChaPoly1305` de `chacha20poly1305` 0.11.0 guarda por valor dentro de la estructura (se borra al soltarse, pero un movimiento del cifrador puede dejar copias) y la subllave de `hchacha` que `chacha20` 0.10.2 deja en la pila al crear el XChaCha; y las copias transitorias que el compilador deje en registros y pila durante la compresión de BLAKE2. Se cierran solo con una primitiva propia, que `core/CLAUDE.md` prohíbe; se revisan al actualizar cualquiera de esas versiones. Lo que sí se borra: la llave derivada y la de bóveda viven en el heap (`SecretBox`); la memoria de trabajo de Argon2id (de 64 a 128 MiB) la reserva y la borra `derive_key`, porque la de `argon2` se libera sin borrar y con paralelismo 1 permite recalcular la llave derivada; y el `Blake2b512` donde `argon2` mete la contraseña maestra, con su búfer de bloque y su estado `h`, porque `blake2` está declarada en `core/Cargo.toml` con la feature `zeroize` (solo para activarla por unificación de features). Los ayudantes que devolvían el cifrador por valor se eliminaron, como esfuerzo razonable sin garantía. (4) `x25519-dalek` y `curve25519-dalek` (#94): `StaticSecret::from([u8; 32])` recibe la llave privada por valor y deja una copia en la pila de quien llama que no se borra, y la multiplicación deja el escalar recortado en registros y pila; lo mismo con el `[u8; 32]` que pasa por la pila al decodificar un `PairingCode` (secreto de 2 minutos y un uso). Solo se cierra con una primitiva propia, prohibida; se mitiga derivando justo después de leer la llave del `SecretBox` y sin retenerla. (5) `snow` 0.10.0 (#95 y #96) no depende de `zeroize` y no borra ningún secreto. Pueden quedar en su memoria, sin borrar, al soltarse el estado: la llave privada estática (una copia de la del dispositivo, en `Dh25519.privkey`), la llave privada efímera, el código de un solo uso (una copia de 32 bytes en `psks`) y lo que se deriva de él al mezclarlo, el estado simétrico (la llave de encadenamiento `ck` y el resumen `h`, de hasta 64 bytes cada uno) y las llaves de cifrado de los dos sentidos (`CipherState` y, tras `into_transport_mode`, el `TransportState`); además, `curve25519-dalek` 4.1.3 y `blake2` 0.10, sin la feature `zeroize`, dejan escalares y estados en pila y registros. No se puede garantizar el borrado porque esos campos viven en tipos de `snow` y sus dependencias que no ofrecen borrarlos ni tienen un `Drop` que lo haga; solo se cerraría con una primitiva o un resolvedor criptográfico propios, que `core/CLAUDE.md` prohíbe. Pasar las llaves como `&[u8]` desde el `SecretBox` y soltar los estados en cuanto termina el emparejamiento reduce las copias y su vida, pero **no es borrado seguro**: no sobrescribe la memoria interna de `snow`. El secreto más delicado que queda es la llave privada de larga duración del dispositivo. En el envío (#96, verificado en `resolvers/default.rs` de `snow` 0.10.0) el estado de transporte no guarda el texto plano (cifra y descifra en el búfer del llamador, que aquí es `Zeroizing`; un fallo de autenticación deja el texto cifrado), pero la llave de cifrado de cada sentido (`CipherChaChaPoly.key`, `[u8; 32]`) y las copias por valor que `ChaCha20Poly1305::new` deja en la pila no se borran, y la contraseña descifrada pasa por registros y pila durante el cifrado; la contraseña en claro se borra al soltar `SendMessage` y esos búferes. La contraseña maestra también vive en la memoria de Dart, que no se puede borrar: nada de esto cierra la exposición completa, es defensa en profundidad |
| Entrada cifrada en dos partes | Búsqueda rápida sin exponer contraseñas en memoria |
| Fecha de modificación y marca de eliminada | Preparan la sincronización futura sin migrar datos |
| Noise (`snow`) | Protocolo estudiado para el intercambio de llaves y el canal cifrado |
| Emparejamiento: máquinas de estado separadas del conductor | Las máquinas de estado (`pairing.rs`) no tocan sockets y se prueban sin red; el conductor síncrono (`transport.rs`) solo mueve tramas con plazos y cancelación, sin tokio ni runtime en el puente de Flutter |
| Un apretón de manos terminado no autoriza | La confirmación del usuario en la PC es obligatoria y está impuesta por tipos: solo `accept` produce el dispositivo de la PC y solo una aceptación autenticada produce la PC del celular; el núcleo nunca persiste la confianza |
| Envío: Noise IK, un solo envío por conexión y plazo de 30 s sin marcas de hora | IK rechaza a un celular no emparejado en la primera trama, sin contestarle; la llave efímera de la PC en cada conexión impide reproducir capturas, así que no hacen falta identificadores ni cachés; los 30 s los cuenta la PC con reloj monótono y solo limitan la recepción completa y autenticada (los relojes de los dos equipos no coinciden); la espera del `Ack` es aparte |
| Tauri 2 | Bandeja del sistema integrada y espacio para una interfaz futura |
| Portapapeles: el borrado condicional lee, compara y vacía bajo una sola apertura | **ESTADO (2026-11-20): la exclusión frente a otras aplicaciones NO se cumple con `OpenClipboard(NULL)`**, que es como abre hoy `clipboard-win` (sonda entre procesos: otro proceso pudo abrir, escribir y hasta borrar mientras se «retenía» la apertura; con una ventana propietaria real sí excluye). La #98 no se da por cerrada hasta decidir la corrección; ver «Hallazgo» en `docs/superpowers/specs/2026-11-20-portapapeles-windows-design.md`. `arboard` abre y cierra el portapapeles de Windows en cada operación y no deja mantenerlo abierto (verificado en `arboard` 3.7.0): «leer y luego vaciar» serían dos aperturas y otra aplicación podría copiar entre las dos, perdiendo su contenido. `clipboard-win` (la dependencia de `arboard` en Windows, declarada también como directa) da una guarda cuyo `Drop` cierra el portapapeles; mientras vive, ninguna otra aplicación puede abrirlo ni cambiarlo. `arboard` sigue escribiendo con las exclusiones. Si no se puede abrir o leer, no se borra nada |
| Portapapeles: la copia en la memoria de Windows no se borra | Verificado en `arboard` 3.7.0 y `clipboard-win` 5.4.3: al escribir, el UTF-8 se convierte a UTF-16 directamente dentro del bloque de memoria global que se entrega a Windows (sin búfer intermedio nuestro); esa memoria pasa a ser del sistema y **no se puede borrar**, solo vaciar el portapapeles. Si `SetClipboardData` falla, el bloque se libera con `GlobalFree` sin sobrescribirse. Lo que sí controlamos: el texto leído para comparar va en un `Zeroizing<Vec<u8>>` con una sola reserva (sin reasignaciones), y `deliver` copia la contraseña una vez a un `SecretString` de vida corta para cruzar al hilo trabajador. No hay primitivas propias; la memoria del secreto **no** se borra por completo: es defensa en profundidad |

## Fuera de alcance del MVP

Relay por internet, búsqueda desde la PC, sincronización completa, autollenado en Android, iOS y Mac.
