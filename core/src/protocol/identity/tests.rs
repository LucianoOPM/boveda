use secrecy::{ExposeSecret, ExposeSecretMut, SecretBox};
use zeroize::ZeroizeOnDrop;

use super::*;

fn assert_zeroize_on_drop<T: ZeroizeOnDrop>() {}

fn hex32(text: &str) -> [u8; 32] {
    assert_eq!(text.len(), 64);
    core::array::from_fn(|i| u8::from_str_radix(&text[2 * i..2 * i + 2], 16).unwrap())
}

fn secret(bytes: [u8; 32]) -> SecretBox<[u8; 32]> {
    let mut secret = SecretBox::<[u8; 32]>::default();
    secret.expose_secret_mut().copy_from_slice(&bytes);
    secret
}

#[test]
fn rfc7748_alice_and_bob_public_keys() {
    // RFC 7748, sección 6.1.
    let cases = [
        (
            "77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a",
            "8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a",
        ),
        (
            "5dab087e624a8a4b79e17f8b83800ee66f3bb1292618b6fd1c2f8b27ff88e0eb",
            "de9edb7d7b7dc1b4d35b61c2ece435373f8343c85b78674dadfc7e146f882b4f",
        ),
    ];
    for (private, public) in cases {
        let derived = PublicKey::from_private(&secret(hex32(private)));
        assert_eq!(derived.as_bytes(), &hex32(public));
    }
}

#[test]
fn public_key_ignores_clamping() {
    let raw: [u8; 32] = core::array::from_fn(|i| (i as u8).wrapping_mul(37).wrapping_add(11));
    let mut clamped = raw;
    clamped[0] &= 248;
    clamped[31] &= 127;
    clamped[31] |= 64;
    assert_ne!(
        raw, clamped,
        "la llave de prueba debe cambiar al recortarla"
    );
    assert_eq!(
        PublicKey::from_private(&secret(raw)),
        PublicKey::from_private(&secret(clamped))
    );
}

#[test]
fn keypair_keeps_the_private_bytes_unchanged() {
    for bytes in [[0xFF; 32], [0u8; 32], core::array::from_fn(|i| i as u8)] {
        let pair = KeyPair::from_private(secret(bytes));
        assert_eq!(pair.private().expose_secret(), &bytes);
    }
}

#[test]
fn keypair_public_matches_from_private() {
    let pair = KeyPair::from_private(secret([0x42; 32]));
    assert_eq!(pair.public(), PublicKey::from_private(pair.private()));
}

#[test]
fn generate_returns_different_keys() {
    let a = KeyPair::generate().unwrap();
    let b = KeyPair::generate().unwrap();
    assert_ne!(a.private().expose_secret(), b.private().expose_secret());
    assert_ne!(a.public(), b.public());
    assert_eq!(a.public(), PublicKey::from_private(a.private()));
    assert_ne!(a.private().expose_secret(), &[0u8; 32]);
}

#[test]
fn fingerprint_is_the_first_8_bytes_in_uppercase_groups() {
    let mut bytes = [0u8; 32];
    bytes[..8].copy_from_slice(&[0xA1, 0xB2, 0xC3, 0xD4, 0xE5, 0xF6, 0x07, 0x18]);
    bytes[8..].fill(0x99);
    let fingerprint = PublicKey::from_bytes(bytes).fingerprint();
    assert_eq!(fingerprint.to_string(), "A1B2-C3D4-E5F6-0718");
    assert_eq!(
        fingerprint.as_bytes(),
        &[0xA1, 0xB2, 0xC3, 0xD4, 0xE5, 0xF6, 0x07, 0x18]
    );
}

#[test]
fn public_key_debug_shows_only_the_fingerprint() {
    let key = PublicKey::from_bytes([0x5A; 32]);
    assert_eq!(format!("{key:?}"), "PublicKey(5A5A-5A5A-5A5A-5A5A)");
}

#[test]
fn keypair_debug_hides_the_private_key() {
    let pair = KeyPair::from_private(secret([0xAB; 32]));
    let debug = format!("{pair:?}");
    let lower = debug.to_lowercase();
    assert!(!lower.contains("abab"), "{debug}");
    assert!(!debug.contains("171"), "{debug}");
    assert!(!debug.contains('['), "{debug}");
    assert!(debug.contains(&pair.public().fingerprint().to_string()));
}

#[test]
fn keypair_is_zeroize_on_drop() {
    assert_zeroize_on_drop::<KeyPair>();
}
