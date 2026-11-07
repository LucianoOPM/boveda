//! Pruebas unitarias de la API `Vault` (`vault/mod.rs`).

use std::time::{Duration, UNIX_EPOCH};

use super::*;

#[test]
fn clock_before_epoch_is_an_error() {
    let before = UNIX_EPOCH - Duration::from_secs(1);
    assert_eq!(millis_since_epoch(before), Err(Error::Clock));
}

#[test]
fn clock_at_epoch_is_zero() {
    assert_eq!(millis_since_epoch(UNIX_EPOCH), Ok(0));
    assert_eq!(
        millis_since_epoch(UNIX_EPOCH + Duration::from_millis(1500)),
        Ok(1500)
    );
}

#[test]
fn now_is_after_2026() {
    // 2026-01-01T00:00:00Z
    assert!(now_millis().unwrap() > 1_767_225_600_000);
}
