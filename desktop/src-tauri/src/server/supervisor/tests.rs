use std::collections::BTreeSet;
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use vault_core::protocol::AckOutcome;

use super::{ServerSupervisor, Supervisor};
use crate::network::{FakeProbe, ProbeError};
use crate::server::test_support::{Fixture, config, wait_until};

const A: Ipv4Addr = Ipv4Addr::new(127, 0, 0, 1);
const B: Ipv4Addr = Ipv4Addr::new(127, 0, 0, 2);
const UNASSIGNED: Ipv4Addr = Ipv4Addr::new(203, 0, 113, 1);

fn set(ips: &[Ipv4Addr]) -> Result<BTreeSet<Ipv4Addr>, ProbeError> {
    // Sin `normalize`: las pruebas usan loopback, que el detector real descarta.
    Ok(ips.iter().copied().collect())
}

fn supervisor(fixture: &Fixture, probe: &FakeProbe) -> Supervisor {
    Supervisor::new(Box::new(probe.clone()), fixture.deps(), config())
}

fn addr(supervisor: &Supervisor) -> SocketAddr {
    SocketAddr::from((A, supervisor.current_port().expect("puerto")))
}

fn is_listening(port: u16) -> bool {
    TcpStream::connect_timeout(&SocketAddr::from((A, port)), Duration::from_millis(300)).is_ok()
}

#[test]
fn empty_to_one_starts_a_generation() {
    let fixture = Fixture::new();
    let probe = FakeProbe::new();
    let mut supervisor = supervisor(&fixture, &probe);
    let t0 = Instant::now();
    supervisor.tick(t0);
    assert_eq!(supervisor.current_port(), None);
    probe.set(set(&[A]));
    supervisor.tick(t0);
    assert_eq!(supervisor.generations_started(), 1);
    assert_eq!(fixture.send(addr(&supervisor)), Ok(AckOutcome::Copied));
    supervisor.shutdown();
}

#[test]
fn one_to_empty_stops_and_port_is_released() {
    let fixture = Fixture::new();
    let probe = FakeProbe::new();
    probe.set(set(&[A]));
    let mut supervisor = supervisor(&fixture, &probe);
    let t0 = Instant::now();
    supervisor.tick(t0);
    let port = supervisor.current_port().expect("puerto");
    assert!(is_listening(port));
    probe.set(set(&[]));
    supervisor.tick(t0);
    assert_eq!(supervisor.current_port(), None);
    assert!(!is_listening(port));
    // El anuncio se retiró al bajar, con el listener todavía abierto.
    let events = fixture.announce.events.lock().expect("candado").clone();
    assert!(
        events.iter().any(|e| e == "withdraw:listener_open"),
        "{events:?}"
    );
}

#[test]
fn same_set_in_other_order_does_nothing() {
    let fixture = Fixture::new();
    let probe = FakeProbe::new();
    probe.set(set(&[A, B]));
    let mut supervisor = supervisor(&fixture, &probe);
    let t0 = Instant::now();
    supervisor.tick(t0);
    probe.set(set(&[B, A]));
    supervisor.tick(t0);
    supervisor.tick(t0);
    assert_eq!(supervisor.generations_started(), 1);
    supervisor.shutdown();
}

#[test]
fn added_ip_restarts_generation() {
    let fixture = Fixture::new();
    let probe = FakeProbe::new();
    probe.set(set(&[A]));
    let mut supervisor = supervisor(&fixture, &probe);
    let t0 = Instant::now();
    supervisor.tick(t0);
    probe.set(set(&[A, B]));
    supervisor.tick(t0);
    assert_eq!(supervisor.generations_started(), 2);
    supervisor.shutdown();
}

#[test]
fn replaced_ip_restarts_generation() {
    let fixture = Fixture::new();
    let probe = FakeProbe::new();
    probe.set(set(&[A]));
    let mut supervisor = supervisor(&fixture, &probe);
    let t0 = Instant::now();
    supervisor.tick(t0);
    probe.set(set(&[B]));
    supervisor.tick(t0);
    assert_eq!(supervisor.generations_started(), 2);
    supervisor.shutdown();
}

#[test]
fn private_public_private_cycle_gets_a_new_token() {
    let fixture = Fixture::new();
    let probe = FakeProbe::new();
    probe.set(set(&[A]));
    let mut supervisor = supervisor(&fixture, &probe);
    let t0 = Instant::now();
    supervisor.tick(t0);
    let first_token = supervisor.current_token().expect("token");
    let first_port = supervisor.current_port().expect("puerto");

    probe.set(set(&[])); // la red pasó a pública
    supervisor.tick(t0);
    assert!(first_token.is_cancelled());
    assert!(!is_listening(first_port));

    probe.set(set(&[A])); // vuelve una red privada
    supervisor.tick(t0);
    let second_token = supervisor.current_token().expect("token nuevo");
    assert!(!second_token.is_cancelled());
    assert_eq!(supervisor.generations_started(), 2);
    assert_eq!(fixture.send(addr(&supervisor)), Ok(AckOutcome::Copied));
    supervisor.shutdown();
}

#[test]
fn probe_error_stops_a_running_server() {
    let fixture = Fixture::new();
    let probe = FakeProbe::new();
    probe.set(set(&[A]));
    let mut supervisor = supervisor(&fixture, &probe);
    let t0 = Instant::now();
    supervisor.tick(t0);
    let port = supervisor.current_port().expect("puerto");
    probe.set(Err(ProbeError));
    supervisor.tick(t0);
    assert_eq!(supervisor.current_port(), None);
    assert!(!is_listening(port));
}

#[test]
fn first_tick_with_probe_error_or_empty_set_starts_nothing() {
    let fixture = Fixture::new();
    let probe = FakeProbe::new();
    probe.set(Err(ProbeError));
    let mut supervisor = supervisor(&fixture, &probe);
    let t0 = Instant::now();
    supervisor.tick(t0);
    probe.set(set(&[]));
    supervisor.tick(t0 + Duration::from_secs(1));
    assert_eq!(supervisor.current_port(), None);
    assert_eq!(supervisor.generations_started(), 0);
    assert_eq!(fixture.announce.created.load(Ordering::SeqCst), 0);
}

#[test]
fn dead_listener_is_rebuilt_with_backoff() {
    let fixture = Fixture::new();
    let probe = FakeProbe::new();
    probe.set(set(&[A]));
    let mut supervisor = supervisor(&fixture, &probe);
    let t0 = Instant::now();
    supervisor.tick(t0);
    // Se mata el hilo de `accept` cancelando su token sin pasar por el supervisor.
    supervisor.current_token().expect("token").cancel();
    assert!(wait_until(
        || !supervisor.current_is_alive(),
        Duration::from_secs(2)
    ));

    supervisor.tick(t0); // detecta la caída y la baja; la espera empieza aquí
    assert_eq!(supervisor.current_port(), None);
    supervisor.tick(t0 + Duration::from_millis(5)); // aún no toca (base de 10 ms)
    assert_eq!(supervisor.generations_started(), 1);
    supervisor.tick(t0 + Duration::from_millis(11));
    assert_eq!(supervisor.generations_started(), 2);
    assert_eq!(fixture.send(addr(&supervisor)), Ok(AckOutcome::Copied));
    supervisor.shutdown();
}

#[test]
fn bind_failure_retries_next_ticks_without_partial_listening() {
    let fixture = Fixture::new();
    let probe = FakeProbe::new();
    probe.set(set(&[A, UNASSIGNED]));
    let mut supervisor = supervisor(&fixture, &probe);
    let t0 = Instant::now();
    for step in 0..5 {
        supervisor.tick(t0 + Duration::from_secs(step));
        assert_eq!(supervisor.current_port(), None);
    }
    assert_eq!(supervisor.generations_started(), 0);
    assert_eq!(fixture.announce.created.load(Ordering::SeqCst), 0);
    // Cuando el conjunto se corrige, sube.
    probe.set(set(&[A]));
    supervisor.tick(t0 + Duration::from_secs(9));
    assert_eq!(supervisor.generations_started(), 1);
    supervisor.shutdown();
}

#[test]
fn tick_retries_mdns_without_restarting_the_server() {
    let fixture = Fixture::new();
    fixture.announce.fail_remaining.store(1, Ordering::SeqCst);
    let probe = FakeProbe::new();
    probe.set(set(&[A]));
    let mut supervisor = supervisor(&fixture, &probe);
    let t0 = Instant::now();
    supervisor.tick(t0);
    let token = supervisor.current_token().expect("token");
    supervisor.tick(t0 + Duration::from_secs(1));
    assert_eq!(fixture.announce.calls.load(Ordering::SeqCst), 2);
    assert_eq!(supervisor.generations_started(), 1);
    assert!(!token.is_cancelled());
    supervisor.shutdown();
}

#[test]
fn server_supervisor_runs_and_stop_is_idempotent_and_bounded() {
    let fixture = Fixture::new();
    let probe = FakeProbe::new();
    probe.set(set(&[A]));
    let server = ServerSupervisor::start(probe.clone(), fixture.deps(), config());
    assert!(wait_until(
        || server.local_port().is_some(),
        Duration::from_secs(2)
    ));
    let port = server.local_port().expect("puerto");
    assert_eq!(
        fixture.send(SocketAddr::from((A, port))),
        Ok(AckOutcome::Copied)
    );

    let started = Instant::now();
    server.stop();
    server.stop();
    assert!(started.elapsed() < Duration::from_secs(3));
    assert_eq!(server.local_port(), None);
    assert!(!is_listening(port));
}

#[test]
fn server_supervisor_follows_the_network_while_running() {
    let fixture = Fixture::new();
    let probe = FakeProbe::new();
    let server = ServerSupervisor::start(probe.clone(), fixture.deps(), config());
    assert_eq!(server.local_port(), None);
    probe.set(set(&[A]));
    assert!(wait_until(
        || server.local_port().is_some(),
        Duration::from_secs(2)
    ));
    probe.set(set(&[]));
    assert!(wait_until(
        || server.local_port().is_none(),
        Duration::from_secs(2)
    ));
    server.stop();
}

fn candidate(
    ip: Ipv4Addr,
    eligible: bool,
    has_gateway: bool,
    metric: u32,
) -> crate::network::Candidate {
    crate::network::Candidate {
        ip,
        eligible,
        has_gateway,
        metric,
    }
}

#[test]
fn snapshot_is_none_without_private_network() {
    let fixture = Fixture::new();
    let probe = FakeProbe::new();
    let mut supervisor = supervisor(&fixture, &probe);
    supervisor.tick(Instant::now());
    assert_eq!(supervisor.snapshot(), None);
}

#[test]
fn snapshot_reports_generation_port_and_candidates() {
    let fixture = Fixture::new();
    let probe = FakeProbe::new();
    let both = vec![candidate(A, true, true, 10), candidate(B, false, false, 50)];
    probe.set_candidates(Ok(both.clone()));
    let mut supervisor = supervisor(&fixture, &probe);
    supervisor.tick(Instant::now());
    let snapshot = supervisor.snapshot().expect("instantánea");
    assert_eq!(snapshot.generation, 1);
    assert_eq!(Some(snapshot.port), supervisor.current_port());
    assert_eq!(snapshot.candidates, both);
    supervisor.shutdown();
}

#[test]
fn generation_id_changes_when_the_set_changes() {
    let fixture = Fixture::new();
    let probe = FakeProbe::new();
    probe.set(set(&[A]));
    let mut supervisor = supervisor(&fixture, &probe);
    let t0 = Instant::now();
    supervisor.tick(t0);
    assert_eq!(supervisor.snapshot().expect("1").generation, 1);
    probe.set(set(&[A, B]));
    supervisor.tick(t0);
    assert_eq!(supervisor.snapshot().expect("2").generation, 2);
    probe.set(set(&[]));
    supervisor.tick(t0);
    assert_eq!(supervisor.snapshot(), None);
    probe.set(set(&[A]));
    supervisor.tick(t0);
    assert_eq!(supervisor.snapshot().expect("3").generation, 3);
    supervisor.shutdown();
}

#[test]
fn generation_id_is_unchanged_across_ticks_with_the_same_set() {
    let fixture = Fixture::new();
    let probe = FakeProbe::new();
    probe.set(set(&[A]));
    let mut supervisor = supervisor(&fixture, &probe);
    let t0 = Instant::now();
    supervisor.tick(t0);
    let first = supervisor.snapshot().expect("instantánea");
    for step in 1..4 {
        supervisor.tick(t0 + Duration::from_secs(step));
    }
    let after = supervisor.snapshot().expect("instantánea");
    assert_eq!(
        (first.generation, first.port),
        (after.generation, after.port)
    );
    supervisor.shutdown();
}

#[test]
fn metric_change_with_the_same_set_updates_candidates_but_not_the_generation() {
    let fixture = Fixture::new();
    let probe = FakeProbe::new();
    probe.set_candidates(Ok(vec![candidate(A, true, true, 10)]));
    let mut supervisor = supervisor(&fixture, &probe);
    let t0 = Instant::now();
    supervisor.tick(t0);
    probe.set_candidates(Ok(vec![candidate(A, true, true, 99)]));
    supervisor.tick(t0);
    let snapshot = supervisor.snapshot().expect("instantánea");
    assert_eq!(snapshot.generation, 1);
    assert_eq!(snapshot.candidates, vec![candidate(A, true, true, 99)]);
    supervisor.shutdown();
}

#[test]
fn rebuilt_listener_gets_a_new_id() {
    let fixture = Fixture::new();
    let probe = FakeProbe::new();
    probe.set(set(&[A]));
    let mut supervisor = supervisor(&fixture, &probe);
    let t0 = Instant::now();
    supervisor.tick(t0);
    supervisor.current_token().expect("token").cancel();
    assert!(wait_until(
        || !supervisor.current_is_alive(),
        Duration::from_secs(2)
    ));
    supervisor.tick(t0);
    assert_eq!(supervisor.snapshot(), None, "caída: no hay instantánea");
    supervisor.tick(t0 + Duration::from_millis(11));
    assert_eq!(supervisor.snapshot().expect("reconstruida").generation, 2);
    supervisor.shutdown();
}

#[test]
fn server_supervisor_publishes_the_snapshot_and_clears_it_on_stop() {
    use crate::server::NetworkView;
    let fixture = Fixture::new();
    let probe = FakeProbe::new();
    probe.set(set(&[A]));
    let running = ServerSupervisor::start(probe, fixture.deps(), config());
    assert!(wait_until(
        || running.snapshot().is_some(),
        Duration::from_secs(2)
    ));
    assert_eq!(running.snapshot().expect("instantánea").generation, 1);
    running.stop();
    assert_eq!(running.snapshot(), None);
}
