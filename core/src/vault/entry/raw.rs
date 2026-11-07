//! Estructuras crudas de serialización de la versión de estructura 1.
//!
//! Toman prestado el texto en los dos sentidos: al codificar, de los tipos
//! validados; al decodificar, del búfer `Zeroizing` de quien llama. Así no
//! quedan copias intermedias del texto plano.
//!
//! El orden de los campos ES el formato en disco v1: no se cambia. Un cambio
//! de campos es una versión de estructura nueva con sus propias estructuras.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub(crate) struct RawSearchDataV1<'a> {
    pub(crate) service: &'a str,
    #[serde(borrow)]
    pub(crate) url: Option<&'a str>,
    #[serde(borrow)]
    pub(crate) username: Option<&'a str>,
    #[serde(borrow)]
    pub(crate) tags: Vec<&'a str>,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct RawSecretV1<'a> {
    pub(crate) password: &'a str,
    #[serde(borrow)]
    pub(crate) notes: Option<&'a str>,
}
