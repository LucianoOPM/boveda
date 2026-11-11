# mobile/ (app Android, Flutter)

Interfaz de la bóveda. Dart nunca hace criptografía ni guarda la bóveda: todo pasa por el núcleo a través del crate puente.

## Comandos (desde mobile/)

- Pruebas: `flutter test`
- Análisis: `flutter analyze`
- Regenerar el puente después de cambiar `rust/src/api/`: `flutter_rust_bridge_codegen generate`. Requiere `flutter_rust_bridge_codegen` 2.13.0 y `cargo-expand` 1.0.127, instalados con `cargo install --version <versión> --locked`, para que local y CI coincidan.
- Compilar para Android (`flutter run`, `flutter build apk`): cargokit compila con el toolchain `stable`, no con `rust-toolchain.toml` (ver #48). Localmente, `stable` debe ser la misma versión que el `channel` fijado: compruébalo con `rustup run stable rustc --version` antes de compilar para el celular.

## Reglas

- `rust/` es una fachada delgada: convierte tipos y llama a `vault-core`, sin lógica propia. Si necesitas lógica nueva, va en `core/`.
- No edites a mano `lib/src/rust/`; es código generado. El codegen no borra los `.dart` de un archivo de `rust/src/api/` eliminado: bórralos tú. Y si cambias un nombre en `api/`, regenera antes de `cargo test`, porque `frb_generated.rs` todavía usa el anterior.
- Ninguna función de `rust/src/api/` que toque la bóveda lleva `#[frb(sync)]`. Solo dos lo llevan: `VaultSession::new`, que no toca la bóveda ni el disco, y `password_strength`, que no toca la `Vault` ni el `Mutex` y es instantánea. Una función síncrona corre en el hilo de la interfaz: si espera el `Mutex` de la sesión mientras otra corre Argon2id, congela la app alrededor de medio segundo.
- Evita en `api/` nombres que sean palabras reservadas de Dart (`export`, `import`...): frb les agrega `_`.
- Una contraseña solo cruza a Dart para mostrarla o copiarla localmente. No la guardes en estado global, en caché ni en logs, y descártala al salir de la pantalla. Lo mismo vale para las notas, que son parte del secreto. Para cambiar solo una de las dos, `edit` recibe solo esa (`newPassword` o `newNotes`): nunca reveles la otra para devolverla.
- Una contraseña o unas notas reveladas se muestran con `Text`, nunca con `SelectableText` ni en un campo de solo lectura: seleccionar permitiría copiarlas sin la marca de sensible.
- Los formularios de entradas no recortan, no validan vacíos ni límites y no quitan duplicados: envían los textos tal cual y muestran en su campo el error que devuelve el núcleo, sin números. La regla de la contraseña maestra (mínimo y fortaleza) no se aplica a la contraseña de una entrada.
- Al copiar localmente, marca el contenido como sensible (`ClipDescription.EXTRA_IS_SENSITIVE`, Android 13+). La API de portapapeles de Flutter no lo permite, así que se hace con un canal de plataforma en Kotlin.
- `FLAG_SECURE` en toda la app: sin capturas de pantalla y contenido oculto en apps recientes.
- El envío a la PC exige huella en cada envío. No agregues una opción para desactivarla.
- La app se bloquea al pasar a segundo plano y tras el tiempo de inactividad configurado.
- En las pantallas de la bóveda, todo campo de texto es `ActivityTextField` o `PasswordField`: escribir cuenta como actividad para el bloqueo por inactividad (el teclado en pantalla no genera eventos globales). Tocar fuera de un campo cierra el teclado: los controles que forman parte del formulario (botón de envío, casillas, íconos fuera del campo) se envuelven en `TextFieldTapRegion` para no cerrarlo, y toda lista desplazable de las pantallas de la bóveda usa `keyboardDismissBehavior: onDrag`.
- Toda operación asíncrona que termina con Rust desbloqueado pasa por `_enterUnlocked` de `VaultGate`, que bloquea si la app está oculta. Las pantallas de la bóveda se apilan en el `Navigator` raíz, que el bloqueo vacía sin animación (`removeRoute`): un `Navigator` anidado propio no se limpiaría.
- La contraseña maestra se pide cada 7 días aunque se use huella.
