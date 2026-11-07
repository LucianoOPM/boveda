# Bóveda

Gestor de contraseñas personal: bóveda cifrada en Android y envío de contraseñas al portapapeles de Windows por la red local. Monorepo con un núcleo en Rust compartido por las dos apps.

- Antes de crear archivos o mover código entre carpetas, lee `ARCHITECTURE.md`.
- El alcance y las decisiones de diseño están en `docs/especificacion.md`; el orden de trabajo, en `docs/roadmap.md`.
- Cada carpeta (`core/`, `mobile/`, `desktop/`) tiene su propio CLAUDE.md con reglas específicas.

## Comandos (desde la raíz)

- Pruebas de Rust: `cargo test --workspace`
- Lint: `cargo clippy --workspace --all-targets -- -D warnings`
- Formato: `cargo fmt --all`
- Dependencias: `cargo audit` y `cargo deny check`

## Reglas para todo el repositorio

- Nunca escribas secretos (contraseña maestra, contraseñas, llaves) en logs, mensajes de error, panics ni salidas de `Debug`. Los tipos que contienen secretos no derivan `Debug` ni `Display`; si lo necesitan, implementan una versión que oculte el valor.
- Nunca agregues al repositorio bóvedas reales ni llaves reales. Las bóvedas de `core/tests/fixtures/` son de ejemplo, con contraseñas de prueba.
- Toda la criptografía vive en `core/`. Si una tarea en `mobile/` o `desktop/` parece necesitarla, agrega una función al núcleo y llámala desde la app.
- No agregues dependencias criptográficas ni de red nuevas sin preguntarme primero.
- Trabaja dentro de la etapa actual del roadmap. No implementes funciones fuera del alcance del MVP (relay, sincronización, autollenado).
- Antes de dar una tarea por terminada, ejecuta las pruebas y el lint de arriba y confirma que pasan.
- Commits pequeños con Conventional Commits (`feat:`, `fix:`, `test:`, `refactor:`, `docs:`, `chore:`).

## Idioma

- Identificadores y nombres en el código: inglés.
- Comentarios, documentación y mensajes de commit: español.
