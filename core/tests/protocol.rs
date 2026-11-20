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
