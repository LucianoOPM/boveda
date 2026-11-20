//! Envoltura mínima de DPAPI (`CryptProtectData` y `CryptUnprotectData`) de
//! **alcance de usuario**: solo la misma cuenta de Windows, en el mismo equipo,
//! puede descifrar lo protegido. Es el único sitio con `unsafe` de DPAPI.
//!
//! La entropía que se pasa **no es un secreto**: solo separa los usos (un blob
//! protegido para la identidad no descifra como si fuera la lista). DPAPI no
//! protege contra otro proceso del mismo usuario.

use std::{fmt, ptr};

use windows_sys::Win32::{
    Foundation::LocalFree,
    Security::Cryptography::{
        CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
    },
};
use zeroize::Zeroizing;

/// Fallo de DPAPI. No lleva datos: solo el código de error de Windows.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct DpapiError {
    code: u32,
}

impl DpapiError {
    /// Código de `GetLastError` (0 si el fallo fue de la envoltura, por ejemplo
    /// una entrada demasiado grande).
    pub fn code(&self) -> u32 {
        self.code
    }
}

impl fmt::Debug for DpapiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DpapiError({})", self.code)
    }
}

impl fmt::Display for DpapiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "DPAPI no pudo completar la operación (código {})",
            self.code
        )
    }
}

impl std::error::Error for DpapiError {}

fn blob_of(bytes: &[u8]) -> Result<CRYPT_INTEGER_BLOB, DpapiError> {
    let len = u32::try_from(bytes.len()).map_err(|_| DpapiError { code: 0 })?;
    Ok(CRYPT_INTEGER_BLOB {
        cbData: len,
        // Windows no escribe en la entrada; el tipo pide `*mut` por la FFI.
        pbData: bytes.as_ptr().cast_mut(),
    })
}

fn last_error() -> DpapiError {
    DpapiError {
        code: std::io::Error::last_os_error()
            .raw_os_error()
            .map_or(0, |c| c as u32),
    }
}

/// Protege `plain` para la cuenta actual. No muestra ninguna ventana.
pub fn protect(plain: &[u8], entropy: &[u8]) -> Result<Vec<u8>, DpapiError> {
    let input = blob_of(plain)?;
    let entropy = blob_of(entropy)?;
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: ptr::null_mut(),
    };
    // SAFETY: `input` y `entropy` apuntan a segmentos vivos durante la llamada y
    // con la longitud indicada; `output` es un destino válido. Sin descripción,
    // ni reservado, ni estructura de aviso. Sin `CRYPTPROTECT_LOCAL_MACHINE`:
    // alcance de usuario.
    let ok = unsafe {
        CryptProtectData(
            &input,
            ptr::null(),
            &entropy,
            ptr::null(),
            ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if ok == 0 {
        return Err(last_error());
    }
    // Un búfer nulo no se puede convertir en segmento ni hay nada que liberar.
    if output.pbData.is_null() {
        return Ok(Vec::new());
    }
    // SAFETY: en éxito, `pbData` apunta a `cbData` bytes reservados por Windows.
    let blob =
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize) }.to_vec();
    // SAFETY: el búfer lo reservó Windows con `LocalAlloc` y es nuestro liberarlo.
    // Contiene solo el texto ya cifrado, así que no hace falta borrarlo.
    unsafe { LocalFree(output.pbData.cast()) };
    Ok(blob)
}

/// Descifra un blob de [`protect`] con la misma entropía y la misma cuenta.
/// El resultado se borra al soltarse.
pub fn unprotect(blob: &[u8], entropy: &[u8]) -> Result<Zeroizing<Vec<u8>>, DpapiError> {
    let input = blob_of(blob)?;
    let entropy = blob_of(entropy)?;
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: ptr::null_mut(),
    };
    // SAFETY: igual que en `protect`; no se pide la descripción (nulo).
    let ok = unsafe {
        CryptUnprotectData(
            &input,
            ptr::null_mut(),
            &entropy,
            ptr::null(),
            ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if ok == 0 {
        return Err(last_error());
    }
    // Un búfer nulo no se puede convertir en segmento ni hay nada que liberar.
    if output.pbData.is_null() {
        return Ok(Zeroizing::new(Vec::new()));
    }
    // SAFETY: en éxito, `pbData` apunta a `cbData` bytes reservados por Windows.
    let plain = unsafe { std::slice::from_raw_parts_mut(output.pbData, output.cbData as usize) };
    let copy = Zeroizing::new(plain.to_vec());
    // El búfer de Windows contiene el texto en claro: se sobrescribe antes de
    // liberarlo.
    zeroize::Zeroize::zeroize(plain);
    // SAFETY: búfer reservado por Windows con `LocalAlloc`.
    unsafe { LocalFree(output.pbData.cast()) };
    Ok(copy)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENTROPY: &[u8] = b"entropia de prueba";

    fn sample() -> Vec<u8> {
        (0u8..32)
            .map(|i| i.wrapping_mul(7).wrapping_add(3))
            .collect()
    }

    #[test]
    fn roundtrip_returns_original() {
        let plain = sample();
        let blob = protect(&plain, ENTROPY).unwrap();
        let back = unprotect(&blob, ENTROPY).unwrap();
        assert_eq!(back.as_slice(), plain.as_slice());
    }

    #[test]
    fn blob_does_not_contain_plaintext() {
        let plain = sample();
        let blob = protect(&plain, ENTROPY).unwrap();
        assert!(blob.windows(plain.len()).all(|w| w != plain.as_slice()));
    }

    #[test]
    fn wrong_entropy_fails() {
        let blob = protect(&sample(), ENTROPY).unwrap();
        assert!(unprotect(&blob, b"otra entropia").is_err());
    }

    #[test]
    fn flipped_byte_fails() {
        let mut blob = protect(&sample(), ENTROPY).unwrap();
        let mid = blob.len() / 2;
        blob[mid] ^= 0x01;
        assert!(unprotect(&blob, ENTROPY).is_err());
    }

    #[test]
    fn empty_and_truncated_blob_fail() {
        assert!(unprotect(b"", ENTROPY).is_err());
        let blob = protect(&sample(), ENTROPY).unwrap();
        assert!(unprotect(&blob[..blob.len() - 1], ENTROPY).is_err());
    }
}
