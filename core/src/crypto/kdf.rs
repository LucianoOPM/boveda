//! Derivación de la llave maestra con Argon2id.

use std::time::{Duration, Instant};

use argon2::{Algorithm, Argon2, Params, Version};
use secrecy::{ExposeSecret, SecretString};
use zeroize::Zeroizing;

use crate::error::{Error, Result};

/// Longitud de la sal en bytes.
pub(crate) const SALT_LEN: usize = 16;

/// Longitud de la llave derivada en bytes.
pub(crate) const DERIVED_KEY_LEN: usize = 32;

/// Llave derivada de la contraseña maestra; se borra de memoria al soltarla.
pub(crate) type DerivedKey = Zeroizing<[u8; DERIVED_KEY_LEN]>;

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

/// Deriva la llave que cifra la llave de bóveda.
///
/// Rechaza parámetros fuera de los límites de [`KdfParams::validate`].
pub(crate) fn derive_key(
    password: &SecretString,
    salt: &[u8; SALT_LEN],
    params: &KdfParams,
) -> Result<DerivedKey> {
    params.validate()?;
    let argon2_params = Params::new(
        params.memory_kib,
        params.iterations,
        params.parallelism,
        Some(DERIVED_KEY_LEN),
    )
    .map_err(|_| Error::InvalidKdfParams)?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, argon2_params);

    let mut key = Zeroizing::new([0u8; DERIVED_KEY_LEN]);
    argon2
        .hash_password_into(password.expose_secret().as_bytes(), salt, key.as_mut())
        .map_err(|_| Error::InvalidKdfParams)?;
    Ok(key)
}

/// Genera una sal nueva con el generador del sistema.
pub(crate) fn generate_salt() -> Result<[u8; SALT_LEN]> {
    let mut salt = [0u8; SALT_LEN];
    getrandom::fill(&mut salt).map_err(|_| Error::Random)?;
    Ok(salt)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIB: u32 = 1024;

    #[test]
    fn min_params_match_spec() {
        assert_eq!(KdfParams::MIN.memory_kib, 64 * MIB);
        assert_eq!(KdfParams::MIN.iterations, 3);
        assert_eq!(KdfParams::MIN.parallelism, 1);
    }

    #[test]
    fn validate_accepts_min_and_max_memory() {
        assert_eq!(KdfParams::MIN.validate(), Ok(()));
        let max = KdfParams {
            memory_kib: KdfParams::MAX_MEMORY_KIB,
            iterations: 10,
            parallelism: 1,
        };
        assert_eq!(max.validate(), Ok(()));
    }

    #[test]
    fn validate_rejects_params_out_of_bounds() {
        let invalid = [
            KdfParams {
                memory_kib: 64 * MIB - 1,
                ..KdfParams::MIN
            },
            KdfParams {
                memory_kib: KdfParams::MAX_MEMORY_KIB + 1,
                ..KdfParams::MIN
            },
            KdfParams {
                iterations: 2,
                ..KdfParams::MIN
            },
            KdfParams {
                parallelism: 0,
                ..KdfParams::MIN
            },
            KdfParams {
                parallelism: 2,
                ..KdfParams::MIN
            },
        ];
        for params in invalid {
            assert_eq!(
                params.validate(),
                Err(Error::InvalidKdfParams),
                "{params:?}"
            );
        }
    }

    #[test]
    fn calibrate_on_slow_device_returns_min() {
        let params = calibrate_with(Duration::from_millis(500), |_| Ok(Duration::from_secs(10)));
        assert_eq!(params, Ok(KdfParams::MIN));
    }

    #[test]
    fn calibrate_on_fast_device_stops_at_caps() {
        let params = calibrate_with(Duration::from_millis(500), |_| Ok(Duration::from_millis(1)));
        assert_eq!(
            params,
            Ok(KdfParams {
                memory_kib: KdfParams::MAX_MEMORY_KIB,
                iterations: KdfParams::MAX_ITERATIONS,
                parallelism: 1,
            })
        );
    }

    #[test]
    fn calibrate_raises_memory_first_then_iterations() {
        // Modelo: el tiempo crece con memoria × iteraciones; los mínimos tardan 120 ms.
        // Con 128 MiB cada iteración cuesta 80 ms: 6 → 480 ms, 7 → 560 ms.
        let min_cost = f64::from(KdfParams::MIN.memory_kib * KdfParams::MIN.iterations);
        let mut seen = Vec::new();
        let params = calibrate_with(Duration::from_millis(500), |p| {
            seen.push(*p);
            let cost = f64::from(p.memory_kib) * f64::from(p.iterations);
            Ok(Duration::from_secs_f64(0.120 * cost / min_cost))
        });

        assert_eq!(
            params,
            Ok(KdfParams {
                memory_kib: 128 * MIB,
                iterations: 6,
                parallelism: 1,
            })
        );
        // Mientras no llega al tope de memoria, las iteraciones no suben.
        assert!(
            seen.iter()
                .all(|p| p.memory_kib == KdfParams::MAX_MEMORY_KIB || p.iterations == 3)
        );
        assert!(seen.iter().all(|p| p.validate().is_ok()));
    }

    #[test]
    fn calibrate_propagates_measure_error() {
        let params = calibrate_with(Duration::from_millis(500), |_| Err(Error::InvalidKdfParams));
        assert_eq!(params, Err(Error::InvalidKdfParams));
    }

    fn password(text: &str) -> SecretString {
        SecretString::from(text)
    }

    #[test]
    fn same_inputs_derive_same_key() {
        let salt = [7u8; SALT_LEN];
        let first = derive_key(&password("correcta"), &salt, &KdfParams::MIN).unwrap();
        let second = derive_key(&password("correcta"), &salt, &KdfParams::MIN).unwrap();
        assert_eq!(*first, *second);
    }

    #[test]
    fn different_salt_password_or_params_derive_different_keys() {
        let salt = [7u8; SALT_LEN];
        let base = derive_key(&password("correcta"), &salt, &KdfParams::MIN).unwrap();

        let other_salt = derive_key(&password("correcta"), &[8u8; SALT_LEN], &KdfParams::MIN);
        let other_password = derive_key(&password("incorrecta"), &salt, &KdfParams::MIN);
        let more_iterations = KdfParams {
            iterations: 4,
            ..KdfParams::MIN
        };
        let other_params = derive_key(&password("correcta"), &salt, &more_iterations);

        for other in [other_salt, other_password, other_params] {
            assert_ne!(*base, *other.unwrap());
        }
    }

    #[test]
    fn derive_key_rejects_params_below_minimum() {
        let weak = KdfParams {
            memory_kib: 8 * MIB,
            ..KdfParams::MIN
        };
        let result = derive_key(&password("correcta"), &[0u8; SALT_LEN], &weak);
        assert_eq!(result.err(), Some(Error::InvalidKdfParams));
    }

    #[test]
    fn generated_salts_are_different() {
        assert_ne!(generate_salt().unwrap(), generate_salt().unwrap());
    }

    /// Mide de verdad en este equipo: tarda varios segundos, por eso se
    /// ejecuta a mano con `cargo test -p vault-core -- --ignored`.
    #[test]
    #[ignore = "lenta: mide Argon2id real varias veces"]
    fn calibrate_on_this_device_stays_within_bounds() {
        let params = calibrate().unwrap();
        assert_eq!(params.validate(), Ok(()));
    }
}
