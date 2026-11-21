use std::thread;
use std::time::{Duration, Instant};

use vault_core::protocol::{AckOutcome, SendError};
use vault_core::vault::NewPairedPc;

use super::*;
use crate::api::entries::{EntryInput, NewSecret};
use crate::api::net::connect;
use crate::test_support::{
    fixture_copy, path_string, quick_send, FakePc, PcSend, FIXTURE_PASSWORD,
};

const PASSWORD: &str = "CLAVE-DISTINTIVA-ñ-7731";
const NOTES: &str = "NOTAS-QUE-NO-SALEN-9915";

struct Setup {
    _dir: tempfile::TempDir,
    session: VaultSession,
    pc: FakePc,
    pc_id: String,
    entry_id: String,
}

fn setup() -> Setup {
    let (dir, path) = fixture_copy();
    let session = VaultSession::new(path_string(&path));
    session.open().unwrap();
    session.unlock(FIXTURE_PASSWORD.into()).unwrap();
    let pc = FakePc::new();
    session
        .with_vault(|vault| {
            vault.add_paired_pc(NewPairedPc {
                public_key: *FakePc::public_key().as_bytes(),
                name: "PC de prueba".into(),
                last_ip: None,
            })
        })
        .unwrap();
    let pc_id = session.paired_pcs().unwrap().pcs.remove(0).id;
    let entry_id = session
        .add(
            EntryInput {
                service: "Distintivo".into(),
                url: None,
                username: None,
                tags: vec![],
            },
            NewSecret {
                password: PASSWORD.into(),
                notes: Some(NOTES.into()),
            },
        )
        .unwrap();
    Setup {
        _dir: dir,
        session,
        pc,
        pc_id,
        entry_id,
    }
}

fn assert_busy_is_free(session: &VaultSession) {
    assert!(
        session.begin_op().is_ok(),
        "el estado «ocupado» sigue tomado"
    );
}

fn wait_until_active(session: &VaultSession) {
    let started = Instant::now();
    while session.net().active.is_none() {
        assert!(started.elapsed() < Duration::from_secs(3));
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn send_copied_end_to_end() {
    let s = setup();
    let server = s.pc.serve_send(quick_send(), PcSend::Copied);
    let outcome = s
        .session
        .send_to_pc(s.pc_id.clone(), s.entry_id.clone(), s.pc.addr().to_string())
        .unwrap();
    assert_eq!(outcome, SendOutcome::Copied);
    let (password, peer) = server.join().unwrap().unwrap();
    // La PC recibe la contraseña y nada más (las notas no salen).
    assert_eq!(password, PASSWORD);
    assert_eq!(
        peer.fingerprint().to_string(),
        s.session.ensure_identity().unwrap().fingerprint
    );
    assert_busy_is_free(&s.session);
}

#[test]
fn send_failed_is_ok_failed() {
    let s = setup();
    let server = s.pc.serve_send(quick_send(), PcSend::Failed);
    assert_eq!(
        s.session
            .send_to_pc(s.pc_id.clone(), s.entry_id.clone(), s.pc.addr().to_string()),
        Ok(SendOutcome::Failed)
    );
    server.join().unwrap().unwrap();
    assert_busy_is_free(&s.session);
}

#[test]
fn password_never_crosses_to_dart() {
    let s = setup();
    let server = s.pc.serve_send(quick_send(), PcSend::Copied);
    let result = s
        .session
        .send_to_pc(s.pc_id.clone(), s.entry_id.clone(), s.pc.addr().to_string());
    server.join().unwrap().unwrap();
    // Ni el resultado ni los errores posibles llevan el secreto.
    let shown = format!("{result:?}");
    assert!(!shown.contains(PASSWORD) && !shown.contains(NOTES));
    let server = s.pc.serve_send(quick_send(), PcSend::Silent);
    let error = s
        .session
        .send_to_pc(s.pc_id.clone(), s.entry_id.clone(), s.pc.addr().to_string())
        .unwrap_err();
    server.join().unwrap().unwrap();
    let shown = format!("{error:?}");
    assert!(!shown.contains(PASSWORD) && !shown.contains(NOTES));
}

#[test]
fn pc_that_does_not_recognise_the_phone_is_connection_closed() {
    let s = setup();
    let server = s.pc.serve_send(quick_send(), PcSend::Forget);
    assert_eq!(
        s.session
            .send_to_pc(s.pc_id.clone(), s.entry_id.clone(), s.pc.addr().to_string()),
        Err(VaultError::ConnectionClosed)
    );
    assert_eq!(server.join().unwrap().unwrap_err(), SendError::NotPaired);
    assert_busy_is_free(&s.session);
}

#[test]
fn unreachable_address_is_connect_failed() {
    let s = setup();
    let address = {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap().to_string()
    };
    assert_eq!(
        s.session
            .send_to_pc(s.pc_id.clone(), s.entry_id.clone(), address),
        Err(VaultError::ConnectFailed)
    );
    assert_busy_is_free(&s.session);
}

#[test]
fn bad_inputs_do_not_touch_the_network_and_free_busy() {
    let s = setup();
    let ok_address = s.pc.addr().to_string();
    for address in [":47615", "1.2.3.4:0", "0.0.0.0:47615", "pc.local:47615", ""] {
        assert_eq!(
            s.session
                .send_to_pc(s.pc_id.clone(), s.entry_id.clone(), address.into()),
            Err(VaultError::InvalidAddress),
            "dirección {address:?}"
        );
    }
    // El `std` de esta versión de Rust sí lee la zona de una IPv6 (`%5`): es una
    // dirección válida que no se puede alcanzar, no un pánico ni un texto
    // inválido.
    assert_eq!(
        s.session.send_to_pc(
            s.pc_id.clone(),
            s.entry_id.clone(),
            "[fe80::1%5]:47615".into()
        ),
        Err(VaultError::ConnectFailed)
    );
    for bad in ["", "abc", &"A".repeat(32)] {
        assert_eq!(
            s.session
                .send_to_pc(bad.into(), s.entry_id.clone(), ok_address.clone()),
            Err(VaultError::InvalidPairedPcId)
        );
        assert_eq!(
            s.session
                .send_to_pc(s.pc_id.clone(), bad.into(), ok_address.clone()),
            Err(VaultError::InvalidEntryId)
        );
    }
    // PC que ya se quitó: id válido, ya no está.
    s.session.remove_paired_pc(s.pc_id.clone()).unwrap();
    assert_eq!(
        s.session
            .send_to_pc(s.pc_id.clone(), s.entry_id.clone(), ok_address.clone()),
        Err(VaultError::PairedPcNotFound)
    );
    // Entrada eliminada.
    s.session
        .with_vault(|vault| {
            vault.add_paired_pc(NewPairedPc {
                public_key: *FakePc::public_key().as_bytes(),
                name: "PC de prueba".into(),
                last_ip: None,
            })
        })
        .unwrap();
    let pc_id = s.session.paired_pcs().unwrap().pcs.remove(0).id;
    s.session.delete(s.entry_id.clone()).unwrap();
    assert_eq!(
        s.session.send_to_pc(pc_id, s.entry_id.clone(), ok_address),
        Err(VaultError::EntryNotFound)
    );
    assert!(s.pc.nobody_connected());
    assert_busy_is_free(&s.session);
}

#[test]
fn locked_or_missing_vault_does_not_connect() {
    let s = setup();
    s.session.lock().unwrap();
    assert_eq!(
        s.session
            .send_to_pc(s.pc_id.clone(), s.entry_id.clone(), s.pc.addr().to_string()),
        Err(VaultError::Locked)
    );
    let empty = VaultSession::new(path_string(
        &tempfile::tempdir().unwrap().path().join("x.db"),
    ));
    assert_eq!(
        empty.send_to_pc(s.pc_id.clone(), s.entry_id.clone(), s.pc.addr().to_string()),
        Err(VaultError::NoVault)
    );
    assert!(s.pc.nobody_connected());
}

#[test]
fn silent_pc_times_out_and_garbage_pc_is_not_authentic() {
    let s = setup();
    let (mute, release) = s.pc.serve_mute();
    let started = Instant::now();
    assert_eq!(
        s.session.send_with(
            s.pc_id.clone(),
            s.entry_id.clone(),
            s.pc.addr().to_string(),
            &quick_send()
        ),
        Err(VaultError::NetworkTimeout)
    );
    assert!(started.elapsed() < Duration::from_secs(3));
    release.send(()).unwrap();
    mute.join().unwrap();
    assert_busy_is_free(&s.session);

    // Una trama 2 de largo distinto de 48: el núcleo no la lee y la contraseña
    // no sale.
    let garbage = s.pc.serve_garbage();
    assert_eq!(
        s.session.send_with(
            s.pc_id.clone(),
            s.entry_id.clone(),
            s.pc.addr().to_string(),
            &quick_send()
        ),
        Err(VaultError::PeerNotAuthentic)
    );
    garbage.join().unwrap();
    assert_busy_is_free(&s.session);
}

#[test]
fn lock_during_send_cancels_without_waiting() {
    let s = setup();
    let (mute, release) = s.pc.serve_mute();
    thread::scope(|scope| {
        let sending = scope.spawn(|| {
            let started = Instant::now();
            let result =
                s.session
                    .send_to_pc(s.pc_id.clone(), s.entry_id.clone(), s.pc.addr().to_string());
            (result, started.elapsed())
        });
        wait_until_active(&s.session);
        thread::sleep(Duration::from_millis(300));

        let started = Instant::now();
        s.session.lock().unwrap();
        assert!(started.elapsed() < Duration::from_secs(1), "lock() esperó");

        let (result, elapsed) = sending.join().unwrap();
        assert_eq!(result, Err(VaultError::Cancelled));
        assert!(elapsed < Duration::from_secs(2), "tardó {elapsed:?}");
    });
    assert_eq!(s.session.is_locked(), Ok(true));
    release.send(()).unwrap();
    mute.join().unwrap();
    assert_busy_is_free(&s.session);
}

#[test]
fn lock_between_prepare_and_connect_sends_nothing() {
    let s = setup();
    let counter = s.pc.serve_count_bytes();
    let op = s.session.begin_op().unwrap();
    let pc = crate::api::pcs::pc_id_from_hex(&s.pc_id).unwrap();
    let entry = crate::api::entries::entry_id_from_hex(&s.entry_id).unwrap();
    let _job = s.session.send_prepare(&op, pc, entry).unwrap();
    s.session.lock().unwrap();
    assert!(matches!(
        connect(&op, s.pc.addr()),
        Err(VaultError::Cancelled)
    ));
    // La conexión se abrió y se soltó sin escribir un solo byte.
    assert_eq!(counter.join().unwrap(), 0);
}

#[test]
fn late_success_after_lock_is_cancelled() {
    let s = setup();
    let server = s.pc.serve_send(quick_send(), PcSend::Copied);
    // El bloqueo cae después de la red y antes de devolver: el `Ack` llegó,
    // pero el resultado ya no es vigente.
    let result = s.session.send_after(
        s.pc_id.clone(),
        s.entry_id.clone(),
        s.pc.addr().to_string(),
        &quick_send(),
        || s.session.lock().unwrap(),
    );
    assert_eq!(result, Err(VaultError::Cancelled));
    // La contraseña sí llegó a la PC: es el caso (c) de la especificación.
    assert_eq!(server.join().unwrap().unwrap().0, PASSWORD);
}

#[test]
fn second_network_operation_is_busy_in_both_orders() {
    let s = setup();
    let other = FakePc::new();

    // Un envío detenido y luego un emparejamiento.
    let (mute, release) = s.pc.serve_mute();
    thread::scope(|scope| {
        let first = scope.spawn(|| {
            s.session
                .send_to_pc(s.pc_id.clone(), s.entry_id.clone(), s.pc.addr().to_string())
        });
        wait_until_active(&s.session);
        assert_eq!(
            s.session
                .pair(other.offer_text(), "Pixel".into())
                .unwrap_err(),
            VaultError::Busy
        );
        assert_eq!(
            s.session
                .send_to_pc(s.pc_id.clone(), s.entry_id.clone(), s.pc.addr().to_string()),
            Err(VaultError::Busy)
        );
        s.session.lock().unwrap();
        assert_eq!(first.join().unwrap(), Err(VaultError::Cancelled));
    });
    release.send(()).unwrap();
    mute.join().unwrap();
    s.session.unlock(FIXTURE_PASSWORD.into()).unwrap();
    assert_busy_is_free(&s.session);

    // Un emparejamiento detenido y luego un envío.
    let (mute, release) = other.serve_mute();
    thread::scope(|scope| {
        let first = scope.spawn(|| s.session.pair(other.offer_text(), "Pixel".into()));
        wait_until_active(&s.session);
        assert_eq!(
            s.session
                .send_to_pc(s.pc_id.clone(), s.entry_id.clone(), s.pc.addr().to_string()),
            Err(VaultError::Busy)
        );
        s.session.lock().unwrap();
        assert!(matches!(first.join().unwrap(), Err(VaultError::Cancelled)));
    });
    release.send(()).unwrap();
    mute.join().unwrap();
}

#[test]
fn reveal_updates_last_used_at_like_any_reveal() {
    let s = setup();
    assert_eq!(
        s.session.entry(s.entry_id.clone()).unwrap().last_used_at,
        None
    );
    let server = s.pc.serve_send(quick_send(), PcSend::Copied);
    s.session
        .send_to_pc(s.pc_id.clone(), s.entry_id.clone(), s.pc.addr().to_string())
        .unwrap();
    server.join().unwrap().unwrap();
    assert!(s
        .session
        .entry(s.entry_id.clone())
        .unwrap()
        .last_used_at
        .is_some());
}

#[test]
fn core_outcome_maps_to_the_facade_outcome() {
    // `Failed` de la PC no es un error del protocolo.
    assert_eq!(SendOutcome::from(AckOutcome::Copied), SendOutcome::Copied);
    assert_eq!(SendOutcome::from(AckOutcome::Failed), SendOutcome::Failed);
}
