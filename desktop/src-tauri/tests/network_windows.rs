//! Prueba del detector real de redes privadas (#100). Depende de las redes de
//! la máquina, así que va `#[ignore]`:
//! `cargo test -p boveda-desktop --test network_windows -- --ignored --nocapture`
#![cfg(windows)]

use boveda_desktop_lib::network::{NetworkProbe, NlmProbe, is_usable};

#[test]
#[ignore = "depende de las redes de la máquina"]
fn real_probe_returns_only_usable_addresses() {
    let mut probe = NlmProbe::new();
    let addresses = probe
        .private_addresses()
        .expect("el detector debe responder");
    println!("IPv4 de redes privadas: {addresses:?}");
    assert!(addresses.iter().all(|ip| is_usable(*ip)));
}
