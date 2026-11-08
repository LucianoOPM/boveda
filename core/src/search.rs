//! Búsqueda tolerante a errores sobre los datos de búsqueda descifrados.
//!
//! Trabaja solo sobre la caché de [`EntrySummary`] que `Vault::unlock` ya
//! descifró: no lee el disco ni toca el secreto. Diseño en
//! `docs/superpowers/specs/2026-11-07-busqueda-design.md`.
//!
//! - Sin texto, todas las entradas: las usadas más recientemente primero.
//! - Con texto, `nucleo-matcher` por subsecuencia: cada palabra debe coincidir
//!   con algún campo (servicio, dominio de la URL, usuario o una etiqueta).
//! - Si nucleo no encuentra nada, respaldo con distancia OSA (`strsim`) por
//!   palabras, para errores de escritura ("gmial" encuentra "Gmail").
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
    let found = fuzzy(entries, query);
    if found.is_empty() {
        typo_fallback(entries, query)
    } else {
        found
    }
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
///
/// Un `://` solo es esquema si no hay `/`, `?` ni `#` antes: en
/// `banco.com/login?next=https://otro.com` el dominio es `banco.com`.
fn domain(url: &str) -> Option<&str> {
    let rest = match url.find("://") {
        Some(i) if !url[..i].contains(['/', '?', '#']) => &url[i + 3..],
        _ => url,
    };
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

/// `text` sin acentos latinos y en minúsculas.
///
/// Mide primero el tamaño exacto y luego llena: el `String` nunca reasigna,
/// así que no deja copias sin borrar.
fn fold(text: &str) -> Zeroizing<String> {
    let folded = || {
        text.chars()
            .map(nucleo_matcher::chars::normalize)
            .flat_map(char::to_lowercase)
    };
    let len = folded().map(char::len_utf8).sum();
    let mut out = Zeroizing::new(String::with_capacity(len));
    out.extend(folded());
    out
}

/// Palabras de un texto plegado: se parte por todo lo que no sea letra o
/// número ("accounts.google.com" da accounts, google y com).
fn words(folded: &str) -> impl Iterator<Item = &str> {
    folded
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
}

/// Distancia máxima tolerada para una palabra de `chars` caracteres: 1 con 3
/// a 5, 2 con 6 o más. Con menos de 3 no hay respaldo.
fn max_distance(chars: usize) -> Option<usize> {
    match chars {
        0..=2 => None,
        3..=5 => Some(1),
        _ => Some(2),
    }
}

/// Los primeros `chars` caracteres de `word`.
fn prefix(word: &str, chars: usize) -> &str {
    word.char_indices()
        .nth(chars)
        .map_or(word, |(end, _)| &word[..end])
}

/// Respaldo para errores de escritura: cada palabra de `query` debe estar a
/// distancia OSA tolerable de alguna palabra de los campos, completa o de su
/// inicio del mismo largo (lo que se está escribiendo). Las palabras de menos
/// de 3 caracteres coinciden solo si nucleo las encuentra.
fn typo_fallback<'a>(entries: &'a [EntrySummary], query: &str) -> Vec<&'a EntrySummary> {
    let query = fold(query);
    // Cada palabra con su tolerancia, o con su átomo de nucleo si es corta.
    // Los átomos se crean una sola vez, no por entrada.
    let query_words: Vec<(&str, Result<usize, Atom>)> = words(&query)
        .map(|q| (q, max_distance(q.chars().count()).ok_or_else(|| atom(q))))
        .collect();
    if query_words.is_empty() {
        return Vec::new();
    }
    let mut matcher = Matcher::new(Config::DEFAULT);
    let mut buf = Zeroizing::new(Vec::with_capacity(char_capacity(entries)));

    let mut found: Vec<(usize, &EntrySummary)> = entries
        .iter()
        .filter_map(|entry| {
            let folded: Vec<Zeroizing<String>> = fields(entry.search()).map(fold).collect();
            let field_words: Vec<(&str, usize)> = folded
                .iter()
                .flat_map(|f| words(f))
                .map(|w| (w, w.chars().count()))
                .collect();
            query_words
                .iter()
                .map(|(q, rule)| match rule {
                    Ok(limit) => {
                        let n = q.chars().count();
                        field_words
                            .iter()
                            // La distancia es al menos la diferencia de largos:
                            // si `m + limit < n`, ni la palabra ni su inicio
                            // pueden coincidir. Evita que un texto pegado muy
                            // largo cueste O(n·m) por cada palabra.
                            .filter(|(_, m)| m + limit >= n)
                            .map(|(w, _)| {
                                strsim::osa_distance(q, w)
                                    .min(strsim::osa_distance(q, prefix(w, n)))
                            })
                            .min()
                            .filter(|d| d <= limit)
                    }
                    Err(atom) => {
                        atom_matches(atom, entry.search(), &mut buf, &mut matcher).map(|_| 0)
                    }
                })
                .try_fold(0usize, |total, d| Some(total + d?))
                .map(|total| (total, entry))
        })
        .collect();
    found.sort_by(|(da, a), (db, b)| da.cmp(db).then_with(|| by_recency(a, b)));
    found.into_iter().map(|(_, entry)| entry).collect()
}

#[cfg(test)]
mod tests;
