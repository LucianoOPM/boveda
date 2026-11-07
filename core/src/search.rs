//! Búsqueda tolerante a errores sobre los datos de búsqueda descifrados.
//!
//! Trabaja solo sobre la caché de [`EntrySummary`] que `Vault::unlock` ya
//! descifró: no lee el disco ni toca el secreto. Diseño en
//! `docs/superpowers/specs/2026-11-07-busqueda-design.md`.
//!
//! - Sin texto, todas las entradas: las usadas más recientemente primero.
//! - Con texto, `nucleo-matcher` por subsecuencia: cada palabra debe coincidir
//!   con algún campo (servicio, dominio de la URL, usuario o una etiqueta).
//!
//! **Memoria.** Los búferes propios con texto plano van en `Zeroizing`. Las
//! copias internas de las librerías no se pueden borrar: el texto de cada
//! `Atom`, lo que el `Matcher` copia en su búfer y los vectores que reserva
//! `strsim`. Es una limitación aceptada: son datos de búsqueda, no secretos, y
//! los mismos textos ya cruzan a Dart, donde el borrado es de mejor esfuerzo.

use std::cmp::Ordering;

use nucleo_matcher::pattern::{Atom, AtomKind, CaseMatching, Normalization};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use zeroize::Zeroizing;

use crate::vault::{EntrySummary, SearchData};

/// Entradas que coinciden con `query`, la mejor primero.
pub(crate) fn search<'a>(entries: &'a [EntrySummary], query: &str) -> Vec<&'a EntrySummary> {
    let query = query.trim();
    if query.is_empty() {
        let mut all: Vec<_> = entries.iter().collect();
        all.sort_by(|a, b| by_recency(a, b));
        return all;
    }
    fuzzy(entries, query)
}

/// `last_used_at` de mayor a menor, con las nunca usadas al final; empate
/// por servicio sin distinguir mayúsculas.
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

/// Dominio de una URL, como trozo de la original (sin reservar memoria).
///
/// Quita el esquema (`algo://`) y las `/` iniciales, corta en el primer `/`,
/// `?` o `#`, se queda con lo que sigue a la última `@` (así `usuario:clave@`
/// nunca entra en la búsqueda) y quita el puerto. Acepta URLs sin esquema.
fn domain(url: &str) -> Option<&str> {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let rest = rest.trim_start_matches('/');
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = authority.rsplit('@').next().unwrap_or("");
    let host = if host.starts_with('[') {
        // IPv6: hasta el `]` incluido.
        host.find(']').map_or(host, |end| &host[..=end])
    } else {
        host.rsplit_once(':').map_or(host, |(host, _)| host)
    };
    (!host.is_empty()).then_some(host)
}

/// Campos en los que se busca: servicio, dominio, usuario y cada etiqueta.
fn fields(search: &SearchData) -> impl Iterator<Item = &str> {
    std::iter::once(search.service())
        .chain(search.url().and_then(domain))
        .chain(search.username())
        .chain(search.tags().iter().map(String::as_str))
}

/// La mayor cantidad de caracteres entre todos los campos de `entries`.
///
/// `Utf32Str::new` agrupa por grafemas, que nunca son más que los
/// caracteres: con esta capacidad su búfer no reasigna ni deja copias.
fn char_capacity(entries: &[EntrySummary]) -> usize {
    entries
        .iter()
        .flat_map(|e| fields(e.search()))
        .map(|field| field.chars().count())
        .max()
        .unwrap_or(0)
}

/// Átomo de nucleo para una palabra: subsecuencia, sin distinguir mayúsculas
/// y con `ä` igual a `a`. Sin operadores ni escapes: todo es literal.
fn atom(word: &str) -> Atom {
    Atom::new(
        word,
        CaseMatching::Ignore,
        Normalization::Smart,
        AtomKind::Fuzzy,
        false,
    )
}

/// Mejor puntaje de `atom` entre los campos de `search`, o `None` si no
/// coincide con ninguno.
fn atom_matches(
    atom: &Atom,
    search: &SearchData,
    buf: &mut Vec<char>,
    matcher: &mut Matcher,
) -> Option<u16> {
    fields(search)
        .filter_map(|field| atom.score(Utf32Str::new(field, buf), matcher))
        .max()
}

/// Buscador principal: cada palabra de `query` debe coincidir con algún
/// campo; el puntaje es la suma del mejor de cada palabra.
fn fuzzy<'a>(entries: &'a [EntrySummary], query: &str) -> Vec<&'a EntrySummary> {
    let atoms: Vec<Atom> = query.split_whitespace().map(atom).collect();
    let mut matcher = Matcher::new(Config::DEFAULT);
    let mut buf = Zeroizing::new(Vec::with_capacity(char_capacity(entries)));

    let mut found: Vec<(u32, &EntrySummary)> = entries
        .iter()
        .filter_map(|entry| {
            atoms
                .iter()
                .map(|atom| atom_matches(atom, entry.search(), &mut buf, &mut matcher))
                .try_fold(0u32, |total, score| Some(total + u32::from(score?)))
                .map(|total| (total, entry))
        })
        .collect();
    found.sort_by(|(sa, a), (sb, b)| sb.cmp(sa).then_with(|| by_recency(a, b)));
    found.into_iter().map(|(_, entry)| entry).collect()
}

#[cfg(test)]
mod tests;
