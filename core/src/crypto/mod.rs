//! Criptografía del núcleo: derivación de la llave maestra (Argon2id), la
//! llave de bóveda cifrada con XChaCha20-Poly1305 y el cifrado de cada parte
//! de una entrada con esa llave.
//!
//! Límite conocido: ni `zeroize` ni `secrecy` borran las copias que deja un
//! `Vec` al reubicarse en memoria, ni lo que ocurra del lado de Dart (la
//! contraseña maestra también vive allí, en una `String` que Dart no deja
//! borrar: lo de este módulo es defensa en profundidad y no cierra la
//! exposición completa). Por eso los búferes con llaves son de tamaño fijo y no
//! crecen. Las llaves viven en el heap (`SecretBox`), así que moverlas copia un
//! puntero.
//!
//! Lo que sí se borra (#54): la memoria de trabajo de Argon2id, de 64 a 128
//! MiB, que reserva y borra `kdf::derive_key` (la que reserva `argon2` se
//! libera sin borrar); y el `Blake2b512` donde `argon2` mete la contraseña
//! maestra, con su búfer de bloque y el estado `h`, porque `blake2` se declara
//! en `Cargo.toml` con la feature `zeroize`.
//!
//! Lo que sigue sin borrarse, y solo se cierra con una primitiva propia (que
//! `core/CLAUDE.md` prohíbe); se revisa al actualizar esas versiones:
//!
//! - el búfer interno del iterador `nfc()` (ver `normalize`);
//! - `full_out`, el arreglo de 64 bytes que `argon2` 0.6.0 deja en la pila de
//!   `blake2b_long` con el hash completo: sus 32 primeros bytes son la llave
//!   derivada, y con una salida de 32 bytes ninguna ruta de la API lo evita;
//! - la llave que `chacha20poly1305` 0.11.0 guarda por valor dentro del
//!   cifrador y la subllave de `hchacha` que `chacha20` 0.10.2 deja en la pila
//!   al crear el XChaCha (por eso el cifrador se crea y se usa en la misma
//!   función: un esfuerzo razonable, sin garantía);
//! - las copias transitorias que el compilador deje en registros y en la pila
//!   durante la compresión de BLAKE2.

pub(crate) mod aead;
pub(crate) mod kdf;
pub(crate) mod keys;
pub(crate) mod normalize;
