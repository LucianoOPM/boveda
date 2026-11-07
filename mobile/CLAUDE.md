# mobile/ (app Android, Flutter)

Interfaz de la bóveda. Dart nunca hace criptografía ni guarda la bóveda: todo pasa por el núcleo a través del crate puente.

## Comandos (desde mobile/)

- Pruebas: `flutter test`
- Análisis: `flutter analyze`
- Regenerar el puente después de cambiar `rust/src/api/`: `flutter_rust_bridge_codegen generate`

## Reglas

- `rust/` es una fachada delgada: convierte tipos y llama a `vault-core`, sin lógica propia. Si necesitas lógica nueva, va en `core/`.
- No edites a mano `lib/src/rust/`; es código generado.
- Una contraseña solo cruza a Dart para mostrarla o copiarla localmente. No la guardes en estado global, en caché ni en logs, y descártala al salir de la pantalla.
- Al copiar localmente, marca el contenido como sensible (`ClipDescription.EXTRA_IS_SENSITIVE`, Android 13+). La API de portapapeles de Flutter no lo permite, así que se hace con un canal de plataforma en Kotlin.
- `FLAG_SECURE` en toda la app: sin capturas de pantalla y contenido oculto en apps recientes.
- El envío a la PC exige huella en cada envío. No agregues una opción para desactivarla.
- La app se bloquea al pasar a segundo plano y tras el tiempo de inactividad configurado.
- La contraseña maestra se pide cada 7 días aunque se use huella.
