# Copiar localmente marcado como sensible - Implementation Plan

**Goal:** Copiar contraseñas y usuarios desde el detalle; solo la contraseña
sale marcada como sensible y nunca queda en el estado de Dart (issue #42).

**Architecture:** `LocalClipboard.kt` administra el portapapeles y su borrado;
`PlatformLocalClipboard` cruza el canal; `EntryDetailScreen` revela la
contraseña justo antes de copiarla y recibe el servicio por inyección.

**Spec:** `docs/superpowers/specs/2026-11-12-copiar-localmente-design.md`.

## Restricciones

- Sin dependencias nuevas ni cambios en Rust o en el puente generado.
- Ningún secreto en logs, errores, estado global o tareas diferidas.
- Nunca borrar un clip posterior del usuario.
- Tras cada `await`, comprobar que la ruta del detalle siga activa.

### Task 1: canal y fachada

**Files:**

- Create: `mobile/lib/services/local_clipboard.dart`
- Create: `mobile/android/app/src/main/kotlin/com/jesus/boveda/LocalClipboard.kt`
- Modify: `mobile/android/app/src/main/kotlin/com/jesus/boveda/MainActivity.kt`
- Test: `mobile/test/local_clipboard_test.dart`

- [x] Probar `copySensitive`, `copyPlain`, feedback y errores genéricos.
- [x] Implementar los dos métodos del canal.
- [x] Marcar solo el clip sensible y programar el borrado condicional.
- [x] Reintentar un borrado vencido en `MainActivity.onWindowFocusChanged`
  (en `onResume` la ventana aún no tiene el foco y la lectura falla).

### Task 2: detalle y pruebas de widgets

**Files:**

- Modify: `mobile/lib/screens/entry_detail_screen.dart`
- Modify: `mobile/test/fakes.dart`
- Test: `mobile/test/entry_detail_screen_test.dart`

- [x] Agregar los botones de contraseña y usuario.
- [x] Revelar para cada copia sin guardar el resultado en `State`.
- [x] Evitar dobles toques y respuestas posteriores al bloqueo.
- [x] Cubrir feedback y todos los `VaultError` relevantes.

### Task 3: documentación y verificación

- [x] Actualizar especificación, arquitectura, reglas de mobile y roadmap.
- [x] Ejecutar `dart format`, `flutter analyze`, `flutter test` y APK debug.
- [x] Ejecutar pruebas, formato y clippy del workspace de Rust.
- [ ] Verificar manualmente en un emulador API 33+ y otro anterior.
