//! Pruebas unitarias de `crypto/normalize.rs`.

use super::*;

fn secret(text: &str) -> SecretString {
    SecretString::from(text)
}

/// `true` si el ayudante usó el texto original, sin copiarlo.
fn is_borrowed(normalized: &NormalizedPassword<'_>) -> bool {
    matches!(normalized, NormalizedPassword::Borrowed(_))
}

#[test]
fn text_already_in_nfc_is_not_copied() {
    for text in [
        "",
        "clave-ascii-12345",
        "caf\u{00e9}",
        "prueba-b\u{00f3}veda-v1",
        "\u{ac00}\u{ac01}",
    ] {
        let password = secret(text);
        let normalized = normalize_nfc(&password);
        assert!(is_borrowed(&normalized), "{text:?}");
        assert_eq!(&*normalized, text);
    }
}

#[test]
fn decomposed_text_becomes_its_composed_form() {
    let password = secret("cafe\u{0301}");
    let normalized = normalize_nfc(&password);
    assert!(!is_borrowed(&normalized));
    assert_eq!(&*normalized, "caf\u{00e9}");
    assert_eq!(normalized.as_bytes(), "caf\u{00e9}".as_bytes());
}

#[test]
fn combining_marks_are_reordered_canonically() {
    // Circunflejo (clase 230) y punto abajo (clase 220) en cualquier orden.
    for text in ["e\u{0302}\u{0323}", "e\u{0323}\u{0302}"] {
        let password = secret(text);
        assert_eq!(&*normalize_nfc(&password), "\u{1ec7}", "{text:?}");
    }
}

#[test]
fn hangul_jamo_are_composed() {
    let password = secret("\u{1100}\u{1161}");
    let normalized = normalize_nfc(&password);
    assert!(!is_borrowed(&normalized));
    assert_eq!(&*normalized, "\u{ac00}");
}

#[test]
fn compatibility_characters_are_left_alone() {
    // NFC, no NFKC: la ligadura `ﬁ` no se convierte en `fi`.
    let password = secret("\u{fb01}-clave");
    let normalized = normalize_nfc(&password);
    assert!(is_borrowed(&normalized));
    assert_eq!(&*normalized, "\u{fb01}-clave");
}

#[test]
fn normalized_copy_has_exact_capacity() {
    // Sin reasignaciones: ningún búfer anterior del `String` guarda restos.
    // Esta prueba por sí sola no lo demuestra (un `Vec` que crece pasa a
    // `max(2·cap, necesario, 8)` y podría coincidir con la longitud); detecta
    // los errores probables (`String::new()` o `with_capacity(text.len())`).
    // La garantía está en el código: `with_capacity` es exacta para `u8`,
    // `nfc()` es determinista sobre el mismo `&str` y `extend` no reserva más
    // porque `Recompositions` no define `size_hint`.
    for text in [
        "cafe\u{0301}",
        "e\u{0302}\u{0323}-e\u{0323}\u{0302}-clave",
        "\u{1100}\u{1161}\u{1100}\u{1161}\u{11a8}",
        "a\u{0344}b",
    ] {
        let password = secret(text);
        match normalize_nfc(&password) {
            NormalizedPassword::Owned(copy) => {
                assert_eq!(copy.capacity(), copy.len(), "{text:?}");
                assert!(!copy.is_empty());
            }
            NormalizedPassword::Borrowed(_) => panic!("{text:?} no estaba en NFC"),
        }
    }
}

#[test]
fn normalization_can_make_the_text_longer_in_bytes() {
    // U+0344 (2 bytes) se descompone en U+0308 U+0301 (4 bytes): la capacidad
    // sale de recorrer el resultado, no del tamaño del original.
    let password = secret("a\u{0344}b");
    let normalized = normalize_nfc(&password);
    assert!(normalized.len() > password.expose_secret().len());
}
