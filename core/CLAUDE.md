# core/ (vault-core)

Núcleo compartido: criptografía, formato de la bóveda, búsqueda y protocolo. Un error aquí puede dejar al usuario sin acceso a sus contraseñas, así que este crate es el más estricto del repositorio.

## Pruebas primero

- Toda función nueva empieza con una prueba que falla; después el código mínimo que la hace pasar.
- Ejecuta `cargo test -p vault-core` antes de cada commit.
- Las pruebas unitarias van en un archivo aparte: `modulo.rs` termina con `#[cfg(test)] mod tests;` y las pruebas viven en `modulo/tests.rs` (con `use super::*;`). Las pruebas de la API pública van en `core/tests/`.

## Criptografía

- Usa solo estos crates: `argon2`, `chacha20poly1305` (`XChaCha20Poly1305`), `snow` (`default-features = false` con solo `use-curve25519`, `use-chacha20poly1305`, `use-blake2` y `use-getrandom`: sin `std`, que en 0.10.0 activa también `ring` y `sha2`), `x25519-dalek` (con las features `static_secrets` y `zeroize`, solo para derivar la llave pública desde los 32 bytes de la llave privada, que nunca se recortan ni se transforman), `zeroize`, `secrecy` y `getrandom` como generador del sistema, `unicode-normalization` (`=0.1.26`) solo para normalizar a NFC la contraseña maestra, y `blake2` (`=0.11.0`) solo para activar su feature `zeroize` por unificación de features (no se usa directamente; ver el comentario de `Cargo.toml`). No implementes primitivas criptográficas propias.
- La contraseña maestra se normaliza a NFC en un solo punto (`crypto/normalize.rs`), antes de Argon2id y antes de contar su longitud. No la proceses sin pasar por ese ayudante.
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
