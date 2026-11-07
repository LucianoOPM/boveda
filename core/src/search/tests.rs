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
