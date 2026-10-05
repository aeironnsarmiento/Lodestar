//! What the app does on its own: op the host when a server comes online, keep the
//! PC awake, restart crashed servers, run restart schedules, start flagged servers
//! with the app, and shut everything down cleanly on quit (U8).

use std::sync::Arc;
use std::time::Duration;

use super::app::App;
use super::instance::Provision;
use crate::lifecycle::crash::CrashDecision;
use crate::lifecycle::scheduler::{self, Action};
use crate::supervisor::{ServerState, StateChange, StopReason};

/// How often schedules are checked (fine enough for the 10-second warning).
pub const SCHEDULER_TICK: Duration = Duration::from_secs(5);

impl App {
    pub(crate) fn install_hooks(self: &Arc<Self>) {
        let weak = Arc::downgrade(self);
        self.supervisor.add_hook(Arc::new(move |change| {
            if let Some(app) = weak.upgrade() {
                app.on_state_change(change);
            }
        }));
    }

    /// Reacts to lifecycle transitions. Runs on the supervisor's task, so anything
    /// slow is spawned.
    fn on_state_change(self: &Arc<Self>, change: &StateChange) {
        let any_active = self.store.list().iter().any(|i| self.supervisor.state(&i.id).is_active());
        self.keep_awake.set(any_active);

        match change.state {
            ServerState::Online => {
                if let Ok(inst) = self.store.get(&change.id) {
                    let name = inst.op_name.trim();
                    if !name.is_empty() {
                        let _ = self.supervisor.send_command(&change.id, &format!("op {name}"));
                    }
                }
            }
            ServerState::Crashed => self.handle_crash(&change.id),
            _ => {}
        }
    }

    fn handle_crash(self: &Arc<Self>, id: &str) {
        let decision = self.crash.lock().unwrap().on_crash(id, self.clock.now(), &self.crash_backoff);
        match decision {
            CrashDecision::RestartAfter { delay, attempt } => {
                let secs = delay.as_secs_f32();
                let msg = format!(
                    "The server crashed. Restarting in {} (attempt {attempt} of {}).",
                    if secs >= 1.0 { format!("{secs:.0} s") } else { format!("{:.0} ms", secs * 1000.0) },
                    self.crash_backoff.len()
                );
                self.supervisor.note(id, &msg);
                self.supervisor.set_notice(id, Some(msg));
                let (app, id) = (self.clone(), id.to_string());
                tokio::spawn(async move {
                    tokio::time::sleep(delay).await;
                    // A user stop while waiting cancels the restart.
                    if app.supervisor.state(&id) != ServerState::Crashed {
                        return;
                    }
                    if let Err(e) = app.launch(&id).await {
                        app.supervisor.note(&id, &format!("Automatic restart failed: {e:#}"));
                    }
                });
            }
            CrashDecision::GiveUp => {
                let msg = "The server kept crashing, so Glasscraft stopped restarting it. Check the console for the error, then launch it again.".to_string();
                self.supervisor.note(id, &msg);
                self.supervisor.give_up(id, msg);
            }
        }
    }

    /// Runs one scheduler pass for every instance and carries out the actions.
    /// Returns them (per instance) for tests and diagnostics.
    pub fn scheduler_tick(self: &Arc<Self>) -> Vec<(String, Vec<Action>)> {
        let now = self.clock.now().naive_local();
        let mut out = Vec::new();
        for inst in self.store.list() {
            let online = self.supervisor.state(&inst.id) == ServerState::Online;
            let players = self.supervisor.players(&inst.id).len();
            let actions = {
                let mut states = self.schedules.lock().unwrap();
                let state = states.entry(inst.id.clone()).or_default();
                scheduler::tick(&inst.restart, state, now, players, online)
            };
            for action in &actions {
                match action {
                    Action::Say(text) => {
                        let _ = self.supervisor.send_command(&inst.id, &format!("say {text}"));
                    }
                    Action::Restart => {
                        let (app, id) = (self.clone(), inst.id.clone());
                        tokio::spawn(async move {
                            app.supervisor.note(&id, "Scheduled restart.");
                            let result = async {
                                app.supervisor.stop_and_wait(&id, StopReason::Schedule).await?;
                                app.launch(&id).await
                            }
                            .await;
                            if let Err(e) = result {
                                app.supervisor.note(&id, &format!("Scheduled restart failed: {e:#}"));
                            }
                        });
                    }
                }
            }
            if !actions.is_empty() {
                out.push((inst.id.clone(), actions));
            }
        }
        out
    }

    pub async fn run_scheduler(self: Arc<Self>) {
        loop {
            tokio::time::sleep(SCHEDULER_TICK).await;
            self.scheduler_tick();
        }
    }

    /// Launches the servers flagged to start with the app (R8).
    pub async fn autostart_instances(self: Arc<Self>) {
        for inst in self.store.list() {
            if !inst.auto_start || inst.provision != Provision::Ready {
                continue;
            }
            if let Err(e) = self.launch(&inst.id).await {
                self.supervisor.note(&inst.id, &format!("Could not start with the app: {e:#}"));
                self.supervisor.set_notice(&inst.id, Some(format!("Could not start with the app: {e:#}")));
            }
        }
    }

    /// Stops every server gracefully (app quit).
    pub async fn shutdown(&self) {
        self.supervisor.stop_all(StopReason::Quit).await;
        self.keep_awake.set(false);
    }
}
