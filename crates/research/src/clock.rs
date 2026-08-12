use chrono::{DateTime, Duration, Utc};
use std::sync::{Arc, Mutex};
use trading_domain::Clock;

#[derive(Debug)]
pub struct SimulationClock {
    now: Mutex<DateTime<Utc>>,
}

impl SimulationClock {
    #[must_use]
    pub fn new(start: DateTime<Utc>) -> Self {
        Self {
            now: Mutex::new(start),
        }
    }

    pub fn set(&self, at: DateTime<Utc>) {
        *self.now.lock().expect("simulation clock lock") = at;
    }

    pub fn advance(&self, by: Duration) {
        let mut now = self.now.lock().expect("simulation clock lock");
        *now += by;
    }
}

impl Clock for SimulationClock {
    fn now(&self) -> DateTime<Utc> {
        *self.now.lock().expect("simulation clock lock")
    }
}

/// Shares one simulation clock with adapters that still take `Box<dyn Clock>`.
#[derive(Debug, Clone)]
pub struct SharedClock(pub Arc<SimulationClock>);

impl Clock for SharedClock {
    fn now(&self) -> DateTime<Utc> {
        self.0.now()
    }
}
