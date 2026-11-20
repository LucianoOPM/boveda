//! Portapapeles simulado para las pruebas. Es un manejador (`Clone`) sobre un
//! estado compartido: la prueba conserva una copia mientras el trabajador usa
//! la otra.

use std::sync::{Arc, Mutex};

use super::backend::{ClearOutcome, ClipboardBackend, WriteError};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) enum FakeContent {
    #[default]
    Empty,
    Text(String),
    Image,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum FakeCall {
    Write,
    Clear(ClearOutcome),
}

#[derive(Default)]
struct State {
    content: FakeContent,
    unavailable_calls: usize,
    next_write_failure: Option<(WriteError, bool)>,
    calls: Vec<FakeCall>,
}

#[derive(Clone, Default)]
pub(super) struct FakeBackend(Arc<Mutex<State>>);

impl FakeBackend {
    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.0.lock().expect("el doble no debe envenenarse")
    }

    pub(super) fn content(&self) -> FakeContent {
        self.state().content.clone()
    }

    /// Lo que otra aplicación (o el usuario) deja en el portapapeles.
    pub(super) fn set_content(&self, content: FakeContent) {
        self.state().content = content;
    }

    /// La próxima escritura falla con `error`; con `leaves_text` el texto
    /// queda puesto de todos modos (el caso de resultado dudoso).
    pub(super) fn fail_next_write(&self, error: WriteError, leaves_text: bool) {
        self.state().next_write_failure = Some((error, leaves_text));
    }

    /// Las próximas `calls` llamadas a `clear_if_matches` encuentran el
    /// portapapeles ocupado. `usize::MAX` para «siempre».
    pub(super) fn set_unavailable_for(&self, calls: usize) {
        self.state().unavailable_calls = calls;
    }

    pub(super) fn calls(&self) -> Vec<FakeCall> {
        self.state().calls.clone()
    }
}

impl ClipboardBackend for FakeBackend {
    fn write_sensitive(&mut self, text: &str) -> Result<(), WriteError> {
        let mut state = self.state();
        state.calls.push(FakeCall::Write);
        if let Some((error, leaves_text)) = state.next_write_failure.take() {
            if leaves_text {
                state.content = FakeContent::Text(text.to_owned());
            }
            return Err(error);
        }
        state.content = FakeContent::Text(text.to_owned());
        Ok(())
    }

    fn clear_if_matches(&mut self, matches: &dyn Fn(&[u8]) -> bool) -> ClearOutcome {
        // Todo ocurre con el mismo candado: es la «única apertura».
        let mut state = self.state();
        let outcome = if state.unavailable_calls > 0 {
            state.unavailable_calls = state.unavailable_calls.saturating_sub(1);
            ClearOutcome::Unavailable
        } else {
            match &state.content {
                FakeContent::Text(text) if matches(text.as_bytes()) => {
                    state.content = FakeContent::Empty;
                    ClearOutcome::Cleared
                }
                _ => ClearOutcome::NotOurs,
            }
        };
        state.calls.push(FakeCall::Clear(outcome));
        outcome
    }
}
