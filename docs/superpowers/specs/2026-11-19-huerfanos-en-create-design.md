# Diseño: archivos huérfanos junto a una bóveda nueva (issue #58)

Fecha: 2026-11-19 · Etapa 2 · Rama `fix/58-huerfanos-en-create`

## Objetivo

La #58 pedía que `Storage::create` borrara los `-journal`, `-wal` y `-shm` huérfanos antes de crear la base, como ya hace `settle_dest` en `import`, por si SQLite aplicaba un diario o un WAL viejo sobre la bóveda recién creada. La issue pedía verificar el riesgo primero, con un diario y un WAL reales y no con bytes basura.

## Verificación del riesgo

Experimento temporal (no se commitea) con `Vault::create_with_params` y `KdfParams::MIN`, sobre una bóveda "vieja" real con una entrada y tres casos de restos junto a la ruta nueva, con la base ya borrada:

1. Un `-journal` caliente real (`make_hot_journal_copy`).
2. Un `-wal` válido sin checkpoint más un `-shm` (`copy_live_wal`).
3. Los tres archivos.

Resultado en los tres casos, en Windows con el SQLite empaquetado:

- `create` termina bien.
- `check_integrity` da `Ok`.
- La bóveda se abre con la contraseña nueva y no quedan datos de la vieja.
- SQLite borra por sí mismo el `-journal` y el `-wal`. Solo sobrevive el `-shm`, que es inofensivo: el modo de la bóveda no usa WAL y la integridad sale bien.

Por qué: `create_new` deja un archivo de 0 bytes y SQLite, al abrir una base de 0 bytes, borra el diario y el WAL que encuentre (no puede haber un diario caliente de una base vacía). `import` es distinto: reemplaza `dest` con un archivo con contenido, y allí el diario viejo sí sería caliente. La lógica es del núcleo de SQLite y no depende de la plataforma, pero solo se comprobó en Windows.

## Decisión (aprobada por el usuario)

No cambiar el código de producción. Agregar pruebas de regresión que fijan lo que SQLite hace hoy, para que una actualización de `rusqlite` o de SQLite que lo cambie se detecte en la CI.

### Fuera de alcance

- Borrar los tres archivos en `create` (defensa en profundidad): descartado, agrega código y puntos de fallo en el crate más estricto para un riesgo que no existe.
- Quitar el `-shm` sobrante.
- Cambiar `create` cuando la base ya existe (sigue dando `AlreadyExists`).

## Approaches

1. **Solo pruebas de regresión** (elegida): sin riesgo para el código y detecta un cambio futuro de SQLite.
2. Implementar el borrado igual, reutilizando `settle_dest`: defensa en profundidad, pero no corrige nada hoy y suma código.
3. Cerrar sin cambios: deja el comportamiento sin protección ante una actualización.

## Criterios de aceptación

- [ ] Tres pruebas (diario real, WAL real con `-shm`, los tres) que crean una bóveda junto a los restos y comprueban: `create` bien, integridad, desbloqueo con la contraseña nueva, sin entradas, y la contraseña vieja rechazada.
- [ ] Las pruebas pasan sin cambiar código de producción; no pretenden detectar el `-shm`.
- [ ] `cargo test --workspace`, `clippy`, `cargo audit` y `cargo deny check` pasan; CI en verde.
