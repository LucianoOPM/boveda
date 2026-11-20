//! Huella de 64 bits para reconocer la contraseña sin guardarla.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};

/// Clave de las huellas de un gestor. Se crea una vez y vive lo que el gestor:
/// `DefaultHasher::new()` no usa una clave aleatoria, `RandomState` sí.
pub(super) struct FingerprintKey(RandomState);

/// Huella de un texto. Solo se guarda esto, nunca la contraseña. No implementa
/// `Debug` para que no acabe en un registro. El riesgo residual de colisión se
/// acepta: la comparación dura como mucho 30 s.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct Fingerprint(u64);

impl FingerprintKey {
    pub(super) fn new() -> Self {
        Self(RandomState::new())
    }

    pub(super) fn fingerprint(&self, bytes: &[u8]) -> Fingerprint {
        let mut hasher = self.0.build_hasher();
        hasher.write(bytes);
        Fingerprint(hasher.finish())
    }
}
