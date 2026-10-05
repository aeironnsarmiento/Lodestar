//! The instance lifecycle (Product Contract state diagram) and the log lines that
//! drive it (KTD12).

use std::sync::OnceLock;

use regex::Regex;
use serde::Serialize;

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum ServerState {
    Stopped,
    /// Java, server files and the world are being made ready.
    Preparing,
    /// The process runs but has not printed its `Done` line yet.
    Starting,
    Online,
    Stopping,
    Crashed,
}

impl ServerState {
    /// The process exists (or is about to) and holds the instance's port.
    pub fn is_active(self) -> bool {
        matches!(self, ServerState::Preparing | ServerState::Starting | ServerState::Online | ServerState::Stopping)
    }

    pub fn has_process(self) -> bool {
        matches!(self, ServerState::Starting | ServerState::Online | ServerState::Stopping)
    }
}

/// Why a stop was requested. An exit without one of these is a crash (KTD13).
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum StopReason {
    User,
    Restart,
    Reset,
    Schedule,
    Quit,
}

fn ready_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"Done \(\d+(?:[.,]\d+)?s\)! For help, type "help""#).unwrap())
}

/// `Done (1.23s)! For help, type "help"` (also with a comma decimal separator).
pub fn is_ready_line(line: &str) -> bool {
    ready_re().is_match(line)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlayerEvent {
    Joined(String),
    Left(String),
}

fn player_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // Anchored on the log prefix so chat like "<Bob> Steve joined the game" does not count.
    RE.get_or_init(|| Regex::new(r"\]: ([A-Za-z0-9_.]{1,16}) (joined|left) the game\s*$").unwrap())
}

pub fn parse_player_event(line: &str) -> Option<PlayerEvent> {
    let c = player_re().captures(line)?;
    let name = c[1].to_string();
    Some(if &c[2] == "joined" { PlayerEvent::Joined(name) } else { PlayerEvent::Left(name) })
}
