//! Ayudantes de las pruebas de la fachada. Solo existe con `cfg(test)`.

use std::path::PathBuf;

/// Contraseña de `core/tests/fixtures/vault-v2.db`.
pub(crate) const FIXTURE_PASSWORD: &str = "prueba-bóveda-v2";

/// Contraseña de `core/tests/fixtures/vault-v3.db`.
pub(crate) const V3_PASSWORD: &str = "prueba-bóveda-v3";

/// Copia `vault-v2.db` a un directorio temporal (debe vivir toda la prueba)
/// y devuelve la ruta absoluta de la copia. Nunca se abre el original.
pub(crate) fn fixture_copy() -> (tempfile::TempDir, PathBuf) {
    copy_fixture("vault-v2.db")
}

/// Como [`fixture_copy`], con `vault-v3.db`: trae la identidad (llave
/// `[7; 32]`) y dos PCs emparejadas, `Escritorio` y `Portátil`.
pub(crate) fn fixture_v3_copy() -> (tempfile::TempDir, PathBuf) {
    copy_fixture("vault-v3.db")
}

fn copy_fixture(name: &str) -> (tempfile::TempDir, PathBuf) {
    let source = format!(
        "{}/../../core/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    let (dir, path) = temp_path();
    std::fs::copy(source, &path).unwrap();
    (dir, path)
}

/// Directorio temporal y una ruta absoluta `boveda.db` que no existe.
pub(crate) fn temp_path() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("boveda.db");
    (dir, path)
}

pub(crate) fn path_string(path: &std::path::Path) -> String {
    path.to_str().unwrap().to_owned()
}
