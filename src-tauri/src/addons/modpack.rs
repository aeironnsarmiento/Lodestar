//! Modpack files: Modrinth `.mrpack` (`modrinth.index.json`) and CurseForge zips
//! (`manifest.json`). Reading one tells us which Minecraft version and loader the
//! server needs; installing it downloads the server-side files and copies the
//! overrides (configs, scripts) into the server folder.

use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};

use super::{safe_relative, AddonSource};
use crate::core::instance::ServerType;

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PackInfo {
    pub format: AddonSource,
    pub name: String,
    pub version: String,
    pub mc_version: String,
    pub server_type: ServerType,
    pub loader_version: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct MrHashes {
    pub sha1: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct MrEnv {
    #[serde(default)]
    pub server: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct MrFile {
    pub path: String,
    pub hashes: MrHashes,
    pub env: Option<MrEnv>,
    #[serde(default)]
    pub downloads: Vec<String>,
}

impl MrFile {
    /// Files marked client-only are left out of a server.
    pub fn for_server(&self) -> bool {
        self.env.as_ref().map(|e| e.server != "unsupported").unwrap_or(true)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MrIndex {
    name: String,
    #[serde(default)]
    version_id: String,
    files: Vec<MrFile>,
    #[serde(default)]
    dependencies: HashMap<String, String>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct CfLoader {
    pub id: String,
    #[serde(default)]
    pub primary: bool,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CfMinecraft {
    pub version: String,
    #[serde(default)]
    pub mod_loaders: Vec<CfLoader>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct CfFile {
    #[serde(rename = "projectID")]
    pub project_id: u64,
    #[serde(rename = "fileID")]
    pub file_id: u64,
    #[serde(default = "yes")]
    pub required: bool,
}

fn yes() -> bool {
    true
}

#[derive(Deserialize)]
struct CfManifest {
    minecraft: CfMinecraft,
    #[serde(default)]
    name: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    files: Vec<CfFile>,
    overrides: Option<String>,
}

pub enum PackFiles {
    Modrinth(Vec<MrFile>),
    Curseforge { files: Vec<CfFile>, overrides: String },
}

pub struct Pack {
    pub info: PackInfo,
    pub files: PackFiles,
}

impl Pack {
    /// Zip folders whose contents are copied into the server folder, in order.
    pub fn override_folders(&self) -> Vec<String> {
        match &self.files {
            PackFiles::Modrinth(_) => vec!["overrides".into(), "server-overrides".into()],
            PackFiles::Curseforge { overrides, .. } => vec![overrides.clone()],
        }
    }
}

fn read_entry(archive: &mut zip::ZipArchive<fs::File>, name: &str) -> Option<String> {
    let mut entry = archive.by_name(name).ok()?;
    let mut text = String::new();
    entry.read_to_string(&mut text).ok()?;
    Some(text)
}

/// Whether a zip is a modpack (rather than, say, a zipped mod).
pub fn looks_like_modpack(path: &Path) -> bool {
    let Ok(file) = fs::File::open(path) else { return false };
    let Ok(mut archive) = zip::ZipArchive::new(file) else { return false };
    archive.by_name("modrinth.index.json").is_ok() || read_entry(&mut archive, "manifest.json").is_some_and(|t| t.contains("\"minecraft\""))
}

fn loader_from_modrinth(deps: &HashMap<String, String>) -> Result<(ServerType, Option<String>)> {
    if deps.contains_key("quilt-loader") {
        bail!("Quilt modpacks are not supported yet. Look for a Fabric version of the pack.");
    }
    for (key, ty) in [("fabric-loader", ServerType::Fabric), ("neoforge", ServerType::Neoforge), ("forge", ServerType::Forge)] {
        if let Some(v) = deps.get(key) {
            return Ok((ty, Some(v.clone())));
        }
    }
    Ok((ServerType::Vanilla, None))
}

/// `forge-47.2.0` → Forge 47.2.0; `neoforge-21.1.77` → NeoForge 21.1.77.
pub fn loader_from_curseforge(loaders: &[CfLoader]) -> Result<(ServerType, Option<String>)> {
    let Some(primary) = loaders.iter().find(|l| l.primary).or_else(|| loaders.first()) else {
        return Ok((ServerType::Vanilla, None));
    };
    let (name, version) = primary.id.split_once('-').unwrap_or((primary.id.as_str(), ""));
    let ty = match name {
        "forge" => ServerType::Forge,
        "neoforge" => ServerType::Neoforge,
        "fabric" => ServerType::Fabric,
        "quilt" => bail!("Quilt modpacks are not supported yet. Look for a Fabric version of the pack."),
        other => bail!("The modpack uses an unknown loader ({other})."),
    };
    Ok((ty, (!version.is_empty()).then(|| version.to_string())))
}

/// Reads a modpack file's index.
pub fn read(path: &Path) -> Result<Pack> {
    let file = fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut archive = zip::ZipArchive::new(file).context("The modpack file is not a valid zip")?;
    if let Some(text) = read_entry(&mut archive, "modrinth.index.json") {
        let index: MrIndex = serde_json::from_str(&text).context("The modpack's modrinth.index.json is not valid")?;
        let mc = index
            .dependencies
            .get("minecraft")
            .cloned()
            .ok_or_else(|| anyhow!("The modpack does not say which Minecraft version it needs."))?;
        let (server_type, loader_version) = loader_from_modrinth(&index.dependencies)?;
        return Ok(Pack {
            info: PackInfo {
                format: AddonSource::Modrinth,
                name: index.name,
                version: index.version_id,
                mc_version: mc,
                server_type,
                loader_version,
            },
            files: PackFiles::Modrinth(index.files),
        });
    }
    if let Some(text) = read_entry(&mut archive, "manifest.json") {
        let m: CfManifest = serde_json::from_str(&text).context("The modpack's manifest.json is not valid")?;
        let (server_type, loader_version) = loader_from_curseforge(&m.minecraft.mod_loaders)?;
        return Ok(Pack {
            info: PackInfo {
                format: AddonSource::Curseforge,
                name: m.name,
                version: m.version,
                mc_version: m.minecraft.version,
                server_type,
                loader_version,
            },
            files: PackFiles::Curseforge { files: m.files, overrides: m.overrides.unwrap_or_else(|| "overrides".into()) },
        });
    }
    bail!("This is not a modpack: it has no modrinth.index.json or manifest.json.")
}

/// Copies every file under `folder/` in the zip into `dest`, keeping the layout.
/// Returns the relative paths written.
pub fn extract_folder(zip_path: &Path, folder: &str, dest: &Path) -> Result<Vec<String>> {
    let file = fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let prefix = format!("{}/", folder.trim_end_matches('/'));
    let mut written = Vec::new();
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let name = entry.name().replace('\\', "/");
        let Some(rel) = name.strip_prefix(&prefix) else { continue };
        if rel.is_empty() || entry.is_dir() {
            continue;
        }
        let target = dest.join(safe_relative(rel)?);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut out = fs::File::create(&target).with_context(|| format!("writing {}", target.display()))?;
        std::io::copy(&mut entry, &mut out)?;
        written.push(rel.to_string());
    }
    Ok(written)
}
