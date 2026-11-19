# Diseño: compilar la librería de Android con el toolchain de `rust-toolchain.toml` (issue #48)

Fecha: 2026-11-18 · Etapa 2 · Rama `chore/48-rust-nativo`

## Objetivo

Hoy cargokit compila la librería de Android con `rustup run stable cargo build` (`rust_builder/cargokit/build_tool/lib/src/builder.dart`) y solo acepta `stable`, `beta` o `nightly` (`enum Toolchain`, `options.dart`): el fijado de Rust de la #32 no cubre la compilación de Android. La librería del celular sale con el `stable` de la máquina o del runner, que cambia sin un commit. En la CI ya falló dos veces seguidas en sentidos opuestos (el `stable` de la imagen era 1.100.0 y luego 1.99.1 con `rust-toolchain.toml` fijando lo contrario), así que el parche de la comprobación de `stable` no es fiable.

La integración `native-assets` de flutter_rust_bridge 2.13.0 compila con un build hook de Dart que ejecuta `rustup run <channel del rust-toolchain.toml del crate> cargo build`. Con ella, la librería de Android sale siempre con la versión fijada.

### Fuera de alcance

- Cambiar la versión de Rust (commit propio de `rust-toolchain.toml`).
- Editar el código generado a mano.
- Firmar la app (etapa 4).

## Lo que confirma la documentación y el código

Context7 (flutter_rust_bridge 2.13.0, "Native assets" y "Migrate from Cargokit to Native Assets"), el proyecto que genera `flutter_rust_bridge_codegen create --integration-backend native-assets` (en una carpeta aparte) y la lectura del código de `native_toolchain_rust` 1.1.1 y `flutter_rust_bridge_hooks` 2.13.0:

- `native_toolchain_rust` lee `<crate>/rust-toolchain.toml` (**dentro del directorio del crate**, `mobile/rust/`), exige `toolchain.channel` fijo (rechaza `stable`, `beta`, `nightly`) y `toolchain.targets` con el target pedido. Ejecuta `rustup show active-toolchain` en el directorio del crate (que descarga el toolchain y sus targets) y después `rustup run <channel> cargo build --manifest-path ... --target ... --target-dir ...`.
- Para Android toma el compilador, el archivador y el NDK de la configuración que le pasa Flutter (`CCompilerConfig`) y fija `CC_*`, `CXX_*`, `AR_*`, `RANLIB_*` y `CARGO_TARGET_*_LINKER`. En `aarch64` y `x86_64` agrega `-C link-arg=-Wl,-z,max-page-size=16384` (16 KB) si no hay rustflags configuradas.
- Revisión de seguridad del paquete (para el PR):
  - Ejecuta solo `rustup` (en el `PATH` o `~/.cargo/bin/rustup`) con `show active-toolchain` y `run ... cargo build`. No hay otro `Process.run`/`Process.start`.
  - No hay código de red en ninguno de los dos paquetes. La única descarga es la de `rustup` (toolchain y targets del `rust-toolchain.toml`) y la de `cargo` (crates), igual que con cargokit.
  - Escribe solo en el directorio de salida del hook (carpeta de compilación), un archivo de bloqueo ahí, y en Windows un directorio corto `frb_native_assets_<hash>` en el temporal del sistema. El `--target-dir` es el del hook, o `CARGO_TARGET_DIR` si el usuario lo definió.
  - Lee, sin escribir, fuera del directorio del crate: archivos `.cargo/config.toml` de los ancestros del directorio actual y del crate y de `CARGO_HOME` (para decidir si agrega la bandera de 16 KB), el `.d` de la compilación y el `Cargo.lock` más cercano.
  - Pasa a `cargo` el entorno completo del proceso más las variables de la compilación, y **registra a nivel info el entorno completo** de cada invocación.
  - Ninguno de los dos paquetes define su propio `hook/build.dart` ni scripts de instalación: no ejecutan nada en quien los usa más allá del builder que invoca `hook/build.dart` del proyecto.
  - Dependencias transitivas: `hooks`, `code_assets`, `logging`, `meta` y `path` (publicadas por dart.dev, BSD-3) y `toml` (MIT).
  - Un único publicador (`gsconrad.com`), versión 1.1.1 de hace pocos días.
- `hook/build.dart` generado: `FlutterRustBridgeNativeAssetsBuilder(cratePath: 'rust')`, que compila siempre en `release` (los hooks no distinguen `debug` de `profile`).
- El código Dart generado por el puente no cambia de forma con este backend (mismo cargador).

## Decisiones (aprobadas por el usuario)

1. **Paquetes de Dart:** `flutter_rust_bridge_hooks` y `native_toolchain_rust` con **versión exacta** (sin `^`) en `mobile/pubspec.yaml`, y `pubspec.lock` en el commit. Actualizarlos es manual y exige repetir esta revisión (queda escrito en `mobile/CLAUDE.md`). Plan B si el paquete se abandona o no convence: volver a cargokit o vendorizar el paquete.
2. **Toolchains:** la raíz no cambia. `mobile/rust/rust-toolchain.toml` lleva el mismo canal, los mismos componentes y los 3 targets de Android (`aarch64-linux-android`, `armv7-linux-androideabi`, `x86_64-linux-android`, los de `abiFilters`). Una comprobación de la CI compara canal y componentes y falla si difieren. Subir Rust pasa a ser un commit de dos archivos.
3. **Modo de compilación:** `release` siempre, el valor por defecto. Los builds de depuración de Flutter compilan el Rust en release porque los hooks no distinguen `debug` de `profile` y la prueba de rendimiento de la #40 corre en profile; el costo es un build más lento (`lto = true`, `codegen-units = 1`). Queda anotado en `mobile/CLAUDE.md`.
4. **Instalación local:** los 3 targets de Android para Rust 1.100.0 y los paquetes de pub.
5. **Validación antes de borrar nada:** `mobile/rust_builder/` y la comprobación de `stable` se quedan hasta que `flutter build apk` compile de verdad. Si falla la compilación de SQLite para Android, se detiene el trabajo con evidencia y opciones. Además: las librerías de las 3 ABIs y la alineación de 16 KB en el APK, y la comparación con cargokit del tamaño del APK y del tiempo de compilación (no bloquea).

## Approaches

1. **`native-assets`** (elegida): respeta `rust-toolchain.toml`, quita el código vendorizado de cargokit y es el camino que documenta flutter_rust_bridge para migrar. Costo: dos paquetes de Dart nuevos con código de compilación en la caché de pub y dos `rust-toolchain.toml`.
2. **Dejar cargokit y arreglar la CI** (instalar el `stable` exacto en el runner): no resuelve la compilación local ni el APK firmado de la etapa 4.
3. **Vendorizar o bifurcar cargokit** para que acepte un canal fijo: mantenimiento propio de código ajeno.

## Criterios de aceptación

- [ ] La librería de Android se compila con la versión del `channel` de `rust-toolchain.toml`, comprobado en el log de la CI y localmente.
- [ ] La app compila, instala y abre en el emulador y en el celular, y `integration_test/` pasa en ambos.
- [ ] La comprobación de `stable` ya no está en `mobile.yml` ni la nota en `mobile/CLAUDE.md`; la CI compara canal y componentes de los dos `rust-toolchain.toml`.
- [ ] El APK trae las librerías de las 3 ABIs; las de 64 bits cumplen la alineación de 16 KB.
- [ ] `ARCHITECTURE.md`, `mobile/CLAUDE.md` y `CLAUDE.md` de la raíz coinciden con la nueva estructura y la regla de subir Rust.
- [ ] `flutter analyze`, `flutter test`, `cargo test --workspace`, `clippy`, `cargo audit` y `cargo deny check` pasan; CI en verde.
