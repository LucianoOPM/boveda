# core/ (vault-core)

Núcleo compartido: criptografía, formato de la bóveda, búsqueda y protocolo. Un error aquí puede dejar al usuario sin acceso a sus contraseñas, así que este crate es el más estricto del repositorio.

## Pruebas primero

- Toda función nueva empieza con una prueba que falla; después el código mínimo que la hace pasar.
- Ejecuta `cargo test -p vault-core` antes de cada commit.
- Las pruebas unitarias van en un archivo aparte: `modulo.rs` termina con `#[cfg(test)] mod tests;` y las pruebas viven en `modulo/tests.rs` (con `use super::*;`). Las pruebas de la API pública van en `core/tests/`.

## Criptografía

- Usa solo estos crates: `argon2`, `chacha20poly1305` (`XChaCha20Poly1305`), `snow`, `x25519-dalek`, `zeroize`, `secrecy` y `getrandom` como generador del sistema. No implementes primitivas criptográficas propias.
- Argon2id calibrado para tardar unos 0.5 s en el dispositivo, sin bajar nunca de 64 MiB de memoria, 3 iteraciones y paralelismo 1. Los parámetros usados se guardan en la cabecera de la bóveda.
- Cada operación de cifrado usa un nonce aleatorio nuevo de 24 bytes. Nunca reutilices ni derives nonces.
- Cada parte cifrada de una entrada usa como dato autenticado el identificador de la entrada y su versión de estructura.
- Los secretos que cruzan la API pública (contraseña maestra, llave de bóveda, contraseña y notas) van en tipos de `secrecy` (`SecretBox`, `SecretString`) y se leen solo con `expose_secret()`. Los búferes internos van en `Zeroizing<...>` o en tipos que implementen `ZeroizeOnDrop`.

## Formato de la bóveda y del protocolo

- Cualquier cambio en el formato de la bóveda o de los mensajes del protocolo sube su número de versión, incluye una migración y agrega una bóveda de ejemplo nueva en `tests/fixtures/`.
- Nunca modifiques ni borres las bóvedas de ejemplo existentes: son la garantía de que las bóvedas antiguas siguen abriendo.

## API

- Sin dependencias de interfaz ni de plataforma (nada de Flutter, Tauri, Android ni Windows).
- La API pública no hace panic: devuelve `Result` con errores tipados (`thiserror`) que nunca contienen secretos.
- Lo que usan las apps se expone desde los módulos `vault` y `protocol`; el resto es privado al crate.

## Valores fijos de la especificación

- Tiempo de vida de una contraseña enviada: 30 s.
- Caducidad del código de emparejamiento: 2 minutos.
