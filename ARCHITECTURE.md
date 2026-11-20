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
│   │   └── protocol/       # identity.rs, pairing.rs, message.rs, transport.rs
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
    ├── src/                # interfaz: ventana del QR y lista de dispositivos
    └── src-tauri/
        ├── Cargo.toml      # sin sección [workspace]: pertenece al de la raíz
        ├── tauri.conf.json
        └── src/
            ├── main.rs
            ├── tray.rs       # ícono y menú de la bandeja
            ├── server.rs     # escucha, mDNS, acepta solo emparejados
            ├── clipboard.rs  # escritura excluida del historial y borrado a 30 s
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
| `protocol/` | Identidades X25519, emparejamiento y envío con Noise (`snow`), descubrimiento mDNS |

Sin dependencias de interfaz ni de plataforma.

### mobile/ (Android, Flutter)

- `rust/`: crate puente expuesto con flutter_rust_bridge. Dart guarda un objeto opaco `VaultSession` (la ruta y un `Mutex<Option<Vault>>`) y le pide todas las operaciones; los errores cruzan como un enum plano `VaultError`, sin datos.
- `lib/src/rust/`: código generado; no se edita a mano.
- `lib/app.dart`: widget raíz que elige la pantalla según el estado de la sesión (sin bóveda, oferta de la huella, bloqueada, desbloqueada, error al abrir), sin go_router. Al desbloquear, muestra `SearchScreen` (`lib/screens/search_screen.dart`) con el total de entradas dañadas del `UnlockReport` para su aviso; la pantalla llama a `search` en cada cambio del texto y descarta las respuestas viejas. Bloquea al pasar la app a `hidden` y tras 5 minutos sin actividad (`defaultAutoLockTimeout`, en `lib/services/auto_lock.dart`): cierra todo lo apilado sobre la raíz y monta un desbloqueo nuevo, porque cada bloqueo cambia la `key` de la pantalla (`_epoch`). Toda transición a un estado con Rust desbloqueado pasa por `_enterUnlocked`, que bloquea si la operación terminó con la app oculta. Los toques llegan por un `Listener` en `MaterialApp.builder` y lo escrito por `ActivityTextField`, a través de `AutoLockScope`. Recibe la sesión, la regla de fortaleza y el servicio de huella inyectados, para que las pruebas de widgets usen dobles escritos a mano.
- `lib/screens/`, `lib/widgets/`: interfaz. Desde `SearchScreen` se apilan en el `Navigator` raíz el detalle (`EntryDetailScreen`) y el formulario (`EntryFormScreen`); al volver, la búsqueda se repite. El detalle no llama a Rust al abrirse: la contraseña y las notas se piden por separado, solo al tocar "mostrar", viven en el estado de esa pantalla y se descartan al ocultarlas, al abrir el formulario y al salir. "Copiar" pide la contraseña por separado y la mantiene solo en una variable local mientras cruza el canal sensible. El formulario envía los textos tal cual (el núcleo normaliza y valida) y solo manda la parte del secreto que el usuario abrió, así que la otra no sale de Rust. Tras crear o editar, la app lee lo guardado con `entry(id)`.
- `lib/services/`: ruta de la bóveda (`noBackupFilesDir`, por un canal de Kotlin), huella (canal `boveda/biometric` y los flujos de activar y desbloquear), portapapeles local (canal `boveda/clipboard`) y selector de archivos del respaldo (canal `boveda/backup`: Dart solo maneja rutas de temporales; la copia con la URI `content://` la hace Kotlin). Los respaldos se exportan e importan desde `SettingsScreen` (engranaje de la pantalla principal) y se restauran desde el primer uso; el selector corre dentro de `runWithoutAutoLock`, y un error al importar desde Ajustes bloquea la interfaz y muestra el motivo en el desbloqueo (aviso de un solo uso en `VaultGate`).
- `android/app/src/main/kotlin/com/jesus/boveda/`: `MainActivity.kt` marca la ventana con `FLAG_SECURE` en `onCreate` (sin capturas y sin contenido en recientes) y registra los canales; `BiometricVault.kt` guarda la copia de la llave de bóveda cifrada con una llave del Keystore que solo se usa con huella (`noBackupFilesDir/biometric/vault_key.bin`) y muestra `BiometricPrompt`; `cancelPrompt()`, llamado desde `MainActivity.onStop`, cancela el diálogo abierto y responde `canceled` de inmediato. `LocalClipboard.kt` marca contraseñas como sensibles y, a los 30 segundos, borra solo el clip que todavía conserve su token.

### desktop/ (Windows, Tauri 2)

- `src-tauri/src/`: bandeja, servidor de red local, portapapeles e identidad protegida con DPAPI.
- `src/`: interfaz mínima (ventana del QR y lista de dispositivos).

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
| Borrado de memoria garantizado solo dentro de `vault-core` | El núcleo guarda secretos y texto plano en `SecretString`/`Zeroizing` y evita reasignaciones que dejen copias; fuera del núcleo es de mejor esfuerzo, porque Dart no permite borrar sus cadenas. Lo mismo vale para los búferes de cruce de flutter_rust_bridge y de `StandardMessageCodec` (canales de Kotlin), para la copia que la fachada entrega al revelar una contraseña o unas notas, y para la copia de la llave de bóveda que entrega para el Keystore. Excepciones documentadas: (1) las copias internas de `nucleo-matcher` y `strsim` durante la búsqueda (datos de búsqueda, no secretos); (2) los búferes internos del iterador `nfc()` de `unicode-normalization` (de 4 elementos en la pila, o en el heap si un grupo de marcas combinantes es más largo): `tinyvec` no los borra, así que pueden quedar unos pocos caracteres de varios grupos anteriores, nunca la contraseña entera, solo con contraseñas maestras que no están en NFC o que contienen marcas combinantes o jamo (ASCII y casi todas las letras precompuestas no pasan por ellos). Se acepta porque las otras opciones eran una primitiva propia con tablas Unicode o dejar la #57 sin resolver; si la crate ofrece borrar ese búfer, se revisa; (3) las copias internas de las llaves y de la contraseña maestra en las dependencias de la derivación y el cifrado: `full_out`, un arreglo de 64 bytes en la pila de `blake2b_long` de `argon2` 0.6.0 con el hash completo (sus 32 primeros bytes son la llave derivada, y no se borra); la llave que `ChaChaPoly1305` de `chacha20poly1305` 0.11.0 guarda por valor dentro de la estructura (se borra al soltarse, pero un movimiento del cifrador puede dejar copias) y la subllave de `hchacha` que `chacha20` 0.10.2 deja en la pila al crear el XChaCha; y las copias transitorias que el compilador deje en registros y pila durante la compresión de BLAKE2. Se cierran solo con una primitiva propia, que `core/CLAUDE.md` prohíbe; se revisan al actualizar cualquiera de esas versiones. Lo que sí se borra: la llave derivada y la de bóveda viven en el heap (`SecretBox`); la memoria de trabajo de Argon2id (de 64 a 128 MiB) la reserva y la borra `derive_key`, porque la de `argon2` se libera sin borrar y con paralelismo 1 permite recalcular la llave derivada; y el `Blake2b512` donde `argon2` mete la contraseña maestra, con su búfer de bloque y su estado `h`, porque `blake2` está declarada en `core/Cargo.toml` con la feature `zeroize` (solo para activarla por unificación de features). Los ayudantes que devolvían el cifrador por valor se eliminaron, como esfuerzo razonable sin garantía. La contraseña maestra también vive en la memoria de Dart, que no se puede borrar: nada de esto cierra la exposición completa, es defensa en profundidad |
| Entrada cifrada en dos partes | Búsqueda rápida sin exponer contraseñas en memoria |
| Fecha de modificación y marca de eliminada | Preparan la sincronización futura sin migrar datos |
| Noise (`snow`) | Protocolo estudiado para el intercambio de llaves y el canal cifrado |
| Tauri 2 | Bandeja del sistema integrada y espacio para una interfaz futura |

## Fuera de alcance del MVP

Relay por internet, búsqueda desde la PC, sincronización completa, autollenado en Android, iOS y Mac.
