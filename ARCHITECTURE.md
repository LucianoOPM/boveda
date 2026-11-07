# Arquitectura

Gestor de contraseñas personal. La bóveda vive en el celular Android; una app de Windows recibe contraseñas por la red local y las deja en el portapapeles. Todo lo crítico (cifrado, formato de la bóveda, búsqueda y protocolo) está escrito una sola vez en Rust y lo usan ambas apps.

Documentos relacionados: `docs/especificacion.md` (qué y por qué) y `docs/roadmap.md` (en qué orden).

## Mapa del repositorio

La estructura de la raíz y de `core/` es la definitiva y debe seguirse desde ahora. La de `mobile/` y `desktop/` es la esperada: la definitiva la generan `flutter create`, `flutter_rust_bridge_codegen integrate` y el creador de Tauri al iniciar las etapas 2 y 3. Si lo generado difiere de este mapa, manda lo generado y se actualiza este archivo.

```
boveda/
├── Cargo.toml              # workspace: core, mobile/rust, desktop/src-tauri
├── Cargo.lock
├── deny.toml               # reglas de cargo deny
├── .gitignore
├── .github/workflows/ci.yml
├── .claude/                # configuración de Claude Code (settings, skills, rules)
├── CLAUDE.md
├── ARCHITECTURE.md
├── docs/
│   ├── especificacion.md
│   └── roadmap.md
│
├── core/                   # crate vault-core (definitivo)
│   ├── Cargo.toml
│   ├── CLAUDE.md
│   ├── src/
│   │   ├── lib.rs
│   │   ├── crypto/         # kdf.rs (Argon2), keys.rs, aead.rs
│   │   ├── vault/          # mod.rs (API), entry.rs, storage.rs, backup.rs
│   │   ├── search.rs
│   │   └── protocol/       # identity.rs, pairing.rs, message.rs, transport.rs
│   └── tests/
│       ├── vault.rs
│       ├── protocol.rs
│       └── fixtures/       # bóvedas de ejemplo fijas
│
├── mobile/                 # app Flutter (esperado, etapa 2)
│   ├── CLAUDE.md
│   ├── pubspec.yaml
│   ├── flutter_rust_bridge.yaml
│   ├── rust/               # crate puente rust_lib_boveda: fachada delgada sobre vault-core
│   │   ├── Cargo.toml      # edición 2021 por el código generado
│   │   └── src/
│   │       ├── api/        # funciones expuestas a Dart (un archivo por área)
│   │       └── frb_generated.rs  # generado; no se edita
│   ├── rust_builder/       # generado por flutter_rust_bridge; no se edita
│   ├── integration_test/   # pruebas en dispositivo que cargan la librería de Rust
│   ├── lib/
│   │   ├── main.dart
│   │   ├── src/rust/       # código generado por flutter_rust_bridge; no se edita
│   │   ├── screens/        # desbloqueo, búsqueda, detalle, edición, ajustes
│   │   ├── widgets/
│   │   └── services/       # huella, Keystore, selector de archivos
│   └── android/
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
| `crypto/` | Argon2id, llave de bóveda, cifrado XChaCha20-Poly1305 |
| `vault/` | API de la bóveda, modelo de entrada, almacenamiento SQLite, respaldos |
| `search.rs` | Búsqueda tolerante a errores sobre los datos de búsqueda |
| `protocol/` | Identidades X25519, emparejamiento y envío con Noise (`snow`), descubrimiento mDNS |

Sin dependencias de interfaz ni de plataforma.

### mobile/ (Android, Flutter)

- `rust/`: crate puente expuesto con flutter_rust_bridge.
- `lib/src/rust/`: código generado; no se edita a mano.
- `lib/screens/`, `lib/widgets/`: interfaz.
- `lib/services/`: huella, Keystore, selector de archivos.

### desktop/ (Windows, Tauri 2)

- `src-tauri/src/`: bandeja, servidor de red local, portapapeles e identidad protegida con DPAPI.
- `src/`: interfaz mínima (ventana del QR y lista de dispositivos).

## Flujos principales

**Desbloqueo.** La contraseña maestra pasa por Argon2id y descifra la llave de bóveda; con huella, el Keystore libera una copia cifrada de esa misma llave.

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
| Entrada cifrada en dos partes | Búsqueda rápida sin exponer contraseñas en memoria |
| Fecha de modificación y marca de eliminada | Preparan la sincronización futura sin migrar datos |
| Noise (`snow`) | Protocolo estudiado para el intercambio de llaves y el canal cifrado |
| Tauri 2 | Bandeja del sistema integrada y espacio para una interfaz futura |

## Fuera de alcance del MVP

Relay por internet, búsqueda desde la PC, sincronización completa, autollenado en Android, iOS y Mac.
