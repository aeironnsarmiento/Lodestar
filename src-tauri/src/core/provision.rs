//! Provisioning: everything a new instance needs before its first launch (F1) —
//! the Java requirement, a managed Java runtime, the server software and, for Fabric,
//! the speed mods. Runs in the background; progress shows on the server card.

use std::sync::Arc;

use anyhow::{bail, Result};

use super::app::App;
use super::instance::{Instance, NewInstance, Provision, ServerType};
use crate::providers::modrinth::{self, ModOutcome};

impl App {
    /// Creates the record and starts provisioning it in the background.
    pub fn create_and_provision(self: &Arc<Self>, new: NewInstance) -> Result<Instance> {
        let inst = self.create_instance(new)?;
        self.spawn_provision(&inst.id);
        Ok(inst)
    }

    /// Retries provisioning after a failure (or finishes an interrupted one).
    pub fn retry_provision(self: &Arc<Self>, id: &str) -> Result<()> {
        let inst = self.store.get(id)?;
        if matches!(inst.provision, Provision::Running { .. }) {
            bail!("\"{}\" is already being set up.", inst.name);
        }
        if self.is_running(id) {
            bail!("Stop \"{}\" first.", inst.name);
        }
        self.spawn_provision(id);
        Ok(())
    }

    pub(crate) fn spawn_provision(self: &Arc<Self>, id: &str) {
        let (app, id) = (self.clone(), id.to_string());
        // Mark it running right away so the card never flashes "ready".
        let _ = self.set_provision(&id, Provision::Running { message: "Getting ready…".into() });
        tokio::spawn(async move {
            let result = app.provision(&id).await;
            let state = match result {
                Ok(()) => Provision::Ready,
                Err(e) => Provision::Failed { message: format!("{e:#}") },
            };
            let _ = app.set_provision(&id, state);
        });
    }

    fn set_provision(&self, id: &str, provision: Provision) -> Result<Instance> {
        let inst = self.store.modify(id, |i| i.provision = provision)?;
        self.notify_instances_changed();
        Ok(inst)
    }

    pub(crate) fn step(&self, id: &str, message: &str) -> Result<()> {
        self.set_provision(id, Provision::Running { message: message.into() }).map(|_| ())
    }

    /// Runs every provisioning step. Safe to repeat: downloads are cached and verified.
    pub async fn provision(&self, id: &str) -> Result<()> {
        // A modpack decides the Minecraft version and loader, so it is read first.
        if self.store.get(id)?.modpack.as_ref().is_some_and(|p| !p.installed) {
            self.prepare_modpack(id).await?;
        }
        let inst = self.store.get(id)?;
        let mc = inst.mc_version.clone();

        self.step(id, "Checking which Java this version needs")?;
        let required = self.providers.java_requirement(&mc).await?;
        let major = crate::java::target_major(required);
        self.store.modify(id, |i| i.java_major = Some(required))?;

        self.step(id, &format!("Downloading Java {major}"))?;
        let progress = self.progress(id, &format!("Downloading Java {major}"));
        let java = self.java.ensure(required, &progress).await?;

        self.step(id, &format!("Downloading {} {mc}", inst.server_type.label()))?;
        let server_dir = self.paths().server_dir(id);
        let progress = self.progress(id, &format!("Downloading {} {mc}", inst.server_type.label()));
        if matches!(inst.server_type, ServerType::Forge | ServerType::Neoforge) {
            self.step(id, &format!("Installing {} {mc} (this can take a minute)", inst.server_type.label()))?;
        }
        let loader = self.loader_to_install(&inst).await?;
        let launch = self
            .providers
            .install(inst.server_type, &mc, loader.as_deref(), &server_dir, Some(&java), &progress)
            .await?;
        self.store.modify(id, |i| i.launch = Some(launch))?;

        if inst.server_type == ServerType::Fabric && inst.speed_mods {
            let tag = format!("fabric-{mc}");
            if inst.managed_mods_for.as_deref() != Some(tag.as_str()) || inst.managed_mods.is_empty() {
                self.step(id, "Installing speed mods (Lithium, FerriteCore)")?;
                let progress = self.progress(id, "Installing speed mods");
                let outcomes = modrinth::install_speed_mods(
                    &self.providers.downloader,
                    &self.providers.endpoints.modrinth,
                    &mc,
                    &server_dir.join("mods"),
                    &inst.managed_mods,
                    &progress,
                )
                .await?;
                let installed: Vec<String> = outcomes
                    .iter()
                    .filter_map(|o| match o {
                        ModOutcome::Installed { filename, .. } => Some(filename.clone()),
                        ModOutcome::Skipped { .. } => None,
                    })
                    .collect();
                for o in &outcomes {
                    if let ModOutcome::Skipped { slug, reason } = o {
                        self.supervisor.note(id, &format!("Skipped {slug}: {reason}"));
                    }
                }
                self.store.modify(id, |i| {
                    i.managed_mods = installed;
                    i.managed_mods_for = Some(tag);
                })?;
            }
        }
        if self.store.get(id)?.modpack.as_ref().is_some_and(|p| !p.installed) {
            self.install_modpack_files(id).await?;
        }
        Ok(())
    }
}
