//! Criptografía del núcleo: derivación de la llave maestra (Argon2id), la
//! llave de bóveda cifrada con XChaCha20-Poly1305 y el cifrado de cada parte
//! de una entrada con esa llave.
//!
//! Límite conocido: ni `zeroize` ni `secrecy` borran las copias que deja un
//! `Vec` al reubicarse en memoria, ni lo que ocurra del lado de Dart. Por eso
//! los búferes con llaves son de tamaño fijo y no crecen. Tampoco borran el
//! búfer interno del iterador `nfc()` (ver `normalize`).

pub(crate) mod aead;
pub(crate) mod kdf;
pub(crate) mod keys;
pub(crate) mod normalize;
