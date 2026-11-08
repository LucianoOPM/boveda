//! Ayudantes de las pruebas de la fachada. Solo existe con `cfg(test)`.

use std::path::PathBuf;

/// Contraseña de `core/tests/fixtures/vault-v2.db`.
pub(crate) const FIXTURE_PASSWORD: &str = "prueba-bóveda-v2";

/// Copia `vault-v2.db` a un directorio temporal (debe vivir toda la prueba)
/// y devuelve la ruta absoluta de la copia. Nunca se abre el original.
pub(crate) fn fixture_copy() -> (tempfile::TempDir, PathBuf) {
    let source = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../core/tests/fixtures/vault-v2.db"
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
