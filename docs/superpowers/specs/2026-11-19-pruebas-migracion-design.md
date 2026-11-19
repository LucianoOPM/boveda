# Diseño: pruebas más estrictas de la migración y de la bóveda de ejemplo v2 (issue #53)

Fecha: 2026-11-19 · Etapa 2 · Rama `test/53-pruebas-migracion`

## Objetivo

Dos pruebas de la #34 pasan aunque el código que deberían proteger estuviera mal. Se endurecen sin tocar código de producción, el formato ni las bóvedas de ejemplo.

1. **Migración fallida.** `failed_migration_keeps_v1_byte_for_byte` usa `deny_writes_for_test()` (`PRAGMA query_only`): la migración falla en su primer paso (`CREATE TABLE header_v2`), antes de escribir nada, así que una migración sin transacción también pasaría.
2. **Bóveda de ejemplo v2.** `fixture_v2_opens_with_known_contents` no prueba que el archivo sea v2: una v1 con la contraseña de la v2 también abriría, porque `unlock` la migraría en silencio.

### Fuera de alcance

- Cambiar el formato o la migración.
- Modificar `vault-v1.db` y `vault-v2.db`.

## Decisiones

- **Migración:** prueba nueva `failed_migration_rolls_back_after_dropping_the_old_header`, junto a la existente (que se conserva: cubre el fallo al empezar). A una copia de `vault-v1.db` se le crea `CREATE VIEW v AS SELECT * FROM header`. Con `legacy_alter_table` apagado, el `ALTER TABLE header_v2 RENAME TO header` falla, y eso ocurre después del `CREATE`, el `INSERT` y el `DROP`. Se comprueba: error de almacenamiento, bóveda bloqueada, archivo igual byte por byte, `format_version` en 1, la tabla `header` con su fila e `integrity_check` en `ok`.
- **Bóveda v2:** prueba nueva `fixture_v2_unlock_does_not_write` en `core/tests/vault.rs` (API pública): compara el archivo con `fs::read` antes y después de desbloquear; una v2 no escribe nada. Se hace en una prueba aparte y sin revelar secretos: `revealed` / `assert_fixture_contents` actualizan `last_used_at`, que es una escritura legítima (se vio al intentarlo dentro de la prueba existente).

## Verificación hecha

- **Mutación de la migración:** se confirmó la transacción a mitad de camino (`COMMIT` justo tras el `INSERT`). La prueba existente siguió pasando y la nueva falló. Se restauró `storage.rs` y no queda en el commit.
- **Detección de una v1:** la misma comparación aplicada a `vault-v1.db` con su contraseña da un archivo distinto tras desbloquear, así que distingue una v1 de una v2.

## Approaches

1. **Pruebas nuevas junto a las existentes** (elegida): no pierden la cobertura del fallo temprano y cada una dice qué protege.
2. Reemplazar las existentes: pierde el caso del fallo al empezar.
3. Ambas con la llave exportada (`unlock_with_key`) en vez de comparar bytes: más indirecto; la comparación es más simple y no depende de otra copia.

## Criterios de aceptación

- [x] La prueba de la migración falla si se quita la transacción (mutación) y pasa con el código actual.
- [x] La prueba de la v2 falla con una v1 (comprobado con `vault-v1.db`) y pasa con `vault-v2.db`.
- [x] Las bóvedas de ejemplo no cambian.
- [ ] `cargo test --workspace`, `clippy`, `cargo audit` y `cargo deny check` pasan; CI en verde.
