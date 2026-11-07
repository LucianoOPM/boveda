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

#[cfg(test)]
mod tests;
