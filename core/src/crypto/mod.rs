//! Criptografía del núcleo: derivación de la llave maestra (Argon2id), la
//! llave de bóveda cifrada con XChaCha20-Poly1305 y el cifrado de cada parte
//! de una entrada con esa llave.
//!
//! Límite conocido: ni `zeroize` ni `secrecy` borran las copias que deja un
//! `Vec` al reubicarse en memoria, ni lo que ocurra del lado de Dart. Por eso
//! los búferes con llaves son de tamaño fijo y no crecen. Las llaves viven en
//! el heap (`SecretBox`), así que moverlas copia un puntero. Tampoco se borran
//! el búfer interno del iterador `nfc()` (ver `normalize`), el arreglo
//! `full_out` de 64 bytes que `argon2` 0.6.0 deja en la pila de `blake2b_long`
//! ni la llave que `chacha20poly1305` 0.11.0 guarda por valor dentro del
//! cifrador (por eso el cifrador se crea y se usa en la misma función: un
//! esfuerzo razonable, sin garantía). Solo se cierran con una primitiva
//! propia, que `core/CLAUDE.md` prohíbe; se revisan al actualizar esas
//! versiones.

pub(crate) mod aead;
pub(crate) mod kdf;
pub(crate) mod keys;
pub(crate) mod normalize;
