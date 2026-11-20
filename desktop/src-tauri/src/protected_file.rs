//! Archivo protegido con DPAPI: `magic | versión (u16 LE) | cuerpo`, todo
//! cifrado con DPAPI de usuario y escrito de forma atómica.
//!
//! DPAPI autentica el contenido: un archivo alterado, truncado, vacío o copiado
//! a otra cuenta no descifra y se rechaza. Leer **nunca modifica** el archivo.

use std::{fmt, fs, io, path::Path};

use zeroize::Zeroizing;

use crate::{
    atomic_file::write_atomic,
    dpapi::{protect, unprotect},
};

/// Por qué no se pudo leer o escribir un archivo protegido. Sin datos del
/// archivo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtectedFileError {
    /// Error de entrada y salida distinto de «el archivo no existe».
    Io,
    /// DPAPI no pudo descifrar: archivo alterado, truncado, vacío, o de otra
    /// cuenta o equipo.
    Protection,
    /// Descifró, pero el magic o la longitud no son los esperados.
    Corrupt,
    /// Descifró, pero la versión de la estructura no es la esperada.
    UnknownVersion,
}

impl fmt::Display for ProtectedFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Io => "no se pudo leer o escribir el archivo",
            Self::Protection => "el archivo no se pudo descifrar con esta cuenta de Windows",
            Self::Corrupt => "el archivo está dañado",
            Self::UnknownVersion => "el archivo tiene una versión desconocida",
        })
    }
}

impl std::error::Error for ProtectedFileError {}

/// Protege `magic | version | body` con `entropy` y lo escribe en `path` de
/// forma atómica: si falla, el archivo anterior queda intacto.
pub fn write_protected(
    path: &Path,
    magic: &[u8],
    version: u16,
    body: &[u8],
    entropy: &[u8],
) -> Result<(), ProtectedFileError> {
    let mut plain = Zeroizing::new(Vec::with_capacity(magic.len() + 2 + body.len()));
    plain.extend_from_slice(magic);
    plain.extend_from_slice(&version.to_le_bytes());
    plain.extend_from_slice(body);
    let blob = protect(&plain, entropy).map_err(|_| ProtectedFileError::Protection)?;
    write_atomic(path, &blob).map_err(|_| ProtectedFileError::Io)
}

/// Lee y descifra `path`. `Ok(None)` **solo** si el archivo no existe; cualquier
/// otro problema es un error y el archivo no se toca. Devuelve el cuerpo sin la
/// cabecera.
pub fn read_protected(
    path: &Path,
    magic: &[u8],
    version: u16,
    entropy: &[u8],
) -> Result<Option<Zeroizing<Vec<u8>>>, ProtectedFileError> {
    let blob = match fs::read(path) {
        Ok(blob) => blob,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(ProtectedFileError::Io),
    };
    let plain = unprotect(&blob, entropy).map_err(|_| ProtectedFileError::Protection)?;
    let header = magic.len() + 2;
    if plain.len() < header || &plain[..magic.len()] != magic {
        return Err(ProtectedFileError::Corrupt);
    }
    let found = u16::from_le_bytes([plain[magic.len()], plain[magic.len() + 1]]);
    if found != version {
        return Err(ProtectedFileError::UnknownVersion);
    }
    Ok(Some(Zeroizing::new(plain[header..].to_vec())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    const MAGIC: &[u8] = b"boveda-prueba";
    const ENTROPY: &[u8] = b"entropia de prueba";
    const BODY: &[u8] = b"cuerpo-reconocible-0123456789";

    fn setup() -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("f.bin");
        (dir, path)
    }

    fn write_ok(path: &std::path::Path) {
        write_protected(path, MAGIC, 1, BODY, ENTROPY).unwrap();
    }

    fn assert_err_and_untouched(
        path: &std::path::Path,
        expected: ProtectedFileError,
        magic: &[u8],
        version: u16,
        entropy: &[u8],
    ) {
        let before = fs::read(path).unwrap();
        let err = read_protected(path, magic, version, entropy).unwrap_err();
        assert_eq!(err, expected);
        assert_eq!(fs::read(path).unwrap(), before, "el archivo no se toca");
    }

    #[test]
    fn roundtrip_returns_body() {
        let (_d, path) = setup();
        write_ok(&path);
        let body = read_protected(&path, MAGIC, 1, ENTROPY).unwrap().unwrap();
        assert_eq!(body.as_slice(), BODY);
    }

    #[test]
    fn missing_file_is_none() {
        let (_d, path) = setup();
        assert!(read_protected(&path, MAGIC, 1, ENTROPY).unwrap().is_none());
    }

    #[test]
    fn file_does_not_contain_body_or_magic() {
        let (_d, path) = setup();
        write_ok(&path);
        let bytes = fs::read(&path).unwrap();
        assert!(bytes.windows(BODY.len()).all(|w| w != BODY));
        assert!(bytes.windows(MAGIC.len()).all(|w| w != MAGIC));
    }

    #[test]
    fn empty_file_is_protection_error() {
        let (_d, path) = setup();
        fs::write(&path, b"").unwrap();
        assert_err_and_untouched(&path, ProtectedFileError::Protection, MAGIC, 1, ENTROPY);
    }

    #[test]
    fn flipped_byte_is_protection_error() {
        let (_d, path) = setup();
        write_ok(&path);
        let mut bytes = fs::read(&path).unwrap();
        let mid = bytes.len() / 2;
        bytes[mid] ^= 0x01;
        fs::write(&path, bytes).unwrap();
        assert_err_and_untouched(&path, ProtectedFileError::Protection, MAGIC, 1, ENTROPY);
    }

    #[test]
    fn truncated_is_protection_error() {
        let (_d, path) = setup();
        write_ok(&path);
        let bytes = fs::read(&path).unwrap();
        fs::write(&path, &bytes[..bytes.len() - 5]).unwrap();
        assert_err_and_untouched(&path, ProtectedFileError::Protection, MAGIC, 1, ENTROPY);
    }

    #[test]
    fn wrong_magic_is_corrupt() {
        let (_d, path) = setup();
        write_protected(&path, b"otro-magic", 1, BODY, ENTROPY).unwrap();
        assert_err_and_untouched(&path, ProtectedFileError::Corrupt, MAGIC, 1, ENTROPY);
    }

    #[test]
    fn too_short_plaintext_is_corrupt() {
        let (_d, path) = setup();
        // Descifra bien, pero no alcanza ni para el magic y la versión.
        let blob = crate::dpapi::protect(b"corto", ENTROPY).unwrap();
        fs::write(&path, blob).unwrap();
        assert_err_and_untouched(&path, ProtectedFileError::Corrupt, MAGIC, 1, ENTROPY);
    }

    #[test]
    fn unknown_version_is_unknown_version() {
        let (_d, path) = setup();
        write_protected(&path, MAGIC, 2, BODY, ENTROPY).unwrap();
        assert_err_and_untouched(&path, ProtectedFileError::UnknownVersion, MAGIC, 1, ENTROPY);
    }

    #[test]
    fn directory_path_is_io_error() {
        let dir = tempfile::tempdir().unwrap();
        let err = read_protected(dir.path(), MAGIC, 1, ENTROPY).unwrap_err();
        assert_eq!(err, ProtectedFileError::Io);
    }

    #[test]
    fn other_entropy_is_protection_error() {
        let (_d, path) = setup();
        write_ok(&path);
        assert_err_and_untouched(&path, ProtectedFileError::Protection, MAGIC, 1, b"otra");
    }

    #[test]
    fn failed_write_to_missing_dir_is_io_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("no-existe").join("f.bin");
        let err = write_protected(&path, MAGIC, 1, BODY, ENTROPY).unwrap_err();
        assert_eq!(err, ProtectedFileError::Io);
    }
}
