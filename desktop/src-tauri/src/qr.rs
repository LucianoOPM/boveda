//! QR de la oferta de emparejamiento (#101).
//!
//! Rust dibuja el QR en SVG y el webview solo muestra la imagen: la oferta, el
//! código de un solo uso y las llaves nunca cruzan al webview como datos.
//!
//! **Contrato con el celular (#103):** el texto del QR es la oferta de
//! `vault_core::protocol::PairingOffer::encode` en base64url sin relleno
//! (RFC 4648 §5), con corrección de errores nivel M. La medición y los motivos
//! están en `docs/especificacion.md`, «Formato del QR».
//!
//! La imagen contiene el código de un solo uso: es información sensible y no
//! debe llegar a logs, archivos ni mensajes de error.

use std::fmt;

use qrcode::{EcLevel, QrCode, render::svg};
use vault_core::protocol::PairingOffer;
use zeroize::Zeroizing;

#[cfg(test)]
mod tests;

/// Nivel de corrección de errores del QR (decidido con la medición de #101).
pub const EC_LEVEL: EcLevel = EcLevel::M;

/// Color de los módulos del QR (`DESIGN.md` §8).
const DARK: &str = "#0F1720";
/// Fondo del QR: blanco puro, nunca invertido (`DESIGN.md` §8).
const LIGHT: &str = "#FFFFFF";

/// Marcador fijo que se muestra atenuado cuando la sesión terminó. No contiene
/// ningún dato de la oferta y no es un QR (no tiene cuadros de posición).
pub const PLACEHOLDER_SVG: &str = concat!(
    "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 49 49\" ",
    "shape-rendering=\"crispEdges\"><rect width=\"49\" height=\"49\" fill=\"#FFFFFF\"/>",
    "<path stroke=\"#0F1720\" stroke-width=\"2\" stroke-dasharray=\"2 2\" fill=\"none\" ",
    "d=\"M4 5H45M4 10H45M4 15H45M4 20H45M4 25H45M4 30H45M4 35H45M4 40H45M4 45H45\"/></svg>"
);

/// No se pudo codificar el QR. Sin datos: la oferta nunca va en el error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QrError {
    /// La oferta no se pudo codificar o no cabe en un QR.
    Encode,
}

impl fmt::Display for QrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("no se pudo generar el código QR")
    }
}

impl std::error::Error for QrError {}

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// Texto del QR: la oferta codificada en base64url sin relleno. Contiene el
/// código de un solo uso, así que se borra al soltarse.
pub fn offer_text(offer: &PairingOffer) -> Result<Zeroizing<String>, QrError> {
    let bytes = offer.encode().map_err(|_| QrError::Encode)?;
    // Capacidad exacta (sin relleno): evita reasignaciones que dejen copias.
    let mut text = Zeroizing::new(String::with_capacity((bytes.len() * 4).div_ceil(3)));
    for chunk in bytes.chunks(3) {
        let n =
            chunk.iter().fold(0u32, |acc, b| (acc << 8) | u32::from(*b)) << (8 * (3 - chunk.len()));
        for i in 0..=chunk.len() {
            text.push(char::from(ALPHABET[((n >> (18 - 6 * i)) & 63) as usize]));
        }
    }
    Ok(text)
}

/// Dibuja el QR en SVG: módulos `#0F1720` sobre `#FFFFFF`, zona de silencio de
/// 4 módulos y una unidad por módulo (`viewBox` en módulos). La ventana lo
/// muestra a 240 × 240 con CSS; `min_dimensions` redondearía hacia arriba.
pub fn render_svg(text: &str) -> Result<Zeroizing<String>, QrError> {
    let code = QrCode::with_error_correction_level(text.as_bytes(), EC_LEVEL)
        .map_err(|_| QrError::Encode)?;
    let svg = code
        .render::<svg::Color>()
        .module_dimensions(1, 1)
        .quiet_zone(true)
        .dark_color(svg::Color(DARK))
        .light_color(svg::Color(LIGHT))
        .build();
    Ok(Zeroizing::new(svg))
}

/// `data:` URI del SVG, con el escape mínimo para usarlo en un `<img src>`.
pub fn data_uri(svg: &str) -> String {
    const PREFIX: &str = "data:image/svg+xml;utf8,";
    let mut uri = String::with_capacity(PREFIX.len() + svg.len() + svg.len() / 4);
    uri.push_str(PREFIX);
    for ch in svg.chars() {
        match ch {
            '%' => uri.push_str("%25"),
            '<' => uri.push_str("%3C"),
            '>' => uri.push_str("%3E"),
            '#' => uri.push_str("%23"),
            '"' => uri.push_str("%22"),
            '\n' | '\r' => {}
            other => uri.push(other),
        }
    }
    uri
}
