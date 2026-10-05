//! Launching and controlling servers: the glue between the store, Java, providers
//! and the supervisor.

use std::sync::Arc;

use anyhow::{anyhow, bail, Result};
use serde::Serialize;

use super::app::App;
use super::instance::{Instance, LaunchInfo, Provision};
use crate::providers::{launch_spec, LaunchSpec};
use crate::worlds::properties::write_server_properties;
use crate::supervisor::console::{log_path, ConsoleLine};
use crate::supervisor::ports::lan_ip;
use crate::supervisor::{Snapshot, StopReason};

/// The addresses friends can join with (R15).
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct JoinInfo {
    pub localhost: String,
    pub lan: Option<String>,
    pub public: Option<String>,
}

fn with_port(host: &str, port: u16) -> String {
    if port == 25565 {
        host.to_string()
    } else {
        format!("{host}:{port}")
    }
}

impl App {
    fn launch_info(inst: &Instance) -> Result<&LaunchInfo> {
        match &inst.provision {
            Provision::Ready => {}
            Provision::Pending | Provision::Running { .. } => bail!("\"{}\" is still being set up.", inst.name),
            Provision::Failed { message } => bail!("\"{}\" could not be set up: {message}", inst.name),
        }
        inst.launch.as_ref().ok_or_else(|| anyhow!("\"{}\" has no server files yet.", inst.name))
    }

    /// Starts an instance: claims its port (Preparing), makes Java and the world
    /// ready, then spawns the server.
    pub async fn launch(self: &Arc<Self>, id: &str) -> Result<()> {
        let inst = self.store.get(id)?;
        Self::launch_info(&inst)?;
        if self.settings().eula_accepted_at.is_none() {
            bail!("Accept the Minecraft EULA before starting a server.");
        }
        self.supervisor.prepare(id, &inst.name, inst.port)?;
        let spec = match self.prepare_launch(&inst).await {
            Ok(spec) => spec,
            Err(e) => {
                let msg = format!("{e:#}");
                self.supervisor.abort_prepare(id, Some(msg.clone()));
                return Err(anyhow!(msg));
            }
        };
        self.supervisor.start(id, &spec, &log_path(&self.paths().server_dir(id)))
    }

    /// Everything that must be ready before the process starts.
    async fn prepare_launch(&self, inst: &Instance) -> Result<LaunchSpec> {
        let launch = Self::launch_info(inst)?;
        let required = inst.java_major.ok_or_else(|| anyhow!("\"{}\" has no Java version recorded.", inst.name))?;
        let java = match &self.java_override {
            Some(program) => program.clone(),
            None => {
                let progress = self.progress(&inst.id, &format!("Downloading Java {}", crate::java::target_major(required)));
                self.java.ensure(required, &progress).await?
            }
        };
        let server_dir = self.paths().server_dir(&inst.id);
        // Only reached after the user accepted the EULA in the app (KTD15).
        std::fs::create_dir_all(&server_dir)?;
        std::fs::write(
            server_dir.join("eula.txt"),
            "# Accepted in Glasscraft: https://aka.ms/MinecraftEULA\neula=true\n",
        )?;
        let seed = inst.initial_seed.clone().unwrap_or_default();
        write_server_properties(&server_dir, inst, &seed)?;
        Ok(launch_spec(&java, &server_dir, launch, inst.ram_mb, &[]))
    }

    /// Records EULA acceptance app-wide (KTD15).
    pub fn accept_eula(&self) -> Result<super::settings::AppSettings> {
        self.store
            .modify_settings(|s| s.eula_accepted_at = Some(chrono::Local::now().to_rfc3339()))
    }

    /// The folder a modded server loads mods or plugins from (R25), created if needed.
    pub fn addons_folder(&self, id: &str) -> Result<std::path::PathBuf> {
        let inst = self.store.get(id)?;
        let folder = inst
            .server_type
            .addons_folder()
            .ok_or_else(|| anyhow!("Vanilla servers have no mods folder."))?;
        let dir = self.paths().server_dir(id).join(folder);
        std::fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    pub fn stop_server(self: &Arc<Self>, id: &str) -> Result<()> {
        self.supervisor.stop(id, StopReason::User)
    }

    pub fn kill_server(&self, id: &str) -> Result<()> {
        self.supervisor.kill(id)
    }

    pub async fn restart_server(self: &Arc<Self>, id: &str) -> Result<()> {
        self.supervisor.stop_and_wait(id, StopReason::Restart).await?;
        self.launch(id).await
    }

    pub fn send_command(&self, id: &str, command: &str) -> Result<()> {
        self.supervisor.send_command(id, command)
    }

    pub fn console(&self, id: &str) -> Vec<ConsoleLine> {
        self.supervisor.console(id)
    }

    pub fn snapshots(&self) -> Vec<Snapshot> {
        self.store.list().iter().map(|i| self.supervisor.snapshot(&i.id)).collect()
    }

    pub fn join_info(&self, id: &str) -> Result<JoinInfo> {
        let inst = self.store.get(id)?;
        Ok(JoinInfo {
            localhost: with_port("localhost", inst.port),
            lan: lan_ip().map(|ip| with_port(&ip, inst.port)),
            public: None,
        })
    }
}
