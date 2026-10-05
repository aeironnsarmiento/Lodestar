//! Fabric speed mods from Modrinth (R24, KTD17).

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::download::{Downloader, Hash, ProgressFn};

/// Vanilla-parity performance mods installed on Fabric instances.
pub const SPEED_MODS: [&str; 2] = ["lithium", "ferrite-core"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModFile {
    pub filename: String,
    pub url: String,
    pub sha1: String,
}

#[derive(Deserialize)]
struct Version {
    version_type: String,
    files: Vec<File>,
}

#[derive(Deserialize)]
struct File {
    filename: String,
    url: String,
    primary: bool,
    hashes: Hashes,
}

#[derive(Deserialize)]
struct Hashes {
    sha1: String,
}

/// Picks the newest release build (or newest of any kind) and its primary file.
/// `None` when the project has no build for the version yet.
pub fn pick(text: &str) -> Result<Option<ModFile>> {
    let versions: Vec<Version> = serde_json::from_str(text).context("Modrinth's response has an unexpected format")?;
    let chosen = versions
        .iter()
        .find(|v| v.version_type == "release")
        .or_else(|| versions.first());
    Ok(chosen.and_then(|v| {
        let f = v.files.iter().find(|f| f.primary).or_else(|| v.files.first())?;
        Some(ModFile { filename: f.filename.clone(), url: f.url.clone(), sha1: f.hashes.sha1.clone() })
    }))
}

pub fn versions_url(base: &str, slug: &str, mc: &str) -> String {
    format!("{base}/project/{slug}/version?loaders=%5B%22fabric%22%5D&game_versions=%5B%22{mc}%22%5D")
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "result", rename_all = "camelCase")]
pub enum ModOutcome {
    Installed { slug: String, filename: String },
    Skipped { slug: String, reason: String },
}

/// Removes previously managed mod files, then installs the speed mods for `mc`.
/// A mod with no build for the version is skipped, not an error. Returns the
/// outcomes; the managed list is every `Installed` filename.
pub async fn install_speed_mods(
    downloader: &Downloader,
    base: &str,
    mc: &str,
    mods_dir: &Path,
    previously_managed: &[String],
    progress: ProgressFn<'_>,
) -> Result<Vec<ModOutcome>> {
    fs::create_dir_all(mods_dir)?;
    for f in previously_managed {
        let p = mods_dir.join(f);
        if p.exists() {
            fs::remove_file(&p).ok();
        }
    }
    let mut out = Vec::new();
    for slug in SPEED_MODS {
        let outcome = match downloader.get_text(&versions_url(base, slug, mc)).await.and_then(|t| pick(&t)) {
            Ok(Some(file)) => {
                let dest = mods_dir.join(&file.filename);
                match downloader.download(&file.url, &dest, Some(&Hash::Sha1(file.sha1.clone())), progress).await {
                    Ok(_) => ModOutcome::Installed { slug: slug.into(), filename: file.filename },
                    Err(e) => ModOutcome::Skipped { slug: slug.into(), reason: format!("{e:#}") },
                }
            }
            Ok(None) => ModOutcome::Skipped { slug: slug.into(), reason: format!("no build for {mc} yet") },
            Err(e) => ModOutcome::Skipped { slug: slug.into(), reason: format!("{e:#}") },
        };
        out.push(outcome);
    }
    Ok(out)
}
