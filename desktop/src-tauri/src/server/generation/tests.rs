use std::collections::BTreeSet;
use std::io::{ErrorKind, Read};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::sync::atomic::Ordering;
use std::thread;
use std::time::{Duration, Instant};

use vault_core::protocol::{AckOutcome, KeyPair, SendError};

use super::Generation;
use crate::server::Config;
use crate::server::test_support::{Fixture, PASSWORD, config, wait_until};

const LOCAL: Ipv4Addr = Ipv4Addr::new(127, 0, 0, 1);
const SECOND: Duration = Duration::from_secs(1);

fn start(fixture: &Fixture, config: &Config) -> Generation {
    let ips = BTreeSet::from([LOCAL]);
    Generation::start(config, &ips, &fixture.deps(), 1).expect("la generación debe subir")
}

fn addr(generation: &Generation) -> SocketAddr {
    SocketAddr::from((LOCAL, generation.port()))
}

/// Conexión que no envía nada.
fn silent(generation: &Generation) -> TcpStream {
    TcpStream::connect_timeout(&addr(generation), SECOND * 2).expect("conectar")
}

/// ¿El servidor cerró la conexión sin escribir ni un byte?
fn closed_without_data(mut stream: TcpStream) -> bool {
    stream.set_read_timeout(Some(SECOND * 2)).expect("plazo");
    let mut byte = [0u8; 1];
    match stream.read(&mut byte) {
        Ok(0) => true,
        Err(error) => matches!(
            error.kind(),
            ErrorKind::ConnectionReset | ErrorKind::ConnectionAborted
        ),
        Ok(_) => false,
    }
}

#[test]
fn paired_phone_gets_copied_after_sink_ok() {
    let fixture = Fixture::new();
    let generation = start(&fixture, &config());
    assert_eq!(fixture.send(addr(&generation)), Ok(AckOutcome::Copied));
    assert_eq!(*fixture.sink.received.lock().expect("candado"), [PASSWORD]);
    generation.stop();
}

#[test]
fn sink_failure_answers_failed() {
    let fixture = Fixture::new();
    fixture.sink.fail.store(true, Ordering::SeqCst);
    let generation = start(&fixture, &config());
    assert_eq!(fixture.send(addr(&generation)), Ok(AckOutcome::Failed));
    generation.stop();
}

#[test]
fn unpaired_phone_is_closed_and_sink_untouched() {
    let fixture = Fixture::new();
    let generation = start(&fixture, &config());
    let stranger = KeyPair::generate().expect("llave");
    assert_eq!(
        fixture.send_as(&stranger, addr(&generation)),
        Err(SendError::Closed)
    );
    assert!(fixture.sink.received.lock().expect("candado").is_empty());
    assert!(!fixture.sink.entered.load(Ordering::SeqCst));
    generation.stop();
}

#[test]
fn fifth_simultaneous_connection_is_closed_without_reading() {
    let fixture = Fixture::new();
    let generation = start(&fixture, &config());
    let held: Vec<_> = (0..4).map(|_| silent(&generation)).collect();
    assert!(wait_until(
        || generation.active_connections() == 4,
        SECOND * 2
    ));
    assert!(closed_without_data(silent(&generation)));
    assert_eq!(generation.active_connections(), 4);
    assert!(!fixture.sink.entered.load(Ordering::SeqCst));
    drop(held);
    generation.stop();
}

#[test]
fn silent_client_times_out_and_frees_its_slot() {
    let fixture = Fixture::new();
    let config = Config {
        max_connections: 1,
        ..config()
    };
    let generation = start(&fixture, &config);
    let quiet = silent(&generation);
    assert!(wait_until(|| generation.active_connections() == 1, SECOND));
    // Mientras ocupa la única ranura, otro cliente se rechaza.
    assert!(closed_without_data(silent(&generation)));
    // `step` de 400 ms: el cliente mudo se cierra solo.
    assert!(wait_until(
        || generation.active_connections() == 0,
        SECOND * 3
    ));
    assert_eq!(fixture.send(addr(&generation)), Ok(AckOutcome::Copied));
    drop(quiet);
    generation.stop();
}

#[test]
fn slot_is_released_when_worker_errors() {
    let fixture = Fixture::new();
    let config = Config {
        max_connections: 1,
        ..config()
    };
    let generation = start(&fixture, &config);
    let stranger = KeyPair::generate().expect("llave");
    for _ in 0..3 {
        assert_eq!(
            fixture.send_as(&stranger, addr(&generation)),
            Err(SendError::Closed)
        );
        assert!(wait_until(|| generation.active_connections() == 0, SECOND));
    }
    assert_eq!(fixture.send(addr(&generation)), Ok(AckOutcome::Copied));
    generation.stop();
}

#[test]
fn slot_is_released_when_worker_panics() {
    let fixture = Fixture::new();
    fixture.sink.panic.store(true, Ordering::SeqCst);
    let config = Config {
        max_connections: 1,
        ..config()
    };
    let generation = start(&fixture, &config);
    // El trabajador entra en pánico dentro del sink: el cliente ve un cierre.
    assert!(fixture.send(addr(&generation)).is_err());
    assert!(wait_until(|| generation.active_connections() == 0, SECOND));
    fixture.sink.panic.store(false, Ordering::SeqCst);
    assert_eq!(fixture.send(addr(&generation)), Ok(AckOutcome::Copied));
    generation.stop();
}

#[test]
fn poisoned_paired_mutex_rejects_everyone() {
    let fixture = Fixture::new();
    let paired = std::sync::Arc::clone(&fixture.paired);
    let _ = thread::spawn(move || {
        let _guard = paired.lock().expect("candado");
        panic!("envenenar el candado");
    })
    .join();
    assert!(fixture.paired.is_poisoned());
    let generation = start(&fixture, &config());
    assert_eq!(fixture.send(addr(&generation)), Err(SendError::Closed));
    assert!(!fixture.sink.entered.load(Ordering::SeqCst));
    generation.stop();
}

#[test]
fn stop_cancels_active_workers_and_is_bounded() {
    let fixture = Fixture::new();
    let config = Config {
        // Plazos largos: solo la cancelación puede cortar al trabajador.
        timeouts: vault_core::protocol::SendTimeouts::default(),
        stop_wait: SECOND * 2,
        ..config()
    };
    let generation = start(&fixture, &config);
    let quiet = silent(&generation);
    assert!(wait_until(|| generation.active_connections() == 1, SECOND));
    let started = Instant::now();
    generation.stop();
    assert!(
        started.elapsed() < SECOND * 2,
        "stop tardó {:?}",
        started.elapsed()
    );
    assert!(closed_without_data(quiet));
}

#[test]
fn stop_does_not_hang_on_a_stuck_sink() {
    let fixture = Fixture::new();
    fixture.sink.hold.store(true, Ordering::SeqCst);
    let config = Config {
        stop_wait: Duration::from_millis(300),
        ..config()
    };
    let generation = start(&fixture, &config);
    let target = addr(&generation);
    thread::scope(|scope| {
        let sender = scope.spawn(|| fixture.send(target));
        assert!(wait_until(
            || fixture.sink.entered.load(Ordering::SeqCst),
            SECOND * 2
        ));
        let started = Instant::now();
        generation.stop();
        assert!(
            started.elapsed() < SECOND,
            "stop tardó {:?}",
            started.elapsed()
        );
        // Se libera el sink para que el trabajador y el cliente terminen.
        fixture.sink.hold.store(false, Ordering::SeqCst);
        let _ = sender.join();
    });
}

#[test]
fn new_generation_has_a_fresh_token() {
    let fixture = Fixture::new();
    let first = start(&fixture, &config());
    let first_token = first.token().clone();
    assert!(!first_token.is_cancelled());
    first.stop();
    assert!(first_token.is_cancelled());
    let second = start(&fixture, &config());
    assert!(!second.token().is_cancelled());
    assert_eq!(fixture.send(addr(&second)), Ok(AckOutcome::Copied));
    second.stop();
}

#[test]
fn is_alive_until_stopped() {
    let fixture = Fixture::new();
    let generation = start(&fixture, &config());
    assert!(generation.is_alive());
    generation.stop();
}

#[test]
fn announces_once_after_binding_with_its_port_and_ips() {
    let fixture = Fixture::new();
    let generation = start(&fixture, &config());
    assert_eq!(fixture.announce.calls.load(Ordering::SeqCst), 1);
    let service = fixture
        .announce
        .last
        .lock()
        .expect("candado")
        .clone()
        .expect("servicio anunciado");
    assert_eq!(service.port, generation.port());
    assert_eq!(service.addrs, BTreeSet::from([LOCAL]));
    assert!(generation.is_announced());
    generation.stop();
}

#[test]
fn withdraw_happens_before_listeners_close() {
    let fixture = Fixture::new();
    let generation = start(&fixture, &config());
    generation.stop();
    let events = fixture.announce.events.lock().expect("candado");
    assert_eq!(
        *events,
        ["announce:ok", "withdraw:listener_open"],
        "el anuncio se retira con el listener todavía abierto"
    );
}

#[test]
fn mdns_failure_does_not_stop_the_listener() {
    let fixture = Fixture::new();
    fixture
        .announce
        .fail_remaining
        .store(1000, Ordering::SeqCst);
    let generation = start(&fixture, &config());
    assert!(!generation.is_announced());
    assert!(generation.is_alive());
    assert_eq!(fixture.send(addr(&generation)), Ok(AckOutcome::Copied));
    assert!(!generation.token().is_cancelled());
    generation.stop();
}

#[test]
fn mdns_retries_without_rebuilding_generation() {
    let fixture = Fixture::new();
    fixture.announce.fail_remaining.store(2, Ordering::SeqCst);
    let mut generation = start(&fixture, &config());
    let port = generation.port();
    let token = generation.token().clone();
    assert_eq!(fixture.announce.calls.load(Ordering::SeqCst), 1);

    // Antes de que toque el reintento no se llama otra vez.
    generation.poll_announce(Instant::now());
    assert_eq!(fixture.announce.calls.load(Ordering::SeqCst), 1);

    let mut later = Instant::now();
    for _ in 0..10 {
        later += Duration::from_secs(1);
        generation.poll_announce(later);
        if generation.is_announced() {
            break;
        }
    }
    assert!(generation.is_announced());
    assert_eq!(fixture.announce.calls.load(Ordering::SeqCst), 3);
    assert_eq!(fixture.announce.created.load(Ordering::SeqCst), 1);
    assert_eq!(generation.port(), port);
    assert!(!token.is_cancelled());
    generation.stop();
}

#[test]
fn no_announcement_without_valid_listener() {
    let fixture = Fixture::new();
    // 203.0.113.1 (TEST-NET-3) no está asignada a esta máquina.
    let ips = BTreeSet::from([LOCAL, Ipv4Addr::new(203, 0, 113, 1)]);
    let result = Generation::start(&config(), &ips, &fixture.deps(), 1);
    assert!(result.is_err());
    assert_eq!(fixture.announce.calls.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.announce.created.load(Ordering::SeqCst), 0);
}
