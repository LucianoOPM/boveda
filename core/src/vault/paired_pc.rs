//! PCs emparejadas: la lista que la bóveda guarda cifrada con la llave de
//! bóveda (formato v3, #93).
//!
//! Cada fila de `paired_pcs` tiene en claro solo su identificador (16 bytes
//! aleatorios) y su versión de estructura; todo lo demás va cifrado, porque el
//! nombre y la IP revelan con qué equipos se usa la bóveda: la llave pública
//! de la PC, el nombre, la última IP y las fechas. Se serializa con `serde` y
//! `postcard` y se cifra con XChaCha20-Poly1305 y su propio dato autenticado,
//! de 27 bytes y sin separadores:
//!
//! | Offset | Campo                 | Tamaño   | Valor                |
//! | ------ | --------------------- | -------- | -------------------- |
//! | 0–8    | Etiqueta              | 9 bytes  | ASCII `boveda-pc`    |
//! | 9–24   | Identificador         | 16 bytes | id de la PC          |
//! | 25–26  | Versión de estructura | 2 bytes  | `u16` little-endian  |
//!
//! Así no se puede mover una PC a otro id, ni intercambiarla con la identidad
//! o con una entrada, ni leerla con otra versión de estructura.
//!
//! Estructura de la versión 1: `public_key` (32 bytes), `name` (de 1 a 128
//! bytes UTF-8, recortado), `last_ip` (ausente, o 4 o 16 bytes), `paired_at`
//! (ms Unix) y `last_seen_at` (ausente, o ms Unix).

use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, Zeroizing};

use super::entry::flavor::ZeroizingVec;
use crate::crypto::aead::{self, EncryptedPart};
use crate::crypto::keys::VaultKey;
use crate::error::{Error, InvalidPairedPcField, Result};

/// Longitud del identificador de una PC emparejada.
pub const PAIRED_PC_ID_LEN: usize = 16;

/// Longitud de la llave pública X25519 de la PC.
pub const PUBLIC_KEY_LEN: usize = 32;

/// Longitud máxima del nombre, en bytes UTF-8 y ya recortado.
pub const PAIRED_PC_NAME_MAX_BYTES: usize = 128;

/// Versión de estructura con la que se escriben las PCs nuevas. Se guarda en
/// claro junto a la fila y forma parte del dato autenticado.
pub(crate) const PC_SCHEMA_VERSION: u16 = 1;

/// Etiqueta fija al inicio del dato autenticado.
const AAD_LABEL: &[u8; 9] = b"boveda-pc";

/// Longitud del dato autenticado.
const AAD_LEN: usize = 27;

/// Capacidad inicial del búfer de serialización; crece si hace falta.
const ENCODE_INITIAL_CAPACITY: usize = 128;

/// Identificador de una PC emparejada: 16 bytes aleatorios, en claro en el
/// archivo. No es secreto.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct PairedPcId([u8; PAIRED_PC_ID_LEN]);

impl PairedPcId {
    /// Genera un identificador nuevo con el generador del sistema.
    pub(crate) fn generate() -> Result<Self> {
        let mut bytes = [0u8; PAIRED_PC_ID_LEN];
        getrandom::fill(&mut bytes).map_err(|_| Error::Random)?;
        Ok(Self(bytes))
    }

    /// Reconstruye un identificador leído del almacenamiento.
    pub fn from_bytes(bytes: [u8; PAIRED_PC_ID_LEN]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; PAIRED_PC_ID_LEN] {
        &self.0
    }
}

/// Datos para emparejar una PC nueva.
#[derive(Debug, Clone)]
pub struct NewPairedPc {
    /// Llave pública X25519 de la PC. Debe ser única en la lista.
    pub public_key: [u8; PUBLIC_KEY_LEN],
    /// Nombre de la PC: se recorta y debe tener de 1 a 128 bytes.
    pub name: String,
    /// Última IP conocida, si ya se sabe.
    pub last_ip: Option<IpAddr>,
}

/// Cambios a una PC emparejada. `None` conserva el valor actual.
#[derive(Debug, Clone, Default)]
pub struct PairedPcUpdate {
    pub name: Option<String>,
    /// Una IP nueva también fija `last_seen_at` con la hora actual.
    pub last_ip: Option<IpAddr>,
}

/// PC emparejada ya descifrada.
///
/// Su `Debug` solo muestra el identificador: el nombre y la IP revelan con
/// qué equipos se usa la bóveda. El nombre se borra de memoria al soltarla.
pub struct PairedPc {
    id: PairedPcId,
    public_key: [u8; PUBLIC_KEY_LEN],
    name: String,
    last_ip: Option<IpAddr>,
    paired_at: i64,
    last_seen_at: Option<i64>,
}

impl PairedPc {
    pub(crate) fn new(
        id: PairedPcId,
        public_key: [u8; PUBLIC_KEY_LEN],
        name: String,
        last_ip: Option<IpAddr>,
        paired_at: i64,
        last_seen_at: Option<i64>,
    ) -> Self {
        Self {
            id,
            public_key,
            name,
            last_ip,
            paired_at,
            last_seen_at,
        }
    }

    pub fn id(&self) -> PairedPcId {
        self.id
    }

    pub fn public_key(&self) -> &[u8; PUBLIC_KEY_LEN] {
        &self.public_key
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn last_ip(&self) -> Option<IpAddr> {
        self.last_ip
    }

    /// Fecha del emparejamiento, en milisegundos Unix.
    pub fn paired_at(&self) -> i64 {
        self.paired_at
    }

    /// Fecha en que se guardó la última IP, en milisegundos Unix. `None` si
    /// nunca se guardó una.
    pub fn last_seen_at(&self) -> Option<i64> {
        self.last_seen_at
    }

    /// Aplica un cambio ya validado: `now` solo va con una IP nueva.
    pub(crate) fn apply(
        &mut self,
        name: Option<String>,
        last_ip: Option<IpAddr>,
        now: Option<i64>,
    ) {
        if let Some(name) = name {
            self.name.zeroize();
            self.name = name;
        }
        if let Some(ip) = last_ip {
            self.last_ip = Some(ip);
            self.last_seen_at = now;
        }
    }
}

impl Drop for PairedPc {
    fn drop(&mut self) {
        self.name.zeroize();
    }
}

impl fmt::Debug for PairedPc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PairedPc")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

/// Lista de PCs emparejadas: las legibles y las dañadas, por separado. Una
/// PC dañada no impide leer las demás.
#[derive(Debug)]
pub struct PairedPcs {
    /// PCs válidas.
    pub valid: Vec<PairedPc>,
    /// Ids de las filas con un id válido pero ilegibles (no se autentican,
    /// versión de estructura desconocida, forma inesperada). Se pueden quitar
    /// con [`Vault::remove_paired_pc`](super::Vault::remove_paired_pc).
    pub corrupt: Vec<PairedPcId>,
    /// Filas dañadas sin un id de 16 bytes: no hay un id que reportar.
    pub unidentified_corrupt: usize,
}

/// Valida y recorta un nombre: de 1 a 128 bytes UTF-8 tras recortar.
pub(crate) fn validate_name(name: &str) -> Result<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(Error::InvalidPairedPc(InvalidPairedPcField::NameEmpty));
    }
    if trimmed.len() > PAIRED_PC_NAME_MAX_BYTES {
        return Err(Error::InvalidPairedPc(InvalidPairedPcField::NameTooLong));
    }
    Ok(trimmed.to_owned())
}

/// Dato autenticado de la PC `id` para esa versión de estructura.
fn associated_data(id: &[u8; PAIRED_PC_ID_LEN], version: u16) -> [u8; AAD_LEN] {
    let mut ad = [0u8; AAD_LEN];
    let (label, rest) = ad.split_at_mut(AAD_LABEL.len());
    let (id_out, version_out) = rest.split_at_mut(PAIRED_PC_ID_LEN);
    label.copy_from_slice(AAD_LABEL);
    id_out.copy_from_slice(id);
    version_out.copy_from_slice(&version.to_le_bytes());
    ad
}

/// Estructura de la versión 1, tal como se serializa.
#[derive(Serialize, Deserialize)]
struct RawPcV1 {
    public_key: [u8; PUBLIC_KEY_LEN],
    name: String,
    /// 4 bytes (IPv4) o 16 (IPv6); otra longitud es una PC dañada.
    last_ip: Option<Vec<u8>>,
    paired_at: i64,
    last_seen_at: Option<i64>,
}

/// Cifra la PC con la llave de bóveda y la versión de estructura actual.
pub(crate) fn seal(key: &VaultKey, pc: &PairedPc) -> Result<EncryptedPart> {
    let raw = RawPcV1 {
        public_key: pc.public_key,
        name: pc.name.clone(),
        last_ip: pc.last_ip.map(|ip| match ip {
            IpAddr::V4(v4) => v4.octets().to_vec(),
            IpAddr::V6(v6) => v6.octets().to_vec(),
        }),
        paired_at: pc.paired_at,
        last_seen_at: pc.last_seen_at,
    };
    let plain: Zeroizing<Vec<u8>> =
        postcard::serialize_with_flavor(&raw, ZeroizingVec::with_capacity(ENCODE_INITIAL_CAPACITY))
            .map_err(|_| Error::CorruptPairedPc)?;
    let mut raw = raw;
    raw.name.zeroize();
    aead::seal_with_ad(key, &associated_data(&pc.id.0, PC_SCHEMA_VERSION), &plain)
}

/// Descifra la PC `id` guardada con `version`.
///
/// Una versión de estructura desconocida, una pieza que no se autentica
/// (otra llave, otro id, otra versión o un byte alterado), bytes mal
/// formados o sobrantes, un nombre que no cumple las reglas o una IP que no
/// mide 4 ni 16 bytes dan [`Error::CorruptPairedPc`].
pub(crate) fn open(
    key: &VaultKey,
    id: PairedPcId,
    version: u16,
    sealed: &EncryptedPart,
) -> Result<PairedPc> {
    let corrupt = || Error::CorruptPairedPc;
    if version != PC_SCHEMA_VERSION {
        return Err(corrupt());
    }
    let plain =
        aead::open_with_ad(key, &associated_data(&id.0, version), sealed).map_err(|_| corrupt())?;
    let raw: RawPcV1 = match postcard::take_from_bytes(&plain) {
        Ok((raw, [])) => raw,
        _ => return Err(corrupt()),
    };
    // El nombre guardado ya está recortado y dentro de los límites; si no,
    // la fila está dañada.
    if raw.name.trim() != raw.name
        || raw.name.is_empty()
        || raw.name.len() > PAIRED_PC_NAME_MAX_BYTES
    {
        return Err(corrupt());
    }
    let last_ip = match raw.last_ip.as_deref() {
        None => None,
        Some(bytes) => Some(match bytes.len() {
            4 => IpAddr::V4(Ipv4Addr::from(
                <[u8; 4]>::try_from(bytes).map_err(|_| corrupt())?,
            )),
            16 => IpAddr::V6(Ipv6Addr::from(
                <[u8; 16]>::try_from(bytes).map_err(|_| corrupt())?,
            )),
            _ => return Err(corrupt()),
        }),
    };
    Ok(PairedPc::new(
        id,
        raw.public_key,
        raw.name,
        last_ip,
        raw.paired_at,
        raw.last_seen_at,
    ))
}

#[cfg(test)]
mod tests;
