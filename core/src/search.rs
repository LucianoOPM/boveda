//! Búsqueda tolerante a errores sobre los datos de búsqueda descifrados.
//!
//! Trabaja solo sobre la caché de [`EntrySummary`] que `Vault::unlock` ya
//! descifró: no lee el disco ni toca el secreto. Diseño en
//! `docs/superpowers/specs/2026-11-07-busqueda-design.md`.
//!
//! - Sin texto, todas las entradas: las usadas más recientemente primero.

use std::cmp::Ordering;

use crate::vault::EntrySummary;

/// Entradas que coinciden con `query`, la mejor primero.
pub(crate) fn search<'a>(entries: &'a [EntrySummary], query: &str) -> Vec<&'a EntrySummary> {
    let query = query.trim();
    if query.is_empty() {
        let mut all: Vec<_> = entries.iter().collect();
        all.sort_by(|a, b| by_recency(a, b));
        return all;
    }
    Vec::new()
}

/// `last_used_at` de mayor a menor, con las nunca usadas al final; empate
/// por servicio sin distinguir mayúsculas.
///
/// Compara con iteradores para no reservar copias del texto en minúsculas.
fn by_recency(a: &EntrySummary, b: &EntrySummary) -> Ordering {
    // `None` es menor que cualquier `Some`: al invertir quedan al final.
    b.last_used_at()
        .cmp(&a.last_used_at())
        .then_with(|| lowercase(a.search().service()).cmp(lowercase(b.search().service())))
}

/// Caracteres de `text` en minúsculas, sin reservar memoria.
fn lowercase(text: &str) -> impl Iterator<Item = char> + '_ {
    text.chars().flat_map(char::to_lowercase)
}

#[cfg(test)]
mod tests;
