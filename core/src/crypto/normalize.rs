//! Normalización NFC de la contraseña maestra (#57).
//!
//! Un teclado puede escribir `é` compuesta (U+00E9) o `e` más un acento
//! combinante (U+0065 U+0301). Son bytes distintos y Argon2id derivaría otra
//! llave, así que la contraseña se lleva a NFC en un solo punto antes de
//! derivar la llave (`kdf::derive_key`) y antes de contar su longitud
//! (`password_strength`). Es NFC y no NFKC: NFKC cambia el significado de
//! caracteres (`ﬁ` por `fi`).
//!
//! # Borrado de memoria
//!
//! - Una contraseña que ya está en NFC (todo el ASCII y las letras
//!   precompuestas) se usa tal cual: no se copia nada.
//! - Si no, la copia normalizada vive en un `Zeroizing<String>` con la
//!   capacidad exacta, calculada antes de llenarla, para que no haya
//!   reasignaciones que dejen restos del secreto en un búfer anterior.
//! - Excepción documentada: el iterador `nfc()` de `unicode-normalization`
//!   guarda un búfer interno de hasta 4 caracteres en la pila (en el heap si un
//!   grupo de marcas combinantes es más largo) y no se puede borrar desde
//!   fuera. Solo se usa con contraseñas que no están en NFC o que contienen
//!   caracteres cuya comprobación rápida no es concluyente (marcas
//!   combinantes y algunos jamo); lo que queda es el grupo de caracteres que
//!   se estaba componiendo, no la contraseña entera. Si la crate ofrece
//!   borrar ese búfer, esta excepción se revisa.

use std::ops::Deref;

use secrecy::{ExposeSecret, SecretString};
use unicode_normalization::{UnicodeNormalization, is_nfc};
use zeroize::Zeroizing;

/// Contraseña en forma NFC.
///
/// No deriva `Debug` ni `Display`: contiene la contraseña.
pub(crate) enum NormalizedPassword<'a> {
    /// Ya estaba en NFC: se usa el texto original, sin copiarlo.
    Borrowed(&'a str),
    /// Copia normalizada; se borra de memoria al soltarla.
    Owned(Zeroizing<String>),
}

impl Deref for NormalizedPassword<'_> {
    type Target = str;

    fn deref(&self) -> &str {
        match self {
            Self::Borrowed(text) => text,
            Self::Owned(text) => text,
        }
    }
}

/// Lleva `password` a NFC.
///
/// Devuelve el texto original si ya está en NFC; si no, una copia
/// normalizada que se borra al soltarla.
pub(crate) fn normalize_nfc(password: &SecretString) -> NormalizedPassword<'_> {
    let text = password.expose_secret();
    if is_nfc(text) {
        return NormalizedPassword::Borrowed(text);
    }
    // Primera pasada: solo suma bytes, no guarda nada. Normalizar puede dar un
    // texto más largo que el original (U+0344 pasa de 2 a 4 bytes).
    let len: usize = text.nfc().map(char::len_utf8).sum();
    let mut copy = Zeroizing::new(String::with_capacity(len));
    copy.extend(text.nfc());
    NormalizedPassword::Owned(copy)
}

#[cfg(test)]
mod tests;
