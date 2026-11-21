use std::io::Read;
use std::net::{SocketAddr, TcpListener};
use std::thread;
use std::time::{Duration, Instant};

use super::*;
use crate::test_support::{backup_and_target, fixture_copy, path_string, FIXTURE_PASSWORD};

fn unlocked() -> (tempfile::TempDir, VaultSession) {
    let (dir, path) = fixture_copy();
    let session = VaultSession::new(path_string(&path));
    session.open().unwrap();
    session.unlock(FIXTURE_PASSWORD.into()).unwrap();
    (dir, session)
}

fn active(session: &VaultSession) -> Option<u64> {
    session.net().active
}

#[test]
fn begin_op_then_second_is_busy() {
    let (_dir, session) = unlocked();
    let first = session.begin_op().unwrap();
    assert!(matches!(session.begin_op(), Err(VaultError::Busy)));
    drop(first);
}

#[test]
fn drop_frees_active() {
    let (_dir, session) = unlocked();
    drop(session.begin_op().unwrap());
    assert_eq!(active(&session), None);
    assert!(session.begin_op().is_ok());
}

#[test]
fn old_guard_drop_does_not_free_a_newer_operation() {
    let (_dir, session) = unlocked();
    let old = session.begin_op().unwrap();
    session.invalidate_net();
    let newer = session.begin_op().unwrap();
    drop(old);
    // La operación nueva sigue ocupando el estado.
    assert!(matches!(session.begin_op(), Err(VaultError::Busy)));
    drop(newer);
    assert!(session.begin_op().is_ok());
}

#[test]
fn lock_invalidates_epoch_cancels_token_and_frees_active() {
    let (_dir, session) = unlocked();
    let op = session.begin_op().unwrap();
    assert!(op.check().is_ok());
    session.lock().unwrap();
    assert!(op.token().is_cancelled());
    assert!(matches!(op.check(), Err(VaultError::Cancelled)));
    assert_eq!(active(&session), None);
    assert_eq!(session.is_locked(), Ok(true));
    // La época nueva sirve para una operación nueva.
    let fresh = session.begin_op().unwrap();
    assert!(fresh.check().is_ok());
}

#[test]
fn lock_takes_vault_first_then_net() {
    let (_dir, session) = unlocked();
    let op = session.begin_op().unwrap();
    let epoch = session.net_epoch();
    let op_id = active(&session);
    thread::scope(|scope| {
        let slot = session.slot().unwrap();
        let locker = scope.spawn(|| session.lock().unwrap());
        // Mientras `vault` sigue tomado por nosotros, `lock()` no debe haber
        // cambiado nada: ni la época, ni el token, ni la operación activa.
        // `try_lock` y no `net_epoch()`: si `lock()` tomara `net` antes que
        // `vault`, esperaría a `vault` con `net` tomado y la prueba debe
        // **fallar**, no colgarse.
        for _ in 0..30 {
            thread::sleep(Duration::from_millis(10));
            let net = session
                .net
                .try_lock()
                .expect("lock() retiene `net` mientras espera `vault`");
            assert_eq!(net.epoch, epoch);
            assert_eq!(net.active, op_id);
            assert!(!op.token().is_cancelled());
            drop(net);
            assert!(slot.as_ref().is_some_and(|v| !v.is_locked()));
        }
        drop(slot);
        locker.join().unwrap();
    });
    // Al soltar `vault`, los tres cambios ocurren juntos.
    assert_eq!(session.net_epoch(), epoch + 1);
    assert!(op.token().is_cancelled());
    assert_eq!(active(&session), None);
    assert_eq!(session.is_locked(), Ok(true));
}

#[test]
fn lock_keeps_vault_while_it_waits_for_net() {
    // `lock()` no puede soltar `vault` antes de tener `net`: si lo hiciera
    // (los dos candados en orden pero sin anidar), una operación vieja podría
    // ver la bóveda desbloqueada por otro hilo con la época todavía vigente y
    // guardar (el hueco que el diseño prohíbe).
    let (_dir, session) = unlocked();
    thread::scope(|scope| {
        let net = session.net();
        let locker = scope.spawn(|| session.lock().unwrap());
        // Con `net` tomado por nosotros, `lock()` toma `vault` y se queda
        // esperando `net`: `vault` tiene que seguir tomado.
        let started = Instant::now();
        while session.vault.try_lock().is_ok() {
            assert!(
                started.elapsed() < Duration::from_secs(3),
                "lock() no llegó a tomar `vault`"
            );
            thread::sleep(Duration::from_millis(5));
        }
        // Y se mantiene tomado mientras `net` sigue ocupado.
        for _ in 0..30 {
            thread::sleep(Duration::from_millis(10));
            assert!(
                session.vault.try_lock().is_err(),
                "lock() soltó `vault` antes de tomar `net`"
            );
        }
        drop(net);
        locker.join().unwrap();
    });
    assert_eq!(session.is_locked(), Ok(true));
}

#[test]
fn extraction_never_sees_new_epoch_with_unlocked_vault() {
    let (_dir, session) = unlocked();
    let op = session.begin_op().unwrap();
    thread::scope(|scope| {
        let slot = session.slot().unwrap();
        let locker = scope.spawn(|| session.lock().unwrap());
        thread::sleep(Duration::from_millis(200));
        // Extracción hecha antes del bloqueo: bóveda desbloqueada y época
        // vigente, las dos a la vez.
        assert!(slot.as_ref().is_some_and(|v| !v.is_locked()));
        assert!(op.check().is_ok());
        drop(slot);
        locker.join().unwrap();
    });
    // Extracción hecha después: bóveda bloqueada y época vieja.
    let slot = session.slot().unwrap();
    assert!(slot.as_ref().is_some_and(|v| v.is_locked()));
    assert!(matches!(op.check(), Err(VaultError::Cancelled)));
}

#[test]
fn import_backup_invalidates_on_success_and_on_failure() {
    let (backup, target) = backup_and_target();
    let backup = path_string(&backup);

    let op = target.begin_op().unwrap();
    assert_eq!(
        target.import_backup(backup.clone(), "incorrecta".into()),
        Err(VaultError::WrongPassword)
    );
    assert!(op.token().is_cancelled());
    assert!(matches!(op.check(), Err(VaultError::Cancelled)));
    assert_eq!(active(&target), None);

    let op = target.begin_op().unwrap();
    assert!(target
        .import_backup(backup, FIXTURE_PASSWORD.into())
        .is_ok());
    assert!(op.token().is_cancelled());
    assert!(matches!(op.check(), Err(VaultError::Cancelled)));
    assert_eq!(active(&target), None);
}

#[test]
fn import_backup_invalidates_at_start_and_at_end() {
    // Dos subidas de época por importación, con éxito o con error: la del
    // final cubre a una operación que haya empezado durante la importación
    // (capturaría la época de la primera subida). No se prueba con un gancho
    // en el código de producción; la mutación que quita la segunda subida
    // hace fallar esta prueba (ver el plan, Tarea 3).
    let (backup, target) = backup_and_target();
    let backup = path_string(&backup);
    let before = target.net_epoch();
    let _ = target.import_backup(backup.clone(), "incorrecta".into());
    assert_eq!(target.net_epoch(), before + 2);
    let before = target.net_epoch();
    target
        .import_backup(backup, FIXTURE_PASSWORD.into())
        .unwrap();
    assert_eq!(target.net_epoch(), before + 2);
}

#[test]
fn connect_refused_is_connect_failed() {
    let (_dir, session) = unlocked();
    let address = {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap()
    };
    let op = session.begin_op().unwrap();
    assert!(matches!(
        connect(&op, address),
        Err(VaultError::ConnectFailed)
    ));
}

#[test]
fn connect_after_invalidation_returns_cancelled_and_sends_nothing() {
    let (_dir, session) = unlocked();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let op = session.begin_op().unwrap();
    session.lock().unwrap();
    assert!(matches!(
        connect(&op, listener.local_addr().unwrap()),
        Err(VaultError::Cancelled)
    ));
    // La conexión se abrió y se soltó sin escribir un solo byte.
    let (mut stream, _) = listener.accept().unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut buffer = [0u8; 16];
    match stream.read(&mut buffer) {
        Ok(read) => assert_eq!(read, 0),
        Err(error) => assert!(matches!(
            error.kind(),
            std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::ConnectionAborted
        )),
    }
}

#[test]
fn lock_while_connecting_does_not_wait() {
    let (_dir, session) = unlocked();
    // Dirección documentada como no enrutable (RFC 5737): la conexión no
    // contesta; si el entorno la rechaza al instante, solo se comprueba lo
    // de `lock()`.
    let address: SocketAddr = "192.0.2.1:9".parse().unwrap();
    thread::scope(|scope| {
        let op = session.begin_op().unwrap();
        let connecting = scope.spawn(move || connect(&op, address).map(|_| ()));
        thread::sleep(Duration::from_millis(200));
        let started = Instant::now();
        session.lock().unwrap();
        assert!(started.elapsed() < Duration::from_secs(1));
        // El estado «ocupado» ya está libre aunque el connect siga corriendo.
        assert!(session.begin_op().is_ok());
        let result = connecting.join().unwrap();
        assert!(matches!(
            result,
            Err(VaultError::Cancelled) | Err(VaultError::ConnectFailed)
        ));
    });
}

#[test]
fn errors_after_an_invalidation_are_cancelled() {
    let (_dir, session) = unlocked();
    let address = {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap()
    };
    let op = session.begin_op().unwrap();
    // Sin invalidación, el error de la conexión se queda como está.
    assert!(matches!(
        connect(&op, address),
        Err(VaultError::ConnectFailed)
    ));
    // Con el bloqueo ya hecho, el mismo fallo es una cancelación.
    session.lock().unwrap();
    assert!(matches!(connect(&op, address), Err(VaultError::Cancelled)));
    assert_eq!(
        op.cancelled_or(VaultError::ConnectionClosed),
        VaultError::Cancelled
    );
    // Una operación vigente conserva su error.
    let fresh = session.begin_op().unwrap();
    assert_eq!(
        fresh.cancelled_or(VaultError::ConnectionClosed),
        VaultError::ConnectionClosed
    );
}
