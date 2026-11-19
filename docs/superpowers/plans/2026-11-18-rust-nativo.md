# Rust nativo en Android (#48) — plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Compilar la librería de Android con el toolchain de `rust-toolchain.toml` (integración `native-assets` de flutter_rust_bridge 2.13.0) y retirar cargokit y la comprobación de `stable`.

**Architecture:** `mobile/hook/build.dart` invoca `FlutterRustBridgeNativeAssetsBuilder`; `mobile/rust/rust-toolchain.toml` fija canal, componentes y los 3 targets de Android; la CI compara ese archivo con el de la raíz. Se elimina `mobile/rust_builder/`.

**Tech Stack:** Flutter 3.47.9, Dart ^3.13.7, `flutter_rust_bridge_hooks` y `native_toolchain_rust` (versiones exactas), Rust 1.100.0.

**Spec:** `docs/superpowers/specs/2026-11-18-rust-nativo-design.md`

## Global Constraints

- Identificadores en inglés; comentarios, documentación y commits en español, Conventional Commits con `(#48)`.
- Versiones **exactas** (sin `^`) de `flutter_rust_bridge_hooks` y `native_toolchain_rust` en `mobile/pubspec.yaml`; `pubspec.lock` en el commit.
- **No borrar** `mobile/rust_builder/` ni la comprobación de `stable` de `mobile.yml` hasta que `flutter build apk` compile de verdad (Tarea 3). Durante la prueba solo se quita la dependencia del `pubspec.yaml`, no el directorio.
- Si falla la compilación de SQLite para Android: parar, evidencia y opciones para el usuario.
- Emulador: avisar antes de usarlo, no compilar a la vez y, al terminar, cerrar emulador, `adb` y Gradle (`./gradlew --stop`).
- Nunca encadenar `flutter analyze`, `flutter test` ni `cargo clippy` con `| grep` o `| tail` antes de un commit: usar un script que imprima los códigos de salida.
- Comandos de Flutter desde `mobile/`; en Git Bash, `adb` necesita `MSYS_NO_PATHCONV=1` y está en `$LOCALAPPDATA/Android/Sdk/platform-tools/adb.exe`.
- No cambiar la versión de Rust (1.100.0) ni editar `mobile/rust_builder/` ni el código generado a mano.

## Review Focus

1. El APK compilado con el hook usa el `rustc` de `rust-toolchain.toml` (Tarea 3) y trae las 3 ABIs; las de 64 bits con alineación de 16 KB.
2. La comprobación de la CI falla si difieren el canal o los componentes de los dos `rust-toolchain.toml` (Tarea 5).
3. Nada de cargokit queda referenciado (Tarea 4).

## Tarea 1: Línea base con cargokit

- [x] `flutter clean` y `flutter build apk --debug --target-platform android-arm,android-arm64,android-x64` con cargokit en `dev`; registrar tiempo y tamaño (`scratchpad/base_result.txt`).

## Tarea 2: Integrar el hook sin borrar cargokit

**Files:** `mobile/hook/build.dart` (nuevo), `mobile/rust/rust-toolchain.toml` (nuevo), `mobile/pubspec.yaml`, `mobile/pubspec.lock`

- [ ] Revisión del código de `native_toolchain_rust` 1.1.1 y `flutter_rust_bridge_hooks` 2.13.0 (hecha; ver el diseño).
- [ ] `mobile/rust/rust-toolchain.toml`: `channel = "1.100.0"`, `components = ["rustfmt", "clippy"]`, `targets` con `aarch64-linux-android`, `armv7-linux-androideabi` y `x86_64-linux-android`; comentario en español que explique por qué hay dos archivos.
- [ ] `mobile/hook/build.dart` como el generado por `integrate` (`FlutterRustBridgeNativeAssetsBuilder(cratePath: 'rust')`).
- [ ] `mobile/pubspec.yaml`: agregar `flutter_rust_bridge_hooks: 2.13.0` (sin `^`) y `native_toolchain_rust: 1.1.1` (sin `^`, porque se pidió fijarlo aunque sea transitiva) y **quitar** la dependencia `rust_lib_boveda: path: rust_builder` (el directorio se queda). `flutter pub get`.
- [ ] Commit solo cuando compile (Tarea 3), no antes.

## Tarea 3: Compilación real y evidencia

- [ ] `flutter build apk --debug --target-platform android-arm,android-arm64,android-x64` con el hook; registrar tiempo y tamaño.
- [ ] Confirmar en el log que `rustup run 1.100.0 cargo build` se ejecutó para los 3 targets y que `rustc` fue 1.100.0 (por ejemplo, `strings` de `.comment` o `rustc -vV` registrado por el hook).
- [ ] En el APK: las 3 ABIs (`lib/arm64-v8a`, `lib/armeabi-v7a`, `lib/x86_64`) con `librust_lib_boveda.so`; con `llvm-readelf -lW` del NDK, alineación `0x4000` de los segmentos `LOAD` en las de 64 bits.
- [ ] Si falla (SQLite con el NDK u otra causa): parar y traer la evidencia y las opciones.

## Tarea 4: Retirar cargokit

**Files:** `mobile/rust_builder/` (borrar), `mobile/analysis_options.yaml`, `.github/workflows/mobile.yml`, `mobile/CLAUDE.md`

- [ ] Solo tras la Tarea 3: borrar `mobile/rust_builder/` y su exclusión en `analysis_options.yaml` si existe; regenerar el puente (`flutter_rust_bridge_codegen generate`) y comprobar que no cambia nada generado.
- [ ] `mobile.yml`: quitar el paso "Stable de la imagen igual a rust-toolchain.toml" y los comentarios que hablan de cargokit; ajustar la ruta de la caché de Rust (el hook compila en la carpeta de salida del hook, no en `mobile/build/rust_lib_boveda/build`) y el paso del APK.
- [ ] `mobile/CLAUDE.md`: quitar la nota de `stable`; agregar las reglas (los dos paquetes se actualizan a mano con revisión de su código; los builds de depuración de Flutter compilan el Rust en release y por qué; hay dos `rust-toolchain.toml`).
- [ ] Commit: `chore: compilar la librería de Android con el toolchain fijado (#48)`.

## Tarea 5: Comprobación de los dos `rust-toolchain.toml` en la CI

- [ ] Paso en `mobile.yml` que compara `channel` y `components` (ordenados) de la raíz y de `mobile/rust/`, y falla con un mensaje claro.
- [ ] Probar el paso localmente con un archivo alterado (debe fallar) y sin alterar (debe pasar).
- [ ] `CLAUDE.md` de la raíz: subir Rust pasa a ser un commit de dos archivos (`rust-toolchain.toml` y `mobile/rust/rust-toolchain.toml`).
- [ ] Commit: `ci: comparar los dos rust-toolchain.toml (#48)`.

## Tarea 6: Documentación

- [ ] `ARCHITECTURE.md`: mapa de `mobile/` sin `rust_builder/` y con `hook/` y `rust/rust-toolchain.toml`; `docs/especificacion.md` si menciona cargokit.
- [ ] Commit: `docs: arquitectura sin cargokit (#48)`.

## Tarea 7: Pruebas en dispositivos y cierre

- [ ] `flutter analyze` y `flutter test`; `cargo test --workspace`, `clippy`, `cargo audit` y `cargo deny check`.
- [ ] Avisar al usuario, arrancar el emulador, instalar y abrir la app, y correr `integration_test/` en el emulador. Después, en el celular del usuario (pregunta antes de `flutter drive`: si la app instalada tiene datos). Cerrar emulador, `adb` y Gradle.
- [ ] Revisión final independiente (opus) y menú de cierre.
- [ ] PR contra `dev` con el formato del PR #11: el resumen de la revisión de `native_toolchain_rust`, el plan B (volver a cargokit o vendorizar), la evidencia de ABIs y 16 KB, la comparación de tamaño y tiempo con cargokit, y `Closes #48`. No abrirlo hasta que termine la CI de `dev`.
