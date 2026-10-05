//! Backend → frontend events (KTD3). Core code emits through [`EventSink`] so it runs
//! the same under Tauri and in tests.

use std::sync::{Arc, Mutex};

use serde::Serialize;
use serde_json::Value;

pub const INSTANCE_STATE: &str = "instance-state";
pub const INSTANCES_CHANGED: &str = "instances-changed";
pub const CONSOLE_BATCH: &str = "console-batch";
pub const METRICS: &str = "metrics";
pub const PLAYERS: &str = "players";
pub const TASK_PROGRESS: &str = "task-progress";
pub const PLAYIT_STATE: &str = "playit-state";
pub const NOTICE: &str = "notice";

pub trait EventSink: Send + Sync {
    fn emit_value(&self, event: &str, payload: Value);
}

pub type Events = Arc<dyn EventSink>;

pub fn emit<T: Serialize>(sink: &dyn EventSink, event: &str, payload: &T) {
    if let Ok(v) = serde_json::to_value(payload) {
        sink.emit_value(event, v);
    }
}

/// Drops everything. For code paths that do not need to report.
pub struct NullSink;

impl EventSink for NullSink {
    fn emit_value(&self, _event: &str, _payload: Value) {}
}

/// Records events in memory (tests and diagnostics).
#[derive(Default)]
pub struct MemorySink {
    events: Mutex<Vec<(String, Value)>>,
}

impl MemorySink {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn all(&self) -> Vec<(String, Value)> {
        self.events.lock().unwrap().clone()
    }

    pub fn named(&self, event: &str) -> Vec<Value> {
        self.events
            .lock()
            .unwrap()
            .iter()
            .filter(|(n, _)| n == event)
            .map(|(_, v)| v.clone())
            .collect()
    }

    pub fn clear(&self) {
        self.events.lock().unwrap().clear();
    }
}

impl EventSink for MemorySink {
    fn emit_value(&self, event: &str, payload: Value) {
        self.events.lock().unwrap().push((event.to_string(), payload));
    }
}

pub struct TauriSink(pub tauri::AppHandle);

impl EventSink for TauriSink {
    fn emit_value(&self, event: &str, payload: Value) {
        use tauri::Emitter;
        let _ = self.0.emit(event, payload);
    }
}
