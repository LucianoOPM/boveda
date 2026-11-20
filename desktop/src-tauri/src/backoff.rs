//! Espera creciente y acotada para los reintentos del servidor (#100).

use std::time::{Duration, Instant};

/// Espera entre reintentos: `base` tras el primer fallo, el doble tras el
/// segundo y así hasta `max`. No duerme ni mira el reloj: quien llama pasa el
/// instante, para probarlo sin esperas.
#[derive(Debug, Clone)]
pub(crate) struct Backoff {
    base: Duration,
    max: Duration,
    failures: u32,
    next_attempt: Option<Instant>,
}

impl Backoff {
    pub(crate) fn new(base: Duration, max: Duration) -> Self {
        Self {
            base,
            max,
            failures: 0,
            next_attempt: None,
        }
    }

    /// `true` si ya se puede intentar de nuevo.
    pub(crate) fn ready(&self, now: Instant) -> bool {
        self.next_attempt.is_none_or(|at| now >= at)
    }

    /// Registra un fallo en `now` y fija el próximo intento.
    pub(crate) fn fail(&mut self, now: Instant) {
        self.failures = self.failures.saturating_add(1);
        let factor = 1u32
            .checked_shl(self.failures.saturating_sub(1))
            .unwrap_or(u32::MAX);
        let wait = self.base.saturating_mul(factor).min(self.max);
        self.next_attempt = Some(now + wait);
    }

    /// Un éxito: se olvidan los fallos.
    pub(crate) fn reset(&mut self) {
        self.failures = 0;
        self.next_attempt = None;
    }

    /// Fallos consecutivos desde el último éxito.
    pub(crate) fn failures(&self) -> u32 {
        self.failures
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::Backoff;

    const BASE: Duration = Duration::from_secs(5);
    const MAX: Duration = Duration::from_secs(60);

    #[test]
    fn new_is_ready_immediately() {
        let backoff = Backoff::new(BASE, MAX);
        assert!(backoff.ready(Instant::now()));
        assert_eq!(backoff.failures(), 0);
    }

    #[test]
    fn fail_waits_base_then_doubles() {
        let t0 = Instant::now();
        let mut backoff = Backoff::new(BASE, MAX);
        backoff.fail(t0);
        assert!(!backoff.ready(t0 + Duration::from_secs(4)));
        assert!(backoff.ready(t0 + Duration::from_secs(5)));
        backoff.fail(t0);
        assert!(!backoff.ready(t0 + Duration::from_secs(9)));
        assert!(backoff.ready(t0 + Duration::from_secs(10)));
    }

    #[test]
    fn never_exceeds_max() {
        let t0 = Instant::now();
        let mut backoff = Backoff::new(BASE, MAX);
        for _ in 0..20 {
            backoff.fail(t0);
        }
        assert!(!backoff.ready(t0 + Duration::from_secs(59)));
        assert!(backoff.ready(t0 + MAX));
        assert_eq!(backoff.failures(), 20);
    }

    #[test]
    fn reset_makes_ready_and_clears_failures() {
        let t0 = Instant::now();
        let mut backoff = Backoff::new(BASE, MAX);
        backoff.fail(t0);
        backoff.reset();
        assert!(backoff.ready(t0));
        assert_eq!(backoff.failures(), 0);
    }
}
