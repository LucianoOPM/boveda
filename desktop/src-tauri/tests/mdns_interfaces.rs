//! Verificación de `mdns-sd` (#100, tarea 1): con la configuración elegida, el
//! anuncio solo es visible por IPv4 en la interfaz privada, nunca por IPv6.
//!
//! Usa multidifusión real de la máquina, así que va `#[ignore]`:
//! `cargo test -p boveda-desktop --test mdns_interfaces -- --ignored --nocapture`
#![cfg(windows)]

use std::net::{IpAddr, Ipv4Addr, UdpSocket};
use std::time::{Duration, Instant};

use mdns_sd::{IfKind, ServiceDaemon, ServiceEvent, ServiceInfo};

const TYPE: &str = "_boveda._tcp.local.";
const WAIT: Duration = Duration::from_secs(6);

/// IPv4 de salida principal de la máquina. `connect` en UDP no envía nada.
fn local_ipv4() -> Ipv4Addr {
    let socket = UdpSocket::bind("0.0.0.0:0").expect("bind");
    socket.connect("192.0.2.1:9").expect("connect");
    match socket.local_addr().expect("local_addr").ip() {
        IpAddr::V4(ip) => ip,
        IpAddr::V6(_) => panic!("se esperaba IPv4"),
    }
}

fn service(ip: Ipv4Addr, instance: &str) -> ServiceInfo {
    ServiceInfo::new(
        TYPE,
        instance,
        &format!("{instance}.local."),
        ip.to_string().as_str(),
        47615,
        &[("v", "1")][..],
    )
    .expect("ServiceInfo")
}

/// Un consultante restringido a una familia de IP. `true` si ve el servicio,
/// ya sea por su PTR (`ServiceFound`) o resuelto: así no depende de que el
/// servicio tenga direcciones de esa familia.
fn resolves(only_v4: bool, instance: &str) -> bool {
    let querier = ServiceDaemon::new().expect("querier");
    if only_v4 {
        querier.disable_interface(IfKind::IPv6).expect("disable v6");
    } else {
        querier.disable_interface(IfKind::IPv4).expect("disable v4");
    }
    let events = querier.browse(TYPE).expect("browse");
    let deadline = Instant::now() + WAIT;
    let mut found = false;
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        match events.recv_timeout(left) {
            Ok(ServiceEvent::ServiceFound(_, fullname)) if fullname.contains(instance) => {
                found = true;
                break;
            }
            Ok(ServiceEvent::ServiceResolved(info)) if info.get_fullname().contains(instance) => {
                found = true;
                break;
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }
    let _ = querier.shutdown();
    found
}

/// Control: con la configuración por defecto, ¿se ve por IPv6? Solo informa.
#[test]
#[ignore = "usa multidifusión real"]
fn control_default_config_visibility() {
    let ip = local_ipv4();
    let announcer = ServiceDaemon::new().expect("announcer");
    announcer
        .register(service(ip, "boveda-ctrl0001"))
        .expect("register");
    println!("por defecto, visible por IPv4: {}", resolves(true, "boveda-ctrl0001"));
    println!("por defecto, visible por IPv6: {}", resolves(false, "boveda-ctrl0001"));
    let _ = announcer.shutdown();
}

/// Configuración candidata: solo la IPv4 privada habilitada.
#[test]
#[ignore = "usa multidifusión real"]
fn only_announces_on_private_ipv4() {
    let ip = local_ipv4();
    let announcer = ServiceDaemon::new().expect("announcer");
    announcer.disable_interface(IfKind::All).expect("disable all");
    announcer
        .enable_interface(IfKind::Addr(IpAddr::V4(ip)))
        .expect("enable addr");
    announcer
        .register(service(ip, "boveda-test0001"))
        .expect("register");

    let over_v4 = resolves(true, "boveda-test0001");
    let over_v6 = resolves(false, "boveda-test0001");
    let _ = announcer.shutdown();

    assert!(over_v4, "el control IPv4 debe ver el servicio (si no, la prueba no vale)");
    assert!(!over_v6, "el servicio no debe ser visible por IPv6");
}
