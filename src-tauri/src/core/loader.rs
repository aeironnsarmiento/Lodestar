//! Choosing the loader build (Forge, NeoForge or Fabric) a server installs: the
//! builds on offer, which of them the mods in the server's folder accept, and
//! reinstalling the server on another build.

use std::sync::Arc;

use anyhow::{bail, Result};
use serde::Serialize;

use super::app::App;
use super::instance::{Instance, Provision, ServerType};
use crate::addons::requirements::{self, LoaderRequirement};
use crate::providers::{loader_from_launch, LoaderTag, LoaderVersion};

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LoaderOption {
    pub id: String,
    pub tag: Option<LoaderTag>,
    /// Every mod in the server's folder accepts this build.
    pub compatible: bool,
    /// Indices into `LoaderChoices::requirements` of the mods that refuse it.
    pub rejected_by: Vec<usize>,
}

#[derive(Serialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct LoaderChoices {
    /// Newest first.
    pub versions: Vec<LoaderOption>,
    /// Mods that rule some builds out.
    pub requirements: Vec<LoaderRequirement>,
    /// What "Automatic" installs.
    pub automatic: Option<String>,
    /// The build in the server folder now.
    pub installed: Option<String>,
}

/// True when every requirement accepts `version`.
pub fn accepts(reqs: &[LoaderRequirement], version: &str) -> bool {
    reqs.iter().all(|r| requirements::range_allows(&r.range, version))
}

/// Builds in the order "Automatic" considers them: Forge's recommended, then its
/// latest promotion, then stable builds newest first, then betas.
fn preference(versions: &[LoaderVersion]) -> Vec<&LoaderVersion> {
    let rank = |v: &LoaderVersion| match v.tag {
        Some(LoaderTag::Recommended) => 0,
        Some(LoaderTag::Latest) => 1,
        None => 2,
        Some(LoaderTag::Beta) => 3,
    };
    let mut out: Vec<&LoaderVersion> = versions.iter().collect();
    // Stable sort keeps newest-first order within each rank.
    out.sort_by_key(|v| rank(v));
    out
}

/// The build "Automatic" installs: the first preferred build every requirement
/// accepts, or the preferred build when none fits them all.
pub fn choose(versions: &[LoaderVersion], reqs: &[LoaderRequirement]) -> Option<String> {
    let order = preference(versions);
    order
        .iter()
        .find(|v| accepts(reqs, &v.id))
        .or(order.first())
        .map(|v| v.id.clone())
}

/// The build an instance's launch files point at.
pub fn installed_loader(inst: &Instance) -> Option<String> {
    loader_from_launch(inst.server_type, &inst.mc_version, inst.launch.as_ref()?)
}

fn has_loader(t: ServerType) -> bool {
    matches!(t, ServerType::Fabric | ServerType::Forge | ServerType::Neoforge)
}

impl App {
    /// Loader requirements of the mods in the instance's folder.
    pub fn mod_requirements(&self, inst: &Instance) -> Vec<LoaderRequirement> {
        match crate::addons::folder(&self.paths().server_dir(&inst.id), inst.server_type) {
            Ok(dir) => requirements::scan(&dir, inst.server_type),
            Err(_) => Vec::new(),
        }
    }

    /// The builds on offer for a server type and Minecraft version, checked against
    /// the mods of instance `id` when one is given.
    pub async fn loader_choices(&self, server_type: ServerType, mc: &str, id: Option<&str>) -> Result<LoaderChoices> {
        let inst = id.map(|id| self.store.get(id)).transpose()?;
        let reqs = inst.as_ref().map(|i| self.mod_requirements(i)).unwrap_or_default();
        let list = self.providers.loader_versions(server_type, mc).await?;
        let automatic = choose(&list, &reqs);
        let versions = list
            .into_iter()
            .map(|v| {
                let rejected_by: Vec<usize> = (0..reqs.len()).filter(|&i| !requirements::range_allows(&reqs[i].range, &v.id)).collect();
                LoaderOption { compatible: rejected_by.is_empty(), rejected_by, id: v.id, tag: v.tag }
            })
            .collect();
        Ok(LoaderChoices {
            versions,
            requirements: reqs,
            automatic,
            installed: inst.as_ref().and_then(installed_loader),
        })
    }

    /// Pins a loader build (`None` = Automatic) and reinstalls the server software.
    pub fn set_loader_version(self: &Arc<Self>, id: &str, version: Option<String>) -> Result<()> {
        let inst = self.store.get(id)?;
        if !has_loader(inst.server_type) {
            bail!("{} servers have no loader version to choose.", inst.server_type.label());
        }
        if matches!(inst.provision, Provision::Running { .. }) {
            bail!("\"{}\" is already being set up.", inst.name);
        }
        if self.is_running(id) {
            bail!("Stop \"{}\" first.", inst.name);
        }
        let version = version.map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
        self.store.modify(id, |i| i.loader_version = version)?;
        self.spawn_provision(id);
        Ok(())
    }

    /// The build provisioning installs: the pinned one, or for Forge and NeoForge
    /// with mods already in the folder, one they all accept. `None` leaves the pick
    /// to the provider's own default.
    pub(crate) async fn loader_to_install(&self, inst: &Instance) -> Result<Option<String>> {
        if inst.loader_version.is_some() || !matches!(inst.server_type, ServerType::Forge | ServerType::Neoforge) {
            return Ok(inst.loader_version.clone());
        }
        let reqs = self.mod_requirements(inst);
        if reqs.is_empty() {
            return Ok(None);
        }
        let list = self.providers.loader_versions(inst.server_type, &inst.mc_version).await?;
        Ok(choose(&list, &reqs))
    }

    /// Console notes for mods that will refuse the installed build.
    pub(crate) fn note_loader_mismatch(&self, inst: &Instance) {
        let Some(installed) = installed_loader(inst) else { return };
        for r in self.mod_requirements(inst) {
            if !requirements::range_allows(&r.range, &installed) {
                self.supervisor.note(
                    &inst.id,
                    &format!(
                        "{} ({}) needs {} {}, but {installed} is installed. Pick another version under Settings → Server software.",
                        r.mod_name,
                        r.file_name,
                        inst.server_type.label(),
                        r.summary
                    ),
                );
            }
        }
    }
}
