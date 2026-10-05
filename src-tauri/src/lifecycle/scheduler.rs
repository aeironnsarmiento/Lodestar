//! Scheduled restarts (R6). Each instance has daily `HH:MM` times and a mode:
//! - warn: `say` countdown warnings at 5 min, 1 min and 10 s, then restart on time;
//! - postpone: if players are online at the due time, restart once the last leaves.
//!
//! [`tick`] is pure: it takes the time and player count and returns what to do, so
//! the countdown logic is tested with an injected clock.

use chrono::{Duration, NaiveDate, NaiveDateTime, NaiveTime};

use crate::core::instance::{RestartMode, RestartSchedule};

/// Warnings before a warn-mode restart: seconds before the due time, and the text.
pub const WARNINGS: [(i64, &str); 3] = [
    (300, "Server restarting in 5 minutes."),
    (60, "Server restarting in 1 minute."),
    (10, "Server restarting in 10 seconds!"),
];

/// A due time is still acted on this long after it passed (covers a late tick).
const GRACE_SECS: i64 = 90;
/// How far ahead a due time is picked up.
const LOOKAHEAD_SECS: i64 = 300 + 30;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Say(String),
    Restart,
}

#[derive(Default, Debug, Clone)]
pub struct ScheduleState {
    pending: Option<NaiveDateTime>,
    warned: Vec<i64>,
    postponed_notice: bool,
    last_fired: Option<NaiveDateTime>,
}

pub fn parse_time(s: &str) -> Option<NaiveTime> {
    NaiveTime::parse_from_str(s.trim(), "%H:%M").ok()
}

fn upcoming(schedule: &RestartSchedule, now: NaiveDateTime, last_fired: Option<NaiveDateTime>) -> Option<NaiveDateTime> {
    let today = now.date();
    let days: [NaiveDate; 2] = [today, today + Duration::days(1)];
    days.iter()
        .flat_map(|d| schedule.times.iter().filter_map(|t| parse_time(t)).map(move |t| d.and_time(t)))
        .filter(|due| last_fired.is_none_or(|l| *due > l))
        .filter(|due| *due > now - Duration::seconds(GRACE_SECS))
        .min()
}

/// Advances one instance's schedule to `now`. `online` is whether the server is
/// running and ready; schedules do nothing for stopped servers.
pub fn tick(schedule: &RestartSchedule, state: &mut ScheduleState, now: NaiveDateTime, players: usize, online: bool) -> Vec<Action> {
    let mut actions = Vec::new();
    if !online || schedule.times.is_empty() {
        // A due time that passes while the server is down is skipped, not queued.
        if let Some(due) = state.pending.take() {
            state.last_fired = Some(due);
        }
        state.warned.clear();
        state.postponed_notice = false;
        return actions;
    }

    let due = match state.pending {
        Some(d) => d,
        None => match upcoming(schedule, now, state.last_fired) {
            Some(d) if d - now <= Duration::seconds(LOOKAHEAD_SECS) => {
                state.pending = Some(d);
                d
            }
            _ => return actions,
        },
    };

    let fire = |state: &mut ScheduleState, actions: &mut Vec<Action>| {
        actions.push(Action::Restart);
        state.last_fired = Some(due);
        state.pending = None;
        state.warned.clear();
        state.postponed_notice = false;
    };

    match schedule.mode {
        RestartMode::Warn => {
            if now >= due {
                fire(state, &mut actions);
            } else {
                // Only the most urgent warning that is due; a late tick skips stale ones.
                let mut latest = None;
                for (offset, text) in WARNINGS {
                    if now >= due - Duration::seconds(offset) && !state.warned.contains(&offset) {
                        state.warned.push(offset);
                        latest = Some(text);
                    }
                }
                if let Some(text) = latest {
                    actions.push(Action::Say(text.to_string()));
                }
            }
        }
        RestartMode::Postpone => {
            if now >= due {
                if players == 0 {
                    fire(state, &mut actions);
                } else if !state.postponed_notice {
                    state.postponed_notice = true;
                    actions.push(Action::Say("Scheduled restart postponed until everyone has left.".into()));
                }
            }
        }
    }
    actions
}
