//! Per-instance console: a ring buffer for the UI, a persisted log file, and the
//! pending lines for the next `console-batch` event (R13, KTD3).

use std::collections::VecDeque;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use serde::Serialize;

/// Lines kept in memory per instance.
pub const RING_CAPACITY: usize = 5_000;
/// The log file is rotated to `.1` when a session starts and it is larger than this.
const ROTATE_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConsoleLine {
    /// Increases by one per line, so the UI can merge a snapshot with live batches.
    pub seq: u64,
    pub text: String,
    /// From a previous app session (read-only history).
    pub history: bool,
}

pub struct Console {
    ring: VecDeque<ConsoleLine>,
    pending: Vec<ConsoleLine>,
    next_seq: u64,
    log: Option<BufWriter<File>>,
}

impl Default for Console {
    fn default() -> Self {
        Self::new()
    }
}

impl Console {
    pub fn new() -> Self {
        Self { ring: VecDeque::with_capacity(1024), pending: Vec::new(), next_seq: 1, log: None }
    }

    /// Loads the tail of a previous session's log as read-only history.
    pub fn load_history(&mut self, log_path: &Path) {
        for text in read_tail(log_path, RING_CAPACITY) {
            self.push_ring(text, true);
        }
    }

    /// Starts a new session: rotates an oversized log and appends a session marker.
    pub fn open_log(&mut self, log_path: &Path) -> std::io::Result<()> {
        if let Some(dir) = log_path.parent() {
            fs::create_dir_all(dir)?;
        }
        if fs::metadata(log_path).map(|m| m.len() > ROTATE_BYTES).unwrap_or(false) {
            let rotated = log_path.with_extension("1.log");
            fs::remove_file(&rotated).ok();
            fs::rename(log_path, rotated)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(log_path)?;
        self.log = Some(BufWriter::new(file));
        let marker = format!("---- Lodestar session {} ----", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"));
        self.push(marker);
        Ok(())
    }

    pub fn close_log(&mut self) {
        if let Some(mut w) = self.log.take() {
            w.flush().ok();
        }
    }

    fn push_ring(&mut self, text: String, history: bool) -> ConsoleLine {
        let line = ConsoleLine { seq: self.next_seq, text, history };
        self.next_seq += 1;
        if self.ring.len() == RING_CAPACITY {
            self.ring.pop_front();
        }
        self.ring.push_back(line.clone());
        line
    }

    /// Records a live line: memory, log file, and the next batch.
    pub fn push(&mut self, text: String) {
        if let Some(w) = self.log.as_mut() {
            writeln!(w, "{text}").ok();
        }
        let line = self.push_ring(text, false);
        self.pending.push(line);
        // A single batch never carries more than the ring holds; the newest win.
        if self.pending.len() > RING_CAPACITY * 2 {
            let drop = self.pending.len() - RING_CAPACITY;
            self.pending.drain(..drop);
        }
    }

    /// Lines for the next `console-batch` event (newest kept if there are too many).
    pub fn take_pending(&mut self) -> Vec<ConsoleLine> {
        if let Some(w) = self.log.as_mut() {
            w.flush().ok();
        }
        let mut lines = std::mem::take(&mut self.pending);
        if lines.len() > RING_CAPACITY {
            lines.drain(..lines.len() - RING_CAPACITY);
        }
        lines
    }

    pub fn lines(&self) -> Vec<ConsoleLine> {
        self.ring.iter().cloned().collect()
    }

    pub fn is_empty(&self) -> bool {
        self.ring.is_empty()
    }
}

/// The last `max_lines` lines of a text file, reading at most the final 2 MiB.
pub fn read_tail(path: &Path, max_lines: usize) -> Vec<String> {
    let Ok(mut f) = File::open(path) else { return Vec::new() };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    let start = len.saturating_sub(2 * 1024 * 1024);
    if f.seek(SeekFrom::Start(start)).is_err() {
        return Vec::new();
    }
    let mut buf = Vec::new();
    if f.read_to_end(&mut buf).is_err() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&buf);
    let mut lines: Vec<&str> = text.lines().collect();
    if start > 0 && !lines.is_empty() {
        lines.remove(0); // probably a partial line
    }
    let skip = lines.len().saturating_sub(max_lines);
    lines[skip..].iter().map(|s| s.to_string()).collect()
}

pub fn log_path(server_dir: &Path) -> PathBuf {
    server_dir.join("logs").join("lodestar-console.log")
}
