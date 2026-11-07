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
