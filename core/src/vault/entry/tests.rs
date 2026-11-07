//! Pruebas unitarias de `vault/entry.rs`.

use super::raw::{RawSearchDataV1, RawSecretV1};
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

// --- Flavor de serialización ---

#[test]
fn flavor_keeps_content_across_growth() {
    use postcard::ser_flavors::Flavor;

    let expected: Vec<u8> = (0..115u8).collect();
    let mut flavor = ZeroizingVec::with_capacity(1);
    let mut offset = 0;
    for len in [1, 3, 10, 100] {
        flavor.try_extend(&expected[offset..offset + len]).unwrap();
        offset += len;
    }
    flavor.try_push(expected[offset]).unwrap();
    assert_eq!(flavor.finalize().unwrap().as_slice(), expected.as_slice());
}

#[test]
fn flavor_serializes_like_to_allocvec() {
    let value = ("abc", Some(7u32), [1u8, 2]);
    let ours = postcard::serialize_with_flavor(&value, ZeroizingVec::with_capacity(1)).unwrap();
    assert_eq!(
        ours.as_slice(),
        postcard::to_allocvec(&value).unwrap().as_slice()
    );
}

// --- Serialización con versión de estructura ---

fn full_search_data() -> SearchData {
    SearchData::new(
        "Banco Ñandú",
        Some("https://ejemplo.mx"),
        Some("álvaro"),
        &["finanzas", "日本"],
    )
    .unwrap()
}

#[test]
fn search_data_v1_bytes_are_fixed() {
    let data = SearchData::new("a", None, Some("b"), &["x"]).unwrap();
    assert_eq!(
        data.encode().unwrap().as_slice(),
        [0x01, 0x61, 0x00, 0x01, 0x01, 0x62, 0x01, 0x01, 0x78]
    );
}

#[test]
fn secret_v1_bytes_are_fixed() {
    assert_eq!(
        secret("p", None).unwrap().encode().unwrap().as_slice(),
        [0x01, 0x70, 0x00]
    );
}

#[test]
fn search_data_round_trips() {
    let data = full_search_data();
    let bytes = data.encode().unwrap();
    assert_eq!(
        SearchData::decode(ENTRY_SCHEMA_VERSION, &bytes).unwrap(),
        data
    );
}

#[test]
fn secret_round_trips() {
    let original = secret("contraseña segura", Some("PIN: 1234\nañadido")).unwrap();
    let bytes = original.encode().unwrap();
    let decoded = Secret::decode(ENTRY_SCHEMA_VERSION, &bytes).unwrap();
    assert_eq!(
        decoded.password().expose_secret(),
        original.password().expose_secret()
    );
    assert_eq!(
        decoded.notes().unwrap().expose_secret(),
        original.notes().unwrap().expose_secret()
    );
}

#[test]
fn unknown_schema_version_is_corrupt() {
    let data = full_search_data().encode().unwrap();
    let sec = secret("p", None).unwrap().encode().unwrap();
    for version in [0, 2, u16::MAX] {
        assert_eq!(
            SearchData::decode(version, &data).unwrap_err(),
            Error::CorruptEntry
        );
        assert_eq!(
            Secret::decode(version, &sec).unwrap_err(),
            Error::CorruptEntry
        );
    }
}

#[test]
fn trailing_bytes_are_corrupt() {
    let mut data = full_search_data().encode().unwrap();
    data.push(0x00);
    assert_eq!(
        SearchData::decode(ENTRY_SCHEMA_VERSION, &data).unwrap_err(),
        Error::CorruptEntry
    );
    let mut sec = secret("p", None).unwrap().encode().unwrap();
    sec.push(0x00);
    assert_eq!(
        Secret::decode(ENTRY_SCHEMA_VERSION, &sec).unwrap_err(),
        Error::CorruptEntry
    );
}

#[test]
fn truncated_bytes_are_corrupt() {
    let data = full_search_data().encode().unwrap();
    assert_eq!(
        SearchData::decode(ENTRY_SCHEMA_VERSION, &data[..data.len() - 1]).unwrap_err(),
        Error::CorruptEntry
    );
    let sec = secret("p", Some("n")).unwrap().encode().unwrap();
    assert_eq!(
        Secret::decode(ENTRY_SCHEMA_VERSION, &sec[..sec.len() - 1]).unwrap_err(),
        Error::CorruptEntry
    );
}

fn raw_search(service: &str, url: Option<&str>, tags: Vec<&str>) -> Vec<u8> {
    postcard::to_allocvec(&RawSearchDataV1 {
        service,
        url,
        username: None,
        tags,
    })
    .unwrap()
}

#[test]
fn rule_violating_bytes_do_not_produce_entry() {
    let blank = raw_search("  ", None, vec![]);
    assert_eq!(
        SearchData::decode(ENTRY_SCHEMA_VERSION, &blank).unwrap_err(),
        Error::CorruptEntry
    );

    let many = distinct_tags(MAX_TAGS + 1);
    let many = raw_search("s", None, many.iter().map(String::as_str).collect());
    assert_eq!(
        SearchData::decode(ENTRY_SCHEMA_VERSION, &many).unwrap_err(),
        Error::CorruptEntry
    );

    let long = "p".repeat(MAX_PASSWORD_LEN + 1);
    let long = postcard::to_allocvec(&RawSecretV1 {
        password: &long,
        notes: None,
    })
    .unwrap();
    assert_eq!(
        Secret::decode(ENTRY_SCHEMA_VERSION, &long).unwrap_err(),
        Error::CorruptEntry
    );
}

#[test]
fn decode_normalizes_like_new() {
    let bytes = raw_search(" GitHub ", Some(" "), vec!["a", "A"]);
    let data = SearchData::decode(ENTRY_SCHEMA_VERSION, &bytes).unwrap();
    assert_eq!(data.service(), "GitHub");
    assert_eq!(data.url(), None);
    assert_eq!(data.tags(), ["a"]);
}

#[test]
fn huge_length_prefix_is_corrupt() {
    // servicio "a", sin URL, sin usuario, número de etiquetas = u32::MAX.
    let bytes = [0x01, 0x61, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0x0F];
    assert_eq!(
        SearchData::decode(ENTRY_SCHEMA_VERSION, &bytes).unwrap_err(),
        Error::CorruptEntry
    );
}

// --- Propiedades ---

mod props {
    use proptest::collection::vec;
    use proptest::option;
    use proptest::prelude::*;

    use super::*;

    /// Texto arbitrario o palabras con espacios y mayúsculas mezcladas, para
    /// ejercitar el recorte y los duplicados de etiquetas.
    fn text() -> impl Strategy<Value = String> {
        prop_oneof![any::<String>(), "[ \t]{0,3}[a-zA-ZñÑ]{1,8}[ \t]{0,3}"]
    }

    fn as_strs(tags: &[String]) -> Vec<&str> {
        tags.iter().map(String::as_str).collect()
    }

    proptest! {
        #[test]
        fn search_data_round_trip_prop(
            service in text(),
            url in option::of(text()),
            username in option::of(text()),
            tags in vec(text(), 0..40),
        ) {
            if let Ok(data) =
                SearchData::new(&service, url.as_deref(), username.as_deref(), &as_strs(&tags))
            {
                let bytes = data.encode().unwrap();
                prop_assert_eq!(SearchData::decode(ENTRY_SCHEMA_VERSION, &bytes).unwrap(), data);
            }
        }

        #[test]
        fn search_data_new_is_idempotent_prop(
            service in text(),
            url in option::of(text()),
            username in option::of(text()),
            tags in vec(text(), 0..40),
        ) {
            if let Ok(data) =
                SearchData::new(&service, url.as_deref(), username.as_deref(), &as_strs(&tags))
            {
                let again = SearchData::new(
                    data.service(),
                    data.url(),
                    data.username(),
                    &as_strs(data.tags()),
                )
                .unwrap();
                prop_assert_eq!(again, data);
            }
        }

        #[test]
        fn secret_round_trip_prop(password in any::<String>(), notes in option::of(any::<String>())) {
            if let Ok(original) = Secret::new(
                SecretString::from(password.as_str()),
                notes.as_deref().map(SecretString::from),
            ) {
                let bytes = original.encode().unwrap();
                let decoded = Secret::decode(ENTRY_SCHEMA_VERSION, &bytes).unwrap();
                prop_assert_eq!(
                    decoded.password().expose_secret(),
                    original.password().expose_secret()
                );
                prop_assert_eq!(
                    decoded.notes().map(|n| n.expose_secret().to_owned()),
                    original.notes().map(|n| n.expose_secret().to_owned())
                );
            }
        }

        #[test]
        fn decode_never_panics_prop(bytes in vec(any::<u8>(), 0..512), version in any::<u16>()) {
            // Solo importa que termine sin panic.
            let _ = SearchData::decode(version, &bytes);
            let _ = Secret::decode(version, &bytes);
        }
    }
}

// --- Arreglos de la revisión final: sin copias antes de validar ---

/// `inner` es un subslice de `outer` (mismo búfer, sin copia).
fn borrows_from(outer: &str, inner: &str) -> bool {
    let range = outer.as_bytes().as_ptr_range();
    range.contains(&inner.as_ptr()) || inner.is_empty()
}

#[test]
fn validation_borrows_inputs_without_copying() {
    let service = " GitHub ";
    let url = " https://github.com ";
    let username = " alice ";
    let tags = [" a ", "A", "b"];
    let fields = validate_search_v1(service, Some(url), Some(username), &tags).unwrap();
    assert!(borrows_from(service, fields.service));
    assert!(borrows_from(url, fields.url.unwrap()));
    assert!(borrows_from(username, fields.username.unwrap()));
    assert_eq!(fields.tags, ["a", "b"]);
    assert!(borrows_from(tags[0], fields.tags[0]));
    assert!(borrows_from(tags[2], fields.tags[1]));
}

#[test]
fn validation_errors_after_earlier_fields_are_reported() {
    // La URL y el usuario son válidos; la etiqueta posterior falla.
    let long_tag = "t".repeat(MAX_TAG_LEN + 1);
    assert_eq!(
        validate_search_v1("s", Some("u"), Some("n"), &[&long_tag]).err(),
        Some(InvalidEntryField::TagTooLong)
    );
}

#[test]
fn same_tag_ignoring_case_handles_length_changes() {
    // 'Ⱥ' ocupa 2 bytes y su minúscula 'ⱥ', 3.
    assert!(same_tag_ignoring_case("Ⱥ", "ⱥ"));
    assert!(same_tag_ignoring_case("Trabajo", "TRABAJO"));
    assert!(!same_tag_ignoring_case("Médico", "Medico"));
    assert!(!same_tag_ignoring_case("ab", "abc"));
    assert_eq!(tags_of(&["Ⱥx", "ⱥX"]).unwrap(), ["Ⱥx"]);
}
