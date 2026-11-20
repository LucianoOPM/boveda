//! Envío de una contraseña a la PC con Noise IK (#96).
//!
//! Máquinas de estado por tipos, sin sockets, y los conductores síncronos que
//! las unen con [`TimedStream`](super::TimedStream). El celular conoce la llave
//! de la PC y la PC aprende la del celular en la primera trama; la PC solo
//! contesta si esa llave está emparejada. El `SendMessage` viaja únicamente
//! después de completar el apretón de manos.
//!
//! # Plazos
//!
//! Hay dos plazos independientes. Los [`SEND_TTL`] (30 s) los cuenta la PC con
//! `Instant` desde que acepta la conexión y limitan la recepción **completa y
//! autenticada** del mensaje: no demuestran cuándo se generó la contraseña. La
//! espera del `Ack` por el celular ([`SendTimeouts::ack_wait`]) es aparte y no
//! amplía esa ventana. Los 30 s del portapapeles empiezan cuando Windows copia
//! la contraseña y son de la #98.

use std::io::ErrorKind;
use std::time::{Duration, Instant};

use secrecy::ExposeSecret;
use snow::params::NoiseParams;
use snow::{Builder, HandshakeState, TransportState};
use zeroize::Zeroizing;

use super::error::ProtocolError;
use super::identity::{KeyPair, PublicKey};
use super::message::{Ack, AckOutcome, SendMessage};
use super::pairing::{Frame, PairingError};
use super::transport::{
    CancelToken, MAX_SEND_FRAME_LEN, TimedStream, read_frame_body, read_frame_header, write_frame,
};

/// Patrón Noise del envío: el celular conoce la llave de la PC y la PC aprende
/// la del celular en la primera trama.
pub const SEND_PATTERN: &str = "Noise_IK_25519_ChaChaPoly_BLAKE2s";

/// Prólogo de Noise: separa este intercambio de cualquier otro.
pub const SEND_PROLOGUE: &[u8] = b"boveda/envio/v1";

/// Tiempo que tiene la PC, desde que acepta la conexión, para tener el mensaje
/// completo y autenticado.
pub const SEND_TTL: Duration = Duration::from_secs(30);

/// Longitud de la primera trama IK: `e` (32), `s` cifrada (48) y la etiqueta de
/// la carga vacía (16).
pub const SEND_FIRST_FRAME_LEN: usize = 96;

/// Longitud de la segunda trama IK: `e` (32) y la etiqueta de la carga vacía
/// (16).
pub const SEND_SECOND_FRAME_LEN: usize = 48;

/// Error del envío. Ninguna variante lleva datos del mensaje, secretos ni texto
/// de `snow`.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SendError {
    /// La llave del celular no está emparejada (**solo lo ve la app de la
    /// PC**: por la conexión se ve igual que un fallo).
    #[error("el dispositivo no está emparejado")]
    NotPaired,
    /// Pasaron los 30 s antes de tener el mensaje completo y autenticado.
    #[error("el mensaje caducó")]
    Expired,
    /// La respuesta de la PC no se autentica: no es la PC conocida.
    #[error("la PC no se pudo autenticar")]
    PeerNotAuthentic,
    /// Se agotó el tiempo de un paso.
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
    Io(ErrorKind),
    /// Una trama pasa del máximo.
    #[error("la trama es demasiado grande")]
    FrameTooLarge,
    /// Falló el apretón de manos Noise o el descifrado.
    #[error("falló el apretón de manos")]
    Handshake,
    /// Un mensaje no cumple el protocolo.
    #[error("mensaje no válido: {0}")]
    Protocol(ProtocolError),
}

impl From<ProtocolError> for SendError {
    fn from(error: ProtocolError) -> Self {
        Self::Protocol(error)
    }
}

impl From<PairingError> for SendError {
    /// Los errores de tramas y plazos pasan igual; lo propio del
    /// emparejamiento no tiene sentido aquí y cuenta como `Handshake`.
    fn from(error: PairingError) -> Self {
        match error {
            PairingError::Timeout => Self::Timeout,
            PairingError::Cancelled => Self::Cancelled,
            PairingError::Closed => Self::Closed,
            PairingError::Io(kind) => Self::Io(kind),
            PairingError::FrameTooLarge => Self::FrameTooLarge,
            PairingError::PeerNotAuthentic => Self::PeerNotAuthentic,
            PairingError::Protocol(error) => Self::Protocol(error),
            _ => Self::Handshake,
        }
    }
}

/// Plazos del envío.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SendTimeouts {
    /// Máximo por trama leída o escrita.
    pub step: Duration,
    /// Máximo, contado desde que la PC acepta la conexión (y, en el celular,
    /// desde que empieza a enviar), hasta tener el mensaje completo y
    /// autenticado. Nunca pasa de [`SEND_TTL`]: un valor mayor se recorta a
    /// 30 s ([`capped_total`](Self::capped_total)); uno menor sirve para pruebas.
    pub total: Duration,
    /// Tiempo que espera el celular el `Ack`. Es independiente de `total` y no
    /// lo amplía.
    pub ack_wait: Duration,
}

impl SendTimeouts {
    /// El plazo total que se aplica: `total`, sin pasar nunca de [`SEND_TTL`].
    pub fn capped_total(&self) -> Duration {
        self.total.min(SEND_TTL)
    }
}

impl Default for SendTimeouts {
    fn default() -> Self {
        Self {
            step: Duration::from_secs(10),
            total: SEND_TTL,
            ack_wait: Duration::from_secs(10),
        }
    }
}

/// Parámetros de Noise del envío. Un patrón que no se pueda leer es
/// `Handshake`: no ocurre con la constante, pero la API no hace panic.
fn noise_params() -> Result<NoiseParams, SendError> {
    SEND_PATTERN.parse().map_err(|_| SendError::Handshake)
}

/// Configuración común de Noise: patrón, prólogo y llave estática propia. La
/// llave se pasa como referencia para no hacer copias propias (ver el borrado
/// de memoria en el diseño).
fn noise_builder(private_key: &[u8; 32]) -> Result<Builder<'_>, SendError> {
    Builder::new(noise_params()?)
        .local_private_key(private_key)
        .and_then(|builder| builder.prologue(SEND_PROLOGUE))
        .map_err(|_| SendError::Handshake)
}

/// Tamaño del búfer de las tramas del apretón de manos (96 la mayor).
const HANDSHAKE_BUFFER: usize = 128;

/// La PC, antes de leer la primera trama de una conexión.
pub struct Responder<'a> {
    identity: &'a KeyPair,
}

impl<'a> Responder<'a> {
    /// Prepara a la PC con su identidad.
    pub fn new(identity: &'a KeyPair) -> Self {
        Self { identity }
    }

    /// Lee la primera trama (`e, es, s, ss`, 96 bytes) y decide si contesta.
    ///
    /// Con la llave estática que recupera `snow` (nunca una identidad que
    /// declare el cliente) consulta `is_paired` **antes** de escribir nada: si
    /// no está emparejada es [`SendError::NotPaired`] y no hay segunda trama.
    /// Una trama de otro largo, que no descifra o con carga es `Handshake`. Los
    /// 30 s de [`SEND_TTL`] cuentan desde `now`.
    pub fn read_first(
        self,
        now: Instant,
        frame: &[u8],
        is_paired: impl Fn(&PublicKey) -> bool,
    ) -> Result<(AwaitingSend, Frame), SendError> {
        if frame.len() != SEND_FIRST_FRAME_LEN {
            return Err(SendError::Handshake);
        }
        let mut handshake = noise_builder(self.identity.private().expose_secret())?
            .build_responder()
            .map_err(|_| SendError::Handshake)?;
        let mut buffer = [0u8; HANDSHAKE_BUFFER];
        let payload = handshake
            .read_message(frame, &mut buffer)
            .map_err(|_| SendError::Handshake)?;
        if payload != 0 {
            return Err(SendError::Handshake);
        }
        let peer: [u8; 32] = handshake
            .get_remote_static()
            .and_then(|key| key.try_into().ok())
            .ok_or(SendError::Handshake)?;
        let peer = PublicKey::from_bytes(peer);
        if !is_paired(&peer) {
            return Err(SendError::NotPaired);
        }
        let len = handshake
            .write_message(&[], &mut buffer)
            .map_err(|_| SendError::Handshake)?;
        let second = buffer[..len].to_vec();
        let transport = handshake
            .into_transport_mode()
            .map_err(|_| SendError::Handshake)?;
        Ok((
            AwaitingSend {
                transport,
                peer,
                deadline: now + SEND_TTL,
            },
            second,
        ))
    }
}

/// La PC contestó la segunda trama y espera el mensaje. El celular ya está
/// autenticado y autorizado; todavía no se leyó ningún contenido.
pub struct AwaitingSend {
    transport: TransportState,
    peer: PublicKey,
    deadline: Instant,
}

impl AwaitingSend {
    /// Llave pública del celular, probada por el apretón de manos.
    pub fn peer(&self) -> PublicKey {
        self.peer
    }

    /// Cambia el plazo (lo usa el conductor con `SendTimeouts::total`).
    pub(crate) fn with_deadline(mut self, deadline: Instant) -> Self {
        self.deadline = deadline;
        self
    }

    /// Lee la trama 3: el `SendMessage` cifrado por el canal Noise.
    ///
    /// `now` es el instante en que la trama llegó completa. Desde `deadline`
    /// (apretón de manos más [`SEND_TTL`]) el mensaje **caduca**: es
    /// [`SendError::Expired`] y ni siquiera se descifra, así que el secreto no
    /// llega a la app. Una trama alterada, sin cifrar o de otra sesión no se
    /// autentica (`Handshake`). Esto consume el estado: una conexión acepta un
    /// solo envío. El texto plano vive solo en búferes `Zeroizing` y en el
    /// `SendMessage`, que se borra al soltarse.
    pub fn read_send(self, now: Instant, frame: &[u8]) -> Result<ReceivedSend, SendError> {
        if now >= self.deadline {
            return Err(SendError::Expired);
        }
        if frame.len() > MAX_SEND_FRAME_LEN {
            return Err(SendError::FrameTooLarge);
        }
        let Self {
            mut transport,
            peer,
            ..
        } = self;
        let mut plain = Zeroizing::new(vec![0u8; frame.len()]);
        let len = transport
            .read_message(frame, &mut plain)
            .map_err(|_| SendError::Handshake)?;
        let message = SendMessage::decode(&plain[..len])?;
        Ok(ReceivedSend {
            transport,
            peer,
            message,
        })
    }
}

/// Un envío autenticado, autorizado y dentro de plazo, a la espera de que la
/// app escriba la contraseña en el portapapeles y responda.
///
/// Su `Debug` solo muestra la huella del celular.
pub struct ReceivedSend {
    transport: TransportState,
    peer: PublicKey,
    message: SendMessage,
}

impl ReceivedSend {
    /// Llave pública del celular.
    pub fn peer(&self) -> PublicKey {
        self.peer
    }

    /// El mensaje recibido (la contraseña se borra al soltarse).
    pub fn message(&self) -> &SendMessage {
        &self.message
    }

    /// Cifra la confirmación (trama 4). Consume el envío: la contraseña se
    /// borra al terminar.
    pub fn ack(mut self, outcome: AckOutcome) -> Result<Frame, SendError> {
        let plain = Ack::new(outcome).encode()?;
        let mut buffer = [0u8; ACK_BUFFER];
        let len = self
            .transport
            .write_message(&plain, &mut buffer)
            .map_err(|_| SendError::Handshake)?;
        Ok(buffer[..len].to_vec())
    }
}

impl std::fmt::Debug for ReceivedSend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ReceivedSend({})", self.peer.fingerprint())
    }
}

/// Tamaño del búfer de la confirmación (19 bytes cifrada).
const ACK_BUFFER: usize = 64;

/// Celular que va a enviar.
pub struct Sender {
    handshake: HandshakeState,
}

impl Sender {
    /// Prepara el envío con la identidad del celular y la llave de la PC.
    pub fn new(identity: &KeyPair, pc_key: &PublicKey) -> Result<Self, SendError> {
        let handshake = noise_builder(identity.private().expose_secret())?
            .remote_public_key(pc_key.as_bytes())
            .and_then(|builder| builder.build_initiator())
            .map_err(|_| SendError::Handshake)?;
        Ok(Self { handshake })
    }

    /// Escribe la primera trama (96 bytes, sin datos de aplicación).
    pub fn first_message(mut self) -> Result<(AwaitingResponse, Frame), SendError> {
        let mut buffer = [0u8; HANDSHAKE_BUFFER];
        let len = self
            .handshake
            .write_message(&[], &mut buffer)
            .map_err(|_| SendError::Handshake)?;
        let frame = buffer[..len].to_vec();
        Ok((
            AwaitingResponse {
                handshake: self.handshake,
            },
            frame,
        ))
    }
}

/// El celular mandó la primera trama y espera la segunda.
pub struct AwaitingResponse {
    handshake: HandshakeState,
}

impl AwaitingResponse {
    /// Lee la trama 2 y escribe la trama 3: el `SendMessage` cifrado.
    ///
    /// Una trama 2 que no se autentica (otra llave privada que la de la PC
    /// conocida, otra sesión o alterada) es [`SendError::PeerNotAuthentic`]: la
    /// contraseña **no sale**. Solo tras autenticarla se cifra el mensaje.
    pub fn read_second(
        mut self,
        frame: &[u8],
        message: &SendMessage,
    ) -> Result<(AwaitingAck, Frame), SendError> {
        if frame.len() != SEND_SECOND_FRAME_LEN {
            return Err(SendError::PeerNotAuthentic);
        }
        let mut buffer = [0u8; HANDSHAKE_BUFFER];
        let payload = self
            .handshake
            .read_message(frame, &mut buffer)
            .map_err(|_| SendError::PeerNotAuthentic)?;
        if payload != 0 {
            return Err(SendError::Handshake);
        }
        let mut transport = self
            .handshake
            .into_transport_mode()
            .map_err(|_| SendError::Handshake)?;
        let plain = message.encode()?;
        // El búfer recibe primero el texto plano y queda con el cifrado: va en
        // `Zeroizing` por ese paso intermedio.
        let mut sealed = Zeroizing::new(vec![0u8; plain.len() + TAG_LEN]);
        let len = transport
            .write_message(&plain, &mut sealed)
            .map_err(|_| SendError::Handshake)?;
        let third = sealed[..len].to_vec();
        Ok((AwaitingAck { transport }, third))
    }
}

/// El celular mandó el mensaje y espera la confirmación de la PC.
pub struct AwaitingAck {
    transport: TransportState,
}

impl AwaitingAck {
    /// Lee la trama 4 (la confirmación, cifrada por el canal Noise). Una trama
    /// alterada, sin cifrar o de otra sesión no se autentica (`Handshake`).
    pub fn read_ack(mut self, frame: &[u8]) -> Result<AckOutcome, SendError> {
        if frame.len() > ACK_BUFFER {
            return Err(SendError::FrameTooLarge);
        }
        let mut buffer = [0u8; ACK_BUFFER];
        let len = self
            .transport
            .read_message(frame, &mut buffer)
            .map_err(|_| SendError::Handshake)?;
        Ok(Ack::decode(&buffer[..len])?.outcome())
    }
}

/// Etiqueta de autenticación de ChaCha20-Poly1305.
const TAG_LEN: usize = 16;

/// Un timeout que ocurre con el plazo total vencido es una caducidad; si no,
/// es solo un paso lento. El resto de los errores pasa igual.
fn classify(error: PairingError, deadline: Instant) -> SendError {
    match SendError::from(error) {
        SendError::Timeout if Instant::now() >= deadline => SendError::Expired,
        other => other,
    }
}

/// Conductor de la PC: atiende **una** conexión entrante hasta tener un
/// [`SendMessage`] autenticado, autorizado y dentro de plazo, y lo devuelve
/// como [`IncomingSend`] a la espera de que la app lo escriba en el
/// portapapeles.
///
/// Los [`SendTimeouts::total`] (30 s por defecto) se cuentan desde que se
/// entra: la app la llama justo al aceptar la conexión. Si un paso agota su
/// plazo con el total ya vencido, el error es [`SendError::Expired`]; si no,
/// [`SendError::Timeout`]. Una llave no emparejada se cierra **sin escribir
/// nada**. La conexión se recibe por valor y se cierra al soltarse, en
/// cualquier camino de salida con error.
pub fn receive_send<S: TimedStream>(
    mut stream: S,
    identity: &KeyPair,
    is_paired: impl Fn(&PublicKey) -> bool,
    cancel: &CancelToken,
    timeouts: &SendTimeouts,
) -> Result<IncomingSend<S>, SendError> {
    let deadline = Instant::now() + timeouts.capped_total();
    let step = timeouts.step;
    let late = |error| classify(error, deadline);

    let len = read_frame_header(&mut stream, deadline, step, cancel).map_err(late)?;
    if len != SEND_FIRST_FRAME_LEN {
        // Se rechaza sin leer el cuerpo ni contestar.
        return Err(SendError::Handshake);
    }
    let first = read_frame_body(&mut stream, len, MAX_SEND_FRAME_LEN, deadline, step, cancel)
        .map_err(late)?;
    let (awaiting, second) =
        Responder::new(identity).read_first(Instant::now(), &first, is_paired)?;
    let awaiting = awaiting.with_deadline(deadline);
    write_frame(
        &mut stream,
        &second,
        MAX_SEND_FRAME_LEN,
        deadline,
        step,
        cancel,
    )
    .map_err(late)?;

    let len = read_frame_header(&mut stream, deadline, step, cancel).map_err(late)?;
    let frame = read_frame_body(&mut stream, len, MAX_SEND_FRAME_LEN, deadline, step, cancel)
        .map_err(late)?;
    let received = awaiting.read_send(Instant::now(), &frame)?;
    Ok(IncomingSend {
        stream,
        received,
        step,
        cancel: cancel.clone(),
    })
}

/// Envío recibido, a la espera de que la app escriba la contraseña en el
/// portapapeles y conteste con [`copied`](Self::copied) o
/// [`failed`](Self::failed).
///
/// El núcleo no lanza hilos ni temporizadores. Soltarlo sin contestar cierra la
/// conexión sin `Ack`, y el celular lo ve como `Closed`. La contraseña se borra
/// al soltarse.
pub struct IncomingSend<S> {
    stream: S,
    received: ReceivedSend,
    step: Duration,
    cancel: CancelToken,
}

impl<S: TimedStream> IncomingSend<S> {
    /// Llave pública del celular.
    pub fn peer(&self) -> PublicKey {
        self.received.peer()
    }

    /// El mensaje recibido.
    pub fn message(&self) -> &SendMessage {
        self.received.message()
    }

    /// La contraseña quedó en el portapapeles: escribe `Copied` y cierra.
    pub fn copied(self) -> Result<(), SendError> {
        self.answer(AckOutcome::Copied)
    }

    /// No se pudo escribir en el portapapeles: escribe `Failed` y cierra.
    pub fn failed(self) -> Result<(), SendError> {
        self.answer(AckOutcome::Failed)
    }

    fn answer(self, outcome: AckOutcome) -> Result<(), SendError> {
        let Self {
            mut stream,
            received,
            step,
            cancel,
        } = self;
        // `ack` consume el envío: la contraseña se borra antes de escribir.
        let frame = received.ack(outcome)?;
        write_frame(
            &mut stream,
            &frame,
            ACK_BUFFER,
            Instant::now() + step,
            step,
            &cancel,
        )?;
        Ok(())
    }
}

/// Conductor del celular: envía `message` a la PC por `stream` (ya conectado)
/// y devuelve la confirmación. La conexión se cierra al terminar, con o sin
/// error.
///
/// Si la segunda trama no se autentica (`PeerNotAuthentic`), la contraseña no
/// sale. Una PC que no tiene emparejado a este celular cierra sin contestar: el
/// celular ve `Closed`, igual que ante un corte de red. Que la PC conteste
/// `Failed` es `Ok(AckOutcome::Failed)`. La espera del `Ack` dura como máximo
/// [`SendTimeouts::ack_wait`], aparte del resto.
pub fn send_to_pc<S: TimedStream>(
    mut stream: S,
    identity: &KeyPair,
    pc_key: &PublicKey,
    message: &SendMessage,
    cancel: &CancelToken,
    timeouts: &SendTimeouts,
) -> Result<AckOutcome, SendError> {
    let deadline = Instant::now() + timeouts.capped_total();
    let step = timeouts.step;

    let (awaiting, first) = Sender::new(identity, pc_key)?.first_message()?;
    write_frame(
        &mut stream,
        &first,
        MAX_SEND_FRAME_LEN,
        deadline,
        step,
        cancel,
    )?;
    let len = read_frame_header(&mut stream, deadline, step, cancel)?;
    if len != SEND_SECOND_FRAME_LEN {
        return Err(SendError::PeerNotAuthentic);
    }
    let second = read_frame_body(&mut stream, len, MAX_SEND_FRAME_LEN, deadline, step, cancel)?;
    let (awaiting_ack, third) = awaiting.read_second(&second, message)?;
    write_frame(
        &mut stream,
        &third,
        MAX_SEND_FRAME_LEN,
        deadline,
        step,
        cancel,
    )?;

    let wait = timeouts.ack_wait;
    let ack_deadline = Instant::now() + wait;
    let len = read_frame_header(&mut stream, ack_deadline, wait, cancel)?;
    let frame = read_frame_body(&mut stream, len, ACK_BUFFER, ack_deadline, wait, cancel)?;
    awaiting_ack.read_ack(&frame)
}

#[cfg(test)]
mod tests;
