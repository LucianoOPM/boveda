//! Dobles y ayudas compartidas por las pruebas del servidor.

use std::net::{SocketAddr, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use secrecy::{ExposeSecret, SecretString};
use vault_core::protocol::{
    AckOutcome, CancelToken, KeyPair, SendError, SendMessage, SendTimeouts, send_to_pc,
};

use super::{Config, DeliverFailed, Deps, PasswordSink};
use crate::announce::{AnnounceError, Announcer, AnnouncerFactory, Service};
use crate::paired_devices::{PairedDevice, PairedDeviceStore};

/// Contraseña que envían las pruebas.
pub(crate) const PASSWORD: &str = "contraseña-de-prueba";

/// Destino de pruebas: guarda lo recibido y puede fallar, entrar en pánico o
/// quedarse esperando a pedido.
#[derive(Default)]
pub(crate) struct TestSink {
    pub(crate) received: Mutex<Vec<String>>,
    pub(crate) fail: AtomicBool,
    pub(crate) panic: AtomicBool,
    /// Mientras sea `true`, `deliver` espera (un portapapeles colgado).
    pub(crate) hold: AtomicBool,
    /// Pasa a `true` cuando `deliver` empieza.
    pub(crate) entered: AtomicBool,
}

impl PasswordSink for TestSink {
    fn deliver(&self, password: &SecretString) -> Result<(), DeliverFailed> {
        self.entered.store(true, Ordering::SeqCst);
        assert!(
            !self.panic.load(Ordering::SeqCst),
            "fallo simulado del sink"
        );
        while self.hold.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(10));
        }
        self.received
            .lock()
            .expect("candado")
            .push(password.expose_secret().to_owned());
        if self.fail.load(Ordering::SeqCst) {
            Err(DeliverFailed)
        } else {
            Ok(())
        }
    }
}

/// Una PC de prueba con un celular ya emparejado.
pub(crate) struct Fixture {
    pub(crate) identity: Arc<KeyPair>,
    pub(crate) phone: KeyPair,
    pub(crate) paired: Arc<Mutex<PairedDeviceStore>>,
    pub(crate) sink: Arc<TestSink>,
    pub(crate) announce: Arc<AnnounceLog>,
    _dir: tempfile::TempDir,
}

impl Fixture {
    pub(crate) fn new() -> Self {
        let dir = tempfile::tempdir().expect("carpeta temporal");
        let phone = KeyPair::generate().expect("llave del celular");
        let mut store =
            PairedDeviceStore::open(dir.path().join("paired-devices.bin")).expect("lista");
        store
            .add(PairedDevice {
                name: "Celular de prueba".to_owned(),
                public_key: *phone.public().as_bytes(),
                paired_at: 0,
            })
            .expect("emparejar");
        Self {
            identity: Arc::new(KeyPair::generate().expect("llave de la PC")),
            phone,
            paired: Arc::new(Mutex::new(store)),
            sink: Arc::new(TestSink::default()),
            announce: Arc::new(AnnounceLog::default()),
            _dir: dir,
        }
    }

    pub(crate) fn deps(&self) -> Deps {
        Deps {
            identity: Arc::clone(&self.identity),
            paired: Arc::clone(&self.paired),
            sink: Arc::clone(&self.sink) as Arc<dyn PasswordSink>,
            announcer: announcer_factory(&self.announce),
        }
    }

    /// Envía [`PASSWORD`] como el celular emparejado.
    pub(crate) fn send(&self, addr: SocketAddr) -> Result<AckOutcome, SendError> {
        self.send_as(&self.phone, addr)
    }

    /// Envía [`PASSWORD`] como cualquier celular.
    pub(crate) fn send_as(
        &self,
        phone: &KeyPair,
        addr: SocketAddr,
    ) -> Result<AckOutcome, SendError> {
        let stream = TcpStream::connect_timeout(&addr, Duration::from_secs(2)).expect("conectar");
        let message = SendMessage::new(SecretString::from(PASSWORD)).expect("mensaje");
        let timeouts = SendTimeouts {
            step: Duration::from_secs(2),
            total: Duration::from_secs(5),
            ack_wait: Duration::from_secs(3),
        };
        send_to_pc(
            stream,
            phone,
            &self.identity.public(),
            &message,
            &CancelToken::new(),
            &timeouts,
        )
    }
}

/// Ajustes rápidos para las pruebas: puerto libre cualquiera y plazos cortos.
pub(crate) fn config() -> Config {
    Config {
        preferred_port: 0,
        accept_poll: Duration::from_millis(10),
        timeouts: SendTimeouts {
            step: Duration::from_millis(400),
            total: Duration::from_secs(3),
            ack_wait: Duration::from_secs(2),
        },
        stop_wait: Duration::from_secs(1),
        retry_base: Duration::from_millis(10),
        retry_max: Duration::from_millis(40),
        check_interval: Duration::from_millis(10),
        ..Config::default()
    }
}

/// Espera hasta `within` a que `condition` sea cierta.
pub(crate) fn wait_until(condition: impl Fn() -> bool, within: Duration) -> bool {
    let deadline = Instant::now() + within;
    while Instant::now() < deadline {
        if condition() {
            return true;
        }
        thread::sleep(Duration::from_millis(5));
    }
    condition()
}

/// Registro compartido del anunciante de pruebas.
#[derive(Default)]
pub(crate) struct AnnounceLog {
    /// `announce:ok`, `announce:fail` y `withdraw:<estado del listener>`.
    pub(crate) events: Mutex<Vec<String>>,
    /// Veces que se llamó a `announce`.
    pub(crate) calls: AtomicUsize,
    /// Anunciantes creados por la fábrica.
    pub(crate) created: AtomicUsize,
    /// Mientras sea `true`, `healthy` dice que el anunciante falló.
    pub(crate) unhealthy: AtomicBool,
    /// Cuántas llamadas a `announce` deben fallar todavía.
    pub(crate) fail_remaining: AtomicUsize,
    /// Último servicio anunciado con éxito.
    pub(crate) last: Mutex<Option<Service>>,
}

/// Anunciante de pruebas: en `withdraw` anota si el listener seguía aceptando,
/// para comprobar el orden de la parada.
pub(crate) struct FakeAnnouncer {
    log: Arc<AnnounceLog>,
    service: Option<Service>,
}

impl FakeAnnouncer {
    pub(crate) fn new(log: Arc<AnnounceLog>) -> Self {
        Self { log, service: None }
    }
}

impl Announcer for FakeAnnouncer {
    fn announce(&mut self, service: &Service) -> Result<(), AnnounceError> {
        self.log.calls.fetch_add(1, Ordering::SeqCst);
        let failing = self
            .log
            .fail_remaining
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |left| {
                left.checked_sub(1)
            })
            .is_ok();
        let mut events = self.log.events.lock().expect("candado");
        if failing {
            events.push("announce:fail".to_owned());
            return Err(AnnounceError);
        }
        events.push("announce:ok".to_owned());
        *self.log.last.lock().expect("candado") = Some(service.clone());
        self.service = Some(service.clone());
        Ok(())
    }

    fn healthy(&mut self) -> bool {
        !self.log.unhealthy.load(Ordering::SeqCst)
    }

    fn withdraw(&mut self) {
        let state = match &self.service {
            Some(service) => {
                let ip = *service.addrs.iter().next().expect("una dirección");
                let addr = SocketAddr::from((ip, service.port));
                if TcpStream::connect_timeout(&addr, Duration::from_millis(500)).is_ok() {
                    "listener_open"
                } else {
                    "listener_closed"
                }
            }
            None => "never_announced",
        };
        self.log
            .events
            .lock()
            .expect("candado")
            .push(format!("withdraw:{state}"));
    }
}

/// Fábrica que entrega un [`FakeAnnouncer`] por generación.
pub(crate) fn announcer_factory(log: &Arc<AnnounceLog>) -> AnnouncerFactory {
    let log = Arc::clone(log);
    Arc::new(move || {
        log.created.fetch_add(1, Ordering::SeqCst);
        Box::new(FakeAnnouncer::new(Arc::clone(&log)))
    })
}
