//! Reset World (F2, R16–R20): stop instantly, create a fresh run folder with a random
//! or entered seed, start again, then prune old runs once the server is back up.

use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Result};

use super::{check_run_name, list_worlds, new_world, prune, world_exists, world_seed, WorldInfo, KEEP_WORLDS};
use crate::core::app::App;
use crate::supervisor::{ServerState, StopReason};

/// How long to wait for a reset server to come back before skipping the prune.
const BACK_UP_TIMEOUT: Duration = Duration::from_secs(300);

impl App {
    /// The run the next start uses, creating the first world (with the seed chosen at
    /// creation) when there is none. Returns the run name and its seed.
    pub fn ensure_current_world(&self, id: &str) -> Result<(String, String)> {
        let inst = self.store.get(id)?;
        let server_dir = self.paths().server_dir(id);
        if let Some(run) = inst.current_world.as_deref().filter(|r| world_exists(&server_dir, r)) {
            return Ok((run.to_string(), world_seed(&server_dir, run).unwrap_or_default()));
        }
        let (run, seed) = new_world(&server_dir, inst.initial_seed.as_deref(), chrono::Local::now())?;
        self.store.modify(id, |i| i.current_world = Some(run.clone()))?;
        self.notify_instances_changed();
        Ok((run, seed))
    }

    pub fn worlds(&self, id: &str) -> Result<Vec<WorldInfo>> {
        let inst = self.store.get(id)?;
        Ok(list_worlds(&self.paths().server_dir(id), inst.current_world.as_deref()))
    }

    /// Makes a kept world the one the next start uses (R20).
    pub fn switch_world(&self, id: &str, run: &str) -> Result<()> {
        check_run_name(&self.paths().server_dir(id), run)?;
        self.store.modify(id, |i| i.current_world = Some(run.to_string()))?;
        self.notify_instances_changed();
        Ok(())
    }

    /// Resets to a fresh world. A running server is stopped immediately (players are
    /// disconnected, no countdown) and force-killed if it does not stop in time; a
    /// stopped one is simply started on the new world. Returns the new world's run name.
    pub async fn reset_world(self: &Arc<Self>, id: &str, seed: Option<String>) -> Result<String> {
        let inst = self.store.get(id)?;
        if self.settings().eula_accepted_at.is_none() {
            bail!("Accept the Minecraft EULA before starting a server.");
        }
        match self.supervisor.state(id) {
            ServerState::Preparing => bail!("\"{}\" is still getting ready; try again in a moment.", inst.name),
            s if s.has_process() => {
                self.supervisor.note(id, "Resetting the world…");
                self.supervisor.stop_and_wait(id, StopReason::Reset).await?;
            }
            _ => {}
        }

        let server_dir = self.paths().server_dir(id);
        let (run, seed) = new_world(&server_dir, seed.as_deref(), chrono::Local::now())?;
        self.store.modify(id, |i| i.current_world = Some(run.clone()))?;
        self.notify_instances_changed();
        self.supervisor.note(id, &format!("New world {run} (seed {seed})"));

        self.launch(id).await?;

        // Prune once the server is back up, so deleting never competes with the boot.
        let (app, id) = (self.clone(), id.to_string());
        tokio::spawn(async move {
            let up = app.supervisor.wait_for(&id, BACK_UP_TIMEOUT, |s| s == ServerState::Online).await;
            if up.is_ok() {
                app.prune_worlds(&id);
            }
        });
        Ok(run)
    }

    /// Deletes runs beyond the newest ten, keeping the current one (R19).
    pub fn prune_worlds(&self, id: &str) -> Vec<String> {
        let Ok(inst) = self.store.get(id) else { return Vec::new() };
        let deleted = prune(&self.paths().server_dir(id), KEEP_WORLDS, inst.current_world.as_deref());
        if !deleted.is_empty() {
            self.notify_instances_changed();
        }
        deleted
    }
}
