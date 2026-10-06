//! Lifecycle automation (U8): crash restarts, scheduled restarts, keep-awake, tray
//! and start at login.

pub mod autostart;
pub mod crash;
pub mod keep_awake;
pub mod scheduler;
pub mod tray;

use std::sync::{Arc, Mutex};

use chrono::{DateTime, Local};

/// The time source for crash windows and schedules, injectable for tests.
pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Local>;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Local> {
        Local::now()
    }
}

/// A clock tests move by hand.
pub struct ManualClock(Mutex<DateTime<Local>>);

impl ManualClock {
    pub fn new(start: DateTime<Local>) -> Arc<Self> {
        Arc::new(Self(Mutex::new(start)))
    }

    pub fn set(&self, t: DateTime<Local>) {
        *self.0.lock().unwrap() = t;
    }
}

impl Clock for ManualClock {
    fn now(&self) -> DateTime<Local> {
        *self.0.lock().unwrap()
    }
}
