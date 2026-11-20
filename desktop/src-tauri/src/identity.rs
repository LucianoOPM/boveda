//! Identidad X25519 de la PC. La llave privada vive en un archivo protegido con
//! DPAPI de usuario (nunca en claro) y se genera **solo** cuando el archivo no
//! existe.
//!
//! Si el archivo existe pero no se puede leer (alterado, truncado, de otra
//! cuenta, de versión desconocida), se devuelve el error y no se toca nada: los
//! celulares ya emparejados confían en esa llave y regenerarla en silencio los
//! dejaría huérfanos. Repararlo es una decisión del usuario.

use std::{fmt, fs, path::PathBuf};

use secrecy::{ExposeSecret, ExposeSecretMut, SecretBox};
use vault_core::protocol::KeyPair;

use crate::protected_file::{ProtectedFileError, read_protected, write_protected};

const MAGIC: &[u8] = b"boveda-pc-identity";
const VERSION: u16 = 1;
/// No es un secreto: solo separa este uso de los demás de DPAPI.
const ENTROPY: &[u8] = b"boveda-desktop identity v1";
const KEY_LEN: usize = 32;
/// Tope del archivo antes de leerlo: el sobre de DPAPI de 53 bytes ocupa unos
/// cientos; más que esto no es nuestro archivo y no se lee entero a memoria.
const MAX_FILE_BYTES: usize = 1024;

/// Por qué no se pudo cargar o crear la identidad. Sin datos de la llave.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityError {
    /// Error de entrada y salida distinto de «el archivo no existe».
    Io,
    /// DPAPI no pudo descifrar el archivo (alterado, truncado, de otra cuenta).
    Protection,
    /// Descifró, pero la estructura o la longitud no son las esperadas.
    Corrupt,
    /// Descifró, pero la versión de la estructura es desconocida.
    UnknownVersion,
    /// El generador de números aleatorios del sistema falló.
    Generation,
}

impl fmt::Display for IdentityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Io => "no se pudo leer o guardar la identidad de la PC",
            Self::Protection => "la identidad de la PC no se pudo descifrar con esta cuenta",
            Self::Corrupt => "el archivo de la identidad de la PC está dañado",
            Self::UnknownVersion => {
                "el archivo de la identidad de la PC tiene una versión desconocida"
            }
            Self::Generation => "no se pudo generar la identidad de la PC",
        })
    }
}

impl std::error::Error for IdentityError {}

impl From<ProtectedFileError> for IdentityError {
    fn from(e: ProtectedFileError) -> Self {
        match e {
            ProtectedFileError::Io => Self::Io,
            ProtectedFileError::Protection => Self::Protection,
            ProtectedFileError::Corrupt => Self::Corrupt,
            ProtectedFileError::UnknownVersion => Self::UnknownVersion,
        }
    }
}

/// Almacén de la identidad de la PC (un archivo).
#[derive(Debug)]
pub struct IdentityStore {
    path: PathBuf,
}

impl IdentityStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Carga la identidad; si el archivo **no existe**, genera una, la guarda y
    /// la devuelve. Cualquier otro problema es un error y no se modifica nada.
    pub fn load_or_create(&self) -> Result<KeyPair, IdentityError> {
        if let Some(body) = read_protected(&self.path, MAGIC, VERSION, ENTROPY, MAX_FILE_BYTES)? {
            if body.len() != KEY_LEN {
                return Err(IdentityError::Corrupt);
            }
            // La llave se conserva tal cual, sin validarla ni transformarla.
            let mut private = SecretBox::<[u8; KEY_LEN]>::default();
            private.expose_secret_mut().copy_from_slice(&body);
            return Ok(KeyPair::from_private(private));
        }

        let pair = KeyPair::generate().map_err(|_| IdentityError::Generation)?;
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|_| IdentityError::Io)?;
        }
        write_protected(
            &self.path,
            MAGIC,
            VERSION,
            pair.private().expose_secret(),
            ENTROPY,
        )?;
        Ok(pair)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use secrecy::ExposeSecret;
    use std::fs;

    fn setup() -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("identity.bin");
        (dir, path)
    }

    /// Falla con `expected`, deja el archivo igual byte por byte y una segunda
    /// llamada vuelve a fallar igual (no regenera).
    fn assert_fails_without_regenerating(path: &std::path::Path, expected: IdentityError) {
        let before = fs::read(path).unwrap();
        let store = IdentityStore::new(path.to_path_buf());
        assert_eq!(store.load_or_create().unwrap_err(), expected);
        assert_eq!(fs::read(path).unwrap(), before);
        assert_eq!(store.load_or_create().unwrap_err(), expected);
        assert_eq!(fs::read(path).unwrap(), before);
    }

    #[test]
    fn first_run_creates_file() {
        let (_d, path) = setup();
        IdentityStore::new(path.clone()).load_or_create().unwrap();
        assert!(path.is_file());
    }

    #[test]
    fn first_run_creates_missing_parent_dir() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub").join("identity.bin");
        IdentityStore::new(path.clone()).load_or_create().unwrap();
        assert!(path.is_file());
    }

    #[test]
    fn file_does_not_contain_private_key() {
        let (_d, path) = setup();
        let pair = IdentityStore::new(path.clone()).load_or_create().unwrap();
        let key = *pair.private().expose_secret();
        let bytes = fs::read(&path).unwrap();
        assert!(bytes.windows(32).all(|w| w != key));
        assert!(bytes.windows(MAGIC.len()).all(|w| w != MAGIC));
    }

    #[test]
    fn second_open_returns_same_keys() {
        let (_d, path) = setup();
        let first = IdentityStore::new(path.clone()).load_or_create().unwrap();
        let second = IdentityStore::new(path).load_or_create().unwrap();
        assert_eq!(first.public(), second.public());
        assert_eq!(
            first.private().expose_secret(),
            second.private().expose_secret()
        );
    }

    #[test]
    fn altered_file_errors_without_regenerating() {
        let (_d, path) = setup();
        IdentityStore::new(path.clone()).load_or_create().unwrap();
        let mut bytes = fs::read(&path).unwrap();
        let mid = bytes.len() / 2;
        bytes[mid] ^= 0x01;
        fs::write(&path, bytes).unwrap();
        assert_fails_without_regenerating(&path, IdentityError::Protection);
    }

    #[test]
    fn truncated_file_errors_without_regenerating() {
        let (_d, path) = setup();
        IdentityStore::new(path.clone()).load_or_create().unwrap();
        let bytes = fs::read(&path).unwrap();
        fs::write(&path, &bytes[..bytes.len() / 2]).unwrap();
        assert_fails_without_regenerating(&path, IdentityError::Protection);
    }

    #[test]
    fn empty_file_errors_without_regenerating() {
        let (_d, path) = setup();
        fs::write(&path, b"").unwrap();
        assert_fails_without_regenerating(&path, IdentityError::Protection);
    }

    #[test]
    fn directory_path_errors_without_regenerating() {
        let dir = tempfile::tempdir().unwrap();
        let store = IdentityStore::new(dir.path().to_path_buf());
        assert_eq!(store.load_or_create().unwrap_err(), IdentityError::Io);
        assert!(dir.path().is_dir());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn unknown_version_errors_without_regenerating() {
        let (_d, path) = setup();
        write_protected(&path, MAGIC, 2, &[7u8; 32], ENTROPY).unwrap();
        assert_fails_without_regenerating(&path, IdentityError::UnknownVersion);
    }

    #[test]
    fn wrong_length_body_errors_without_regenerating() {
        for len in [0usize, 31, 33] {
            let (_d, path) = setup();
            write_protected(&path, MAGIC, VERSION, &vec![7u8; len], ENTROPY).unwrap();
            assert_fails_without_regenerating(&path, IdentityError::Corrupt);
        }
    }

    #[test]
    fn devices_file_is_not_a_valid_identity() {
        let (_d, path) = setup();
        write_protected(
            &path,
            b"boveda-pc-devices",
            1,
            &[7u8; 32],
            b"boveda-desktop paired-devices v1",
        )
        .unwrap();
        assert_fails_without_regenerating(&path, IdentityError::Protection);
    }

    #[test]
    fn errors_do_not_show_key() {
        let (_d, path) = setup();
        let pair = IdentityStore::new(path).load_or_create().unwrap();
        let key = *pair.private().expose_secret();
        let hex: String = key.iter().map(|b| format!("{b:02x}")).collect();
        let dec = format!("{key:?}");
        for err in [
            IdentityError::Io,
            IdentityError::Protection,
            IdentityError::Corrupt,
            IdentityError::UnknownVersion,
            IdentityError::Generation,
        ] {
            for text in [format!("{err:?}"), err.to_string()] {
                assert!(!text.contains(&hex) && !text.contains(&dec));
            }
        }
    }
}
