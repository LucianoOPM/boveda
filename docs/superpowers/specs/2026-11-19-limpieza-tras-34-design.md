# Diseño: limpieza menor tras la #34 (issue #55)

Fecha: 2026-11-19 · Etapa 2 · Rama `chore/55-limpieza-tras-34`, sobre `fix/54-llave-derivada-en-heap`

## Objetivo

Dos hallazgos menores de la revisión final de la #34 (PR #52). No cambia el comportamiento ni el formato.

### Fuera de alcance

- Una variante nueva de `Error`: tocaría el enum público, el mapeo de la fachada (`maps_every_core_error`) y la capa de Dart para un caso que no ocurre.
- Regenerar `vault-v2.db` o cualquier bóveda de ejemplo.

## Decisiones (aprobadas por el usuario)

1. **`seal_key_check`** (`core/src/crypto/keys.rs`): sus dos errores inalcanzables (crear el cifrador con una llave de 32 bytes y cifrar un texto vacío) pasan a `Error::MalformedEncryptedKey`, el mismo que usa `encrypt_vault_key` cuando falla el cifrado. Antes eran `WrongVaultKey` y `Random`: llamado desde `create`, la migración o el cambio de contraseña, un `WrongVaultKey` sugería una llave incorrecta que nadie ingresó. `WrongVaultKey` queda solo donde es la respuesta real (`verify_key_check`, `unlock_with_key`). `Random` se conserva para el nonce, que sí puede fallar.
2. **`fill_fixture`** (`core/src/vault/tests.rs`): las notas de la bóveda de ejemplo vuelven a escribirse como `"Línea uno\nLínea dos"` y no con un salto de línea dentro de la cadena, que dependía del final de línea del archivo fuente. Con `* text=auto eol=lf` en `.gitattributes` el contenido es el mismo; `vault-v2.db` ya está generada y no se toca.

## Sin prueba nueva

Los dos errores de la decisión 1 no se pueden provocar: la llave siempre mide 32 bytes y cifrar un texto vacío no falla. Forzarlos exigiría inyectar fallos en el cifrador solo para probar un caso inalcanzable. La decisión 2 no cambia ningún valor. La garantía es que las pruebas existentes (`seal_key_check`, la bóveda de ejemplo y la cabecera) siguen pasando sin cambios.

## Orden respecto a la #54

El PR #74 (la #54) reescribe `keys.rs` y elimina `vault_key_cipher`. Esta rama parte de la de la #54 para no chocar; su PR se abre contra `dev` cuando el #74 esté mergeado y la rama se haya actualizado.

## Approaches

1. **Reusar `MalformedEncryptedKey`** (elegida): sin API nueva, coherente con `encrypt_vault_key`.
2. Variante nueva de `Error`: más clara, pero con costo en la fachada y en Dart.
3. Dejarlo como está: mantiene un error engañoso.

## Criterios de aceptación

- [x] Ambas casillas resueltas, sin cambios de comportamiento.
- [ ] `cargo test -p vault-core` y `clippy` pasan, y la CI está en verde.
