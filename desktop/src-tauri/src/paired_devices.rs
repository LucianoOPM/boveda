//! Lista de celulares emparejados de la PC: un archivo protegido **entero** con
//! DPAPI de usuario. Como DPAPI autentica el contenido, un archivo alterado,
//! truncado o copiado a otra cuenta no descifra y se rechaza; no hace falta otra
//! firma ni otra llave.
//!
//! Si el archivo existe y no se puede leer, se devuelve el error: **nunca** se
//! reemplaza por una lista vacía, para no perder los emparejamientos en
//! silencio. Cada cambio se guarda de forma atómica y solo si termina se
//! actualiza la lista en memoria.
//!
//! Fuera de la garantía (aceptado): otro proceso del mismo usuario y restaurar
//! una copia antigua válida del archivo.

use std::{fmt, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::protected_file::{ProtectedFileError, read_protected, write_protected};

const MAGIC: &[u8] = b"boveda-pc-devices";
const VERSION: u16 = 1;
/// No es un secreto: separa este uso de los demás de DPAPI.
const ENTROPY: &[u8] = b"boveda-desktop paired-devices v1";

/// Máximo de celulares emparejados.
pub const MAX_DEVICES: usize = 64;
/// Máximo del nombre, en bytes UTF-8 (igual que `vault::PairedPc`).
pub const MAX_NAME_BYTES: usize = 128;
/// Tope del cuerpo antes de deserializar: cada celular ocupa como máximo el
/// nombre más su longitud (2), la llave (32) y la fecha (10), más la cuenta.
const MAX_BODY_BYTES: usize = MAX_DEVICES * (MAX_NAME_BYTES + 2 + 32 + 10) + 8;

/// Tope del archivo antes de leerlo: el cuerpo máximo más el margen del sobre de
/// DPAPI (cientos de bytes). Más que esto no es nuestro archivo y no se lee
/// entero a memoria.
const MAX_FILE_BYTES: usize = MAX_BODY_BYTES + 1024;

/// Un celular emparejado. Se identifica por su llave pública, que es única.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairedDevice {
    /// Nombre, ya recortado, de 1 a 128 bytes UTF-8.
    pub name: String,
    /// Llave pública X25519 del celular.
    pub public_key: [u8; 32],
    /// Cuándo se emparejó, en milisegundos Unix (lo da quien llama).
    pub paired_at: i64,
}

// No muestra el nombre ni la llave: no deben salir en logs ni en errores.
impl fmt::Debug for PairedDevice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PairedDevice").finish_non_exhaustive()
    }
}

/// Por qué falló una operación de la lista. Sin datos de los celulares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairedDevicesError {
    /// Error de entrada y salida distinto de «el archivo no existe».
    Io,
    /// DPAPI no pudo descifrar el archivo (alterado, truncado, de otra cuenta).
    Protection,
    /// Descifró, pero la estructura o su contenido no son válidos.
    Corrupt,
    /// Descifró, pero la versión de la estructura es desconocida.
    UnknownVersion,
    /// Ya hay un celular con esa llave pública.
    AlreadyPaired,
    /// No hay ningún celular con esa llave pública.
    NotFound,
    /// El nombre está vacío o mide más de 128 bytes.
    InvalidName,
    /// Ya hay 64 celulares.
    TooMany,
}

impl fmt::Display for PairedDevicesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Io => "no se pudo leer o guardar la lista de celulares emparejados",
            Self::Protection => "la lista de celulares no se pudo descifrar con esta cuenta",
            Self::Corrupt => "el archivo de celulares emparejados está dañado",
            Self::UnknownVersion => {
                "el archivo de celulares emparejados tiene una versión desconocida"
            }
            Self::AlreadyPaired => "ese celular ya está emparejado",
            Self::NotFound => "ese celular no está emparejado",
            Self::InvalidName => "el nombre del celular debe tener de 1 a 128 bytes",
            Self::TooMany => "ya hay el máximo de celulares emparejados",
        })
    }
}

impl std::error::Error for PairedDevicesError {}

impl From<ProtectedFileError> for PairedDevicesError {
    fn from(e: ProtectedFileError) -> Self {
        match e {
            ProtectedFileError::Io => Self::Io,
            ProtectedFileError::Protection => Self::Protection,
            ProtectedFileError::Corrupt => Self::Corrupt,
            ProtectedFileError::UnknownVersion => Self::UnknownVersion,
        }
    }
}

/// Nombre recortado y comprobado.
fn clean_name(name: &str) -> Result<String, PairedDevicesError> {
    let name = name.trim();
    if name.is_empty() || name.len() > MAX_NAME_BYTES {
        return Err(PairedDevicesError::InvalidName);
    }
    Ok(name.to_owned())
}

/// Lista de celulares emparejados, respaldada por un archivo.
#[derive(Debug)]
pub struct PairedDeviceStore {
    path: PathBuf,
    devices: Vec<PairedDevice>,
}

impl PairedDeviceStore {
    /// Abre la lista. Archivo ausente → lista vacía (sin escribir nada). Si
    /// existe, debe descifrarse y validarse **por completo** o se devuelve el
    /// error; nunca se reemplaza por una lista vacía.
    pub fn open(path: PathBuf) -> Result<Self, PairedDevicesError> {
        let devices = match read_protected(&path, MAGIC, VERSION, ENTROPY, MAX_FILE_BYTES)? {
            None => Vec::new(),
            Some(body) => decode(&body)?,
        };
        Ok(Self { path, devices })
    }

    /// Los celulares, en orden de inserción.
    pub fn list(&self) -> &[PairedDevice] {
        &self.devices
    }

    /// Agrega un celular (el nombre se recorta). Si no se puede guardar, la
    /// lista queda como estaba.
    pub fn add(&mut self, mut device: PairedDevice) -> Result<(), PairedDevicesError> {
        device.name = clean_name(&device.name)?;
        if self
            .devices
            .iter()
            .any(|d| d.public_key == device.public_key)
        {
            return Err(PairedDevicesError::AlreadyPaired);
        }
        if self.devices.len() >= MAX_DEVICES {
            return Err(PairedDevicesError::TooMany);
        }
        let mut next = self.devices.clone();
        next.push(device);
        self.save(&next)?;
        self.devices = next;
        Ok(())
    }

    /// Quita el celular con esa llave pública. Si no se puede guardar, la lista
    /// queda como estaba.
    pub fn remove(&mut self, public_key: &[u8; 32]) -> Result<(), PairedDevicesError> {
        let Some(index) = self
            .devices
            .iter()
            .position(|d| &d.public_key == public_key)
        else {
            return Err(PairedDevicesError::NotFound);
        };
        let mut next = self.devices.clone();
        next.remove(index);
        self.save(&next)?;
        self.devices = next;
        Ok(())
    }

    fn save(&self, devices: &[PairedDevice]) -> Result<(), PairedDevicesError> {
        let body = postcard::to_allocvec(devices).map_err(|_| PairedDevicesError::Io)?;
        write_protected(&self.path, MAGIC, VERSION, &body, ENTROPY)?;
        Ok(())
    }
}

/// Deserializa y valida el cuerpo completo: tope de tamaño, sin bytes
/// sobrantes, nombres válidos, llaves únicas y a lo sumo 64 celulares.
fn decode(body: &[u8]) -> Result<Vec<PairedDevice>, PairedDevicesError> {
    if body.len() > MAX_BODY_BYTES {
        return Err(PairedDevicesError::Corrupt);
    }
    let (devices, rest) = postcard::take_from_bytes::<Vec<PairedDevice>>(body)
        .map_err(|_| PairedDevicesError::Corrupt)?;
    if !rest.is_empty() || devices.len() > MAX_DEVICES {
        return Err(PairedDevicesError::Corrupt);
    }
    for (i, device) in devices.iter().enumerate() {
        // Lo que escribimos siempre está recortado: otra cosa es una alteración.
        let valid_name = clean_name(&device.name).is_ok_and(|n| n == device.name);
        let unique = devices[..i]
            .iter()
            .all(|d| d.public_key != device.public_key);
        if !valid_name || !unique {
            return Err(PairedDevicesError::Corrupt);
        }
    }
    Ok(devices)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, fs::OpenOptions, os::windows::fs::OpenOptionsExt, path::Path};

    const FILE_SHARE_READ: u32 = 1;

    fn setup() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("paired-devices.bin");
        (dir, path)
    }

    fn dev(n: u8) -> PairedDevice {
        PairedDevice {
            name: format!("Celular {n}"),
            public_key: [n; 32],
            paired_at: 1_700_000_000_000 + i64::from(n),
        }
    }

    fn write_body(path: &Path, body: &[u8]) {
        write_protected(path, MAGIC, VERSION, body, ENTROPY).unwrap();
    }

    fn assert_open_fails_untouched(path: &Path, expected: PairedDevicesError) {
        let before = fs::read(path).unwrap();
        let err = PairedDeviceStore::open(path.to_path_buf()).unwrap_err();
        assert_eq!(err, expected);
        assert_eq!(fs::read(path).unwrap(), before, "el archivo no se toca");
    }

    #[test]
    fn open_missing_file_is_empty_and_writes_nothing() {
        let (d, path) = setup();
        let store = PairedDeviceStore::open(path).unwrap();
        assert!(store.list().is_empty());
        assert_eq!(fs::read_dir(d.path()).unwrap().count(), 0);
    }

    #[test]
    fn add_list_remove() {
        let (_d, path) = setup();
        let mut store = PairedDeviceStore::open(path).unwrap();
        store.add(dev(1)).unwrap();
        store.add(dev(2)).unwrap();
        assert_eq!(store.list(), &[dev(1), dev(2)]);
        store.remove(&[1; 32]).unwrap();
        assert_eq!(store.list(), &[dev(2)]);
    }

    #[test]
    fn survives_reopen() {
        let (_d, path) = setup();
        let mut store = PairedDeviceStore::open(path.clone()).unwrap();
        store.add(dev(1)).unwrap();
        store.add(dev(2)).unwrap();
        store.remove(&[1; 32]).unwrap();
        let reopened = PairedDeviceStore::open(path).unwrap();
        assert_eq!(reopened.list(), &[dev(2)]);
    }

    #[test]
    fn remove_last_then_reopen_is_empty_valid_list() {
        let (_d, path) = setup();
        let mut store = PairedDeviceStore::open(path.clone()).unwrap();
        store.add(dev(1)).unwrap();
        store.remove(&[1; 32]).unwrap();
        let reopened = PairedDeviceStore::open(path).unwrap();
        assert!(reopened.list().is_empty());
    }

    #[test]
    fn rejects_duplicate_key() {
        let (_d, path) = setup();
        let mut store = PairedDeviceStore::open(path).unwrap();
        store.add(dev(1)).unwrap();
        let mut again = dev(2);
        again.public_key = [1; 32];
        assert_eq!(store.add(again), Err(PairedDevicesError::AlreadyPaired));
        assert_eq!(store.list(), &[dev(1)]);
    }

    #[test]
    fn name_is_trimmed() {
        let (_d, path) = setup();
        let mut store = PairedDeviceStore::open(path.clone()).unwrap();
        let mut d = dev(1);
        d.name = "  Pixel de Ana \t".into();
        store.add(d).unwrap();
        assert_eq!(store.list()[0].name, "Pixel de Ana");
        assert_eq!(
            PairedDeviceStore::open(path).unwrap().list()[0].name,
            "Pixel de Ana"
        );
    }

    #[test]
    fn rejects_empty_whitespace_only_and_over_128_bytes_name() {
        let (_d, path) = setup();
        let mut store = PairedDeviceStore::open(path).unwrap();
        for name in [
            String::new(),
            "   \t ".to_string(),
            "a".repeat(129),
            "€".repeat(43), // 43 × 3 = 129 bytes
        ] {
            let mut d = dev(1);
            d.name = name;
            assert_eq!(store.add(d), Err(PairedDevicesError::InvalidName));
        }
        assert!(store.list().is_empty());
        // El límite exacto sí se acepta: 128 bytes.
        let mut ok = dev(1);
        ok.name = "a".repeat(128);
        store.add(ok).unwrap();
    }

    #[test]
    fn rejects_65th_device() {
        let (_d, path) = setup();
        let mut store = PairedDeviceStore::open(path).unwrap();
        for i in 0..64u8 {
            let mut d = dev(i);
            d.public_key[1] = 1;
            store.add(d).unwrap();
        }
        let mut extra = dev(200);
        extra.public_key[1] = 1;
        assert_eq!(store.add(extra), Err(PairedDevicesError::TooMany));
        assert_eq!(store.list().len(), 64);
    }

    #[test]
    fn remove_unknown_is_not_found() {
        let (_d, path) = setup();
        let mut store = PairedDeviceStore::open(path).unwrap();
        assert_eq!(store.remove(&[9; 32]), Err(PairedDevicesError::NotFound));
    }

    #[test]
    fn file_does_not_contain_name_or_public_key() {
        let (_d, path) = setup();
        let mut store = PairedDeviceStore::open(path.clone()).unwrap();
        let mut d = dev(5);
        d.name = "Pixel-de-Luciano-123".into();
        store.add(d).unwrap();
        let bytes = fs::read(&path).unwrap();
        let name = b"Pixel-de-Luciano-123";
        assert!(bytes.windows(name.len()).all(|w| w != name));
        assert!(bytes.windows(32).all(|w| w != [5u8; 32]));
        assert!(bytes.windows(MAGIC.len()).all(|w| w != MAGIC));
    }

    #[test]
    fn altered_file_is_protection_error_and_untouched() {
        let (_d, path) = setup();
        PairedDeviceStore::open(path.clone())
            .unwrap()
            .add(dev(1))
            .unwrap();
        let mut bytes = fs::read(&path).unwrap();
        let mid = bytes.len() / 2;
        bytes[mid] ^= 0x01;
        fs::write(&path, bytes).unwrap();
        assert_open_fails_untouched(&path, PairedDevicesError::Protection);
    }

    #[test]
    fn truncated_file_is_protection_error_and_untouched() {
        let (_d, path) = setup();
        PairedDeviceStore::open(path.clone())
            .unwrap()
            .add(dev(1))
            .unwrap();
        let bytes = fs::read(&path).unwrap();
        fs::write(&path, &bytes[..bytes.len() - 5]).unwrap();
        assert_open_fails_untouched(&path, PairedDevicesError::Protection);
    }

    #[test]
    fn empty_file_is_protection_error_and_untouched() {
        let (_d, path) = setup();
        fs::write(&path, b"").unwrap();
        assert_open_fails_untouched(&path, PairedDevicesError::Protection);
    }

    #[test]
    fn wrong_magic_is_corrupt_and_untouched() {
        let (_d, path) = setup();
        write_protected(&path, b"otro-magic", VERSION, &[0], ENTROPY).unwrap();
        assert_open_fails_untouched(&path, PairedDevicesError::Corrupt);
    }

    #[test]
    fn unknown_version_is_reported_and_untouched() {
        let (_d, path) = setup();
        write_protected(&path, MAGIC, 2, &[0], ENTROPY).unwrap();
        assert_open_fails_untouched(&path, PairedDevicesError::UnknownVersion);
    }

    #[test]
    fn invalid_structure_is_corrupt_and_untouched() {
        let (_d, path) = setup();
        write_body(&path, &[0xFF]);
        assert_open_fails_untouched(&path, PairedDevicesError::Corrupt);
    }

    #[test]
    fn trailing_bytes_after_body_is_corrupt() {
        let (_d, path) = setup();
        let mut body = postcard::to_allocvec(&vec![dev(1)]).unwrap();
        body.push(0);
        write_body(&path, &body);
        assert_open_fails_untouched(&path, PairedDevicesError::Corrupt);
    }

    #[test]
    fn body_claiming_huge_count_is_rejected_without_allocating() {
        let (_d, path) = setup();
        // Varint de 2^56 elementos y ningún dato detrás.
        write_body(
            &path,
            &[0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x01],
        );
        assert_open_fails_untouched(&path, PairedDevicesError::Corrupt);
    }

    #[test]
    fn file_over_max_len_is_corrupt_without_decrypting() {
        let (_d, path) = setup();
        // Basura que, sin el tope de lectura, daría `Protection` al descifrarla.
        fs::write(&path, vec![0xAB; MAX_FILE_BYTES + 1]).unwrap();
        assert_open_fails_untouched(&path, PairedDevicesError::Corrupt);
    }

    #[test]
    fn debug_and_errors_do_not_show_name_or_key() {
        // Guarda de regresión: con un celular real y un error real de ese mismo
        // archivo, nada de lo que se puede imprimir lleva el nombre ni la llave.
        let (_d, path) = setup();
        let mut store = PairedDeviceStore::open(path.clone()).unwrap();
        let mut d = dev(5);
        d.name = "Pixel-de-Luciano-123".into();
        store.add(d).unwrap();
        let mut bytes = fs::read(&path).unwrap();
        let mid = bytes.len() / 2;
        bytes[mid] ^= 0x01;
        fs::write(&path, bytes).unwrap();
        let err = PairedDeviceStore::open(path).unwrap_err();

        for text in [
            format!("{err:?}"),
            err.to_string(),
            format!("{store:?}"),
            format!("{:?}", store.list()),
        ] {
            assert!(!text.contains("Luciano"), "{text}");
            assert!(!text.contains("[5, 5"), "{text}");
        }
    }

    #[test]
    fn file_with_duplicate_keys_is_corrupt() {
        let (_d, path) = setup();
        let mut second = dev(2);
        second.public_key = [1; 32];
        write_body(
            &path,
            &postcard::to_allocvec(&vec![dev(1), second]).unwrap(),
        );
        assert_open_fails_untouched(&path, PairedDevicesError::Corrupt);
    }

    #[test]
    fn file_with_invalid_name_is_corrupt() {
        let (_d, path) = setup();
        for name in ["", " pegado ", "a"] {
            let mut d = dev(1);
            d.name = name.into();
            write_body(&path, &postcard::to_allocvec(&vec![d]).unwrap());
            if name == "a" {
                assert!(PairedDeviceStore::open(path.clone()).is_ok());
            } else {
                assert_open_fails_untouched(&path, PairedDevicesError::Corrupt);
            }
        }
    }

    #[test]
    fn identity_file_is_not_a_valid_list() {
        let (_d, path) = setup();
        write_protected(
            &path,
            b"boveda-pc-identity",
            1,
            &[7u8; 32],
            b"boveda-desktop identity v1",
        )
        .unwrap();
        assert_open_fails_untouched(&path, PairedDevicesError::Protection);
    }

    fn lock(path: &Path) -> fs::File {
        // Sin FILE_SHARE_DELETE, Windows no deja reemplazar el archivo.
        OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .open(path)
            .unwrap()
    }

    #[test]
    fn failed_add_keeps_memory_and_disk() {
        let (_d, path) = setup();
        let mut store = PairedDeviceStore::open(path.clone()).unwrap();
        store.add(dev(1)).unwrap();
        let before = fs::read(&path).unwrap();
        let _lock = lock(&path);
        assert_eq!(store.add(dev(2)), Err(PairedDevicesError::Io));
        assert_eq!(store.list(), &[dev(1)]);
        assert_eq!(fs::read(&path).unwrap(), before);
    }

    #[test]
    fn failed_remove_keeps_memory_and_disk() {
        let (_d, path) = setup();
        let mut store = PairedDeviceStore::open(path.clone()).unwrap();
        store.add(dev(1)).unwrap();
        store.add(dev(2)).unwrap();
        let before = fs::read(&path).unwrap();
        let _lock = lock(&path);
        assert_eq!(store.remove(&[1; 32]), Err(PairedDevicesError::Io));
        assert_eq!(store.list(), &[dev(1), dev(2)]);
        assert_eq!(fs::read(&path).unwrap(), before);
    }

    #[test]
    fn debug_hides_name_and_key() {
        let mut d = dev(5);
        d.name = "Pixel-de-Luciano-123".into();
        let text = format!("{d:?}");
        assert!(!text.contains("Luciano"));
        assert!(!text.contains("[5, 5"));
        assert!(!text.contains("0505"));
    }
}
