//! Criptografía del núcleo: derivación de la llave maestra (Argon2id) y la
//! llave de bóveda cifrada con XChaCha20-Poly1305.
//!
//! Límite conocido: ni `zeroize` ni `secrecy` borran las copias que deja un
//! `Vec` al reubicarse en memoria, ni lo que ocurra del lado de Dart. Por eso
//! los búferes con llaves son de tamaño fijo y no crecen.

pub(crate) mod kdf;
pub(crate) mod keys;
