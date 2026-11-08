use super::*;

fn s(text: &str) -> SecretString {
    SecretString::from(text)
}

#[test]
fn empty_is_too_short() {
    assert_eq!(password_strength(&s("")), PasswordStrength::TooShort);
}

#[test]
fn eleven_is_too_short() {
    assert_eq!(
        password_strength(&s(&"a".repeat(11))),
        PasswordStrength::TooShort
    );
}

#[test]
fn twelve_is_acceptable() {
    assert_eq!(
        password_strength(&s(&"a".repeat(12))),
        PasswordStrength::Acceptable
    );
}

#[test]
fn fifteen_is_acceptable() {
    assert_eq!(
        password_strength(&s(&"a".repeat(15))),
        PasswordStrength::Acceptable
    );
}

#[test]
fn sixteen_is_strong() {
    assert_eq!(
        password_strength(&s(&"a".repeat(16))),
        PasswordStrength::Strong
    );
}

/// Cuenta caracteres, no bytes: 12 "ñ" son 24 bytes.
#[test]
fn counts_chars_not_bytes() {
    assert_eq!(
        password_strength(&s(&"ñ".repeat(11))),
        PasswordStrength::TooShort
    );
    assert_eq!(
        password_strength(&s(&"ñ".repeat(12))),
        PasswordStrength::Acceptable
    );
}

#[test]
fn emoji_counts_one() {
    let password = format!("{}🔐", "a".repeat(11));
    assert_eq!(
        password_strength(&s(&password)),
        PasswordStrength::Acceptable
    );
}

/// No se recorta: los espacios cuentan.
#[test]
fn spaces_count() {
    assert_eq!(
        password_strength(&s(&" ".repeat(12))),
        PasswordStrength::Acceptable
    );
}

#[test]
fn very_long_is_strong() {
    assert_eq!(
        password_strength(&s(&"x".repeat(10_000))),
        PasswordStrength::Strong
    );
}

#[test]
fn require_rejects_short() {
    assert_eq!(
        require_master_password(&s(&"a".repeat(11))),
        Err(Error::PasswordTooShort)
    );
    assert_eq!(require_master_password(&s(&"a".repeat(12))), Ok(()));
}
