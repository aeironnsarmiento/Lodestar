//! Mods and plugins: the server's `mods`/`plugins` folder, what Lodestar knows about
//! each file there, and installs from Modrinth and CurseForge (and modpacks).
//!
//! Each instance keeps `addons.json` next to `instance.json`: per file (by its enabled
//! name), where it came from and which version it is. Files dropped in by hand are
//! identified on Modrinth by hash when the Mods tab opens.

pub mod curseforge;
pub mod http;
pub mod modpack;
pub mod modrinth;
pub mod requirements;
pub mod service;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::core::instance::ServerType;
use crate::core::store::{read_json, write_json_atomic};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum AddonSource {
    Modrinth,
    Curseforge,
}

impl AddonSource {
    pub fn label(self) -> &'static str {
        match self {
            AddonSource::Modrinth => "Modrinth",
            AddonSource::Curseforge => "CurseForge",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProjectKind {
    Mod,
    Plugin,
    Modpack,
}

/// What Lodestar knows about one file in the mods or plugins folder.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct AddonMeta {
    pub source: Option<AddonSource>,
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub title: Option<String>,
    pub version_number: Option<String>,
    pub icon_url: Option<String>,
    pub page_url: Option<String>,
    /// Installed as part of the server's modpack.
    pub from_modpack: bool,
    /// Already looked up on Modrinth without a match; not asked again.
    pub unidentified: bool,
    pub sha1: Option<String>,
}

/// One file in the folder, as the Mods tab shows it.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AddonEntry {
    /// The enabled name (`foo.jar`), whether or not the file is disabled.
    pub file_name: String,
    pub enabled: bool,
    pub size: u64,
    #[serde(flatten)]
    pub meta: AddonMeta,
}

pub type Manifest = BTreeMap<String, AddonMeta>;

pub const DISABLED_SUFFIX: &str = ".disabled";

/// The folder a server type loads add-ons from, and what they are called.
pub fn kind_for(server_type: ServerType) -> Option<ProjectKind> {
    match server_type {
        ServerType::Vanilla => None,
        ServerType::Paper => Some(ProjectKind::Plugin),
        ServerType::Fabric | ServerType::Forge | ServerType::Neoforge => Some(ProjectKind::Mod),
    }
}

/// Modrinth loader names a server type can run.
pub fn modrinth_loaders(server_type: ServerType) -> &'static [&'static str] {
    match server_type {
        ServerType::Vanilla => &[],
        ServerType::Paper => &["paper", "spigot", "bukkit", "purpur"],
        ServerType::Fabric => &["fabric"],
        ServerType::Forge => &["forge"],
        ServerType::Neoforge => &["neoforge"],
    }
}

pub fn manifest_path(instance_dir: &Path) -> PathBuf {
    instance_dir.join("addons.json")
}

pub fn load_manifest(instance_dir: &Path) -> Manifest {
    let path = manifest_path(instance_dir);
    if path.exists() {
        read_json(&path).unwrap_or_default()
    } else {
        Manifest::new()
    }
}

pub fn save_manifest(instance_dir: &Path, manifest: &Manifest) -> Result<()> {
    write_json_atomic(&manifest_path(instance_dir), manifest)
}

fn is_addon_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".jar") || lower.ends_with(&format!(".jar{DISABLED_SUFFIX}"))
}

/// A bare file name (no folders, no `..`), as add-on names must be.
pub fn check_file_name(name: &str) -> Result<()> {
    let ok = !name.is_empty()
        && !name.contains(['/', '\\', ':'])
        && name != "."
        && name != ".."
        && Path::new(name).components().count() == 1;
    if ok {
        Ok(())
    } else {
        bail!("\"{name}\" is not a valid file name.")
    }
}

/// A relative path inside a modpack, checked so it cannot escape the server folder.
pub fn safe_relative(path: &str) -> Result<PathBuf> {
    let p = Path::new(path);
    if path.is_empty() || p.is_absolute() || path.contains(':') {
        bail!("The modpack has an unsafe path: {path}");
    }
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::Normal(part) => out.push(part),
            Component::CurDir => {}
            _ => bail!("The modpack has an unsafe path: {path}"),
        }
    }
    if out.as_os_str().is_empty() {
        bail!("The modpack has an empty path.");
    }
    Ok(out)
}

/// Lists the folder, newest manifest data attached. Manifest entries for files that
/// are gone are dropped (and the manifest saved) so it never grows stale.
pub fn list(dir: &Path, instance_dir: &Path) -> Result<Vec<AddonEntry>> {
    let mut manifest = load_manifest(instance_dir);
    let mut out = Vec::new();
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            if !is_addon_name(&name) {
                continue;
            }
            let (file_name, enabled) = match name.strip_suffix(DISABLED_SUFFIX) {
                Some(base) => (base.to_string(), false),
                None => (name.clone(), true),
            };
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            let meta = manifest.get(&file_name).cloned().unwrap_or_default();
            out.push(AddonEntry { file_name, enabled, size, meta });
        }
    }
    let before = manifest.len();
    manifest.retain(|k, _| out.iter().any(|e| &e.file_name == k));
    if manifest.len() != before {
        save_manifest(instance_dir, &manifest)?;
    }
    out.sort_by_key(|e| e.meta.title.clone().unwrap_or_else(|| e.file_name.clone()).to_lowercase());
    Ok(out)
}

/// The path a file currently has (enabled or disabled), if it exists.
pub fn current_path(dir: &Path, file_name: &str) -> Option<PathBuf> {
    let enabled = dir.join(file_name);
    if enabled.is_file() {
        return Some(enabled);
    }
    let disabled = dir.join(format!("{file_name}{DISABLED_SUFFIX}"));
    disabled.is_file().then_some(disabled)
}

/// Enables or disables a file by renaming it to or from `<name>.disabled`.
pub fn set_enabled(dir: &Path, file_name: &str, enabled: bool) -> Result<()> {
    check_file_name(file_name)?;
    let on = dir.join(file_name);
    let off = dir.join(format!("{file_name}{DISABLED_SUFFIX}"));
    let (from, to) = if enabled { (off, on) } else { (on, off) };
    if from.is_file() {
        fs::rename(&from, &to).with_context(|| format!("renaming {}", from.display()))?;
    } else if !to.is_file() {
        bail!("{file_name} is not in the folder any more.");
    }
    Ok(())
}

/// Deletes a file (either state) and forgets it.
pub fn remove(dir: &Path, instance_dir: &Path, file_name: &str) -> Result<()> {
    check_file_name(file_name)?;
    if let Some(path) = current_path(dir, file_name) {
        fs::remove_file(&path).with_context(|| format!("deleting {}", path.display()))?;
    }
    let mut manifest = load_manifest(instance_dir);
    if manifest.remove(file_name).is_some() {
        save_manifest(instance_dir, &manifest)?;
    }
    Ok(())
}

#[derive(Serialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub added: Vec<String>,
    pub skipped: Vec<String>,
}

/// Copies dropped or picked `.jar` files into the folder. Anything else is skipped
/// with a reason; modpacks are pointed at the New server flow.
pub fn import_files(dir: &Path, instance_dir: &Path, paths: &[PathBuf]) -> Result<ImportResult> {
    fs::create_dir_all(dir)?;
    let mut manifest = load_manifest(instance_dir);
    let mut result = ImportResult::default();
    for path in paths {
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let lower = name.to_ascii_lowercase();
        if lower.ends_with(".mrpack") || (lower.ends_with(".zip") && modpack::looks_like_modpack(path)) {
            result.skipped.push(format!("{name}: modpacks become their own server — use New server → From a modpack"));
            continue;
        }
        if !lower.ends_with(".jar") {
            result.skipped.push(format!("{name}: only .jar files go in this folder"));
            continue;
        }
        if !path.is_file() {
            result.skipped.push(format!("{name}: not a file"));
            continue;
        }
        // Replacing a disabled copy of the same file re-enables it.
        let disabled = dir.join(format!("{name}{DISABLED_SUFFIX}"));
        if disabled.exists() {
            fs::remove_file(&disabled).ok();
        }
        fs::copy(path, dir.join(&name)).with_context(|| format!("copying {name}"))?;
        manifest.insert(name.clone(), AddonMeta::default());
        result.added.push(name);
    }
    save_manifest(instance_dir, &manifest)?;
    Ok(result)
}

/// The add-on folder of an instance, or an error for vanilla.
pub fn folder(server_dir: &Path, server_type: ServerType) -> Result<PathBuf> {
    let name = server_type
        .addons_folder()
        .ok_or_else(|| anyhow!("Vanilla servers cannot load mods or plugins."))?;
    Ok(server_dir.join(name))
}
