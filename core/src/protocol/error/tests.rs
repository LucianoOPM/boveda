use super::*;

fn all_errors() -> Vec<ProtocolError> {
    vec![
        ProtocolError::TooLarge,
        ProtocolError::Truncated,
        ProtocolError::UnsupportedVersion(7),
        ProtocolError::UnexpectedKind(9),
        ProtocolError::Malformed,
        ProtocolError::TrailingBytes,
        ProtocolError::InvalidField(InvalidField::Ttl),
        ProtocolError::Random,
        ProtocolError::Encode,
    ]
}

#[test]
fn every_error_has_a_non_empty_message() {
    for error in all_errors() {
        assert!(!error.to_string().is_empty(), "{error:?}");
    }
}

#[test]
fn every_invalid_field_has_a_non_empty_message() {
    for field in [
        InvalidField::Ttl,
        InvalidField::PasswordEmpty,
        InvalidField::PasswordTooLong,
        InvalidField::Port,
        InvalidField::DeviceName,
        InvalidField::Address,
    ] {
        assert!(!field.to_string().is_empty(), "{field:?}");
    }
}

#[test]
fn messages_only_carry_the_variant_number() {
    assert_eq!(
        ProtocolError::UnsupportedVersion(7).to_string(),
        "versión del protocolo no soportada: 7"
    );
    assert_eq!(
        ProtocolError::UnexpectedKind(9).to_string(),
        "tipo de mensaje inesperado: 9"
    );
}

#[test]
fn errors_are_copy_and_comparable() {
    fn assert_copy<T: Copy + Eq>() {}
    assert_copy::<ProtocolError>();
    assert_copy::<InvalidField>();
}
