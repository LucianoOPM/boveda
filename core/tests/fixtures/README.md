# Bóvedas de ejemplo

Bóvedas fijas que las pruebas de `core/tests/vault.rs` deben abrir siempre. Garantizan que una bóveda creada hoy siga abriendo en cualquier versión futura del núcleo: si un cambio altera el formato por accidente, estas pruebas fallan.

Son bóvedas de prueba, con contraseñas de prueba. Nunca agregues aquí bóvedas ni llaves reales.

## Reglas

- El número del nombre (`vault-vN.db`) es la `format_version` de la cabecera.
- Nunca se modifican ni se borran. Son la prueba de que las bóvedas antiguas siguen abriendo.
- Cada versión de formato nueva lleva un archivo nuevo (`vault-v2.db`, ...) y su propio generador, y se agrega una sección en este README.
- Las pruebas trabajan sobre una copia en un directorio temporal: abrir la bóveda fija sus pragmas y `reveal` escribe `last_used_at`, así que abrir el archivo del repositorio lo modificaría.

## `vault-v1.db`

- Contraseña maestra: `prueba-bóveda-v1` (con "ó").
- Argon2id con los mínimos de la especificación (`KdfParams::MIN`): 64 MiB, 3 iteraciones y paralelismo 1.
- Generador: `generate_fixture_v1`, en `core/src/vault/tests.rs`. Se ejecutó una sola vez con:

  ```
  cargo test -p vault-core --lib -- --ignored --exact vault::tests::generate_fixture_v1
  ```

  Nunca uses `--ignored` sin filtro (por ejemplo, para la prueba lenta de calibración): también ejecutaría el generador, que fallaría con `AlreadyExists` o, si falta el archivo, crearía una bóveda distinta.

  Si el archivo ya existe, el generador falla con `AlreadyExists` y no lo toca.

Contenido:

| Servicio | URL | Usuario | Etiquetas | Contraseña | Notas | Estado |
|---|---|---|---|---|---|---|
| `Correo Electrónico` | `https://correo.example.com/entrar` | `ana@example.com` | `Trabajo`, `Médico`, `Año` | `clave-completa-ñ` | `Línea uno` + salto de línea + `Línea dos` | viva, nunca usada |
| `Banco` | — | — | — | `clave-minima` | — | viva, nunca usada |
| `Nube` | — | — | — | `clave-nube` | — | viva, con `last_used_at` |
| `Eliminada` | — | — | — | `clave-eliminada` | — | lápida |

Los identificadores son aleatorios y las fechas son las de su generación; las pruebas buscan cada entrada por servicio.
