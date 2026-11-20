use super::fingerprint::FingerprintKey;

#[test]
fn same_bytes_same_fingerprint_within_a_key() {
    let key = FingerprintKey::new();
    assert!(key.fingerprint(b"prueba-no-real") == key.fingerprint(b"prueba-no-real"));
}

#[test]
fn different_bytes_different_fingerprint() {
    let key = FingerprintKey::new();
    assert!(key.fingerprint(b"prueba-no-real-1") != key.fingerprint(b"prueba-no-real-2"));
}

#[test]
fn different_keys_give_different_fingerprints() {
    // La clave de `RandomState` es aleatoria: dos gestores no comparten
    // huellas (la probabilidad de coincidir por azar es de 2^-64).
    let first = FingerprintKey::new();
    let second = FingerprintKey::new();
    assert!(first.fingerprint(b"prueba-no-real") != second.fingerprint(b"prueba-no-real"));
}
