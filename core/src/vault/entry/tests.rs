//! Pruebas unitarias de `vault/entry.rs`.

use super::*;

// --- Identificador ---

#[test]
fn generated_ids_are_16_bytes_and_distinct() {
    let a = EntryId::generate().unwrap();
    let b = EntryId::generate().unwrap();
    assert_eq!(a.as_bytes().len(), ENTRY_ID_LEN);
    assert_ne!(a, b);
}

#[test]
fn id_round_trips_through_bytes() {
    let bytes: [u8; 16] = core::array::from_fn(|i| i as u8);
    assert_eq!(EntryId::from_bytes(bytes).as_bytes(), &bytes);
}

// --- Ayudas ---

fn assert_zeroize_on_drop<T: ZeroizeOnDrop>() {}

fn search(service: &str) -> Result<SearchData> {
    SearchData::new(service, None, None, &[])
}

fn tags_of(tags: &[&str]) -> Result<Vec<String>> {
    SearchData::new("s", None, None, tags).map(|d| d.tags().to_vec())
}

fn invalid(field: InvalidEntryField) -> Error {
    Error::InvalidEntry(field)
}

// --- Datos de búsqueda ---

#[test]
fn service_is_trimmed() {
    assert_eq!(search(" GitHub ").unwrap().service(), "GitHub");
}

#[test]
fn blank_service_is_rejected() {
    assert_eq!(
        search("   ").unwrap_err(),
        invalid(InvalidEntryField::ServiceEmpty)
    );
}

#[test]
fn nbsp_only_service_is_rejected() {
    assert_eq!(
        search("\u{00A0}").unwrap_err(),
        invalid(InvalidEntryField::ServiceEmpty)
    );
}

#[test]
fn service_limit_is_measured_after_trim() {
    let service = format!("  {}  ", "a".repeat(MAX_SERVICE_LEN));
    assert_eq!(search(&service).unwrap().service().len(), MAX_SERVICE_LEN);
}

#[test]
fn service_limit_is_in_bytes() {
    assert!(search(&"ñ".repeat(128)).is_ok());
    assert_eq!(
        search(&"ñ".repeat(129)).unwrap_err(),
        invalid(InvalidEntryField::ServiceTooLong)
    );
}

#[test]
fn blank_url_and_username_become_none() {
    for blank in ["", "  "] {
        let data = SearchData::new("s", Some(blank), Some(blank), &[]).unwrap();
        assert_eq!(data.url(), None);
        assert_eq!(data.username(), None);
    }
}

#[test]
fn url_and_username_are_trimmed() {
    let data = SearchData::new("s", Some(" a "), Some(" b "), &[]).unwrap();
    assert_eq!(data.url(), Some("a"));
    assert_eq!(data.username(), Some("b"));
}

#[test]
fn url_limit() {
    let ok = "u".repeat(MAX_URL_LEN);
    let long = "u".repeat(MAX_URL_LEN + 1);
    assert!(SearchData::new("s", Some(&ok), None, &[]).is_ok());
    assert_eq!(
        SearchData::new("s", Some(&long), None, &[]).unwrap_err(),
        invalid(InvalidEntryField::UrlTooLong)
    );
}

#[test]
fn username_limit() {
    let ok = "u".repeat(MAX_USERNAME_LEN);
    let long = "u".repeat(MAX_USERNAME_LEN + 1);
    assert!(SearchData::new("s", None, Some(&ok), &[]).is_ok());
    assert_eq!(
        SearchData::new("s", None, Some(&long), &[]).unwrap_err(),
        invalid(InvalidEntryField::UsernameTooLong)
    );
}

#[test]
fn tags_are_trimmed_and_blank_dropped() {
    assert_eq!(tags_of(&[" a ", "", "  ", "b"]).unwrap(), ["a", "b"]);
}

#[test]
fn duplicate_tags_ignore_case_keep_first_form_and_order() {
    assert_eq!(
        tags_of(&["Trabajo", "casa", "trabajo", " TRABAJO "]).unwrap(),
        ["Trabajo", "casa"]
    );
}

#[test]
fn accented_tags_are_distinct() {
    assert_eq!(
        tags_of(&["Médico", "Medico"]).unwrap(),
        ["Médico", "Medico"]
    );
}

#[test]
fn tag_limit() {
    assert!(tags_of(&[&"t".repeat(MAX_TAG_LEN)]).is_ok());
    assert_eq!(
        tags_of(&[&"t".repeat(MAX_TAG_LEN + 1)]).unwrap_err(),
        invalid(InvalidEntryField::TagTooLong)
    );
}

fn distinct_tags(count: usize) -> Vec<String> {
    (0..count).map(|i| format!("tag{i}")).collect()
}

#[test]
fn tag_count_limit() {
    let ok = distinct_tags(MAX_TAGS);
    let ok: Vec<&str> = ok.iter().map(String::as_str).collect();
    assert_eq!(tags_of(&ok).unwrap().len(), MAX_TAGS);

    let many = distinct_tags(MAX_TAGS + 1);
    let many: Vec<&str> = many.iter().map(String::as_str).collect();
    assert_eq!(
        tags_of(&many).unwrap_err(),
        invalid(InvalidEntryField::TooManyTags)
    );
}

#[test]
fn tag_count_is_measured_after_dedup() {
    let mut tags = distinct_tags(MAX_TAGS);
    tags.extend((0..8).map(|i| format!("TAG{i}")));
    let tags: Vec<&str> = tags.iter().map(String::as_str).collect();
    assert_eq!(tags.len(), 40);
    assert_eq!(tags_of(&tags).unwrap().len(), MAX_TAGS);
}

#[test]
fn search_data_debug_hides_values() {
    let data =
        SearchData::new("GitHub", Some("example.com"), Some("alice"), &["personal"]).unwrap();
    let debug = format!("{data:?}");
    for value in ["GitHub", "example.com", "alice", "personal"] {
        assert!(!debug.contains(value), "Debug muestra {value}: {debug}");
    }
}

#[test]
fn search_data_zeroizes_on_drop() {
    assert_zeroize_on_drop::<SearchData>();
}

// --- Secreto ---

fn secret(password: &str, notes: Option<&str>) -> Result<Secret> {
    Secret::new(SecretString::from(password), notes.map(SecretString::from))
}

#[test]
fn empty_password_is_rejected() {
    assert_eq!(
        secret("", None).unwrap_err(),
        invalid(InvalidEntryField::PasswordEmpty)
    );
}

#[test]
fn password_is_not_trimmed() {
    assert_eq!(
        secret(" x ", None).unwrap().password().expose_secret(),
        " x "
    );
}

#[test]
fn spaces_only_password_is_kept() {
    assert_eq!(
        secret("   ", None).unwrap().password().expose_secret(),
        "   "
    );
}

#[test]
fn password_limit() {
    assert!(secret(&"p".repeat(MAX_PASSWORD_LEN), None).is_ok());
    assert_eq!(
        secret(&"p".repeat(MAX_PASSWORD_LEN + 1), None).unwrap_err(),
        invalid(InvalidEntryField::PasswordTooLong)
    );
}

#[test]
fn blank_notes_become_none() {
    for blank in ["", " \n "] {
        assert!(secret("p", Some(blank)).unwrap().notes().is_none());
    }
}

#[test]
fn notes_are_not_trimmed() {
    let s = secret("p", Some(" n ")).unwrap();
    assert_eq!(s.notes().unwrap().expose_secret(), " n ");
}

#[test]
fn notes_limit() {
    assert!(secret("p", Some(&"n".repeat(MAX_NOTES_LEN))).is_ok());
    assert_eq!(
        secret("p", Some(&"n".repeat(MAX_NOTES_LEN + 1))).unwrap_err(),
        invalid(InvalidEntryField::NotesTooLong)
    );
}

#[test]
fn secret_debug_hides_values() {
    let debug = format!("{:?}", secret("hunter2", Some("pin 1234")).unwrap());
    assert!(!debug.contains("hunter2"), "{debug}");
    assert!(!debug.contains("1234"), "{debug}");
}

// Si `T` implementa `Display`, la constante inherente de `Probe<T>` tiene
// prioridad sobre la del rasgo y vale `true`.
trait NoDisplay {
    const IMPLS_DISPLAY: bool = false;
}
impl<T: ?Sized> NoDisplay for T {}
struct Probe<T: ?Sized>(core::marker::PhantomData<T>);
#[allow(dead_code)]
impl<T: ?Sized + fmt::Display> Probe<T> {
    const IMPLS_DISPLAY: bool = true;
}

#[test]
fn secret_does_not_implement_display() {
    // Control: el truco detecta un tipo que sí implementa `Display`.
    const { assert!(<Probe<String>>::IMPLS_DISPLAY) };
    const { assert!(!<Probe<Secret>>::IMPLS_DISPLAY) };
    const { assert!(!<Probe<SearchData>>::IMPLS_DISPLAY) };
}

#[test]
fn secret_zeroizes_on_drop() {
    assert_zeroize_on_drop::<Secret>();
}
