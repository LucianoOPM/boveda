//! Escritura atómica de archivos en Windows.
//!
//! Se escribe un temporal en el **mismo directorio** (así el reemplazo no cruza
//! volúmenes), se sincroniza y se reemplaza el destino con una sola llamada a
//! `MoveFileExW`, sin borrar antes el archivo anterior: si algo falla, el
//! original queda intacto.
//!
//! Lo que se promete es lo que da Windows (`sync_all` y `MOVEFILE_WRITE_THROUGH`);
//! no se promete durabilidad absoluta ante un corte eléctrico.

use std::{
    ffi::OsString,
    fs::{self, OpenOptions},
    io::{self, Write},
    os::windows::ffi::OsStrExt,
    path::Path,
};

use windows_sys::Win32::Storage::FileSystem::{
    MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
};

fn wide(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}

/// Escribe `bytes` en `path` reemplazando su contenido de forma atómica.
///
/// Ante cualquier error el destino conserva su contenido anterior y el
/// temporal (`<nombre>.tmp`) se borra.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    write_with(path, bytes, MOVEFILE_REPLACE_EXISTING)
}

/// Como [`write_atomic`], pero **nunca reemplaza**: si `path` ya existe
/// (aunque haya aparecido después de que quien llama comprobó que no estaba),
/// falla con `ErrorKind::AlreadyExists` y el archivo existente queda intacto.
pub fn write_atomic_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    write_with(path, bytes, 0)
}

fn write_with(path: &Path, bytes: &[u8], replace_flag: u32) -> io::Result<()> {
    let file_name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "ruta sin nombre de archivo"))?;
    let mut tmp_name = OsString::from(file_name);
    tmp_name.push(".tmp");
    let tmp = path.with_file_name(tmp_name);

    // Un temporal de una ejecución interrumpida se descarta; nunca el destino.
    match fs::remove_file(&tmp) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }

    let result = write_and_replace(&tmp, path, bytes, replace_flag);
    if result.is_err() {
        // Si no se puede borrar, queda el temporal; el error original manda.
        let _ = fs::remove_file(&tmp);
    }
    result
}

fn write_and_replace(tmp: &Path, path: &Path, bytes: &[u8], replace_flag: u32) -> io::Result<()> {
    {
        let mut file = OpenOptions::new().write(true).create_new(true).open(tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    let from = wide(tmp);
    let to = wide(path);
    // SAFETY: `from` y `to` son cadenas UTF-16 terminadas en NUL que viven toda
    // la llamada.
    let ok = unsafe {
        MoveFileExW(
            from.as_ptr(),
            to.as_ptr(),
            replace_flag | MOVEFILE_WRITE_THROUGH,
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::windows::fs::OpenOptionsExt;

    const FILE_SHARE_READ: u32 = 1;

    fn names(dir: &std::path::Path) -> Vec<String> {
        let mut v: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
    }

    #[test]
    fn creates_new_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.bin");
        write_atomic(&path, b"hola").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"hola");
    }

    #[test]
    fn replaces_existing_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.bin");
        fs::write(&path, b"viejo contenido largo").unwrap();
        write_atomic(&path, b"nuevo").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"nuevo");
    }

    #[test]
    fn leaves_no_temp_file_on_success() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.bin");
        write_atomic(&path, b"x").unwrap();
        assert_eq!(names(dir.path()), vec!["a.bin"]);
    }

    #[test]
    fn failed_replace_keeps_original_and_cleans_temp() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.bin");
        fs::write(&path, b"original").unwrap();
        // Sin FILE_SHARE_DELETE, Windows no deja reemplazar el destino.
        let _lock = OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .open(&path)
            .unwrap();
        assert!(write_atomic(&path, b"nuevo").is_err());
        assert_eq!(fs::read(&path).unwrap(), b"original");
        assert_eq!(names(dir.path()), vec!["a.bin"]);
    }

    #[test]
    fn stale_temp_does_not_block() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.bin");
        fs::write(dir.path().join("a.bin.tmp"), b"basura").unwrap();
        write_atomic(&path, b"bien").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"bien");
        assert_eq!(names(dir.path()), vec!["a.bin"]);
    }

    #[test]
    fn write_atomic_new_creates_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.bin");
        write_atomic_new(&path, b"nuevo").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"nuevo");
        assert_eq!(names(dir.path()), vec!["a.bin"]);
    }

    #[test]
    fn write_atomic_new_never_replaces_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.bin");
        fs::write(&path, b"original").unwrap();
        let err = write_atomic_new(&path, b"nuevo").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(&path).unwrap(), b"original");
        assert_eq!(names(dir.path()), vec!["a.bin"]);
    }

    #[test]
    fn missing_parent_dir_errors() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("no-existe").join("a.bin");
        assert!(write_atomic(&path, b"x").is_err());
        assert_eq!(names(dir.path()), Vec::<String>::new());
    }
}
