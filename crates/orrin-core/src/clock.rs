use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

pub type Millis = u64;

pub fn now_ms() -> Millis {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Clock(pub u64);

#[derive(Debug, Default)]
pub struct Ticker(AtomicU64);

impl Ticker {
    pub fn starting_at(clock: Clock) -> Self {
        Self(AtomicU64::new(clock.0))
    }

    pub fn now(&self) -> Clock {
        Clock(self.0.load(Ordering::SeqCst))
    }

    pub fn tick(&self) -> Clock {
        Clock(self.0.fetch_add(1, Ordering::SeqCst) + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticker_moves_forward() {
        let ticker = Ticker::default();
        assert_eq!(ticker.tick(), Clock(1));
        assert_eq!(ticker.tick(), Clock(2));
        assert_eq!(ticker.now(), Clock(2));
    }
}
