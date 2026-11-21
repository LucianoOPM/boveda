//! Elección de la dirección que va en el QR (#101).

use std::collections::BTreeSet;
use std::net::Ipv4Addr;

use crate::network::Candidate;

/// Elige la dirección del QR entre los candidatos que el servidor escucha ahora.
///
/// Solo cuentan los candidatos elegibles (activos, físicos, Ethernet o Wi-Fi) cuya
/// IP está en `listening`: una interfaz que no lo es nunca se elige, tenga la
/// métrica que tenga. Entre ellos, primero los que tienen puerta de enlace; luego
/// la menor métrica; el empate lo rompe la IPv4 numéricamente menor. Sin puerta de
/// enlace en ninguno (red local sin router), se usa la menor métrica entre los
/// elegibles. Sin elegibles, `None`: no hay QR.
pub fn choose_address(
    candidates: &[Candidate],
    listening: &BTreeSet<Ipv4Addr>,
) -> Option<Ipv4Addr> {
    candidates
        .iter()
        .filter(|candidate| candidate.eligible && listening.contains(&candidate.ip))
        .min_by_key(|candidate| (!candidate.has_gateway, candidate.metric, candidate.ip))
        .map(|candidate| candidate.ip)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(last: u8) -> Ipv4Addr {
        Ipv4Addr::new(192, 168, 1, last)
    }

    fn cand(last: u8, eligible: bool, has_gateway: bool, metric: u32) -> Candidate {
        Candidate {
            ip: ip(last),
            eligible,
            has_gateway,
            metric,
        }
    }

    fn all(candidates: &[Candidate]) -> BTreeSet<Ipv4Addr> {
        candidates.iter().map(|c| c.ip).collect()
    }

    fn choose(candidates: &[Candidate]) -> Option<Ipv4Addr> {
        choose_address(candidates, &all(candidates))
    }

    #[test]
    fn prefers_gateway_over_lower_metric() {
        let list = [cand(1, true, false, 1), cand(2, true, true, 50)];
        assert_eq!(choose(&list), Some(ip(2)));
    }

    #[test]
    fn lowest_metric_among_gateways() {
        let list = [
            cand(1, true, true, 50),
            cand(2, true, true, 10),
            cand(3, true, true, 30),
        ];
        assert_eq!(choose(&list), Some(ip(2)));
    }

    #[test]
    fn ties_break_on_lowest_ip() {
        let list = [cand(9, true, true, 10), cand(4, true, true, 10)];
        assert_eq!(choose(&list), Some(ip(4)));
    }

    #[test]
    fn without_gateway_uses_lowest_metric_among_eligible() {
        let list = [cand(1, true, false, 40), cand(2, true, false, 20)];
        assert_eq!(choose(&list), Some(ip(2)));
    }

    #[test]
    fn ineligible_is_never_chosen_even_with_lowest_metric() {
        let list = [cand(1, false, true, 1), cand(2, true, true, 100)];
        assert_eq!(choose(&list), Some(ip(2)));
        let list = [cand(1, false, false, 1), cand(2, true, false, 100)];
        assert_eq!(choose(&list), Some(ip(2)));
    }

    #[test]
    fn only_ineligible_gives_none() {
        assert_eq!(
            choose(&[cand(1, false, true, 1), cand(2, false, false, 2)]),
            None
        );
    }

    #[test]
    fn address_not_listening_is_ignored() {
        let list = [cand(1, true, true, 1), cand(2, true, true, 100)];
        let listening = BTreeSet::from([ip(2)]);
        assert_eq!(choose_address(&list, &listening), Some(ip(2)));
    }

    #[test]
    fn empty_gives_none() {
        assert_eq!(choose(&[]), None);
        assert_eq!(
            choose_address(&[cand(1, true, true, 1)], &BTreeSet::new()),
            None
        );
    }
}
