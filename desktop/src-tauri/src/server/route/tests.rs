use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::{Duration, Instant};

use vault_core::protocol::CancelToken;

use super::{Route, classify};

const STEP: Duration = Duration::from_millis(300);

/// Un par de conexiones: (la del celular, la que acepta el servidor).
fn pair() -> (TcpStream, TcpStream) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let client = TcpStream::connect(listener.local_addr().expect("dirección")).expect("conectar");
    let (server, _) = listener.accept().expect("aceptar");
    (client, server)
}

fn header(len: u16) -> [u8; 2] {
    len.to_be_bytes()
}

#[test]
fn length_48_routes_to_pairing() {
    let (mut client, server) = pair();
    client.write_all(&header(48)).expect("escribir");
    assert_eq!(classify(&server, &CancelToken::new(), STEP), Route::Pairing);
}

#[test]
fn length_96_routes_to_send() {
    let (mut client, server) = pair();
    client.write_all(&header(96)).expect("escribir");
    assert_eq!(classify(&server, &CancelToken::new(), STEP), Route::Send);
}

#[test]
fn other_length_is_rejected() {
    for len in [0u16, 1, 47, 49, 95, 97, 512, 0xFFFF] {
        let (mut client, server) = pair();
        client.write_all(&header(len)).expect("escribir");
        assert_eq!(
            classify(&server, &CancelToken::new(), STEP),
            Route::Reject,
            "largo {len}"
        );
    }
}

#[test]
fn peek_does_not_consume_the_header() {
    let (mut client, mut server) = pair();
    client.write_all(&header(48)).expect("escribir");
    assert_eq!(classify(&server, &CancelToken::new(), STEP), Route::Pairing);
    let mut read = [0u8; 2];
    server.read_exact(&mut read).expect("leer el encabezado");
    assert_eq!(read, header(48));
}

#[test]
fn header_split_in_two_writes_is_still_routed() {
    let (mut client, server) = pair();
    let [high, low] = header(96);
    client.write_all(&[high]).expect("primer byte");
    let writer = thread::spawn(move || {
        thread::sleep(Duration::from_millis(60));
        client.write_all(&[low]).expect("segundo byte");
        client
    });
    assert_eq!(
        classify(&server, &CancelToken::new(), Duration::from_secs(2)),
        Route::Send
    );
    drop(writer.join().expect("hilo"));
}

#[test]
fn one_byte_then_silence_times_out_as_reject() {
    let (mut client, server) = pair();
    client.write_all(&[0]).expect("un byte");
    let started = Instant::now();
    assert_eq!(classify(&server, &CancelToken::new(), STEP), Route::Reject);
    assert!(started.elapsed() >= STEP, "debe esperar el plazo completo");
    assert!(started.elapsed() < STEP * 5, "y no mucho más");
}

#[test]
fn one_byte_then_close_is_reject() {
    let (mut client, server) = pair();
    client.write_all(&[0]).expect("un byte");
    drop(client);
    assert_eq!(classify(&server, &CancelToken::new(), STEP), Route::Reject);
}

#[test]
fn immediate_close_is_reject() {
    let (client, server) = pair();
    drop(client);
    assert_eq!(classify(&server, &CancelToken::new(), STEP), Route::Reject);
}

#[test]
fn silence_times_out_as_reject() {
    let (_client, server) = pair();
    assert_eq!(classify(&server, &CancelToken::new(), STEP), Route::Reject);
}

#[test]
fn cancel_during_wait_is_reject_quickly() {
    let (_client, server) = pair();
    let cancel = CancelToken::new();
    let remote = cancel.clone();
    let canceller = thread::spawn(move || {
        thread::sleep(Duration::from_millis(50));
        remote.cancel();
    });
    let started = Instant::now();
    assert_eq!(
        classify(&server, &cancel, Duration::from_secs(5)),
        Route::Reject
    );
    assert!(started.elapsed() < Duration::from_secs(2));
    canceller.join().expect("hilo");
}
