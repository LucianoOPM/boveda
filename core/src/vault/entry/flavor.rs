//! Flavor de postcard que serializa dentro de un `Zeroizing<Vec<u8>>`.
//!
//! Un `Vec` que crece por su cuenta reserva un búfer nuevo, copia y libera el
//! anterior sin borrarlo: el texto plano de la entrada quedaría en memoria
//! liberada. Aquí el crecimiento se hace a mano y el búfer anterior se borra
//! (toda su capacidad) al reemplazarlo.

use postcard::ser_flavors::Flavor;
use zeroize::Zeroizing;

pub(crate) struct ZeroizingVec(Zeroizing<Vec<u8>>);

impl ZeroizingVec {
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self(Zeroizing::new(Vec::with_capacity(capacity)))
    }
}

impl Flavor for ZeroizingVec {
    type Output = Zeroizing<Vec<u8>>;

    fn try_push(&mut self, data: u8) -> postcard::Result<()> {
        self.try_extend(&[data])
    }

    fn try_extend(&mut self, data: &[u8]) -> postcard::Result<()> {
        let needed = self
            .0
            .len()
            .checked_add(data.len())
            .ok_or(postcard::Error::SerializeBufferFull)?;
        if needed > self.0.capacity() {
            let capacity = needed.max(self.0.capacity().saturating_mul(2));
            let mut grown = Zeroizing::new(Vec::with_capacity(capacity));
            grown.extend_from_slice(&self.0);
            // Al reemplazarlo, el búfer anterior se suelta y `Zeroizing` lo
            // borra completo.
            self.0 = grown;
        }
        // Ya hay capacidad suficiente: esto no reasigna.
        self.0.extend_from_slice(data);
        Ok(())
    }

    fn finalize(self) -> postcard::Result<Self::Output> {
        Ok(self.0)
    }
}
