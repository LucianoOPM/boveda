//! Pruebas unitarias de la búsqueda (`search.rs`).

use std::sync::atomic::{AtomicU8, Ordering as AtomicOrdering};

use super::*;
use crate::vault::{EntryId, SearchData};

// --- Utilitarios ------------------------------------------------------

/// Entrada en memoria; el id sale de un contador para que sean distintos.
fn entry(
    service: &str,
    url: Option<&str>,
    user: Option<&str>,
    tags: &[&str],
    last_used: Option<i64>,
) -> EntrySummary {
    static NEXT: AtomicU8 = AtomicU8::new(0);
    let id = EntryId::from_bytes([NEXT.fetch_add(1, AtomicOrdering::Relaxed); 16]);
    let search = SearchData::new(service, url, user, tags).unwrap();
    EntrySummary::for_test(id, search, 0, last_used)
}

/// Entrada con solo servicio y fecha de uso.
fn named(service: &str, last_used: Option<i64>) -> EntrySummary {
    entry(service, None, None, &[], last_used)
}

/// Servicios del resultado, en orden.
fn services(found: &[&EntrySummary]) -> Vec<String> {
    found
        .iter()
        .map(|e| e.search().service().to_owned())
        .collect()
}

// --- Sin texto --------------------------------------------------------

#[test]
fn empty_query_orders_by_recent_use() {
    let entries = [
        named("A", Some(10)),
        named("B", None),
        named("C", Some(30)),
        named("alfa", None),
    ];
    assert_eq!(services(&search(&entries, "")), ["C", "A", "alfa", "B"]);
}

#[test]
fn whitespace_query_is_empty() {
    let entries = [named("A", Some(10)), named("B", None), named("C", Some(30))];
    assert_eq!(
        services(&search(&entries, "   \t")),
        services(&search(&entries, ""))
    );
}

#[test]
fn recency_tie_breaks_by_service_ignoring_case() {
    let entries = [named("banco", Some(5)), named("Amazon", Some(5))];
    assert_eq!(services(&search(&entries, "")), ["Amazon", "banco"]);
}

// --- Dominio de la URL -------------------------------------------------

#[test]
fn domain_extracts_host() {
    let cases = [
        (
            "https://accounts.google.com/signin",
            Some("accounts.google.com"),
        ),
        ("google.com/mail", Some("google.com")),
        (
            "https://ana:secreto@banco.example:8443/x",
            Some("banco.example"),
        ),
        ("https://a@b@banco.example", Some("banco.example")),
        ("http://[::1]:8080/", Some("[::1]")),
        ("example.com?x=1#y", Some("example.com")),
        ("//google.com/x", Some("google.com")),
        ("ftp://host", Some("host")),
        ("https://", None),
        ("", None),
    ];
    for (url, expected) in cases {
        assert_eq!(domain(url), expected, "url: {url}");
    }
}

// --- Buscador principal ----------------------------------------------

#[test]
fn subsequence_gm_finds_gmail() {
    let entries = [named("Gmail", None), named("Facebook", None)];
    assert_eq!(services(&search(&entries, "gm")), ["Gmail"]);
}

#[test]
fn searches_domain_not_path() {
    let entries = [entry(
        "Cuenta",
        Some("https://accounts.google.com/signin"),
        None,
        &[],
        None,
    )];
    assert_eq!(services(&search(&entries, "google")), ["Cuenta"]);
    assert!(search(&entries, "signin").is_empty());
}

#[test]
fn finds_by_username() {
    let entries = [
        entry("Banco", None, Some("ana.perez"), &[], None),
        named("Otro", None),
    ];
    assert_eq!(services(&search(&entries, "anap")), ["Banco"]);
}

#[test]
fn finds_by_tag() {
    let entries = [
        entry("Gmail", None, None, &["trabajo"], None),
        entry("Outlook", None, None, &["casa"], None),
    ];
    assert_eq!(services(&search(&entries, "trab")), ["Gmail"]);
}

#[test]
fn words_match_across_fields() {
    let entries = [
        entry("Gmail", None, None, &["trabajo"], None),
        entry("Outlook", None, None, &["casa"], None),
    ];
    assert_eq!(services(&search(&entries, "gm trab")), ["Gmail"]);
    assert!(search(&entries, "gm casa").is_empty());
}

#[test]
fn unaccented_query_finds_accented_service() {
    let entries = [named("Teléfono", None), named("Banco", None)];
    assert_eq!(services(&search(&entries, "telefono")), ["Teléfono"]);
}

#[test]
fn better_score_wins_over_recency() {
    let entries = [named("Grupo Mail", Some(100)), named("Gmail", None)];
    assert_eq!(
        services(&search(&entries, "gmail")),
        ["Gmail", "Grupo Mail"]
    );
}

#[test]
fn equal_score_breaks_by_recency() {
    let older = named("Gmail", Some(10));
    let newer = named("Gmail", Some(20));
    let newer_id = newer.id();
    let entries = [older, newer];
    assert_eq!(search(&entries, "gmail")[0].id(), newer_id);
}

#[test]
fn non_latin_and_emoji_do_not_panic() {
    let entries = [
        named("Почта", None),
        named("📧 Correo", None),
        named("Cafe\u{301}", None),
    ];
    assert_eq!(services(&search(&entries, "почта")), ["Почта"]);
    assert_eq!(services(&search(&entries, "correo")), ["📧 Correo"]);
    assert_eq!(services(&search(&entries, "cafe")), ["Cafe\u{301}"]);
}

// --- Respaldo con OSA ------------------------------------------------

/// Gmail y Facebook, sin URL ni etiquetas.
fn gmail_and_facebook() -> [EntrySummary; 2] {
    [named("Gmail", None), named("Facebook", None)]
}

#[test]
fn gmial_finds_gmail_via_fallback() {
    assert_eq!(services(&search(&gmail_and_facebook(), "gmial")), ["Gmail"]);
}

#[test]
fn prefix_while_typing() {
    assert_eq!(services(&search(&gmail_and_facebook(), "gmia")), ["Gmail"]);
}

#[test]
fn fallback_uses_domain_words() {
    let entries = [entry(
        "Cuenta",
        Some("https://accounts.google.com"),
        None,
        &[],
        None,
    )];
    assert_eq!(services(&search(&entries, "googel")), ["Cuenta"]);
}

#[test]
fn fallback_folds_accents() {
    let entries = [named("Teléfono", None)];
    assert_eq!(services(&search(&entries, "telefnoo")), ["Teléfono"]);
}

#[test]
fn distant_query_finds_nothing() {
    let entries = [named("Gmail", None)];
    assert!(search(&entries, "face").is_empty());
}

#[test]
fn threshold_short_words() {
    let entries = [named("Gmail", None)];
    assert_eq!(services(&search(&entries, "gmaxl")), ["Gmail"]);
    assert!(search(&entries, "gmaxx").is_empty());
}

#[test]
fn threshold_long_words() {
    let entries = [named("Google", None)];
    assert_eq!(services(&search(&entries, "goozze")), ["Google"]);
    assert!(search(&entries, "goozzz").is_empty());
}

#[test]
fn short_query_has_no_fallback() {
    let entries = [named("Gmail", None)];
    assert!(search(&entries, "gx").is_empty());
}

#[test]
fn short_word_uses_nucleo_in_fallback() {
    let entries = [named("Gmail", None)];
    assert_eq!(services(&search(&entries, "gm gmial")), ["Gmail"]);
    assert!(search(&entries, "zq gmial").is_empty());
}

#[test]
fn fuzzy_result_preempts_fallback() {
    let entries = [named("Gmail", None), named("Gmial Pro", None)];
    assert_eq!(services(&search(&entries, "gmial")), ["Gmial Pro"]);
}

#[test]
fn operators_are_literal() {
    let entries = gmail_and_facebook();
    assert_eq!(services(&search(&entries, "!gm")), ["Gmail"]);
    assert_eq!(services(&search(&entries, "^gm")), ["Gmail"]);
}

#[test]
fn punctuation_only_query_finds_nothing() {
    let entries = gmail_and_facebook();
    assert!(search(&entries, "...").is_empty());
    assert!(search(&entries, "-").is_empty());
}

#[test]
fn very_long_query_does_not_panic() {
    let entries = gmail_and_facebook();
    assert!(search(&entries, &"a".repeat(5000)).is_empty());
}

#[test]
fn fold_removes_accents_and_case() {
    assert_eq!(fold("ÁrBol Ñandú").as_str(), "arbol nandu");
}

#[test]
fn max_distance_by_length() {
    assert_eq!(max_distance(0), None);
    assert_eq!(max_distance(2), None);
    assert_eq!(max_distance(3), Some(1));
    assert_eq!(max_distance(5), Some(1));
    assert_eq!(max_distance(6), Some(2));
    assert_eq!(max_distance(40), Some(2));
}

proptest::proptest! {
    #[test]
    fn search_never_panics(query in ".{0,64}", service in ".{1,40}") {
        proptest::prop_assume!(SearchData::new(&service, None, None, &[]).is_ok());
        let entries = [named(&service, None)];
        let _ = search(&entries, &query);
    }
}

// --- Hallazgos de la revisión final ----------------------------------

#[test]
fn scheme_inside_query_string_is_not_a_scheme() {
    assert_eq!(
        domain("banco.com/login?next=https://otro.com"),
        Some("banco.com")
    );
    assert_eq!(domain("banco.com?r=https://otro.com"), Some("banco.com"));
    assert_eq!(
        domain("https://banco.com/login?next=https://otro.com"),
        Some("banco.com")
    );
}

#[test]
fn long_pasted_query_is_fast_with_many_entries() {
    let entries: Vec<EntrySummary> = (0..2000)
        .map(|i| {
            entry(
                &format!("servicio{i}"),
                Some("https://accounts.example.com/x"),
                Some("usuario@example.com"),
                &["trabajo", "correo"],
                None,
            )
        })
        .collect();
    let query = "a".repeat(5000);
    let start = std::time::Instant::now();
    assert!(search(&entries, &query).is_empty());
    let elapsed = start.elapsed();
    assert!(elapsed.as_millis() < 500, "tardó {elapsed:?}");
}
