use std::collections::BTreeSet;
use std::net::Ipv4Addr;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use vault_core::protocol::KeyPair;

use super::{AnnounceSlot, Announcer, Service};
use crate::backoff::Backoff;
use crate::server::test_support::{AnnounceLog, FakeAnnouncer};

const LOCAL: Ipv4Addr = Ipv4Addr::new(192, 168, 1, 20);
const BASE: Duration = Duration::from_secs(5);

fn service() -> Service {
    let key = KeyPair::generate().expect("llave");
    Service::new(BTreeSet::from([LOCAL]), 47615, &key.public().fingerprint())
}

fn slot(log: &Arc<AnnounceLog>) -> AnnounceSlot {
    let announcer: Box<dyn Announcer> = Box::new(FakeAnnouncer::new(Arc::clone(log)));
    AnnounceSlot::new(announcer, service(), Backoff::new(BASE, BASE * 12))
}

#[test]
fn service_never_contains_host_name_and_txt_is_exact() {
    let key = KeyPair::generate().expect("llave");
    let fingerprint = key.public().fingerprint();
    let one = Service::new(BTreeSet::from([LOCAL]), 47615, &fingerprint);
    let other = Service::new(BTreeSet::from([LOCAL]), 47615, &fingerprint);

    let suffix = one.instance.strip_prefix("boveda-").expect("prefijo");
    assert_eq!(suffix.len(), 8);
    assert!(suffix.chars().all(|c| c.is_ascii_hexdigit()));
    assert_eq!(one.host, format!("{}.local.", one.instance));
    assert_ne!(one.instance, other.instance, "el nombre es aleatorio");
    assert_eq!(
        one.txt(),
        [("v", "1".to_owned()), ("id", fingerprint.to_string())]
    );
    if let Ok(computer) = std::env::var("COMPUTERNAME") {
        assert!(
            !one.instance
                .to_lowercase()
                .contains(&computer.to_lowercase())
        );
    }
    assert_eq!((one.port, one.addrs.len()), (47615, 1));
}

#[test]
fn slot_announces_on_first_poll_and_then_leaves_it_alone() {
    let log = Arc::new(AnnounceLog::default());
    let mut slot = slot(&log);
    let t0 = Instant::now();
    slot.poll(t0);
    assert!(slot.is_announced());
    slot.poll(t0 + BASE * 10);
    assert_eq!(log.calls.load(Ordering::SeqCst), 1);
}

#[test]
fn slot_retries_with_growing_waits_until_it_works() {
    let log = Arc::new(AnnounceLog::default());
    log.fail_remaining.store(2, Ordering::SeqCst);
    let mut slot = slot(&log);
    let t0 = Instant::now();

    slot.poll(t0); // falla; próximo intento a +5 s
    assert!(!slot.is_announced());
    slot.poll(t0 + Duration::from_secs(4));
    assert_eq!(log.calls.load(Ordering::SeqCst), 1, "aún no toca");

    slot.poll(t0 + BASE); // falla; el siguiente espera 10 s
    assert_eq!(log.calls.load(Ordering::SeqCst), 2);
    slot.poll(t0 + BASE + Duration::from_secs(9));
    assert_eq!(log.calls.load(Ordering::SeqCst), 2, "aún no toca");

    slot.poll(t0 + BASE + Duration::from_secs(10)); // ahora sí funciona
    assert_eq!(log.calls.load(Ordering::SeqCst), 3);
    assert!(slot.is_announced());
}

#[test]
fn withdraw_calls_the_announcer_and_clears_the_state() {
    let log = Arc::new(AnnounceLog::default());
    let mut slot = slot(&log);
    slot.poll(Instant::now());
    slot.withdraw();
    assert!(!slot.is_announced());
    let events = log.events.lock().expect("candado");
    assert!(events.last().expect("evento").starts_with("withdraw:"));
}

#[test]
fn withdraw_also_cleans_up_after_a_failed_announce() {
    let log = Arc::new(AnnounceLog::default());
    log.fail_remaining.store(1, Ordering::SeqCst);
    let mut slot = slot(&log);
    slot.poll(Instant::now());
    slot.withdraw();
    let events = log.events.lock().expect("candado");
    assert_eq!(events.last().expect("evento"), "withdraw:never_announced");
}
