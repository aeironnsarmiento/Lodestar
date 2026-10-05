//! Crash policy (R5, KTD13): an exit nobody asked for is a crash. The server is
//! restarted after 5 s, 15 s and 45 s; if it crashes again after those three
//! automatic restarts within a rolling 10 minutes, Glasscraft stops retrying and
//! leaves it Stopped with a crash notice. These limits are fixed in v1.

use std::collections::HashMap;
use std::time::Duration;

use chrono::{DateTime, Local};

pub const BACKOFF: [Duration; 3] = [Duration::from_secs(5), Duration::from_secs(15), Duration::from_secs(45)];
pub const WINDOW: chrono::Duration = chrono::Duration::minutes(10);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrashDecision {
    /// Restart after this delay; `attempt` counts from 1.
    RestartAfter { delay: Duration, attempt: usize },
    GiveUp,
}

#[derive(Default)]
pub struct CrashPolicy {
    /// Automatic restarts per instance within the window.
    restarts: HashMap<String, Vec<DateTime<Local>>>,
}

impl CrashPolicy {
    /// Decides what to do about a crash at `now`, using `backoff` delays (tests pass
    /// shorter ones). Restarts older than the window no longer count.
    pub fn on_crash(&mut self, id: &str, now: DateTime<Local>, backoff: &[Duration; 3]) -> CrashDecision {
        let history = self.restarts.entry(id.to_string()).or_default();
        history.retain(|t| now - *t < WINDOW);
        if history.len() >= backoff.len() {
            history.clear();
            return CrashDecision::GiveUp;
        }
        history.push(now);
        CrashDecision::RestartAfter { delay: backoff[history.len() - 1], attempt: history.len() }
    }

    /// Forgets an instance's crash history (a manual start or stop starts fresh).
    pub fn reset(&mut self, id: &str) {
        self.restarts.remove(id);
    }
}
