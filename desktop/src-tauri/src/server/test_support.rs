//! Dobles y ayudas compartidas por las pruebas del servidor.

use std::net::{SocketAddr, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use secrecy::{ExposeSecret, SecretString};
use vault_core::protocol::{
    AckOutcome, CancelToken, KeyPair, SendError, SendMessage, SendTimeouts, send_to_pc,
};

use super::{Config, DeliverFailed, Deps, PasswordSink};
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
            _dir: dir,
        }
    }

    pub(crate) fn deps(&self) -> Deps {
        Deps {
            identity: Arc::clone(&self.identity),
            paired: Arc::clone(&self.paired),
            sink: Arc::clone(&self.sink) as Arc<dyn PasswordSink>,
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
