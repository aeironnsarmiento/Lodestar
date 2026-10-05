//! PaperMC Fill v3 API.

use std::collections::BTreeMap;

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;

use super::{compare_versions, is_prerelease, VersionEntry, VersionKind};

#[derive(Deserialize)]
struct Project {
    versions: BTreeMap<String, Vec<String>>,
}

/// All Paper versions, newest first; pre-releases are tagged as snapshots.
pub fn parse_project(text: &str) -> Result<Vec<VersionEntry>> {
    let project: Project = serde_json::from_str(text).context("Paper's version list has an unexpected format")?;
    let mut ids: Vec<String> = project.versions.into_values().flatten().collect();
    ids.sort_by(|a, b| compare_versions(b, a));
    ids.dedup();
    Ok(ids
        .into_iter()
        .map(|id| VersionEntry {
            kind: if is_prerelease(&id) { VersionKind::Snapshot } else { VersionKind::Release },
            id,
            release_time: None,
        })
        .collect())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaperBuild {
    pub name: String,
    pub url: String,
    pub sha256: String,
}

#[derive(Deserialize)]
struct Build {
    downloads: BTreeMap<String, Download>,
}

#[derive(Deserialize)]
struct Download {
    name: String,
    url: String,
    checksums: Checksums,
}

#[derive(Deserialize)]
struct Checksums {
    sha256: String,
}

/// The `server:default` download of a build.
pub fn parse_build(text: &str) -> Result<PaperBuild> {
    let build: Build = serde_json::from_str(text).context("Paper's build info has an unexpected format")?;
    let d = build
        .downloads
        .get("server:default")
        .ok_or_else(|| anyhow!("Paper's latest build has no server download"))?;
    Ok(PaperBuild {
        name: d.name.clone(),
        url: d.url.clone(),
        sha256: d.checksums.sha256.clone(),
    })
}
