use super::*;

#[test]
fn delegates_to_core() {
    assert_eq!(
        password_strength("a".repeat(11)),
        PasswordStrength::TooShort
    );
    assert_eq!(
        password_strength("a".repeat(12)),
        PasswordStrength::Acceptable
    );
    assert_eq!(password_strength("a".repeat(16)), PasswordStrength::Strong);
}
