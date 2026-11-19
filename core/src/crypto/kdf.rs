//! Derivación de la llave maestra con Argon2id.

use std::time::{Duration, Instant};

use argon2::{Algorithm, Argon2, Block, Params, Version};
use secrecy::{SecretBox, SecretString};
use zeroize::{Zeroize, Zeroizing};

use super::normalize::normalize_nfc;
use crate::error::{Error, Result};

/// Longitud de la sal en bytes.
pub(crate) const SALT_LEN: usize = 16;

/// Longitud de la llave derivada en bytes.
pub(crate) const DERIVED_KEY_LEN: usize = 32;

/// Llave derivada de la contraseña maestra; se borra de memoria al soltarla.
///
/// Vive en el heap (`SecretBox`, como `VaultKey`): mover la llave copia un
/// puntero y no los 32 bytes, que es lo que dejaría copias sin borrar en la
/// pila con un arreglo (#54). Su `Debug` no muestra el valor.
pub(crate) type DerivedKey = SecretBox<[u8; DERIVED_KEY_LEN]>;

/// Tiempo objetivo de una derivación en el dispositivo.
const CALIBRATION_TARGET: Duration = Duration::from_millis(500);

/// Parámetros de Argon2id guardados en la cabecera de la bóveda.
///
/// No son secretos: viajan en claro dentro de la cabecera.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct KdfParams {
    /// Memoria en KiB.
    pub(crate) memory_kib: u32,
    /// Número de pasadas sobre la memoria.
    pub(crate) iterations: u32,
    /// Carriles en paralelo.
    pub(crate) parallelism: u32,
}

impl KdfParams {
    /// Mínimos de la especificación: 64 MiB, 3 iteraciones y paralelismo 1.
    pub(crate) const MIN: Self = Self {
        memory_kib: 64 * 1024,
        iterations: 3,
        parallelism: 1,
    };

    /// Tope de memoria: 128 MiB, para que la bóveda abra en celulares con
    /// menos memoria libre que el que la creó.
    pub(crate) const MAX_MEMORY_KIB: u32 = 128 * 1024;

    /// Tope de iteraciones de `calibrate()`; evita un ciclo sin fin si la
    /// medición nunca alcanza el objetivo.
    pub(crate) const MAX_ITERATIONS: u32 = 16;

    /// Comprueba que los parámetros estén dentro de los límites del formato.
    pub(crate) fn validate(&self) -> Result<()> {
        let memory_ok = (Self::MIN.memory_kib..=Self::MAX_MEMORY_KIB).contains(&self.memory_kib);
        let iterations_ok = self.iterations >= Self::MIN.iterations;
        let parallelism_ok = self.parallelism == Self::MIN.parallelism;
        if memory_ok && iterations_ok && parallelism_ok {
            Ok(())
        } else {
            Err(Error::InvalidKdfParams)
        }
    }

    /// Siguiente candidato de `calibrate()`: primero sube la memoria hasta el
    /// tope y después las iteraciones. El paralelismo queda fijo en 1.
    fn next_step(&self) -> Option<Self> {
        if self.memory_kib < Self::MAX_MEMORY_KIB {
            Some(Self {
                memory_kib: (self.memory_kib + CALIBRATION_MEMORY_STEP_KIB)
                    .min(Self::MAX_MEMORY_KIB),
                ..*self
            })
        } else if self.iterations < Self::MAX_ITERATIONS {
            Some(Self {
                iterations: self.iterations + 1,
                ..*self
            })
        } else {
            None
        }
    }
}

/// Incremento de memoria entre mediciones de `calibrate()`: 16 MiB.
const CALIBRATION_MEMORY_STEP_KIB: u32 = 16 * 1024;

/// Elige los parámetros cuyo tiempo medido queda más cerca de `target`.
///
/// Empieza en los mínimos y sube un paso a la vez (ver
/// [`KdfParams::next_step`]); nunca devuelve menos que [`KdfParams::MIN`].
fn calibrate_with(
    target: Duration,
    mut measure: impl FnMut(&KdfParams) -> Result<Duration>,
) -> Result<KdfParams> {
    let mut current = KdfParams::MIN;
    let mut previous: Option<(KdfParams, Duration)> = None;
    loop {
        let elapsed = measure(&current)?;
        if elapsed >= target {
            // Entre el último candidato por debajo y este, el más cercano.
            return Ok(match previous {
                Some((params, time)) if target - time < elapsed - target => params,
                _ => current,
            });
        }
        match current.next_step() {
            Some(next) => {
                previous = Some((current, elapsed));
                current = next;
            }
            None => return Ok(current),
        }
    }
}

/// Mide en este dispositivo y elige parámetros que tarden unos 0.5 s.
///
/// Solo se usa al crear la bóveda o al cambiar la contraseña maestra.
pub(crate) fn calibrate() -> Result<KdfParams> {
    // Contraseña y sal de prueba: solo importa cuánto tarda la derivación.
    let password = SecretString::from("calibrate");
    let salt = [0u8; SALT_LEN];
    calibrate_with(CALIBRATION_TARGET, |params| {
        let start = Instant::now();
        derive_key(&password, &salt, params)?;
        Ok(start.elapsed())
    })
}

#[cfg(test)]
thread_local! {
    /// Llamadas a [`derive_key`] en este hilo. Solo existe en pruebas, para
    /// comprobar sin medir tiempos que un flujo deriva una sola vez. Cada
    /// prueba lo pone en cero al empezar: `thread_local` aísla las pruebas
    /// entre sí, pero no de lo que haya corrido antes en el mismo hilo.
    pub(crate) static DERIVE_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Deriva la llave que cifra la llave de bóveda.
///
/// Rechaza parámetros fuera de los límites de [`KdfParams::validate`].
pub(crate) fn derive_key(
    password: &SecretString,
    salt: &[u8; SALT_LEN],
    params: &KdfParams,
) -> Result<DerivedKey> {
    #[cfg(test)]
    DERIVE_CALLS.with(|calls| calls.set(calls.get() + 1));
    let argon2 = argon2_for(params)?;

    // En NFC, para que una misma contraseña derive la misma llave sin importar
    // cómo la escribió el teclado (#57). Una contraseña ASCII no cambia.
    let normalized = normalize_nfc(password);

    // La memoria de trabajo de Argon2id (de 64 a 128 MiB) es nuestra y se
    // borra al terminar: la que reserva `hash_password_into` se libera sin
    // borrar y, con paralelismo 1, permite recalcular la llave derivada.
    let mut memory = work_memory(argon2.params().block_count())?;

    // Argon2id escribe directamente en el búfer del heap: la llave nunca pasa
    // por un arreglo propio en la pila. `init_with_mut` no devuelve `Result`,
    // así que el error se captura afuera; si Argon2id devuelve un error, el
    // `SecretBox` se suelta y se borra sin copiar nada a otro lugar (un panic
    // dentro del closure no está cubierto: `init_with_mut` no borra en ese
    // caso, y la API de `argon2` no hace panic con parámetros ya validados).
    // Empieza en `Err`: si el closure no corriera, `derive_key` fallaría en
    // vez de devolver una llave de ceros.
    //
    // Las copias internas de las dependencias que quedan fuera de nuestro
    // alcance están documentadas en la excepción de `crypto`.
    let mut hashed: Result<()> = Err(Error::InvalidKdfParams);
    let key = DerivedKey::init_with_mut(|out| {
        hashed = hash_into(
            &argon2,
            normalized.as_bytes(),
            salt,
            out.as_mut(),
            memory.as_mut_slice(),
        );
    });
    hashed?;
    Ok(key)
}

/// Argon2id con los parámetros de la cabecera. Rechaza los que estén fuera de
/// los límites de [`KdfParams::validate`].
fn argon2_for(params: &KdfParams) -> Result<Argon2<'static>> {
    params.validate()?;
    let argon2_params = Params::new(
        params.memory_kib,
        params.iterations,
        params.parallelism,
        Some(DERIVED_KEY_LEN),
    )
    .map_err(|_| Error::InvalidKdfParams)?;
    Ok(Argon2::new(
        Algorithm::Argon2id,
        Version::V0x13,
        argon2_params,
    ))
}

/// Reserva la memoria de trabajo de Argon2id con el tamaño exacto que piden
/// los parámetros. La capacidad reservada coincide con la longitud, así que
/// el `Vec` no vuelve a reservar memoria y no deja copias en un búfer anterior.
/// Se borra al soltarla, también si algo hace panic.
///
/// Si no hay memoria, devuelve [`Error::InvalidKdfParams`]: la API pública no
/// hace panic ni aborta.
fn work_memory(blocks: usize) -> Result<Zeroizing<Vec<Block>>> {
    let mut memory = Zeroizing::new(Vec::new());
    memory
        .try_reserve_exact(blocks)
        .map_err(|_| Error::InvalidKdfParams)?;
    memory.resize(blocks, Block::new());
    Ok(memory)
}

/// Corre Argon2id sobre la memoria de trabajo `memory` y la borra al terminar,
/// con éxito o con error. Es una función aparte, que recibe el búfer, para
/// poder comprobar en una prueba que queda en ceros.
fn hash_into(
    argon2: &Argon2<'_>,
    password: &[u8],
    salt: &[u8],
    out: &mut [u8],
    memory: &mut [Block],
) -> Result<()> {
    let hashed = argon2
        .hash_password_into_with_memory(password, salt, out, &mut *memory)
        .map_err(|_| Error::InvalidKdfParams);
    // `Block` implementa `Zeroize`, pero no `DefaultIsZeroes`, así que no hay
    // `Zeroize` para el slice entero: se borra bloque por bloque.
    memory.iter_mut().for_each(Zeroize::zeroize);
    hashed
}

/// Genera una sal nueva con el generador del sistema.
pub(crate) fn generate_salt() -> Result<[u8; SALT_LEN]> {
    let mut salt = [0u8; SALT_LEN];
    getrandom::fill(&mut salt).map_err(|_| Error::Random)?;
    Ok(salt)
}

#[cfg(test)]
mod tests;
