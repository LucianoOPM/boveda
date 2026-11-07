//! Cifrado de cada parte de una entrada (datos de búsqueda y secreto) con
//! XChaCha20-Poly1305 y la llave de bóveda.
//!
//! Cada parte lleva un dato autenticado que la liga a su entrada, a su
//! versión de estructura y a su tipo. Son 28 bytes de ancho fijo, sin
//! separadores:
//!
//! | Offset | Campo                 | Tamaño   | Valor                                 |
//! | ------ | --------------------- | -------- | ------------------------------------- |
//! | 0–8    | Etiqueta              | 9 bytes  | ASCII `boveda-v1`                     |
//! | 9–24   | Identificador         | 16 bytes | id de la entrada                      |
//! | 25–26  | Versión de estructura | 2 bytes  | `u16` little-endian                   |
//! | 27     | Parte                 | 1 byte   | `1` = datos de búsqueda, `2` = secreto |
//!
//! Así se rechaza mover un secreto a otra entrada, intercambiar las dos
//! partes de una misma entrada o leerlas con otra versión de estructura.
//!
//! **Fuera del modelo de amenazas de la v1:** volver una entrada a una
//! versión anterior (rollback). Quien pueda escribir el archivo puede
//! reemplazar ambas partes de una entrada por una copia vieja y válida de la
//! misma entrada, y no se detecta.
//!
//! Este módulo solo conoce bytes: no depende del modelo de entrada de
//! `vault`.
//!
//! Una parte cifrada se guarda como `nonce (24) | texto cifrado | etiqueta (16)`.
//! El texto en claro solo vive en búferes `Zeroizing` con la capacidad
//! reservada de antemano, para que nunca se reasignen dejando copias.

use std::fmt;

use chacha20poly1305::aead::Generate;
use chacha20poly1305::{AeadInOut, KeyInit, XChaCha20Poly1305, XNonce};
use secrecy::ExposeSecret;
use zeroize::Zeroizing;

use super::keys::{NONCE_LEN, TAG_LEN, VaultKey};
use crate::error::{Error, Result};

/// Longitud del identificador de entrada. Coincide con
/// `vault::entry::ENTRY_ID_LEN`; se repite aquí para que `crypto` no dependa
/// de `vault` (una prueba comprueba que son iguales).
pub(crate) const ENTRY_ID_LEN: usize = 16;

/// Longitud del dato autenticado de una parte.
pub(crate) const AAD_LEN: usize = 28;

/// Etiqueta fija al inicio del dato autenticado.
const AAD_LABEL: &[u8; 9] = b"boveda-v1";

/// Tipo de parte de una entrada. Su valor forma parte del dato autenticado:
/// no se cambia.
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Part {
    /// Datos de búsqueda: servicio, URL, usuario y etiquetas.
    Search = 1,
    /// Secreto: contraseña y notas.
    Secret = 2,
}

/// Arma el dato autenticado de una parte, en la pila.
fn associated_data(
    id: &[u8; ENTRY_ID_LEN],
    entry_schema_version: u16,
    part: Part,
) -> [u8; AAD_LEN] {
    let mut ad = [0u8; AAD_LEN];
    let (label, rest) = ad.split_at_mut(AAD_LABEL.len());
    let (id_out, rest) = rest.split_at_mut(ENTRY_ID_LEN);
    let (version_out, part_out) = rest.split_at_mut(2);
    label.copy_from_slice(AAD_LABEL);
    id_out.copy_from_slice(id);
    version_out.copy_from_slice(&entry_schema_version.to_le_bytes());
    part_out[0] = part as u8;
    ad
}

/// Longitud mínima de una parte cifrada: nonce más etiqueta (texto vacío).
pub(crate) const MIN_SEALED_LEN: usize = NONCE_LEN + TAG_LEN;

/// Parte cifrada: `nonce (24) | texto cifrado | etiqueta (16)`, tal como se
/// guarda en el almacenamiento.
///
/// No deriva `Debug`: aunque está cifrada, solo muestra su longitud.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct EncryptedPart(Vec<u8>);

impl EncryptedPart {
    /// Reconstruye una parte leída del almacenamiento. No valida: la
    /// longitud la comprueba [`open_part`].
    pub(crate) fn from_bytes(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Debug for EncryptedPart {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EncryptedPart")
            .field("len", &self.0.len())
            .finish()
    }
}

/// Crea el cifrador a partir de la llave de bóveda (se borra al soltarlo).
fn cipher(key: &VaultKey) -> Result<XChaCha20Poly1305> {
    // La longitud es fija (32 bytes), así que este error no ocurre en la práctica.
    XChaCha20Poly1305::new_from_slice(key.expose_secret()).map_err(|_| Error::CorruptEntry)
}

/// Cifra una parte de una entrada con un nonce aleatorio nuevo.
///
/// Un fallo del generador aleatorio devuelve [`Error::Random`].
pub(crate) fn seal_part(
    key: &VaultKey,
    id: &[u8; ENTRY_ID_LEN],
    entry_schema_version: u16,
    part: Part,
    plaintext: &[u8],
) -> Result<EncryptedPart> {
    let nonce = XNonce::try_generate().map_err(|_| Error::Random)?;
    let ad = associated_data(id, entry_schema_version, part);

    // Capacidad exacta reservada de antemano: ni la copia del texto en claro
    // ni la etiqueta hacen que el búfer se reasigne.
    let capacity = NONCE_LEN
        .checked_add(plaintext.len())
        .and_then(|n| n.checked_add(TAG_LEN))
        .ok_or(Error::CorruptEntry)?;
    let mut buf = Zeroizing::new(Vec::with_capacity(capacity));
    buf.extend_from_slice(&nonce);
    buf.extend_from_slice(plaintext);

    let body = buf.get_mut(NONCE_LEN..).ok_or(Error::CorruptEntry)?;
    let tag = cipher(key)?
        .encrypt_inout_detached(&nonce, &ad, body.into())
        .map_err(|_| Error::CorruptEntry)?;
    buf.extend_from_slice(&tag);

    // Solo tras cifrar con éxito el contenido sale del búfer `Zeroizing`: si
    // algo falla antes, el texto en claro se borra al soltarlo.
    Ok(EncryptedPart(std::mem::take(&mut *buf)))
}

/// Descifra una parte de una entrada.
///
/// Una parte demasiado corta o que no se autentica (llave, id, versión o
/// parte distintos, o un byte alterado) devuelve [`Error::CorruptEntry`].
pub(crate) fn open_part(
    key: &VaultKey,
    id: &[u8; ENTRY_ID_LEN],
    entry_schema_version: u16,
    part: Part,
    sealed: &EncryptedPart,
) -> Result<Zeroizing<Vec<u8>>> {
    if sealed.0.len() < MIN_SEALED_LEN {
        return Err(Error::CorruptEntry);
    }
    let (nonce, body) = sealed.0.split_at(NONCE_LEN);
    let nonce = XNonce::try_from(nonce).map_err(|_| Error::CorruptEntry)?;
    let ad = associated_data(id, entry_schema_version, part);

    // Se descifra en un búfer `Zeroizing`; `decrypt_in_place` lo trunca a la
    // longitud del texto en claro sin reasignar.
    let mut buf = Zeroizing::new(body.to_vec());
    cipher(key)?
        .decrypt_in_place(&nonce, &ad, &mut *buf)
        .map_err(|_| Error::CorruptEntry)?;
    Ok(buf)
}

#[cfg(test)]
mod tests;
