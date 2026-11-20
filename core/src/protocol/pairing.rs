//! Emparejamiento Noise entre el celular y la PC (#95).
//!
//! Máquinas de estado por tipos, sin sockets: cada paso consume el estado
//! anterior y devuelve el siguiente. Completar el apretón de manos **no
//! autoriza nada**: el celular solo obtiene un [`PairedPc`] con una
//! aceptación autenticada de la PC, y la PC solo obtiene un [`PairedDevice`]
//! cuando el usuario acepta. El núcleo nunca persiste la relación de confianza.
//!
//! # Borrado de memoria
//!
//! `snow` 0.10.0 no depende de `zeroize` y no borra ningún secreto: quedan
//! en su memoria la llave privada estática y la efímera, el código de un solo
//! uso (copia en `psks`) y lo que se deriva de él, el estado simétrico y las
//! llaves de cifrado de los dos sentidos. Se pasan como `&[u8]` desde su
//! `SecretBox` y los estados se sueltan en cuanto dejan de servir, lo que
//! reduce copias y su vida, pero **no es borrado seguro**. Es una excepción
//! aceptada (ver `ARCHITECTURE.md`).

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use secrecy::ExposeSecret;

use snow::params::NoiseParams;
use snow::{Builder, HandshakeState, TransportState};

use super::error::ProtocolError;
use super::identity::{KeyPair, PublicKey};
use super::message::{DeviceName, PairingCode, PairingOffer, PairingRequest};
use crate::vault::NewPairedPc;

/// Patrón Noise del emparejamiento: el celular conoce la llave de la PC (QR)
/// y el código de un solo uso entra como PSK en la posición 0.
pub const PAIRING_PATTERN: &str = "Noise_XKpsk0_25519_ChaChaPoly_BLAKE2s";

/// Prólogo de Noise: separa este intercambio de cualquier otro.
pub const PAIRING_PROLOGUE: &[u8] = b"boveda/emparejamiento/v1";

/// Vida del código de un solo uso, desde que se crea la sesión.
pub const PAIRING_CODE_TTL: Duration = Duration::from_secs(120);

/// Intentos fallidos que aguanta un código antes de invalidarse.
pub const MAX_FAILED_ATTEMPTS: u8 = 3;

/// Longitud de la primera trama Noise: `e` (32) y la etiqueta de 16 bytes.
pub const FIRST_FRAME_LEN: usize = 48;

/// Longitud máxima de una trama del emparejamiento, sin su prefijo de largo.
pub const MAX_FRAME_LEN: usize = 512;

/// Parámetros de Noise del emparejamiento. Un patrón que no se pueda leer es
/// `Handshake`: no ocurre con la constante, pero la API no hace panic.
// Se usa desde la construcción de los extremos (Task 4).
#[cfg_attr(not(test), allow(dead_code))]
fn noise_params() -> Result<NoiseParams, PairingError> {
    PAIRING_PATTERN.parse().map_err(|_| PairingError::Handshake)
}

/// Una trama del emparejamiento, sin su prefijo de largo.
pub type Frame = Vec<u8>;

/// Por qué la PC rechazó una conexión. **Solo para la app local** (por
/// ejemplo, ofrecer "Generar otro código"): nunca se envía ni cambia lo que
/// ve quien intenta conectarse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// La primera trama no se autentica con el código.
    WrongCode,
    /// La primera trama no mide 48 bytes.
    Malformed,
    /// El código caducó.
    Expired,
    /// El código ya se usó.
    AlreadyUsed,
    /// El código agotó sus intentos fallidos.
    TooManyAttempts,
    /// La app canceló el código.
    Cancelled,
}

/// Error del emparejamiento. Ninguna variante lleva datos del mensaje, secretos
/// ni texto de `snow`.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PairingError {
    /// La PC rechazó la conexión (solo lo ve la app de la PC).
    #[error("conexión rechazada")]
    Refused(Refusal),
    /// La respuesta de la PC no se autentica: no es la PC del QR.
    #[error("la PC no se pudo autenticar")]
    PeerNotAuthentic,
    /// La PC rechazó el emparejamiento.
    #[error("la PC rechazó el emparejamiento")]
    Rejected,
    /// No hubo una decisión a tiempo.
    #[error("se agotó el tiempo de la confirmación")]
    ConfirmationTimeout,
    /// Se agotó el tiempo de un paso o del apretón de manos.
    #[error("se agotó el tiempo de espera")]
    Timeout,
    /// La operación se canceló.
    #[error("operación cancelada")]
    Cancelled,
    /// La otra parte cerró la conexión.
    #[error("la conexión se cerró")]
    Closed,
    /// Error de entrada o salida; solo se conserva su clase.
    #[error("error de entrada o salida")]
    Io(std::io::ErrorKind),
    /// Una trama pasa del máximo.
    #[error("la trama es demasiado grande")]
    FrameTooLarge,
    /// Falló el apretón de manos Noise.
    #[error("falló el apretón de manos")]
    Handshake,
    /// Un mensaje no cumple el protocolo.
    #[error("mensaje no válido: {0}")]
    Protocol(ProtocolError),
}

impl From<ProtocolError> for PairingError {
    fn from(error: ProtocolError) -> Self {
        Self::Protocol(error)
    }
}

/// Lo que obtiene el celular tras una aceptación autenticada de la PC.
///
/// La llave y la dirección salen del QR y ya las autenticó Noise. La PC no
/// envía su nombre en esta versión: la #103 decide cómo lo obtiene el celular.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PairedPc {
    public_key: PublicKey,
    address: SocketAddr,
}

impl PairedPc {
    // Lo construye `AwaitingReply::read_reply` (Task 5).
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn new(public_key: PublicKey, address: SocketAddr) -> Self {
        Self {
            public_key,
            address,
        }
    }

    /// Llave pública de la PC.
    pub fn public_key(&self) -> PublicKey {
        self.public_key
    }

    /// Dirección donde estaba la PC.
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    /// Nombre provisional derivado de la llave, `PC A1B2-C3D4-E5F6-0718`:
    /// cumple el rango de la bóveda sin inventar información.
    pub fn provisional_name(&self) -> String {
        format!("PC {}", self.public_key.fingerprint())
    }

    /// Datos para guardar la PC en la bóveda. Sin `name`, usa
    /// [`provisional_name`](Self::provisional_name).
    pub fn into_new_paired_pc(self, name: Option<String>) -> NewPairedPc {
        NewPairedPc {
            public_key: *self.public_key.as_bytes(),
            name: name.unwrap_or_else(|| self.provisional_name()),
            last_ip: Some(self.address.ip().to_canonical()),
        }
    }
}

/// Lo que obtiene la PC cuando el usuario acepta: la llave del celular y su
/// nombre (solo informativo). La identidad es la llave. Su `Debug` solo
/// muestra la huella.
#[derive(Clone, PartialEq, Eq)]
pub struct PairedDevice {
    public_key: PublicKey,
    name: DeviceName,
}

impl PairedDevice {
    // Lo construye `PendingPairing::accept` (Task 5).
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn new(public_key: PublicKey, name: DeviceName) -> Self {
        Self { public_key, name }
    }

    /// Llave pública del celular.
    pub fn public_key(&self) -> PublicKey {
        self.public_key
    }

    /// Nombre que el celular dio.
    pub fn name(&self) -> &DeviceName {
        &self.name
    }
}

impl std::fmt::Debug for PairedDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PairedDevice({})", self.public_key.fingerprint())
    }
}

/// Estados del código de un solo uso en la PC.
#[derive(Debug, Clone, Copy)]
enum CodeState {
    /// Puede autenticar una primera trama; lleva los intentos fallidos.
    Active { failed: u8 },
    /// Una primera trama se autenticó: el código ya se gastó (final).
    Consumed,
    /// Pasaron los 120 s (final).
    Expired,
    /// Agotó los intentos fallidos (final).
    Exhausted,
    /// La app lo canceló (final).
    Cancelled,
}

/// Código de un solo uso en la PC: cuándo nació, cuántos intentos fallidos
/// lleva y si ya se gastó.
///
/// Todas las transiciones son atómicas porque pasan por `&mut self`: el
/// conductor toma un `Mutex` solo durante `accept_first`, que no hace entrada
/// ni salida, así que dos conexiones con el código correcto no pueden
/// consumirlo las dos.
///
/// Se crea en el mismo momento en que se genera el código y se muestra el QR:
/// los 2 minutos cuentan desde `new`, con reloj monótono (`Instant`).
pub struct PairingSession {
    identity: KeyPair,
    code: PairingCode,
    created: Instant,
    state: CodeState,
}

impl PairingSession {
    /// Sesión nueva con la identidad de la PC y el código del QR.
    pub fn new(identity: KeyPair, code: PairingCode, now: Instant) -> Self {
        Self {
            identity,
            code,
            created: now,
            state: CodeState::Active { failed: 0 },
        }
    }

    /// Procesa la primera trama de una conexión.
    ///
    /// Solo un código `Active` se usa para descifrar. Si la trama se
    /// autentica, el código pasa a `Consumed` aunque el usuario rechace
    /// después; si no, suma un intento fallido y al tercero queda
    /// `Exhausted`. La caducidad se evalúa primero. El motivo del rechazo es
    /// solo para la app local: la conexión nunca lo ve.
    pub fn accept_first(&mut self, now: Instant, frame: &[u8]) -> Result<AwaitingThird, Refusal> {
        self.refresh(now);
        if let Some(refusal) = self.final_refusal() {
            return Err(refusal);
        }
        if frame.len() != FIRST_FRAME_LEN {
            return Err(self.record_failure(Refusal::Malformed));
        }
        match self.respond(frame) {
            Ok(third) => {
                self.state = CodeState::Consumed;
                Ok(third)
            }
            Err(_) => Err(self.record_failure(Refusal::WrongCode)),
        }
    }

    /// Anota que la primera trama anunció un largo distinto de 48 bytes:
    /// cuenta como intento fallido si el código está `Active`.
    pub fn note_malformed(&mut self, now: Instant) -> Refusal {
        self.refresh(now);
        match self.final_refusal() {
            Some(refusal) => refusal,
            None => self.record_failure(Refusal::Malformed),
        }
    }

    /// La app cancela el código (por ejemplo, al cerrar la ventana del QR).
    /// Un código ya gastado sigue gastado.
    pub fn cancel(&mut self) {
        if matches!(self.state, CodeState::Active { .. }) {
            self.state = CodeState::Cancelled;
        }
    }

    fn refresh(&mut self, now: Instant) {
        if matches!(self.state, CodeState::Active { .. })
            && now.saturating_duration_since(self.created) >= PAIRING_CODE_TTL
        {
            self.state = CodeState::Expired;
        }
    }

    fn final_refusal(&self) -> Option<Refusal> {
        match self.state {
            CodeState::Active { .. } => None,
            CodeState::Consumed => Some(Refusal::AlreadyUsed),
            CodeState::Expired => Some(Refusal::Expired),
            CodeState::Exhausted => Some(Refusal::TooManyAttempts),
            CodeState::Cancelled => Some(Refusal::Cancelled),
        }
    }

    fn record_failure(&mut self, refusal: Refusal) -> Refusal {
        if let CodeState::Active { failed } = self.state {
            let failed = failed.saturating_add(1);
            self.state = if failed >= MAX_FAILED_ATTEMPTS {
                CodeState::Exhausted
            } else {
                CodeState::Active { failed }
            };
        }
        refusal
    }

    /// Lee la trama 1 con el código como PSK y escribe la trama 2.
    fn respond(&self, frame: &[u8]) -> Result<AwaitingThird, PairingError> {
        let mut handshake = noise_builder(
            self.identity.private().expose_secret(),
            self.code.expose_bytes(),
        )
        .and_then(|builder| builder.build_responder())
        .map_err(|_| PairingError::Handshake)?;
        let mut buffer = [0u8; MAX_FRAME_LEN];
        let payload = handshake
            .read_message(frame, &mut buffer)
            .map_err(|_| PairingError::Handshake)?;
        if payload != 0 {
            return Err(PairingError::Handshake);
        }
        let len = handshake
            .write_message(&[], &mut buffer)
            .map_err(|_| PairingError::Handshake)?;
        Ok(AwaitingThird {
            handshake,
            second: buffer[..len].to_vec(),
        })
    }
}

/// Configuración común de Noise: patrón, prólogo, llave estática propia y el
/// código como PSK en la posición 0. Las llaves se pasan como referencias
/// para no hacer copias propias (ver "Borrado de memoria").
fn noise_builder<'a>(
    private_key: &'a [u8; 32],
    code: &'a [u8; 32],
) -> Result<Builder<'a>, snow::Error> {
    let params = noise_params().map_err(|_| snow::Error::Input)?;
    Builder::new(params)
        .local_private_key(private_key)?
        .psk(0, code)?
        .prologue(PAIRING_PROLOGUE)
}

/// La PC autenticó la trama 1 y tiene lista la trama 2.
// El apretón de manos lo consume `read_third` (Task 5).
#[allow(dead_code)]
pub struct AwaitingThird {
    handshake: HandshakeState,
    second: Frame,
}

impl AwaitingThird {
    /// La trama 2, sin prefijo de largo, para mandarla al celular.
    pub fn second_message(&self) -> &[u8] {
        &self.second
    }
}

/// Celular que va a empezar el emparejamiento.
pub struct Initiator {
    handshake: HandshakeState,
    name: DeviceName,
    pc_key: PublicKey,
    address: SocketAddr,
}

impl Initiator {
    /// Prepara el intercambio con la oferta del QR y el nombre del celular.
    pub fn new(
        identity: &KeyPair,
        offer: &PairingOffer,
        name: DeviceName,
    ) -> Result<Self, PairingError> {
        let pc_key = offer.public_key();
        let handshake = noise_builder(
            identity.private().expose_secret(),
            offer.code().expose_bytes(),
        )
        .and_then(|builder| builder.remote_public_key(pc_key.as_bytes()))
        .and_then(|builder| builder.build_initiator())
        .map_err(|_| PairingError::Handshake)?;
        Ok(Self {
            handshake,
            name,
            pc_key,
            address: offer.address(),
        })
    }

    /// Escribe la trama 1 (`e, es`, 48 bytes).
    pub fn first_message(mut self) -> Result<(AwaitingSecond, Frame), PairingError> {
        let mut buffer = [0u8; MAX_FRAME_LEN];
        let len = self
            .handshake
            .write_message(&[], &mut buffer)
            .map_err(|_| PairingError::Handshake)?;
        let frame = buffer[..len].to_vec();
        Ok((
            AwaitingSecond {
                handshake: self.handshake,
                name: self.name,
                pc_key: self.pc_key,
                address: self.address,
            },
            frame,
        ))
    }
}

/// El celular mandó la trama 1 y espera la 2.
pub struct AwaitingSecond {
    handshake: HandshakeState,
    name: DeviceName,
    pc_key: PublicKey,
    address: SocketAddr,
}

impl AwaitingSecond {
    /// Lee la trama 2 y escribe la 3 (`s, se` y el nombre). Una trama 2 que
    /// no se autentica es `PeerNotAuthentic`: quien responde no tiene la
    /// llave privada de la llave pública del QR. El celular **no** manda la
    /// trama 3 en ese caso.
    pub fn read_second(mut self, frame: &[u8]) -> Result<(AwaitingReply, Frame), PairingError> {
        let mut buffer = [0u8; MAX_FRAME_LEN];
        let payload = self
            .handshake
            .read_message(frame, &mut buffer)
            .map_err(|_| PairingError::PeerNotAuthentic)?;
        if payload != 0 {
            return Err(PairingError::Handshake);
        }
        let request = PairingRequest::new(self.name).encode()?;
        let len = self
            .handshake
            .write_message(&request, &mut buffer)
            .map_err(|_| PairingError::Handshake)?;
        let frame = buffer[..len].to_vec();
        let transport = self
            .handshake
            .into_transport_mode()
            .map_err(|_| PairingError::Handshake)?;
        Ok((
            AwaitingReply {
                transport,
                pc_key: self.pc_key,
                address: self.address,
            },
            frame,
        ))
    }
}

/// El celular mandó la trama 3: espera la decisión de la PC. Aquí termina el
/// intercambio Noise, **no** el emparejamiento.
// `read_reply` lo agrega la Task 5.
#[allow(dead_code)]
pub struct AwaitingReply {
    transport: TransportState,
    pc_key: PublicKey,
    address: SocketAddr,
}

#[cfg(test)]
mod tests;
