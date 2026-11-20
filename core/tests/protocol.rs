//! Pruebas de punta a punta de la API pública `vault_core::protocol` (#94):
//! solo tipos y codificación, sin red.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use proptest::prelude::*;
use secrecy::{ExposeSecret, SecretString};
use vault_core::protocol::{
    Ack, AckOutcome, KeyPair, MAX_MESSAGE_LEN, MAX_PAIRING_OFFER_LEN, PairingCode, PairingOffer,
    ProtocolError, PublicKey, SendMessage,
};

const ADDRESS: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 20)), 50000);

fn offer(address: SocketAddr) -> PairingOffer {
    let pair = KeyPair::generate().unwrap();
    PairingOffer::new(pair.public(), address, PairingCode::generate().unwrap()).unwrap()
}

fn send(text: &str) -> SendMessage {
    SendMessage::new(SecretString::from(text)).unwrap()
}

/// Decodifica `bytes` como los tres tipos; ninguno debe entrar en pánico.
fn decode_all(bytes: &[u8]) -> [bool; 3] {
    [
        PairingOffer::decode(bytes).is_ok(),
        SendMessage::decode(bytes).is_ok(),
        Ack::decode(bytes).is_ok(),
    ]
}

#[test]
fn offer_roundtrip_with_generated_keys() {
    for address in [
        ADDRESS,
        SocketAddr::new(
            IpAddr::V6(Ipv6Addr::new(0xfd00, 0, 0, 0, 0, 0, 0, 1)),
            65535,
        ),
    ] {
        let original = offer(address);
        let bytes = original.encode().unwrap();
        assert!(bytes.len() <= MAX_PAIRING_OFFER_LEN);
        let decoded = PairingOffer::decode(&bytes).unwrap();
        assert_eq!(decoded.public_key(), original.public_key());
        assert_eq!(decoded.address(), address);
        assert_eq!(
            decoded.code().expose_bytes(),
            original.code().expose_bytes()
        );
    }
}

#[test]
fn send_and_ack_roundtrip() {
    let decoded = SendMessage::decode(&send("contraseña-ñ-🔐").encode().unwrap()).unwrap();
    assert_eq!(decoded.password().expose_secret(), "contraseña-ñ-🔐");
    assert_eq!(decoded.ttl_secs(), 30);
    for outcome in [AckOutcome::Copied, AckOutcome::Failed] {
        let ack = Ack::decode(&Ack::new(outcome).encode().unwrap()).unwrap();
        assert_eq!(ack.outcome(), outcome);
    }
}

#[test]
fn messages_are_not_interchangeable() {
    let offer_bytes = offer(ADDRESS).encode().unwrap();
    let send_bytes = send("clave").encode().unwrap();
    let ack_bytes = Ack::new(AckOutcome::Copied).encode().unwrap();
    assert_eq!(decode_all(&offer_bytes), [true, false, false]);
    assert_eq!(decode_all(&send_bytes), [false, true, false]);
    assert_eq!(decode_all(&ack_bytes), [false, false, true]);
}

#[test]
fn public_key_matches_the_private_key() {
    let pair = KeyPair::generate().unwrap();
    assert_eq!(PublicKey::from_private(pair.private()), pair.public());
    let fingerprint = pair.public().fingerprint().to_string();
    assert_eq!(fingerprint.len(), 19);
    assert_eq!(fingerprint.matches('-').count(), 3);
    assert_eq!(fingerprint, fingerprint.to_uppercase());
}

#[test]
fn secrets_do_not_leak_through_debug() {
    let pair = KeyPair::generate().unwrap();
    let message = send("contraseña-reconocible");
    let code = PairingCode::generate().unwrap();
    let debug = format!("{pair:?} {message:?} {code:?}");
    assert!(!debug.contains("reconocible"));
    assert!(!debug.contains(&format!("{:?}", pair.private().expose_secret())));
    assert!(!debug.contains(&format!("{:?}", code.expose_bytes())));
}

#[test]
fn error_is_a_standard_error() {
    fn assert_error<E: std::error::Error + Send + Sync + 'static>() {}
    assert_error::<ProtocolError>();
}

proptest! {
    #[test]
    fn any_valid_password_roundtrips(chars in prop::collection::vec(any::<char>(), 1..200)) {
        let text: String = chars.into_iter().collect();
        let decoded = SendMessage::decode(&send(&text).encode().unwrap()).unwrap();
        prop_assert_eq!(decoded.password().expose_secret().as_bytes(), text.as_bytes());
    }

    #[test]
    fn any_valid_offer_roundtrips(
        ip in any::<[u8; 4]>(),
        v6 in any::<bool>(),
        ip6 in any::<[u8; 16]>(),
        port in 1u16..=u16::MAX,
    ) {
        let ip = if v6 { IpAddr::from(ip6) } else { IpAddr::from(ip) };
        prop_assume!(!ip.is_unspecified() && !ip.is_multicast());
        let address = SocketAddr::new(ip, port);
        let original = offer(address);
        let bytes = original.encode().unwrap();
        prop_assert!(bytes.len() <= MAX_PAIRING_OFFER_LEN);
        let decoded = PairingOffer::decode(&bytes).unwrap();
        prop_assert_eq!(decoded.address(), address);
        prop_assert_eq!(decoded.public_key(), original.public_key());
        prop_assert_eq!(decoded.code().expose_bytes(), original.code().expose_bytes());
    }

    #[test]
    fn random_bytes_never_panic(bytes in prop::collection::vec(any::<u8>(), 0..5000)) {
        let _ = decode_all(&bytes);
    }

    #[test]
    fn a_changed_byte_never_panics(index in any::<prop::sample::Index>(), value in any::<u8>()) {
        let valid = [
            offer(ADDRESS).encode().unwrap().to_vec(),
            send("una contraseña de prueba").encode().unwrap().to_vec(),
            Ack::new(AckOutcome::Failed).encode().unwrap().to_vec(),
        ];
        for mut bytes in valid {
            let i = index.index(bytes.len());
            bytes[i] = value;
            let _ = decode_all(&bytes);
        }
    }

    #[test]
    fn any_suffix_is_always_rejected(suffix in prop::collection::vec(any::<u8>(), 1..64)) {
        let valid = [
            offer(ADDRESS).encode().unwrap().to_vec(),
            send("una contraseña de prueba").encode().unwrap().to_vec(),
            Ack::new(AckOutcome::Copied).encode().unwrap().to_vec(),
        ];
        for mut bytes in valid {
            bytes.extend_from_slice(&suffix);
            prop_assert!(bytes.len() <= MAX_MESSAGE_LEN);
            prop_assert_eq!(decode_all(&bytes), [false, false, false]);
        }
    }
}

// ---------------------------------------------------------------------------
// Emparejamiento Noise (#95): los dos extremos en el mismo proceso, por
// `TcpStream` en 127.0.0.1. Solo la API pública.
// ---------------------------------------------------------------------------

use std::io::{ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Barrier, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use secrecy::SecretBox;
use vault_core::protocol::{
    CancelToken, DeviceName, Initiator, PAIRING_CODE_TTL, PairedPc, PairingError, PairingSession,
    PairingTimeouts, PendingConnection, Refusal, pair_with_pc, serve_pairing,
};

fn secret_keys(n: u8) -> KeyPair {
    KeyPair::from_private(SecretBox::new(Box::new([n; 32])))
}

fn secret_code(n: u8) -> PairingCode {
    PairingCode::from_bytes(SecretBox::new(Box::new([n; 32])))
}

/// La PC tiene la llave 1 y el código 7; el celular, la llave 2.
fn pc_key() -> PublicKey {
    secret_keys(1).public()
}

fn phone_name() -> DeviceName {
    DeviceName::new("Pixel de Ana").unwrap()
}

fn quick() -> PairingTimeouts {
    PairingTimeouts {
        step: Duration::from_millis(500),
        handshake: Duration::from_secs(4),
        confirmation: Duration::from_secs(4),
        reply_wait: Duration::from_secs(4),
    }
}

fn framed(bytes: &[u8]) -> Vec<u8> {
    let mut out = (bytes.len() as u16).to_be_bytes().to_vec();
    out.extend_from_slice(bytes);
    out
}

type Served = Result<PendingConnection<TcpStream>, PairingError>;

/// La PC: un oyente en localhost y su sesión con el código de un solo uso.
struct Pc {
    listener: TcpListener,
    addr: SocketAddr,
    session: Arc<Mutex<PairingSession>>,
}

impl Pc {
    fn new() -> Self {
        Self::with_session(PairingSession::new(
            secret_keys(1),
            secret_code(7),
            Instant::now(),
        ))
    }

    fn with_session(session: PairingSession) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        Self {
            listener,
            addr,
            session: Arc::new(Mutex::new(session)),
        }
    }

    /// Atiende una conexión en otro hilo.
    fn serve(&self, timeouts: PairingTimeouts) -> JoinHandle<Served> {
        self.serve_with(CancelToken::new(), timeouts)
    }

    fn serve_with(&self, cancel: CancelToken, timeouts: PairingTimeouts) -> JoinHandle<Served> {
        let listener = self.listener.try_clone().unwrap();
        let session = self.session.clone();
        thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            serve_pairing(stream, &session, &cancel, &timeouts)
        })
    }

    fn offer(&self, code_n: u8) -> PairingOffer {
        PairingOffer::new(pc_key(), self.addr, secret_code(code_n)).unwrap()
    }

    /// El celular completo, con el código `code_n`.
    fn phone(&self, code_n: u8, timeouts: PairingTimeouts) -> Result<PairedPc, PairingError> {
        let stream = TcpStream::connect(self.addr).unwrap();
        pair_with_pc(
            stream,
            &secret_keys(2),
            &self.offer(code_n),
            phone_name(),
            &CancelToken::new(),
            &timeouts,
        )
    }

    /// Un cliente a mano: manda solo la trama 1 y devuelve todo lo que la PC
    /// le escribió hasta cerrar.
    fn first_frame_only(&self, code_n: u8) -> Vec<u8> {
        let (_, frame) = Initiator::new(&secret_keys(2), &self.offer(code_n), phone_name())
            .unwrap()
            .first_message()
            .unwrap();
        let mut stream = TcpStream::connect(self.addr).unwrap();
        stream.write_all(&framed(&frame)).unwrap();
        read_until_close(stream)
    }
}

/// Lee hasta que el otro extremo cierre. Si no cierra a tiempo, la prueba falla.
fn read_until_close(mut stream: TcpStream) -> Vec<u8> {
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let mut out = Vec::new();
    let mut buffer = [0u8; 256];
    loop {
        match stream.read(&mut buffer) {
            Ok(0) => return out,
            Ok(n) => out.extend_from_slice(&buffer[..n]),
            Err(e)
                if matches!(
                    e.kind(),
                    ErrorKind::ConnectionReset | ErrorKind::ConnectionAborted
                ) =>
            {
                return out;
            }
            Err(e) => panic!("la PC no cerró la conexión: {e}"),
        }
    }
}

fn read_exact_frame(stream: &mut TcpStream) -> Vec<u8> {
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let mut header = [0u8; 2];
    stream.read_exact(&mut header).unwrap();
    let mut body = vec![0u8; usize::from(u16::from_be_bytes(header))];
    stream.read_exact(&mut body).unwrap();
    body
}

#[test]
fn pairing_with_correct_code_both_know_each_other_after_confirm() {
    let pc = Pc::new();
    let server = pc.serve(quick());
    let phone = thread::spawn({
        let addr = pc.addr;
        move || {
            let stream = TcpStream::connect(addr).unwrap();
            let offer = PairingOffer::new(pc_key(), addr, secret_code(7)).unwrap();
            pair_with_pc(
                stream,
                &secret_keys(2),
                &offer,
                phone_name(),
                &CancelToken::new(),
                &quick(),
            )
        }
    });
    let connection = server.join().unwrap().unwrap();
    assert_eq!(
        connection.pending().peer_public_key(),
        secret_keys(2).public()
    );
    assert_eq!(
        connection.pending().fingerprint(),
        secret_keys(2).public().fingerprint()
    );
    assert_eq!(connection.pending().peer_name().as_str(), "Pixel de Ana");
    let device = connection.confirm().unwrap();
    let paired_pc = phone.join().unwrap().unwrap();
    assert_eq!(device.public_key(), secret_keys(2).public());
    assert_eq!(device.name().as_str(), "Pixel de Ana");
    assert_eq!(paired_pc.public_key(), pc_key());
    assert_eq!(paired_pc.address(), pc.addr);
}

#[test]
fn wrong_expired_reused_and_exhausted_codes_look_the_same_to_the_phone() {
    // Cada caso: (la PC rechaza con ese motivo, y el celular ve `Closed` sin
    // haber recibido ni un byte).
    let old = Instant::now()
        .checked_sub(PAIRING_CODE_TTL + Duration::from_secs(1))
        .expect("el reloj monótono es más joven que 2 minutos");
    let expired = Pc::with_session(PairingSession::new(secret_keys(1), secret_code(7), old));

    let reused = Pc::new();
    // Primer uso: el apretón de manos llega hasta consumir el código.
    let first = reused.serve(quick());
    let (_awaiting, frame) = Initiator::new(&secret_keys(2), &reused.offer(7), phone_name())
        .unwrap()
        .first_message()
        .unwrap();
    let mut stream = TcpStream::connect(reused.addr).unwrap();
    stream.write_all(&framed(&frame)).unwrap();
    let _frame2 = read_exact_frame(&mut stream);
    drop(stream);
    let _ = first.join().unwrap();

    let exhausted = Pc::new();
    for _ in 0..3 {
        let server = exhausted.serve(quick());
        assert!(exhausted.first_frame_only(8).is_empty());
        assert_eq!(
            server.join().unwrap().err(),
            Some(PairingError::Refused(Refusal::WrongCode))
        );
    }

    let wrong = Pc::new();
    let cases: [(&Pc, u8, Refusal); 4] = [
        (&wrong, 8, Refusal::WrongCode),
        (&expired, 7, Refusal::Expired),
        (&reused, 7, Refusal::AlreadyUsed),
        (&exhausted, 7, Refusal::TooManyAttempts),
    ];
    for (pc, code_n, reason) in cases {
        let server = pc.serve(quick());
        let phone = pc.phone(code_n, quick());
        assert_eq!(
            server.join().unwrap().err(),
            Some(PairingError::Refused(reason)),
            "{reason:?}"
        );
        assert_eq!(phone.err(), Some(PairingError::Closed), "{reason:?}");

        let server = pc.serve(quick());
        let received = pc.first_frame_only(code_n);
        let _ = server.join().unwrap();
        assert!(
            received.is_empty(),
            "{reason:?}: la PC escribió {received:?}"
        );
    }
}

#[test]
fn pc_reject_gives_rejected_and_no_trust() {
    let pc = Pc::new();
    let server = pc.serve(quick());
    let phone = thread::spawn({
        let (addr, offer) = (pc.addr, pc.offer(7));
        move || {
            let stream = TcpStream::connect(addr).unwrap();
            pair_with_pc(
                stream,
                &secret_keys(2),
                &offer,
                phone_name(),
                &CancelToken::new(),
                &quick(),
            )
        }
    });
    server.join().unwrap().unwrap().reject().unwrap();
    assert_eq!(phone.join().unwrap().err(), Some(PairingError::Rejected));
}

#[test]
fn no_decision_in_time_gives_no_device() {
    // El usuario no decide: el celular se rinde a su plazo.
    let pc = Pc::new();
    let server = pc.serve(quick());
    let short = PairingTimeouts {
        reply_wait: Duration::from_millis(400),
        ..quick()
    };
    let phone = pc.phone(7, short);
    let connection = server.join().unwrap().unwrap();
    assert_eq!(phone.err(), Some(PairingError::ConfirmationTimeout));
    drop(connection);

    // La app contesta tarde: no hay dispositivo y el celular ve un rechazo.
    let pc = Pc::new();
    let late = PairingTimeouts {
        confirmation: Duration::ZERO,
        ..quick()
    };
    let server = pc.serve(late);
    let phone = thread::spawn({
        let (addr, offer) = (pc.addr, pc.offer(7));
        move || {
            let stream = TcpStream::connect(addr).unwrap();
            pair_with_pc(
                stream,
                &secret_keys(2),
                &offer,
                phone_name(),
                &CancelToken::new(),
                &quick(),
            )
        }
    });
    let connection = server.join().unwrap().unwrap();
    assert_eq!(
        connection.confirm().err(),
        Some(PairingError::ConfirmationTimeout)
    );
    assert_eq!(phone.join().unwrap().err(), Some(PairingError::Rejected));
}

#[test]
fn dropping_pending_connection_closes() {
    let pc = Pc::new();
    let server = pc.serve(quick());
    let phone = thread::spawn({
        let (addr, offer) = (pc.addr, pc.offer(7));
        move || {
            let stream = TcpStream::connect(addr).unwrap();
            pair_with_pc(
                stream,
                &secret_keys(2),
                &offer,
                phone_name(),
                &CancelToken::new(),
                &quick(),
            )
        }
    });
    drop(server.join().unwrap().unwrap());
    assert_eq!(phone.join().unwrap().err(), Some(PairingError::Closed));
}

#[test]
fn pc_with_another_private_key_cannot_answer_the_phone() {
    // El QR trae la llave 1, pero quien responde tiene la llave 3: no puede
    // descifrar la trama 1, no responde, y el celular no manda nada más.
    let impostor = Pc::with_session(PairingSession::new(
        secret_keys(3),
        secret_code(7),
        Instant::now(),
    ));
    let server = impostor.serve(quick());
    let phone = {
        let stream = TcpStream::connect(impostor.addr).unwrap();
        // La oferta lleva la llave de la PC *verdadera* (1).
        let offer = PairingOffer::new(pc_key(), impostor.addr, secret_code(7)).unwrap();
        pair_with_pc(
            stream,
            &secret_keys(2),
            &offer,
            phone_name(),
            &CancelToken::new(),
            &quick(),
        )
    };
    assert_eq!(phone.err(), Some(PairingError::Closed));
    assert_eq!(
        server.join().unwrap().err(),
        Some(PairingError::Refused(Refusal::WrongCode))
    );
}

#[test]
fn forged_second_frame_is_not_authentic_and_phone_sends_no_third_frame() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let fake = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let _frame1 = read_exact_frame(&mut stream);
        stream.write_all(&framed(&[0x42; 48])).unwrap();
        // Todo lo que el celular mande después de la trama 2 falsa.
        read_until_close(stream)
    });
    let stream = TcpStream::connect(addr).unwrap();
    let offer = PairingOffer::new(pc_key(), addr, secret_code(7)).unwrap();
    let result = pair_with_pc(
        stream,
        &secret_keys(2),
        &offer,
        phone_name(),
        &CancelToken::new(),
        &quick(),
    );
    assert_eq!(result.err(), Some(PairingError::PeerNotAuthentic));
    assert!(
        fake.join().unwrap().is_empty(),
        "el celular mandó la trama 3"
    );
}

/// Un intermediario entre el celular y la PC: reenvía las tramas 1 a 3 y
/// entrega al celular lo que `transform` haga con la trama 4. Devuelve la
/// trama 4 original.
fn relay_with(
    pc_addr: SocketAddr,
    transform: impl FnOnce(Vec<u8>) -> Vec<u8> + Send + 'static,
) -> (SocketAddr, JoinHandle<Vec<u8>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = thread::spawn(move || {
        let (mut phone, _) = listener.accept().unwrap();
        let mut pc = TcpStream::connect(pc_addr).unwrap();
        let frame1 = read_exact_frame(&mut phone);
        pc.write_all(&framed(&frame1)).unwrap();
        let frame2 = read_exact_frame(&mut pc);
        phone.write_all(&framed(&frame2)).unwrap();
        let frame3 = read_exact_frame(&mut phone);
        pc.write_all(&framed(&frame3)).unwrap();
        let frame4 = read_exact_frame(&mut pc);
        phone
            .write_all(&framed(&transform(frame4.clone())))
            .unwrap();
        frame4
    });
    (addr, handle)
}

#[test]
fn altered_unencrypted_and_foreign_replies_do_not_pair() {
    // La PC acepta de verdad; lo que cambia es lo que le llega al celular.
    let accept = |pc: &Pc, relay_addr: SocketAddr| {
        let server = pc.serve(quick());
        let stream = TcpStream::connect(relay_addr).unwrap();
        let offer = PairingOffer::new(pc_key(), relay_addr, secret_code(7)).unwrap();
        let phone = thread::spawn(move || {
            pair_with_pc(
                stream,
                &secret_keys(2),
                &offer,
                phone_name(),
                &CancelToken::new(),
                &quick(),
            )
        });
        let connection = server.join().unwrap().unwrap();
        let _ = connection.confirm();
        phone.join().unwrap()
    };

    // 1. Una sesión legítima, para guardar su trama 4.
    let pc = Pc::new();
    let (relay, captured) = relay_with(pc.addr, |frame| frame);
    assert!(accept(&pc, relay).is_ok());
    let genuine = captured.join().unwrap();

    // 2. Un bit cambiado.
    let pc = Pc::new();
    let (relay, _) = relay_with(pc.addr, |mut frame| {
        frame[3] ^= 0x01;
        frame
    });
    assert_eq!(accept(&pc, relay).err(), Some(PairingError::Handshake));

    // 3. Sin cifrar: la forma de un `Accepted`, pero en claro.
    let pc = Pc::new();
    let (relay, _) = relay_with(pc.addr, |_| vec![1, 5, 0]);
    assert_eq!(accept(&pc, relay).err(), Some(PairingError::Handshake));

    // 4. Una aceptación auténtica, pero de otra sesión.
    let pc = Pc::new();
    let (relay, _) = relay_with(pc.addr, move |_| genuine);
    assert_eq!(accept(&pc, relay).err(), Some(PairingError::Handshake));
}

#[test]
fn two_concurrent_connections_only_one_consumes_the_code() {
    for _ in 0..25 {
        let pc = Pc::new();
        let barrier = Arc::new(Barrier::new(2));
        let servers: Vec<_> = (0..2).map(|_| pc.serve(quick())).collect();
        let clients: Vec<_> = (0..2)
            .map(|_| {
                let barrier = barrier.clone();
                let (addr, offer) = (pc.addr, pc.offer(7));
                thread::spawn(move || {
                    let stream = TcpStream::connect(addr).unwrap();
                    barrier.wait();
                    pair_with_pc(
                        stream,
                        &secret_keys(2),
                        &offer,
                        phone_name(),
                        &CancelToken::new(),
                        &quick(),
                    )
                })
            })
            .collect();
        let results: Vec<Served> = servers.into_iter().map(|s| s.join().unwrap()).collect();
        let accepted = results.iter().filter(|r| r.is_ok()).count();
        let refused: Vec<_> = results.iter().filter_map(|r| r.as_ref().err()).collect();
        assert_eq!(accepted, 1, "solo una conexión puede consumir el código");
        assert_eq!(refused, [&PairingError::Refused(Refusal::AlreadyUsed)]);
        drop(results);
        for client in clients {
            let _ = client.join();
        }
    }
}

#[test]
fn silent_client_times_out_and_cancel_returns_quickly() {
    let pc = Pc::new();
    let server = pc.serve(PairingTimeouts {
        step: Duration::from_millis(300),
        ..quick()
    });
    let _silent = TcpStream::connect(pc.addr).unwrap();
    assert_eq!(server.join().unwrap().err(), Some(PairingError::Timeout));

    let cancel = CancelToken::new();
    let pc = Pc::new();
    let server = pc.serve_with(
        cancel.clone(),
        PairingTimeouts {
            step: Duration::from_secs(30),
            handshake: Duration::from_secs(60),
            ..quick()
        },
    );
    let _silent = TcpStream::connect(pc.addr).unwrap();
    thread::sleep(Duration::from_millis(100));
    let started = Instant::now();
    cancel.cancel();
    assert_eq!(server.join().unwrap().err(), Some(PairingError::Cancelled));
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "{:?}",
        started.elapsed()
    );
}

#[test]
fn phone_cancel_returns_quickly_while_waiting_for_the_pc() {
    // Una "PC" que acepta la conexión y nunca contesta.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let _mute = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        thread::sleep(Duration::from_secs(5));
        drop(stream);
    });
    let stream = TcpStream::connect(addr).unwrap();
    let offer = PairingOffer::new(pc_key(), addr, secret_code(7)).unwrap();
    let cancel = CancelToken::new();
    let other = cancel.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(100));
        other.cancel();
    });
    let long = PairingTimeouts {
        step: Duration::from_secs(30),
        handshake: Duration::from_secs(60),
        reply_wait: Duration::from_secs(60),
        ..quick()
    };
    let started = Instant::now();
    let result = pair_with_pc(
        stream,
        &secret_keys(2),
        &offer,
        phone_name(),
        &cancel,
        &long,
    );
    assert_eq!(result.err(), Some(PairingError::Cancelled));
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "{:?}",
        started.elapsed()
    );
}

#[test]
fn connection_cut_mid_frame_does_not_burn_attempts() {
    let pc = Pc::new();
    for _ in 0..5 {
        let server = pc.serve(quick());
        let mut stream = TcpStream::connect(pc.addr).unwrap();
        stream.write_all(&[0, 48, 1, 2, 3]).unwrap();
        drop(stream);
        assert_eq!(server.join().unwrap().err(), Some(PairingError::Closed));
    }
    // Cinco cortes no gastaron ningún intento: el código correcto sigue sirviendo.
    let server = pc.serve(quick());
    let phone = thread::spawn({
        let (addr, offer) = (pc.addr, pc.offer(7));
        move || {
            let stream = TcpStream::connect(addr).unwrap();
            pair_with_pc(
                stream,
                &secret_keys(2),
                &offer,
                phone_name(),
                &CancelToken::new(),
                &quick(),
            )
        }
    });
    server.join().unwrap().unwrap().reject().unwrap();
    assert_eq!(phone.join().unwrap().err(), Some(PairingError::Rejected));
}

#[test]
fn slow_drip_hits_the_handshake_deadline() {
    let pc = Pc::new();
    let server = pc.serve(PairingTimeouts {
        step: Duration::from_secs(30),
        handshake: Duration::from_millis(600),
        ..quick()
    });
    let mut stream = TcpStream::connect(pc.addr).unwrap();
    let started = Instant::now();
    // Un byte cada 100 ms: la trama 1 nunca se completa a tiempo.
    let dripper = thread::spawn(move || {
        for byte in [0u8, 48, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14] {
            if stream.write_all(&[byte]).is_err() {
                return;
            }
            thread::sleep(Duration::from_millis(100));
        }
    });
    assert_eq!(server.join().unwrap().err(), Some(PairingError::Timeout));
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "{:?}",
        started.elapsed()
    );
    dripper.join().unwrap();
}

#[test]
fn bad_first_frame_lengths_count_and_get_no_bytes() {
    let pc = Pc::new();
    for header in [[0u8, 0], [0xFF, 0xFF], [0x02, 0x01]] {
        let server = pc.serve(quick());
        let mut stream = TcpStream::connect(pc.addr).unwrap();
        stream.write_all(&header).unwrap();
        let received = read_until_close(stream);
        assert!(received.is_empty(), "{header:?}");
        assert_eq!(
            server.join().unwrap().err(),
            Some(PairingError::Refused(Refusal::Malformed))
        );
    }
    // Tres encabezados malos agotaron el código.
    let server = pc.serve(quick());
    assert!(pc.first_frame_only(7).is_empty());
    assert_eq!(
        server.join().unwrap().err(),
        Some(PairingError::Refused(Refusal::TooManyAttempts))
    );
}

#[test]
fn unpaired_device_without_code_gets_no_bytes() {
    let pc = Pc::new();
    let server = pc.serve(quick());
    let mut stream = TcpStream::connect(pc.addr).unwrap();
    stream.write_all(&framed(&[0x5A; 48])).unwrap();
    assert!(read_until_close(stream).is_empty());
    assert_eq!(
        server.join().unwrap().err(),
        Some(PairingError::Refused(Refusal::WrongCode))
    );
}

#[test]
fn provisional_name_builds_a_new_paired_pc() {
    let pc = Pc::new();
    let server = pc.serve(quick());
    let phone = thread::spawn({
        let (addr, offer) = (pc.addr, pc.offer(7));
        move || {
            let stream = TcpStream::connect(addr).unwrap();
            pair_with_pc(
                stream,
                &secret_keys(2),
                &offer,
                phone_name(),
                &CancelToken::new(),
                &quick(),
            )
        }
    });
    server.join().unwrap().unwrap().confirm().unwrap();
    let paired = phone.join().unwrap().unwrap();
    let fingerprint = pc_key().fingerprint().to_string();
    assert_eq!(paired.provisional_name(), format!("PC {fingerprint}"));
    let new = paired.into_new_paired_pc(None);
    assert_eq!(new.name, format!("PC {fingerprint}"));
    assert_eq!(new.public_key, *pc_key().as_bytes());
    assert_eq!(new.last_ip, Some(pc.addr.ip()));
}

#[test]
fn confirm_fails_when_the_phone_already_disconnected_over_tcp() {
    // Con TCP real, la primera escritura tras el cierre del otro extremo sale
    // bien: `confirm` debe notar la desconexión antes de escribir.
    let pc = Pc::new();
    let server = pc.serve(quick());
    let (awaiting, frame1) = Initiator::new(&secret_keys(2), &pc.offer(7), phone_name())
        .unwrap()
        .first_message()
        .unwrap();
    let mut stream = TcpStream::connect(pc.addr).unwrap();
    stream.write_all(&framed(&frame1)).unwrap();
    let frame2 = read_exact_frame(&mut stream);
    let (_reply, frame3) = awaiting.read_second(&frame2).unwrap();
    stream.write_all(&framed(&frame3)).unwrap();
    let connection = server.join().unwrap().unwrap();
    // El celular se va mientras el diálogo sigue abierto en la PC.
    drop(stream);
    thread::sleep(Duration::from_millis(100));
    assert_eq!(connection.confirm().err(), Some(PairingError::Closed));
}

#[test]
fn confirm_fails_if_the_phone_sends_something_during_the_dialog() {
    let pc = Pc::new();
    let server = pc.serve(quick());
    let (awaiting, frame1) = Initiator::new(&secret_keys(2), &pc.offer(7), phone_name())
        .unwrap()
        .first_message()
        .unwrap();
    let mut stream = TcpStream::connect(pc.addr).unwrap();
    stream.write_all(&framed(&frame1)).unwrap();
    let frame2 = read_exact_frame(&mut stream);
    let (_reply, frame3) = awaiting.read_second(&frame2).unwrap();
    stream.write_all(&framed(&frame3)).unwrap();
    let connection = server.join().unwrap().unwrap();
    // El celular no debe mandar nada mientras espera la decisión.
    stream.write_all(&[1, 2, 3]).unwrap();
    thread::sleep(Duration::from_millis(100));
    assert!(connection.confirm().is_err());
    drop(stream);
}

// ---------------------------------------------------------------------------
// Envío con Noise IK (#96): los dos extremos en el mismo proceso, por
// `TcpStream` en 127.0.0.1. Solo la API pública.
// ---------------------------------------------------------------------------

use vault_core::protocol::{
    AwaitingResponse, MAX_SEND_FRAME_LEN, Responder, SendError, SendTimeouts, Sender, receive_send,
    send_to_pc,
};

/// La PC tiene la llave 1 (la de `pc_key()`) y el celular emparejado, la 2.
fn paired_phone(key: &PublicKey) -> bool {
    *key == secret_keys(2).public()
}

fn send_quick() -> SendTimeouts {
    SendTimeouts {
        step: Duration::from_millis(500),
        total: Duration::from_secs(4),
        ack_wait: Duration::from_secs(2),
    }
}

fn connected() -> (TcpStream, TcpStream) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let (server, _) = listener.accept().unwrap();
    (client, server)
}

fn phone_sends(
    client: TcpStream,
    text: String,
    cancel: CancelToken,
    timeouts: SendTimeouts,
) -> JoinHandle<Result<AckOutcome, SendError>> {
    thread::spawn(move || {
        send_to_pc(
            client,
            &secret_keys(2),
            &pc_key(),
            &send(&text),
            &cancel,
            &timeouts,
        )
    })
}

/// El celular a mano: manda la trama 1 y lee la 2 (la que contesta la PC).
fn phone_handshake(client: &mut TcpStream) -> (AwaitingResponse, Vec<u8>) {
    let (waiting, first) = Sender::new(&secret_keys(2), &pc_key())
        .unwrap()
        .first_message()
        .unwrap();
    client.write_all(&framed(&first)).unwrap();
    (waiting, read_exact_frame(client))
}

#[test]
fn send_succeeds_and_phone_gets_copied() {
    let (client, server) = connected();
    let phone = phone_sends(client, "hunter2 ñ".into(), CancelToken::new(), send_quick());
    let incoming = receive_send(
        server,
        &secret_keys(1),
        paired_phone,
        &CancelToken::new(),
        &send_quick(),
    )
    .unwrap();
    assert_eq!(incoming.message().password().expose_secret(), "hunter2 ñ");
    assert_eq!(incoming.peer(), secret_keys(2).public());
    incoming.copied().unwrap();
    assert_eq!(phone.join().unwrap().unwrap(), AckOutcome::Copied);
}

#[test]
fn send_with_a_1024_byte_password_succeeds() {
    let (client, server) = connected();
    let password = "ñ".repeat(512);
    assert_eq!(password.len(), 1024);
    let phone = phone_sends(client, password.clone(), CancelToken::new(), send_quick());
    let incoming = receive_send(
        server,
        &secret_keys(1),
        paired_phone,
        &CancelToken::new(),
        &send_quick(),
    )
    .unwrap();
    assert_eq!(incoming.message().password().expose_secret(), &password);
    incoming.copied().unwrap();
    assert_eq!(phone.join().unwrap().unwrap(), AckOutcome::Copied);
}

#[test]
fn ack_failed_reaches_the_phone() {
    let (client, server) = connected();
    let phone = phone_sends(client, "x".into(), CancelToken::new(), send_quick());
    let incoming = receive_send(
        server,
        &secret_keys(1),
        paired_phone,
        &CancelToken::new(),
        &send_quick(),
    )
    .unwrap();
    incoming.failed().unwrap();
    assert_eq!(phone.join().unwrap().unwrap(), AckOutcome::Failed);
}

#[test]
fn unpaired_phone_gets_closed_and_the_pc_writes_nothing() {
    // A mano: ni un solo byte de la PC.
    let (mut client, server) = connected();
    let pc = thread::spawn(move || {
        receive_send(
            server,
            &secret_keys(1),
            |_| false,
            &CancelToken::new(),
            &send_quick(),
        )
        .err()
    });
    let (_, first) = Sender::new(&secret_keys(2), &pc_key())
        .unwrap()
        .first_message()
        .unwrap();
    client.write_all(&framed(&first)).unwrap();
    assert!(read_until_close(client).is_empty());
    assert_eq!(pc.join().unwrap(), Some(SendError::NotPaired));

    // Con `send_to_pc`: el mismo `Closed` que un corte de red.
    let (client, server) = connected();
    let phone = phone_sends(client, "x".into(), CancelToken::new(), send_quick());
    let result = receive_send(
        server,
        &secret_keys(1),
        |_| false,
        &CancelToken::new(),
        &send_quick(),
    );
    assert!(matches!(result, Err(SendError::NotPaired)));
    assert_eq!(phone.join().unwrap().err(), Some(SendError::Closed));
}

#[test]
fn a_phone_unknown_to_the_pc_is_asked_with_its_authenticated_key() {
    // La PC solo ve la llave que Noise autenticó, la del celular 3.
    let (client, server) = connected();
    let thief = thread::spawn(move || {
        send_to_pc(
            client,
            &secret_keys(3),
            &pc_key(),
            &send("x"),
            &CancelToken::new(),
            &send_quick(),
        )
    });
    let asked = Mutex::new(None);
    let result = receive_send(
        server,
        &secret_keys(1),
        |key| {
            *asked.lock().unwrap() = Some(*key);
            paired_phone(key)
        },
        &CancelToken::new(),
        &send_quick(),
    );
    assert!(matches!(result, Err(SendError::NotPaired)));
    assert_eq!(*asked.lock().unwrap(), Some(secret_keys(3).public()));
    assert_eq!(thief.join().unwrap().err(), Some(SendError::Closed));
}

#[test]
fn replayed_first_frame_gets_a_reply_but_never_a_message() {
    // Sesión real: el celular legítimo envía y la PC entrega el mensaje. Un
    // atacante en la red captura sus bytes: la trama 1 y la trama 3.
    let (waiting, first) = Sender::new(&secret_keys(2), &pc_key())
        .unwrap()
        .first_message()
        .unwrap();
    let (mut client, server) = connected();
    let real_pc = thread::spawn(move || {
        let incoming = receive_send(
            server,
            &secret_keys(1),
            paired_phone,
            &CancelToken::new(),
            &send_quick(),
        )
        .unwrap();
        let text = incoming.message().password().expose_secret().to_string();
        incoming.copied().unwrap();
        text
    });
    client.write_all(&framed(&first)).unwrap();
    let second = read_exact_frame(&mut client);
    let (_, captured_third) = waiting.read_second(&second, &send("real")).unwrap();
    client.write_all(&framed(&captured_third)).unwrap();
    assert_eq!(real_pc.join().unwrap(), "real");

    // El atacante reenvía la trama 1 a la PC: contesta (con una efímera nueva)...
    let (mut attacker, server) = connected();
    let pc = thread::spawn(move || {
        receive_send(
            server,
            &secret_keys(1),
            paired_phone,
            &CancelToken::new(),
            &send_quick(),
        )
        .err()
    });
    attacker.write_all(&framed(&first)).unwrap();
    let replayed_second = read_exact_frame(&mut attacker);
    assert_eq!(replayed_second.len(), 48);
    assert_ne!(replayed_second, second);
    // ...pero la trama 3 capturada no descifra con las llaves de esta sesión,
    // y sin las llaves privadas del celular no puede armar otra.
    attacker.write_all(&framed(&captured_third)).unwrap();
    assert_eq!(pc.join().unwrap(), Some(SendError::Handshake));
}

#[test]
fn send_frame_from_another_session_delivers_nothing() {
    // Sesión A: el celular real completa su apretón de manos y cifra su mensaje.
    let (mut client_a, server_a) = connected();
    let pc_a = thread::spawn(move || {
        receive_send(
            server_a,
            &secret_keys(1),
            paired_phone,
            &CancelToken::new(),
            &send_quick(),
        )
        .err()
    });
    let (waiting, second) = phone_handshake(&mut client_a);
    let (_, third_a) = waiting
        .read_second(&second, &send("de la sesión A"))
        .unwrap();
    // Se retiene sin entregar; la sesión A termina por plazo.
    // Sesión B: otra conexión de la PC recibe la trama capturada de A.
    let (mut client_b, server_b) = connected();
    let pc_b = thread::spawn(move || {
        receive_send(
            server_b,
            &secret_keys(1),
            paired_phone,
            &CancelToken::new(),
            &send_quick(),
        )
        .err()
    });
    let (_, _second_b) = phone_handshake(&mut client_b);
    client_b.write_all(&framed(&third_a)).unwrap();
    assert_eq!(pc_b.join().unwrap(), Some(SendError::Handshake));
    drop(client_a);
    assert!(pc_a.join().unwrap().is_some());
}

#[test]
fn altered_send_frame_delivers_nothing() {
    for position in [0usize, 30, 1] {
        let (mut client, server) = connected();
        let pc = thread::spawn(move || {
            receive_send(
                server,
                &secret_keys(1),
                paired_phone,
                &CancelToken::new(),
                &send_quick(),
            )
            .err()
        });
        let (waiting, second) = phone_handshake(&mut client);
        let (_, mut third) = waiting.read_second(&second, &send("hola")).unwrap();
        let position = position.min(third.len() - 1);
        third[position] ^= 0x01;
        client.write_all(&framed(&third)).unwrap();
        assert_eq!(pc.join().unwrap(), Some(SendError::Handshake));
    }
}

#[test]
fn late_send_expires_with_short_timeouts_and_delivers_nothing() {
    let (mut client, server) = connected();
    let timeouts = SendTimeouts {
        step: Duration::from_secs(2),
        total: Duration::from_millis(600),
        ack_wait: Duration::from_secs(1),
    };
    let pc = thread::spawn(move || {
        receive_send(
            server,
            &secret_keys(1),
            paired_phone,
            &CancelToken::new(),
            &timeouts,
        )
        .err()
    });
    let (waiting, second) = phone_handshake(&mut client);
    let (_, third) = waiting.read_second(&second, &send("tarde")).unwrap();
    thread::sleep(Duration::from_millis(900));
    let _ = client.write_all(&framed(&third));
    assert_eq!(pc.join().unwrap(), Some(SendError::Expired));
}

#[test]
fn oversized_and_zero_length_frames_are_rejected() {
    let limit = u16::try_from(MAX_SEND_FRAME_LEN + 1).unwrap().to_be_bytes();
    for (header, expected) in [
        ([0u8, 0], SendError::Protocol(ProtocolError::Malformed)),
        (limit, SendError::FrameTooLarge),
        ([0xFF, 0xFF], SendError::FrameTooLarge),
    ] {
        let (mut client, server) = connected();
        let pc = thread::spawn(move || {
            receive_send(
                server,
                &secret_keys(1),
                paired_phone,
                &CancelToken::new(),
                &send_quick(),
            )
            .err()
        });
        let _ = phone_handshake(&mut client);
        // Solo el encabezado: no hay cuerpo que leer ni memoria que reservar.
        client.write_all(&header).unwrap();
        assert_eq!(pc.join().unwrap(), Some(expected));
    }
}

#[test]
fn phone_disconnecting_mid_send_closes_the_pc() {
    let (mut client, server) = connected();
    let pc = thread::spawn(move || {
        receive_send(
            server,
            &secret_keys(1),
            paired_phone,
            &CancelToken::new(),
            &send_quick(),
        )
        .err()
    });
    let (waiting, second) = phone_handshake(&mut client);
    let (_, third) = waiting.read_second(&second, &send("hola")).unwrap();
    // La mitad de la trama 3 y se corta.
    client
        .write_all(&framed(&third)[..third.len() / 2])
        .unwrap();
    drop(client);
    assert_eq!(pc.join().unwrap(), Some(SendError::Closed));
}

#[test]
fn pc_disconnecting_mid_send_closes_the_phone() {
    let (client, server) = connected();
    let phone = phone_sends(client, "hola".into(), CancelToken::new(), send_quick());
    let mut server = server;
    // La PC lee la trama 1, contesta con basura a medias y cierra.
    let _ = read_exact_frame(&mut server);
    server.write_all(&[0, 48, 1, 2, 3]).unwrap();
    drop(server);
    assert_eq!(phone.join().unwrap().err(), Some(SendError::Closed));
}

#[test]
fn second_send_in_the_same_connection_is_not_processed() {
    let (mut client, server) = connected();
    let pc = thread::spawn(move || {
        let incoming = receive_send(
            server,
            &secret_keys(1),
            paired_phone,
            &CancelToken::new(),
            &send_quick(),
        )
        .unwrap();
        let text = incoming.message().password().expose_secret().to_string();
        incoming.copied().unwrap();
        text
    });
    let (waiting, second) = phone_handshake(&mut client);
    let (_, third) = waiting.read_second(&second, &send("uno")).unwrap();
    // El mensaje y, pegado, una trama más (aquí, una copia del mismo mensaje).
    let mut both = framed(&third);
    both.extend_from_slice(&framed(&third));
    client.write_all(&both).unwrap();
    // La PC entrega exactamente uno y cierra.
    assert_eq!(pc.join().unwrap(), "uno");
    // El celular recibe, como mucho, la confirmación del primero (cerrar con
    // datos sin leer puede llegar como reinicio y perderla: ambas son válidas).
    let received = read_until_close(client);
    assert!(received.is_empty() || received.len() == 2 + 19);
}

#[test]
fn pc_with_another_private_key_never_gets_the_password() {
    // El celular conoce la llave 1; quien contesta tiene la 4.
    let (client, server) = connected();
    let phone = phone_sends(
        client,
        "no debe llegar".into(),
        CancelToken::new(),
        send_quick(),
    );
    let result = receive_send(
        server,
        &secret_keys(4),
        |_| true,
        &CancelToken::new(),
        &send_quick(),
    );
    assert!(matches!(result, Err(SendError::Handshake)));
    assert_eq!(phone.join().unwrap().err(), Some(SendError::Closed));
}

#[test]
fn forged_second_frame_is_peer_not_authentic_and_phone_sends_nothing_more() {
    let (client, mut server) = connected();
    let phone = phone_sends(
        client,
        "no debe salir".into(),
        CancelToken::new(),
        send_quick(),
    );
    let _first = read_exact_frame(&mut server);
    // Una "PC" que no tiene la llave privada contesta con 48 bytes inventados.
    server.write_all(&framed(&[0x42; 48])).unwrap();
    assert_eq!(
        phone.join().unwrap().err(),
        Some(SendError::PeerNotAuthentic)
    );
    // Nada más sale del celular: solo el cierre.
    assert!(read_until_close(server).is_empty());
}

#[test]
fn cancel_is_effective_on_both_sides() {
    // PC: un cliente que conecta y no manda nada.
    let (_client, server) = connected();
    let cancel = CancelToken::new();
    let token = cancel.clone();
    let pc = thread::spawn(move || {
        let started = Instant::now();
        let result = receive_send(
            server,
            &secret_keys(1),
            paired_phone,
            &cancel,
            &SendTimeouts::default(),
        );
        (result.err(), started.elapsed())
    });
    thread::sleep(Duration::from_millis(100));
    token.cancel();
    let (error, elapsed) = pc.join().unwrap();
    assert_eq!(error, Some(SendError::Cancelled));
    assert!(elapsed < Duration::from_secs(1));

    // Celular: una PC que acepta y no contesta.
    let (client, _server) = connected();
    let cancel = CancelToken::new();
    let token = cancel.clone();
    let phone = phone_sends(client, "x".into(), cancel, SendTimeouts::default());
    thread::sleep(Duration::from_millis(100));
    let started = Instant::now();
    token.cancel();
    assert_eq!(phone.join().unwrap().err(), Some(SendError::Cancelled));
    assert!(started.elapsed() < Duration::from_secs(1));
}

#[test]
fn password_never_appears_in_debug_or_errors() {
    let secret = "CENTINELA-contraseña-0451";
    let (client, server) = connected();
    let phone = phone_sends(client, secret.into(), CancelToken::new(), send_quick());
    let incoming = receive_send(
        server,
        &secret_keys(1),
        paired_phone,
        &CancelToken::new(),
        &send_quick(),
    )
    .unwrap();
    let shown = format!("{:?} {:?}", incoming.message(), incoming.peer());
    assert!(!shown.contains(secret) && !shown.contains("0451"));
    incoming.copied().unwrap();
    phone.join().unwrap().unwrap();

    for error in [
        SendError::NotPaired,
        SendError::Expired,
        SendError::Handshake,
        SendError::PeerNotAuthentic,
        SendError::FrameTooLarge,
        SendError::Closed,
        SendError::Timeout,
        SendError::Cancelled,
    ] {
        assert!(!format!("{error} {error:?}").contains(secret));
    }
}
