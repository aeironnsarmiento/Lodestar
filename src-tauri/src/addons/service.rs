//! What the Mods tab and the mod browser call: list and manage a server's add-ons,
//! search Modrinth and CurseForge, install with required dependencies, check for
//! updates, and install modpacks during provisioning.

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, bail, Result};
use futures_util::{stream, StreamExt};
use serde::Serialize;

use super::http::{ProjectVersion, RemoteFile, SearchPage, SearchQuery};
use super::modpack::{self, PackFiles, PackInfo};
use super::{curseforge, modrinth};
use super::{
    current_path, folder, import_files, kind_for, list, load_manifest, modrinth_loaders, remove, save_manifest, set_enabled, AddonEntry,
    AddonMeta, AddonSource, ImportResult, Manifest, ProjectKind, DISABLED_SUFFIX,
};
use crate::core::app::App;
use crate::core::instance::{Instance, MissingFile, ModpackRef};
use crate::download::{file_hash, no_progress, Hash};

/// Parallel downloads for modpacks.
const PACK_CONCURRENCY: usize = 6;

/// A file that could not be downloaded automatically.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BlockedFile {
    pub title: String,
    pub file_name: String,
    pub page_url: String,
}

#[derive(Serialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct InstallResult {
    /// Titles of what was installed, the chosen project first.
    pub installed: Vec<String>,
    pub blocked: Vec<BlockedFile>,
    /// Dependencies with no compatible version.
    pub notes: Vec<String>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AddonUpdate {
    pub file_name: String,
    pub version_id: String,
    pub version_number: String,
}

fn sha1_of(path: &Path) -> Option<String> {
    file_hash(path, &Hash::Sha1(String::new())).ok()
}

impl App {
    fn curseforge_key(&self) -> String {
        self.settings().curseforge_api_key
    }

    fn client(&self) -> &reqwest::Client {
        self.providers.downloader.client()
    }

    fn modrinth_base(&self) -> &str {
        &self.providers.endpoints.modrinth
    }

    fn curseforge_base(&self) -> &str {
        &self.providers.endpoints.curseforge
    }

    fn addon_dirs(&self, id: &str) -> Result<(Instance, PathBuf, PathBuf)> {
        let inst = self.store.get(id)?;
        let dir = folder(&self.paths().server_dir(id), inst.server_type)?;
        Ok((inst, dir, self.paths().instance_dir(id)))
    }

    pub fn list_addons(&self, id: &str) -> Result<Vec<AddonEntry>> {
        let (_, dir, inst_dir) = self.addon_dirs(id)?;
        list(&dir, &inst_dir)
    }

    pub fn set_addon_enabled(&self, id: &str, file_name: &str, enabled: bool) -> Result<()> {
        let (_, dir, _) = self.addon_dirs(id)?;
        set_enabled(&dir, file_name, enabled)
    }

    pub fn remove_addon(&self, id: &str, file_name: &str) -> Result<()> {
        let (_, dir, inst_dir) = self.addon_dirs(id)?;
        remove(&dir, &inst_dir, file_name)
    }

    pub fn import_addons(&self, id: &str, paths: &[PathBuf]) -> Result<ImportResult> {
        let (_, dir, inst_dir) = self.addon_dirs(id)?;
        import_files(&dir, &inst_dir, paths)
    }

    /// Looks up files of unknown origin on Modrinth by their SHA-1 so they show a
    /// proper name, icon and version, and can be updated.
    pub async fn identify_addons(&self, id: &str) -> Result<Vec<AddonEntry>> {
        let (_, dir, inst_dir) = self.addon_dirs(id)?;
        let entries = list(&dir, &inst_dir)?;
        let mut manifest = load_manifest(&inst_dir);
        let mut wanted: Vec<(String, String)> = Vec::new();
        for e in entries.iter().filter(|e| e.meta.source.is_none() && !e.meta.unidentified) {
            let Some(path) = current_path(&dir, &e.file_name) else { continue };
            let sha1 = e.meta.sha1.clone().or_else(|| sha1_of(&path));
            if let Some(h) = sha1 {
                wanted.push((e.file_name.clone(), h));
            }
        }
        if wanted.is_empty() {
            return Ok(entries);
        }
        let hashes: Vec<String> = wanted.iter().map(|(_, h)| h.clone()).collect();
        let found = modrinth::by_hashes(self.client(), self.modrinth_base(), &hashes).await?;
        let project_ids: Vec<String> = found.values().map(|v| v.project_id.clone()).collect::<HashSet<_>>().into_iter().collect();
        let projects = modrinth::projects(self.client(), self.modrinth_base(), &project_ids).await.unwrap_or_default();
        for (file_name, hash) in wanted {
            let meta = manifest.entry(file_name).or_default();
            meta.sha1 = Some(hash.clone());
            match found.get(&hash) {
                Some(v) => {
                    let project = projects.iter().find(|p| p.id == v.project_id);
                    meta.source = Some(AddonSource::Modrinth);
                    meta.project_id = Some(v.project_id.clone());
                    meta.version_id = Some(v.id.clone());
                    meta.version_number = Some(v.version_number.clone());
                    meta.title = project.map(|p| p.title.clone());
                    meta.icon_url = project.and_then(|p| p.icon_url.clone());
                    meta.page_url = project.map(|p| p.page_url());
                }
                None => meta.unidentified = true,
            }
        }
        save_manifest(&inst_dir, &manifest)?;
        list(&dir, &inst_dir)
    }

    pub async fn search_projects(&self, source: AddonSource, kind: ProjectKind, query: SearchQuery) -> Result<SearchPage> {
        match source {
            AddonSource::Modrinth => modrinth::search(self.client(), self.modrinth_base(), kind, &query).await,
            AddonSource::Curseforge => {
                curseforge::search(self.client(), self.curseforge_base(), &self.curseforge_key(), kind, &query).await
            }
        }
    }

    /// Versions of a project. Loader and game version filters are optional so the
    /// browser can show everything for modpacks.
    pub async fn project_versions(
        &self,
        source: AddonSource,
        project_id: &str,
        loaders: &[String],
        game_version: Option<&str>,
    ) -> Result<Vec<ProjectVersion>> {
        match source {
            AddonSource::Modrinth => {
                let loaders: Vec<&str> = loaders.iter().map(String::as_str).collect();
                let versions = modrinth::versions(self.client(), self.modrinth_base(), project_id, &loaders, game_version).await?;
                Ok(versions.iter().map(|v| v.to_project_version()).collect())
            }
            AddonSource::Curseforge => {
                let loader = loaders.iter().find(|l| curseforge::loader_type(l).is_some()).map(String::as_str);
                let files =
                    curseforge::files(self.client(), self.curseforge_base(), &self.curseforge_key(), project_id, game_version, loader).await?;
                Ok(files.iter().map(|f| f.to_project_version()).collect())
            }
        }
    }

    /// Installs a project into a server, plus the dependencies it requires. Without a
    /// version, the newest one compatible with the server is used. A project that is
    /// already installed is replaced (this is how updates happen).
    pub async fn install_project(
        &self,
        id: &str,
        source: AddonSource,
        project_id: &str,
        version_id: Option<&str>,
    ) -> Result<InstallResult> {
        let (inst, dir, inst_dir) = self.addon_dirs(id)?;
        std::fs::create_dir_all(&dir)?;
        let loaders = modrinth_loaders(inst.server_type);
        let mut manifest = load_manifest(&inst_dir);
        let mut result = InstallResult::default();
        let mut seen: HashSet<(AddonSource, String)> = HashSet::new();
        let mut queue: VecDeque<(AddonSource, String, Option<String>, bool)> = VecDeque::new();
        queue.push_back((source, project_id.to_string(), version_id.map(String::from), false));

        while let Some((source, project, version, is_dependency)) = queue.pop_front() {
            if !seen.insert((source, project.clone())) {
                continue;
            }
            let already = manifest
                .iter()
                .any(|(_, m)| m.source == Some(source) && m.project_id.as_deref() == Some(project.as_str()));
            if is_dependency && already {
                continue;
            }
            let resolved = match source {
                AddonSource::Modrinth => self.resolve_modrinth(&project, version.as_deref(), loaders, &inst.mc_version).await,
                AddonSource::Curseforge => self.resolve_curseforge(&project, version.as_deref(), loaders, &inst.mc_version).await,
            };
            let (file, meta, deps) = match resolved {
                Ok(Some(r)) => r,
                Ok(None) if is_dependency => {
                    result.notes.push(format!("A required dependency ({project}) has no version for this server."));
                    continue;
                }
                Ok(None) => bail!(
                    "This project has no version for {} {}.",
                    inst.server_type.label(),
                    inst.mc_version
                ),
                Err(e) if is_dependency => {
                    result.notes.push(format!("Could not install a required dependency: {e:#}"));
                    continue;
                }
                Err(e) => return Err(e),
            };
            let title = meta.title.clone().unwrap_or_else(|| file.file_name.clone());
            let Some(url) = file.url.clone() else {
                result.blocked.push(BlockedFile {
                    title,
                    file_name: file.file_name.clone(),
                    page_url: meta.page_url.clone().unwrap_or_default(),
                });
                continue;
            };
            super::check_file_name(&file.file_name)?;

            // Replace earlier files of the same project, keeping a disabled one disabled.
            let mut was_disabled = false;
            let old: Vec<String> = manifest
                .iter()
                .filter(|(_, m)| m.source == Some(source) && m.project_id.as_deref() == Some(project.as_str()))
                .map(|(k, _)| k.clone())
                .collect();
            let dest = dir.join(&file.file_name);
            let hash = file.sha1.clone().map(Hash::Sha1);
            self.providers.downloader.download(&url, &dest, hash.as_ref(), &no_progress).await?;
            for name in old {
                if dir.join(format!("{name}{DISABLED_SUFFIX}")).exists() {
                    was_disabled = true;
                }
                if name != file.file_name {
                    if let Some(p) = current_path(&dir, &name) {
                        std::fs::remove_file(p).ok();
                    }
                    manifest.remove(&name);
                }
            }
            let stale_disabled = dir.join(format!("{}{DISABLED_SUFFIX}", file.file_name));
            if stale_disabled.exists() {
                std::fs::remove_file(&stale_disabled).ok();
            }
            if was_disabled {
                set_enabled(&dir, &file.file_name, false).ok();
            }
            manifest.insert(file.file_name.clone(), AddonMeta { sha1: file.sha1.clone(), ..meta });
            save_manifest(&inst_dir, &manifest)?;
            result.installed.push(title);
            for (dep_project, dep_version) in deps {
                queue.push_back((source, dep_project, dep_version, true));
            }
        }
        Ok(result)
    }

    async fn resolve_modrinth(
        &self,
        project: &str,
        version: Option<&str>,
        loaders: &[&str],
        mc: &str,
    ) -> Result<Option<(RemoteFile, AddonMeta, Vec<(String, Option<String>)>)>> {
        let chosen = match version {
            Some(v) => modrinth::version(self.client(), self.modrinth_base(), v).await?,
            None => {
                let all = modrinth::versions(self.client(), self.modrinth_base(), project, loaders, Some(mc)).await?;
                match modrinth::pick_best(&all) {
                    Some(v) => v.clone(),
                    None => return Ok(None),
                }
            }
        };
        let Some(file) = chosen.remote_file() else { return Ok(None) };
        let info = modrinth::projects(self.client(), self.modrinth_base(), std::slice::from_ref(&chosen.project_id)).await?;
        let p = info.iter().find(|p| p.id == chosen.project_id);
        let meta = AddonMeta {
            source: Some(AddonSource::Modrinth),
            project_id: Some(chosen.project_id.clone()),
            version_id: Some(chosen.id.clone()),
            title: p.map(|p| p.title.clone()),
            version_number: Some(chosen.version_number.clone()),
            icon_url: p.and_then(|p| p.icon_url.clone()),
            page_url: p.map(|p| p.page_url()),
            ..Default::default()
        };
        Ok(Some((file, meta, chosen.required_projects())))
    }

    async fn resolve_curseforge(
        &self,
        project: &str,
        version: Option<&str>,
        loaders: &[&str],
        mc: &str,
    ) -> Result<Option<(RemoteFile, AddonMeta, Vec<(String, Option<String>)>)>> {
        let key = self.curseforge_key();
        let loader = loaders.iter().copied().find(|l| curseforge::loader_type(l).is_some());
        let chosen = match version {
            Some(v) => curseforge::file(self.client(), self.curseforge_base(), &key, project, v).await?,
            None => {
                let all = curseforge::files(self.client(), self.curseforge_base(), &key, project, Some(mc), loader).await?;
                match curseforge::pick_best(&all) {
                    Some(f) => f.clone(),
                    None => return Ok(None),
                }
            }
        };
        let id: u64 = project.parse().map_err(|_| anyhow!("\"{project}\" is not a CurseForge project id."))?;
        let info = curseforge::mods_by_id(self.client(), self.curseforge_base(), &key, &[id]).await?;
        let m = info.iter().find(|m| m.id == id);
        let meta = AddonMeta {
            source: Some(AddonSource::Curseforge),
            project_id: Some(project.to_string()),
            version_id: Some(chosen.id.to_string()),
            title: m.map(|m| m.name.clone()),
            version_number: Some(chosen.display_name.clone()),
            icon_url: m.and_then(|m| m.icon()),
            page_url: m.map(|m| m.page_url()),
            ..Default::default()
        };
        let deps = chosen.required_mods().into_iter().map(|d| (d.to_string(), None)).collect();
        Ok(Some((chosen.remote_file(), meta, deps)))
    }

    /// Newer compatible versions of installed add-ons. CurseForge files are checked
    /// only when an API key is set.
    pub async fn check_addon_updates(&self, id: &str) -> Result<Vec<AddonUpdate>> {
        let (inst, dir, inst_dir) = self.addon_dirs(id)?;
        let entries = list(&dir, &inst_dir)?;
        let loaders = modrinth_loaders(inst.server_type);
        let mut updates = Vec::new();

        let modrinth_files: Vec<(&AddonEntry, String)> = entries
            .iter()
            .filter(|e| e.meta.source == Some(AddonSource::Modrinth) && !e.meta.from_modpack)
            .filter_map(|e| {
                let hash = e.meta.sha1.clone().or_else(|| current_path(&dir, &e.file_name).and_then(|p| sha1_of(&p)))?;
                Some((e, hash))
            })
            .collect();
        let hashes: Vec<String> = modrinth_files.iter().map(|(_, h)| h.clone()).collect();
        let newest = modrinth::updates(self.client(), self.modrinth_base(), &hashes, loaders, &inst.mc_version).await?;
        for (entry, hash) in &modrinth_files {
            if let Some(v) = newest.get(hash) {
                if Some(&v.id) != entry.meta.version_id.as_ref() {
                    updates.push(AddonUpdate {
                        file_name: entry.file_name.clone(),
                        version_id: v.id.clone(),
                        version_number: v.version_number.clone(),
                    });
                }
            }
        }

        let key = self.curseforge_key();
        if !key.trim().is_empty() {
            let loader = loaders.iter().copied().find(|l| curseforge::loader_type(l).is_some());
            for entry in entries.iter().filter(|e| e.meta.source == Some(AddonSource::Curseforge) && !e.meta.from_modpack) {
                let Some(project) = entry.meta.project_id.as_deref() else { continue };
                let Ok(files) =
                    curseforge::files(self.client(), self.curseforge_base(), &key, project, Some(&inst.mc_version), loader).await
                else {
                    continue;
                };
                if let Some(best) = curseforge::pick_best(&files) {
                    if Some(best.id.to_string()) != entry.meta.version_id {
                        updates.push(AddonUpdate {
                            file_name: entry.file_name.clone(),
                            version_id: best.id.to_string(),
                            version_number: best.display_name.clone(),
                        });
                    }
                }
            }
        }
        Ok(updates)
    }

    /// Updates one add-on to a specific version of its project.
    pub async fn update_addon(&self, id: &str, file_name: &str, version_id: &str) -> Result<InstallResult> {
        let (_, _, inst_dir) = self.addon_dirs(id)?;
        let manifest = load_manifest(&inst_dir);
        let meta = manifest.get(file_name).ok_or_else(|| anyhow!("{file_name} is not known; open the Mods tab again."))?;
        let (Some(source), Some(project)) = (meta.source, meta.project_id.clone()) else {
            bail!("{file_name} was added by hand, so Lodestar cannot update it.");
        };
        self.install_project(id, source, &project, Some(version_id)).await
    }

    /// Reads a modpack file picked on this PC, for the New server dialog.
    pub fn inspect_modpack_file(&self, path: &Path) -> Result<PackInfo> {
        Ok(modpack::read(path)?.info)
    }

    fn pack_path(&self, id: &str) -> PathBuf {
        self.paths().instance_dir(id).join("modpack").join("pack.zip")
    }

    /// Fetches the pack file and sets the server's Minecraft version and loader from it.
    pub(crate) async fn prepare_modpack(&self, id: &str) -> Result<()> {
        let inst = self.store.get(id)?;
        let pack = inst.modpack.clone().ok_or_else(|| anyhow!("This server has no modpack."))?;
        self.step(id, &format!("Downloading the {} modpack", pack.title))?;
        let dest = self.pack_path(id);
        std::fs::create_dir_all(dest.parent().unwrap())?;
        let progress = self.progress(id, &format!("Downloading the {} modpack", pack.title));
        if let Some(local) = pack.file.as_deref().filter(|f| !f.is_empty()) {
            let local = Path::new(local);
            if local != dest {
                std::fs::copy(local, &dest).map_err(|e| anyhow!("Could not read the modpack file {}: {e}", local.display()))?;
            }
        } else {
            let (Some(source), Some(project), Some(version)) = (pack.source, pack.project_id.as_deref(), pack.version_id.as_deref()) else {
                bail!("The modpack has no download to install from.");
            };
            let file = match source {
                AddonSource::Modrinth => modrinth::version(self.client(), self.modrinth_base(), version)
                    .await?
                    .remote_file()
                    .ok_or_else(|| anyhow!("That modpack version has no file."))?,
                AddonSource::Curseforge => {
                    curseforge::file(self.client(), self.curseforge_base(), &self.curseforge_key(), project, version).await?.remote_file()
                }
            };
            let url = file.url.ok_or_else(|| {
                anyhow!("The author does not let apps download this modpack. Download the zip from its CurseForge page, then use New server → From a modpack → Import file.")
            })?;
            let hash = file.sha1.map(Hash::Sha1);
            self.providers.downloader.download(&url, &dest, hash.as_ref(), &progress).await?;
        }
        let info = modpack::read(&dest)?.info;
        self.store.modify(id, |i| {
            i.server_type = info.server_type;
            i.mc_version = info.mc_version.clone();
            i.loader_version = info.loader_version.clone();
            if let Some(p) = i.modpack.as_mut() {
                if p.version_number.is_none() {
                    p.version_number = Some(info.version.clone()).filter(|v| !v.is_empty());
                }
            }
        })?;
        self.notify_instances_changed();
        Ok(())
    }

    /// Puts the pack's server files and overrides in the server folder, replacing the
    /// files an earlier version of the pack installed.
    pub(crate) async fn install_modpack_files(&self, id: &str) -> Result<()> {
        let inst = self.store.get(id)?;
        let server_dir = self.paths().server_dir(id);
        let inst_dir = self.paths().instance_dir(id);
        let pack = modpack::read(&self.pack_path(id))?;
        let title = inst.modpack.as_ref().map(|p| p.title.clone()).unwrap_or_default();

        // Drop what the previous pack version installed.
        let mut manifest: Manifest = load_manifest(&inst_dir);
        let addon_dir = folder(&server_dir, inst.server_type).ok();
        let old: Vec<String> = manifest.iter().filter(|(_, m)| m.from_modpack).map(|(k, _)| k.clone()).collect();
        for name in old {
            if let Some(d) = &addon_dir {
                if let Some(p) = current_path(d, &name) {
                    std::fs::remove_file(p).ok();
                }
            }
            manifest.remove(&name);
        }

        let mut downloads: Vec<(PathBuf, RemoteFile)> = Vec::new();
        let mut missing: Vec<MissingFile> = Vec::new();
        match &pack.files {
            PackFiles::Modrinth(files) => {
                for f in files.iter().filter(|f| f.for_server()) {
                    let rel = super::safe_relative(&f.path)?;
                    let file_name = rel.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    downloads.push((
                        rel,
                        RemoteFile { url: f.downloads.first().cloned(), file_name, sha1: Some(f.hashes.sha1.clone()) },
                    ));
                }
            }
            PackFiles::Curseforge { files, .. } => {
                let key = self.curseforge_key();
                let ids: Vec<u64> = files.iter().filter(|f| f.required).map(|f| f.file_id).collect();
                let resolved = curseforge::files_by_id(self.client(), self.curseforge_base(), &key, &ids).await?;
                let blocked_mods: Vec<u64> = resolved.iter().filter(|f| f.download_url.is_none()).map(|f| f.mod_id).collect();
                let mods = curseforge::mods_by_id(self.client(), self.curseforge_base(), &key, &blocked_mods).await.unwrap_or_default();
                for f in resolved {
                    let remote = f.remote_file();
                    if remote.url.is_none() {
                        let m = mods.iter().find(|m| m.id == f.mod_id);
                        missing.push(MissingFile {
                            file_name: f.file_name.clone(),
                            title: m.map(|m| m.name.clone()).unwrap_or_else(|| f.display_name.clone()),
                            page_url: m.map(|m| m.page_url()).unwrap_or_default(),
                        });
                        continue;
                    }
                    downloads.push((PathBuf::from("mods").join(&f.file_name), remote));
                }
            }
        }

        let total = downloads.len();
        self.step(id, &format!("Installing {title} (0 of {total} files)"))?;
        let done = std::sync::atomic::AtomicUsize::new(0);
        let (done, title_ref) = (&done, &title);
        let mut jobs = Vec::with_capacity(total);
        for (rel, file) in &downloads {
            let dest = server_dir.join(rel);
            let url = file.url.clone();
            let name = file.file_name.clone();
            let hash = file.sha1.clone().map(Hash::Sha1);
            jobs.push(async move {
                let url = url.ok_or_else(|| anyhow!("{name} has no download link"))?;
                self.providers.downloader.download(&url, &dest, hash.as_ref(), &no_progress).await?;
                let n = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                if n % 5 == 0 || n == total {
                    let _ = self.step(id, &format!("Installing {title_ref} ({n} of {total} files)"));
                }
                Ok::<(), anyhow::Error>(())
            });
        }
        let results: Vec<Result<()>> = stream::iter(jobs).buffer_unordered(PACK_CONCURRENCY).collect().await;
        if let Some(Err(e)) = results.into_iter().find(|r| r.is_err()) {
            return Err(e);
        }

        self.step(id, &format!("Copying {title} settings"))?;
        let mut copied = Vec::new();
        for folder_name in pack.override_folders() {
            copied.extend(modpack::extract_folder(&self.pack_path(id), &folder_name, &server_dir)?);
        }

        // Remember which add-on files belong to the pack.
        let addon_prefix = inst.server_type.addons_folder().map(|f| format!("{f}/"));
        if let Some(prefix) = addon_prefix {
            let mut record = |rel: &str, sha1: Option<String>| {
                let rel = rel.replace('\\', "/");
                if let Some(name) = rel.strip_prefix(&prefix).filter(|n| !n.contains('/')) {
                    let name = name.strip_suffix(DISABLED_SUFFIX).unwrap_or(name).to_string();
                    manifest.insert(name, AddonMeta { from_modpack: true, sha1, ..Default::default() });
                }
            };
            for (rel, file) in &downloads {
                record(&rel.to_string_lossy(), file.sha1.clone());
            }
            for rel in &copied {
                record(rel, None);
            }
        }
        save_manifest(&inst_dir, &manifest)?;

        if !missing.is_empty() {
            let names: Vec<&str> = missing.iter().map(|m| m.title.as_str()).collect();
            self.supervisor.note(
                id,
                &format!("Download these modpack files by hand and drop them into the Mods tab: {}", names.join(", ")),
            );
        }
        self.store.modify(id, |i| {
            if let Some(p) = i.modpack.as_mut() {
                p.installed = true;
                p.missing = missing;
            }
        })?;
        Ok(())
    }

    /// Switches a modpack server to another version of its pack and reinstalls.
    pub fn update_modpack(self: &Arc<Self>, id: &str, version_id: &str, version_number: Option<String>) -> Result<()> {
        let inst = self.store.get(id)?;
        if self.is_running(id) {
            bail!("Stop \"{}\" before changing its modpack version.", inst.name);
        }
        let Some(pack) = inst.modpack.as_ref() else { bail!("\"{}\" was not made from a modpack.", inst.name) };
        if pack.source.is_none() || pack.project_id.is_none() {
            bail!("This modpack came from a file, so Lodestar cannot fetch other versions of it.");
        }
        self.store.modify(id, |i| {
            if let Some(p) = i.modpack.as_mut() {
                *p = ModpackRef {
                    version_id: Some(version_id.to_string()),
                    version_number: version_number.clone(),
                    file: None,
                    installed: false,
                    missing: Vec::new(),
                    ..p.clone()
                };
            }
        })?;
        self.retry_provision(id)
    }
}

/// Which kind of project a server takes, for the browser.
pub fn kind_of(inst: &Instance) -> Option<ProjectKind> {
    kind_for(inst.server_type)
}
