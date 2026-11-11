# Arquitectura

Gestor de contraseñas personal. La bóveda vive en el celular Android; una app de Windows recibe contraseñas por la red local y las deja en el portapapeles. Todo lo crítico (cifrado, formato de la bóveda, búsqueda y protocolo) está escrito una sola vez en Rust y lo usan ambas apps.

Documentos relacionados: `docs/especificacion.md` (qué y por qué) y `docs/roadmap.md` (en qué orden).

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
│   │   ├── crypto/         # kdf.rs (Argon2), keys.rs, aead.rs
│   │   ├── vault/          # mod.rs (API), header.rs, entry.rs, storage.rs, backup.rs, password.rs
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
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── api/        # expuesto a Dart, un archivo por área:
│   │       │               #   mod.rs, error.rs (VaultError), session.rs (VaultSession),
│   │       │               #   entries.rs (buscar, leer por id, agregar, editar,
│   │       │               #   eliminar y revelar), backup.rs y password.rs (fortaleza
│   │       │               #   de la contraseña maestra); pruebas en <área>/tests.rs
│   │       ├── test_support.rs   # ayudantes de pruebas (copia de vault-v2.db)
│   │       └── frb_generated.rs  # generado; no se edita
│   ├── rust_builder/       # generado por flutter_rust_bridge (cargokit); no se edita
│   ├── test/               # pruebas de widgets en la PC, con el puente simulado
│   ├── integration_test/   # pruebas en dispositivo que cargan la librería de Rust
│   ├── test_driver/        # punto de entrada para correr integration_test con flutter drive
│   ├── lib/
│   │   ├── main.dart       # arma las dependencias reales y arranca la app
│   │   ├── app.dart        # BovedaApp y VaultGate (la pantalla según el estado de la sesión, con la oferta de la huella tras el primer uso; guarda el total de dañadas del desbloqueo para el aviso; bloqueo automático al ocultarse y por inactividad; desbloqueada, muestra la búsqueda, y cada bloqueo descarta su texto); bootstrap: si el arranque falla, "No se pudo iniciar la app"
│   │   ├── src/rust/       # código generado por flutter_rust_bridge; no se edita
│   │   ├── screens/        # first_run_screen.dart, unlock_screen.dart (contraseña y huella), biometric_offer_screen.dart, search_screen.dart (pantalla principal: barra de búsqueda, lista filtrada por el núcleo y botón para crear), entry_detail_screen.dart (detalle: mostrar u ocultar la contraseña y las notas, eliminar), entry_form_screen.dart (crear, editar y reparar una entrada), placeholder_screen.dart (mensajes de error); futuro: ajustes
│   │   ├── widgets/        # activity_text_field.dart (campo que cuenta como actividad y cierra el teclado al tocar fuera; los controles del formulario van en `TextFieldTapRegion`), password_field.dart, tags_field.dart (etiquetas como chips), strength_indicator.dart, damaged_entries_banner.dart
│   │   └── services/       # vault_location.dart (ruta en noBackupFilesDir), auto_lock.dart (tiempo de bloqueo y alcance de la actividad), biometric_service.dart (canal de la huella), biometric_unlock.dart (activar y desbloquear con huella); futuro: selector
│   ├── android/            # app/src/main/kotlin/.../MainActivity.kt (canales; cancela la huella en onStop) y BiometricVault.kt (Keystore y huella)
│   └── build/              # ignorado; cargokit compila Rust en build/rust_lib_boveda/build
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
| `crypto/` | Argon2id (`kdf.rs`); generar la llave de bóveda, derivar la llave maestra y cifrar o descifrar con ella la llave de bóveda, y crear y comprobar el verificador `key_check` (`keys.rs`); cifrar y descifrar cada parte de una entrada con XChaCha20-Poly1305 y su dato autenticado (`aead.rs`) |
| `vault/` | API de la bóveda (`mod.rs`, estructura `Vault`); cabecera (formato 2, con `key_check`): codificarla como dato autenticado, leerla y validarla, y armar la v2 al desbloquear una v1 (`header.rs`); modelo de entrada (`entry.rs`) y entrada cifrada (`entry/sealed.rs`), almacenamiento SQLite con lápidas para las eliminadas y la migración de la cabecera v1 a v2 (`storage.rs`), respaldos: exportar con `VACUUM INTO` e importar validando una copia temporal (`backup.rs`), regla de la contraseña maestra: longitud mínima y fortaleza (`password.rs`) |
| `search.rs` | Búsqueda tolerante a errores sobre los datos de búsqueda |
| `protocol/` | Identidades X25519, emparejamiento y envío con Noise (`snow`), descubrimiento mDNS |

Sin dependencias de interfaz ni de plataforma.

### mobile/ (Android, Flutter)

- `rust/`: crate puente expuesto con flutter_rust_bridge. Dart guarda un objeto opaco `VaultSession` (la ruta y un `Mutex<Option<Vault>>`) y le pide todas las operaciones; los errores cruzan como un enum plano `VaultError`, sin datos.
- `lib/src/rust/`: código generado; no se edita a mano.
- `lib/app.dart`: widget raíz que elige la pantalla según el estado de la sesión (sin bóveda, oferta de la huella, bloqueada, desbloqueada, error al abrir), sin go_router. Al desbloquear, muestra `SearchScreen` (`lib/screens/search_screen.dart`) con el total de entradas dañadas del `UnlockReport` para su aviso; la pantalla llama a `search` en cada cambio del texto y descarta las respuestas viejas. Bloquea al pasar la app a `hidden` y tras 5 minutos sin actividad (`defaultAutoLockTimeout`, en `lib/services/auto_lock.dart`): cierra todo lo apilado sobre la raíz y monta un desbloqueo nuevo, porque cada bloqueo cambia la `key` de la pantalla (`_epoch`). Toda transición a un estado con Rust desbloqueado pasa por `_enterUnlocked`, que bloquea si la operación terminó con la app oculta. Los toques llegan por un `Listener` en `MaterialApp.builder` y lo escrito por `ActivityTextField`, a través de `AutoLockScope`. Recibe la sesión, la regla de fortaleza y el servicio de huella inyectados, para que las pruebas de widgets usen dobles escritos a mano.
- `lib/screens/`, `lib/widgets/`: interfaz. Desde `SearchScreen` se apilan en el `Navigator` raíz el detalle (`EntryDetailScreen`) y el formulario (`EntryFormScreen`); al volver, la búsqueda se repite. El detalle no llama a Rust al abrirse: la contraseña y las notas se piden por separado, solo al tocar "mostrar", viven en el estado de esa pantalla y se descartan al ocultarlas, al abrir el formulario y al salir. El formulario envía los textos tal cual (el núcleo normaliza y valida) y solo manda la parte del secreto que el usuario abrió, así que la otra no sale de Rust. Tras crear o editar, la app lee lo guardado con `entry(id)`.
- `lib/services/`: ruta de la bóveda (`noBackupFilesDir`, por un canal de Kotlin), huella (canal `boveda/biometric` y los flujos de activar y desbloquear), selector de archivos.
- `android/app/src/main/kotlin/com/jesus/boveda/`: `MainActivity.kt` registra los canales; `BiometricVault.kt` guarda la copia de la llave de bóveda cifrada con una llave del Keystore que solo se usa con huella (`noBackupFilesDir/biometric/vault_key.bin`) y muestra `BiometricPrompt`; `cancelPrompt()`, llamado desde `MainActivity.onStop`, cancela el diálogo abierto y responde `canceled` de inmediato.

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
| Borrado de memoria garantizado solo dentro de `vault-core` | El núcleo guarda secretos y texto plano en `SecretString`/`Zeroizing` y evita reasignaciones que dejen copias; fuera del núcleo es de mejor esfuerzo, porque Dart no permite borrar sus cadenas. Lo mismo vale para los búferes de cruce de flutter_rust_bridge y de `StandardMessageCodec` (canales de Kotlin), para la copia que la fachada entrega al revelar una contraseña o unas notas, y para la copia de la llave de bóveda que entrega para el Keystore. Excepción documentada: las copias internas de `nucleo-matcher` y `strsim` durante la búsqueda (datos de búsqueda, no secretos) |
| Entrada cifrada en dos partes | Búsqueda rápida sin exponer contraseñas en memoria |
| Fecha de modificación y marca de eliminada | Preparan la sincronización futura sin migrar datos |
| Noise (`snow`) | Protocolo estudiado para el intercambio de llaves y el canal cifrado |
| Tauri 2 | Bandeja del sistema integrada y espacio para una interfaz futura |

## Fuera de alcance del MVP

Relay por internet, búsqueda desde la PC, sincronización completa, autollenado en Android, iOS y Mac.
