use std::net::{Ipv6Addr, SocketAddr};

use qrcode::{EcLevel, QrCode, Version};
use secrecy::SecretBox;
use vault_core::protocol::{PairingCode, PairingOffer, PublicKey};

const ALPHABET_B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
const ALPHABET_B32: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

/// Codifica en base64url sin relleno (RFC 4648 §5), solo para medir.
fn base64url(bytes: &[u8]) -> String {
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n =
            chunk.iter().fold(0u32, |acc, b| (acc << 8) | u32::from(*b)) << (8 * (3 - chunk.len()));
        for i in 0..=chunk.len() {
            out.push(char::from(
                ALPHABET_B64[((n >> (18 - 6 * i)) & 63) as usize],
            ));
        }
    }
    out
}

/// Codifica en base32 en mayúsculas sin relleno (RFC 4648 §6), solo para medir.
fn base32(bytes: &[u8]) -> String {
    let mut out = String::new();
    let (mut buffer, mut bits) = (0u32, 0u32);
    for byte in bytes {
        buffer = (buffer << 8) | u32::from(*byte);
        bits += 8;
        while bits >= 5 {
            out.push(char::from(
                ALPHABET_B32[((buffer >> (bits - 5)) & 31) as usize],
            ));
            bits -= 5;
        }
    }
    if bits > 0 {
        out.push(char::from(
            ALPHABET_B32[((buffer << (5 - bits)) & 31) as usize],
        ));
    }
    out
}

fn offer(address: SocketAddr) -> PairingOffer {
    let code = PairingCode::from_bytes(SecretBox::new(Box::new([9u8; 32])));
    PairingOffer::new(PublicKey::from_bytes([7u8; 32]), address, code).expect("oferta")
}

fn describe(label: &str, data: &[u8], level: EcLevel) {
    let code = QrCode::with_error_correction_level(data, level).expect("QR");
    let modules = code.width();
    let version = match code.version() {
        Version::Normal(n) => n,
        Version::Micro(n) => n,
    };
    let svg_len = code
        .render::<qrcode::render::svg::Color>()
        .module_dimensions(1, 1)
        .build()
        .len();
    println!(
        "{label:<28} {level:?} versión {version:>2}  módulos {modules:>2}  px/módulo {:.2}  svg {svg_len} B  datos {} B",
        240.0 / (modules as f64 + 8.0),
        data.len()
    );
}

/// Medición pura (sin red ni portapapeles) para elegir la codificación del QR.
#[test]
#[ignore = "medición: se ejecuta a mano con --ignored --nocapture"]
fn measure_offer_encodings() {
    let v4: SocketAddr = "192.168.100.200:47615".parse().expect("v4");
    let v6 = SocketAddr::from((Ipv6Addr::new(0xfd00, 1, 2, 3, 4, 5, 6, 7), 47615));
    for (family, address) in [("IPv4", v4), ("IPv6", v6)] {
        let bytes = offer(address).encode().expect("codificar");
        let variants: [(&str, Vec<u8>); 3] = [
            ("bytes crudos", bytes.to_vec()),
            ("base64url", base64url(&bytes).into_bytes()),
            ("base32 mayúsculas", base32(&bytes).into_bytes()),
        ];
        for (name, data) in &variants {
            for level in [EcLevel::L, EcLevel::M, EcLevel::Q] {
                describe(&format!("{family} {name}"), data, level);
            }
        }
    }
}

// ---- Contrato del QR (#101) -------------------------------------------------

use super::{EC_LEVEL, PLACEHOLDER_SVG, data_uri, offer_text, render_svg};

/// Decodificador de base64url sin relleno, escrito aparte del de producción.
fn unbase64url(text: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let (mut buffer, mut bits) = (0u32, 0u32);
    for ch in text.bytes() {
        let value = ALPHABET_B64
            .iter()
            .position(|c| *c == ch)
            .expect("carácter de base64url") as u32;
        buffer = (buffer << 6) | value;
        bits += 6;
        if bits >= 8 {
            out.push(((buffer >> (bits - 8)) & 0xff) as u8);
            bits -= 8;
        }
    }
    out
}

fn fixed_offer() -> PairingOffer {
    offer("10.0.0.5:47615".parse().expect("dirección"))
}

#[test]
fn offer_text_is_the_documented_encoding() {
    // 01 01 | llave 07×32 | 00 0A 00 00 05 | FF F3 02 | código 09×32, en base64url.
    let expected = "AQEHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwAKAAAF__MCCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQk";
    assert_eq!(&*offer_text(&fixed_offer()).expect("texto"), expected);
}

#[test]
fn offer_text_round_trips_through_decode() {
    let original = fixed_offer();
    let text = offer_text(&original).expect("texto");
    let decoded = PairingOffer::decode(&unbase64url(&text)).expect("decodificar");
    assert_eq!(decoded.address(), original.address());
    assert_eq!(decoded.public_key(), original.public_key());
    assert_eq!(
        decoded.code().expose_bytes(),
        original.code().expose_bytes()
    );
}

#[test]
fn svg_uses_the_design_colors_and_size() {
    let svg = render_svg(&offer_text(&fixed_offer()).expect("texto")).expect("SVG");
    assert!(svg.contains("#0F1720"), "módulos del diseño");
    assert!(svg.contains("#FFFFFF"), "fondo blanco puro");
    // Ningún otro color literal.
    let colors = svg.matches('#').count();
    assert_eq!(colors, 2, "solo los dos colores del diseño");
    // 41 módulos + zona de silencio de 4 por lado; 1 unidad por módulo.
    assert!(svg.contains("viewBox=\"0 0 49 49\""));
}

#[test]
fn qr_meets_the_scan_criteria() {
    assert!(matches!(
        EC_LEVEL,
        qrcode::EcLevel::M | qrcode::EcLevel::Q | qrcode::EcLevel::H
    ));
    for address in [
        "192.168.100.200:47615".parse::<SocketAddr>().expect("v4"),
        SocketAddr::from((Ipv6Addr::new(0xfd00, 1, 2, 3, 4, 5, 6, 7), 47615)),
    ] {
        let text = offer_text(&offer(address)).expect("texto");
        let code = QrCode::with_error_correction_level(text.as_bytes(), EC_LEVEL).expect("QR");
        let px_per_module = 240.0 / (code.width() as f64 + 8.0);
        assert!(
            px_per_module >= 4.0,
            "{address}: {px_per_module} px por módulo"
        );
    }
}

#[test]
fn data_uri_has_no_raw_markup_characters() {
    let svg = render_svg(&offer_text(&fixed_offer()).expect("texto")).expect("SVG");
    let uri = data_uri(&svg);
    let body = uri
        .strip_prefix("data:image/svg+xml;utf8,")
        .expect("prefijo");
    for forbidden in ['<', '>', '#', '"'] {
        assert!(
            !body.contains(forbidden),
            "carácter sin escapar: {forbidden}"
        );
    }
    // El escape se puede deshacer y devuelve el SVG original.
    let undone = body
        .replace("%3C", "<")
        .replace("%3E", ">")
        .replace("%23", "#")
        .replace("%22", "\"")
        .replace("%25", "%");
    assert_eq!(undone, *svg);
}

#[test]
fn placeholder_is_a_fixed_svg_without_the_offer() {
    assert!(PLACEHOLDER_SVG.starts_with("<svg"));
    assert!(PLACEHOLDER_SVG.contains("viewBox"));
    assert!(!PLACEHOLDER_SVG.contains("<script"));
    // Un QR real tiene tres cuadros de posición; el marcador no es un QR.
    let real = render_svg(&offer_text(&fixed_offer()).expect("texto")).expect("SVG");
    assert_ne!(PLACEHOLDER_SVG, &*real);
}
