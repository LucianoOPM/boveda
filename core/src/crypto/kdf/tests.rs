//! Pruebas unitarias de `crypto/kdf.rs`.

use secrecy::ExposeSecret;

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
    assert_eq!(first.expose_secret(), second.expose_secret());
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
        assert_ne!(base.expose_secret(), other.unwrap().expose_secret());
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

/// Garantía de tipo (#54): `DerivedKey` y `VaultKey` son un puntero al heap, así
/// que mover una llave copia 8 bytes y no los 32 de la llave.
///
/// No prueba que no haya copias en la pila (las copias internas de las
/// dependencias, o las que decida el compilador): eso se cubre con la
/// revisión de código y con las excepciones documentadas.
#[test]
fn derived_and_vault_keys_are_pointer_sized() {
    use crate::crypto::keys::VaultKey;
    use std::mem::size_of;

    assert_eq!(size_of::<DerivedKey>(), size_of::<usize>());
    assert_eq!(size_of::<VaultKey>(), size_of::<usize>());
}

/// Llave de la contraseña NFC `café-clave-larga` (con `é` U+00E9), sal
/// `00 01 .. 0f` y `KdfParams::MIN`, calculada con `node:crypto` (`argon2Sync`)
/// sobre los bytes UTF-8 de la forma NFC.
const CAFE_KEY_HEX: &str = "8fcbd7402846edc9eae3230ad6f866b007f7d5d807df8cc300ae02a0da0816aa";

fn key_hex(text: &str) -> String {
    let salt: [u8; SALT_LEN] = core::array::from_fn(|i| i as u8);
    let key = derive_key(&password(text), &salt, &KdfParams::MIN).unwrap();
    key.expose_secret()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[test]
fn composed_and_decomposed_passwords_derive_the_same_key() {
    // U+00E9 contra U+0065 U+0301: la misma contraseña escrita desde otro teclado.
    assert_eq!(key_hex("caf\u{00e9}-clave-larga"), CAFE_KEY_HEX);
    assert_eq!(key_hex("cafe\u{0301}-clave-larga"), CAFE_KEY_HEX);
}

#[test]
fn a_different_accent_derives_a_different_key() {
    assert_ne!(key_hex("cafe\u{0300}-clave-larga"), CAFE_KEY_HEX);
    assert_ne!(key_hex("cafe-clave-larga"), CAFE_KEY_HEX);
}

/// Vector fijo: la llave de una contraseña ASCII no cambia con la
/// normalización NFC (#57). Se calculó con el código anterior al cambio y se
/// verificó con el Argon2id de `node:crypto` (`argon2Sync`).
#[test]
fn ascii_password_derives_the_reference_key() {
    let salt: [u8; SALT_LEN] = core::array::from_fn(|i| i as u8);
    let key = derive_key(&password("clave-ascii-12345"), &salt, &KdfParams::MIN).unwrap();
    let hex: String = key
        .expose_secret()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(
        hex,
        "7f51bec591d86903e9b95fc8bfcaa86ea4d19111408e28f5a05f4f960d9ffe2a"
    );
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
