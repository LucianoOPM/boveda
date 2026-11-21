use std::thread;
use std::time::{Duration, Instant};

use vault_core::protocol::{pair_with_pc, DeviceName, PairingOffer};
use vault_core::vault::NewPairedPc;

use super::*;
use crate::api::net::connect;
use crate::test_support::{
    backup_and_target, corrupt_first_row, fixture_copy, path_string, quick_pairing, FakePc,
    PcDecision, FIXTURE_PASSWORD,
};

const PHONE_NAME: &str = "Pixel de Ana";

fn unlocked() -> (tempfile::TempDir, VaultSession) {
    let (dir, path) = fixture_copy();
    let session = VaultSession::new(path_string(&path));
    session.open().unwrap();
    session.unlock(FIXTURE_PASSWORD.into()).unwrap();
    (dir, session)
}

fn pc_fingerprint() -> String {
    FakePc::public_key().fingerprint().to_string()
}

fn assert_busy_is_free(session: &VaultSession) {
    assert!(
        session.begin_op().is_ok(),
        "el estado «ocupado» sigue tomado"
    );
}

/// Espera (con tope) a que haya una operación de red en curso.
fn wait_until_active(session: &VaultSession) {
    let started = Instant::now();
    while session.net().active.is_none() {
        assert!(started.elapsed() < Duration::from_secs(3));
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn pair_end_to_end_saves_the_pc_with_provisional_name() {
    let (_dir, session) = unlocked();
    let pc = FakePc::new();
    let server = pc.serve_pairing(quick_pairing(), PcDecision::Accept);

    let summary = session.pair(pc.offer_text(), PHONE_NAME.into()).unwrap();

    let device = server.join().unwrap().unwrap().unwrap();
    assert_eq!(device.name().as_str(), PHONE_NAME);
    assert_eq!(
        device.public_key().fingerprint().to_string(),
        session.ensure_identity().unwrap().fingerprint
    );
    assert_eq!(summary.name, format!("PC {}", pc_fingerprint()));
    assert_eq!(summary.fingerprint, pc_fingerprint());
    assert_eq!(summary.last_ip.as_deref(), Some("127.0.0.1"));
    let list = session.paired_pcs().unwrap();
    assert_eq!(list.pcs.len(), 1);
    assert_eq!(list.pcs[0].id, summary.id);
    assert_busy_is_free(&session);
}

#[test]
fn pair_with_pc_already_paired_updates_ip_and_keeps_identity() {
    let (_dir, session) = unlocked();
    session
        .with_vault(|vault| {
            vault.add_paired_pc(NewPairedPc {
                public_key: *FakePc::public_key().as_bytes(),
                name: "Mi PC".into(),
                last_ip: Some("10.9.9.9".parse().unwrap()),
            })
        })
        .unwrap();
    let before = session.paired_pcs().unwrap().pcs.remove(0);
    let pc = FakePc::new();
    let server = pc.serve_pairing(quick_pairing(), PcDecision::Accept);

    let summary = session.pair(pc.offer_text(), PHONE_NAME.into()).unwrap();

    server.join().unwrap().unwrap();
    assert_eq!(summary.id, before.id);
    assert_eq!(summary.name, "Mi PC");
    assert_eq!(summary.paired_at, before.paired_at);
    assert_eq!(summary.last_ip.as_deref(), Some("127.0.0.1"));
    assert_eq!(session.paired_pcs().unwrap().pcs.len(), 1);
}

#[test]
fn pair_rejected_by_pc_saves_nothing() {
    let (_dir, session) = unlocked();
    let pc = FakePc::new();
    let server = pc.serve_pairing(quick_pairing(), PcDecision::Reject);
    assert_eq!(
        session
            .pair(pc.offer_text(), PHONE_NAME.into())
            .unwrap_err(),
        VaultError::PairingRejected
    );
    server.join().unwrap().unwrap();
    assert!(session.paired_pcs().unwrap().pcs.is_empty());
    assert_busy_is_free(&session);
}

#[test]
fn pc_that_drops_the_pending_pairing_is_connection_closed() {
    let (_dir, session) = unlocked();
    let pc = FakePc::new();
    let server = pc.serve_pairing(quick_pairing(), PcDecision::Drop);
    assert_eq!(
        session
            .pair(pc.offer_text(), PHONE_NAME.into())
            .unwrap_err(),
        VaultError::ConnectionClosed
    );
    server.join().unwrap().unwrap();
    assert!(session.paired_pcs().unwrap().pcs.is_empty());
    assert_busy_is_free(&session);
}

#[test]
fn same_qr_twice_second_is_connection_closed_and_nothing_changes() {
    let (_dir, session) = unlocked();
    let pc = FakePc::new();
    let first = pc.serve_pairing(quick_pairing(), PcDecision::Accept);
    let text = pc.offer_text();
    let saved = session.pair(text.clone(), PHONE_NAME.into()).unwrap();
    first.join().unwrap().unwrap();

    // El código ya se consumió: la PC cierra sin escribir nada.
    let second = pc.serve_pairing(quick_pairing(), PcDecision::Accept);
    assert_eq!(
        session.pair(text, PHONE_NAME.into()).unwrap_err(),
        VaultError::ConnectionClosed
    );
    assert!(second.join().unwrap().is_err());
    let list = session.paired_pcs().unwrap();
    assert_eq!(list.pcs.len(), 1);
    assert_eq!(list.pcs[0].id, saved.id);
    assert_eq!(list.pcs[0].name, saved.name);
    assert_busy_is_free(&session);
}

#[test]
fn bad_inputs_do_not_touch_the_network() {
    let (_dir, session) = unlocked();
    let pc = FakePc::new();
    let good = pc.offer_text();
    for text in [
        format!("{good}\n"),
        format!(" {good}"),
        format!("{good}="),
        String::new(),
        "no-es-un-qr".to_owned(),
    ] {
        assert_eq!(
            session.pair(text, PHONE_NAME.into()).unwrap_err(),
            VaultError::InvalidPairingOffer
        );
    }
    for name in ["", "   ", "a\u{0007}b", &"x".repeat(129)] {
        assert_eq!(
            session.pair(good.clone(), name.into()).unwrap_err(),
            VaultError::InvalidDeviceName,
            "nombre {name:?}"
        );
    }
    assert!(pc.nobody_connected());
    assert_busy_is_free(&session);
}

#[test]
fn locked_or_missing_vault_does_not_connect() {
    let (_dir, session) = unlocked();
    let pc = FakePc::new();
    session.lock().unwrap();
    assert_eq!(
        session
            .pair(pc.offer_text(), PHONE_NAME.into())
            .unwrap_err(),
        VaultError::Locked
    );
    let empty = VaultSession::new(path_string(
        &tempfile::tempdir().unwrap().path().join("x.db"),
    ));
    assert_eq!(
        empty.pair(pc.offer_text(), PHONE_NAME.into()).unwrap_err(),
        VaultError::NoVault
    );
    assert!(pc.nobody_connected());
    assert_busy_is_free(&session);
}

#[test]
fn silent_pc_times_out_and_garbage_pc_is_a_protocol_failure() {
    let (_dir, session) = unlocked();

    let pc = FakePc::new();
    let (mute, release) = pc.serve_mute();
    let started = Instant::now();
    assert_eq!(
        session
            .pair_with(pc.offer_text(), PHONE_NAME.into(), &quick_pairing())
            .unwrap_err(),
        VaultError::NetworkTimeout
    );
    assert!(started.elapsed() < Duration::from_secs(3));
    release.send(()).unwrap();
    mute.join().unwrap();
    assert_busy_is_free(&session);

    let pc = FakePc::new();
    let garbage = pc.serve_garbage();
    assert_eq!(
        session
            .pair_with(pc.offer_text(), PHONE_NAME.into(), &quick_pairing())
            .unwrap_err(),
        VaultError::ProtocolFailure
    );
    garbage.join().unwrap();
    assert_busy_is_free(&session);
}

#[test]
fn lock_during_pairing_cancels_without_waiting() {
    let (_dir, session) = unlocked();
    let pc = FakePc::new();
    let (mute, release) = pc.serve_mute();
    thread::scope(|scope| {
        let pairing = scope.spawn(|| {
            let started = Instant::now();
            let result = session.pair(pc.offer_text(), PHONE_NAME.into());
            (result, started.elapsed())
        });
        wait_until_active(&session);
        thread::sleep(Duration::from_millis(300));

        let started = Instant::now();
        session.lock().unwrap();
        assert!(started.elapsed() < Duration::from_secs(1), "lock() esperó");

        let (result, elapsed) = pairing.join().unwrap();
        assert_eq!(result.unwrap_err(), VaultError::Cancelled);
        assert!(elapsed < Duration::from_secs(2), "tardó {elapsed:?}");
    });
    assert_eq!(session.is_locked(), Ok(true));
    release.send(()).unwrap();
    mute.join().unwrap();
    assert_busy_is_free(&session);
}

/// El celular completo contra `pc`, paso a paso con la operación `op`, sin
/// guardar: devuelve lo que `pairing_persist` recibiría.
fn paired_on_the_wire(
    session: &VaultSession,
    op: &OpGuard<'_>,
    pc: &FakePc,
) -> vault_core::protocol::PairedPc {
    let keys = session.pairing_prepare(op).unwrap();
    let offer = PairingOffer::decode_qr_text(&pc.offer_text()).unwrap();
    let server = pc.serve_pairing(quick_pairing(), PcDecision::Accept);
    let stream = connect(op, pc.addr()).unwrap();
    let paired = pair_with_pc(
        stream,
        &keys,
        &offer,
        DeviceName::new(PHONE_NAME).unwrap(),
        op.token(),
        &quick_pairing(),
    )
    .unwrap();
    server.join().unwrap().unwrap();
    paired
}

#[test]
fn accepted_then_lock_before_persist_gives_locked_and_saves_nothing() {
    let (_dir, session) = unlocked();
    let pc = FakePc::new();
    let op = session.begin_op().unwrap();
    let paired = paired_on_the_wire(&session, &op, &pc);

    // El `Accepted` ya llegó; el bloqueo cae antes de guardar.
    session.lock().unwrap();
    assert_eq!(
        session.pairing_persist(&op, &paired).unwrap_err(),
        VaultError::Locked
    );
    session.unlock(FIXTURE_PASSWORD.into()).unwrap();
    assert!(session.paired_pcs().unwrap().pcs.is_empty());
}

#[test]
fn lock_then_unlock_does_not_revive_the_old_operation() {
    let (_dir, session) = unlocked();
    let pc = FakePc::new();
    let op = session.begin_op().unwrap();
    let paired = paired_on_the_wire(&session, &op, &pc);

    session.lock().unwrap();
    session.unlock(FIXTURE_PASSWORD.into()).unwrap();
    // La bóveda está desbloqueada otra vez, pero la época es otra.
    assert_eq!(
        session.pairing_persist(&op, &paired).unwrap_err(),
        VaultError::Locked
    );
    assert!(session.paired_pcs().unwrap().pcs.is_empty());
    drop(op);

    // Una operación nueva sí funciona (otro QR, otro código).
    let fresh = FakePc::new();
    let server = fresh.serve_pairing(quick_pairing(), PcDecision::Accept);
    let summary = session.pair(fresh.offer_text(), PHONE_NAME.into()).unwrap();
    server.join().unwrap().unwrap();
    assert_eq!(summary.fingerprint, pc_fingerprint());
    assert_eq!(session.paired_pcs().unwrap().pcs.len(), 1);
}

#[test]
fn import_backup_during_pairing_cancels_the_operation() {
    for password in ["incorrecta", FIXTURE_PASSWORD] {
        let (backup, target) = backup_and_target();
        let pc = FakePc::new();
        let op = target.begin_op().unwrap();
        let paired = paired_on_the_wire(&target, &op, &pc);

        let _ = target.import_backup(path_string(&backup), password.into());
        // Con éxito la bóveda queda desbloqueada y con error, bloqueada: en
        // los dos casos la operación anterior no guarda nada.
        assert_eq!(
            target.pairing_persist(&op, &paired).unwrap_err(),
            VaultError::Locked
        );
        if !target.is_locked().unwrap() {
            assert!(target.paired_pcs().unwrap().pcs.is_empty());
        }
    }
}

#[test]
fn second_network_operation_is_busy_and_busy_is_released() {
    let (_dir, session) = unlocked();
    let pc = FakePc::new();
    let (mute, release) = pc.serve_mute();
    thread::scope(|scope| {
        let first = scope.spawn(|| session.pair(pc.offer_text(), PHONE_NAME.into()));
        wait_until_active(&session);

        let started = Instant::now();
        assert_eq!(
            session
                .pair(pc.offer_text(), PHONE_NAME.into())
                .unwrap_err(),
            VaultError::Busy
        );
        assert!(started.elapsed() < Duration::from_millis(500));

        session.lock().unwrap();
        assert_eq!(first.join().unwrap().unwrap_err(), VaultError::Cancelled);
    });
    release.send(()).unwrap();
    mute.join().unwrap();
    assert_busy_is_free(&session);
}

#[test]
fn no_secret_crosses_in_results_or_errors() {
    let (_dir, session) = unlocked();
    let pc = FakePc::new();
    let server = pc.serve_pairing(quick_pairing(), PcDecision::Accept);
    let summary = session.pair(pc.offer_text(), PHONE_NAME.into()).unwrap();
    server.join().unwrap().unwrap();
    let shown = format!("{summary:?}");
    for secret in [PHONE_NAME, "127.0.0.1", &pc_fingerprint(), &summary.name] {
        assert!(!shown.contains(secret), "el Debug muestra {secret:?}");
    }
}

#[test]
fn pair_with_a_pc_whose_existing_row_is_corrupt_adds_a_new_one() {
    // La fila dañada no se puede leer, así que no cuenta como «ya emparejada»
    // (igual que en `add_paired_pc` del núcleo): queda aparte, y la PC nueva
    // entra como válida.
    let (dir, path) = fixture_copy();
    {
        let first = VaultSession::new(path_string(&path));
        first.open().unwrap();
        first.unlock(FIXTURE_PASSWORD.into()).unwrap();
        first
            .with_vault(|vault| {
                vault.add_paired_pc(NewPairedPc {
                    public_key: *FakePc::public_key().as_bytes(),
                    name: "Dañada".into(),
                    last_ip: None,
                })
            })
            .unwrap();
    }
    corrupt_first_row(&path, "paired_pcs", "data");
    let session = VaultSession::new(path_string(&path));
    session.open().unwrap();
    session.unlock(FIXTURE_PASSWORD.into()).unwrap();
    let before = session.paired_pcs().unwrap();
    assert_eq!((before.pcs.len(), before.corrupt.len()), (0, 1));

    let pc = FakePc::new();
    let server = pc.serve_pairing(quick_pairing(), PcDecision::Accept);
    let summary = session.pair(pc.offer_text(), PHONE_NAME.into()).unwrap();
    server.join().unwrap().unwrap();

    let after = session.paired_pcs().unwrap();
    assert_eq!(after.pcs.len(), 1);
    assert_eq!(after.pcs[0].id, summary.id);
    assert_eq!(after.corrupt, before.corrupt);
    drop(dir);
}
