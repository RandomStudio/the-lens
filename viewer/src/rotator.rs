use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

pub struct Rotator {
    angle: Arc<AtomicU64>,
}

impl Rotator {
    pub fn new() -> Self {
        Self {
            angle: Arc::new(AtomicU64::new(0f64.to_bits())),
        }
    }

    pub(crate) fn shared(&self) -> Arc<AtomicU64> {
        Arc::clone(&self.angle)
    }

    pub fn angle(&self) -> f64 {
        f64::from_bits(self.angle.load(Ordering::Relaxed))
    }
}
