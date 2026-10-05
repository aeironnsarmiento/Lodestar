//! Mojang's `piston-meta` version manifest and per-version metadata.

use anyhow::{Context, Result};
use serde::Deserialize;

use super::{VersionEntry, VersionKind};

#[derive(Deserialize, Clone, Debug)]
pub struct Manifest {
    pub latest: Latest,
    pub versions: Vec<ManifestVersion>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Latest {
    pub release: String,
    pub snapshot: String,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ManifestVersion {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub url: String,
    pub release_time: String,
}

impl Manifest {
    pub fn find(&self, id: &str) -> Option<&ManifestVersion> {
        self.versions.iter().find(|v| v.id == id).or_else(|| {
            // Paper names pre-releases `1.21.11-rc3`; Mojang uses `1.21.11-rc-3`.
            let normalized = normalize(id);
            self.versions.iter().find(|v| normalize(&v.id) == normalized)
        })
    }

    /// Fills in release dates for lists that come from other providers.
    pub fn annotate_release_times(&self, list: &mut [VersionEntry]) {
        for entry in list.iter_mut() {
            if entry.release_time.is_none() {
                entry.release_time = self.find(&entry.id).map(|v| v.release_time.clone());
            }
        }
    }
}

fn normalize(id: &str) -> String {
    id.to_ascii_lowercase().replace('-', "")
}

pub fn parse_manifest(text: &str) -> Result<Manifest> {
    serde_json::from_str(text).context("Mojang's version manifest has an unexpected format")
}

/// Releases and snapshots, newest first. Pre-1.0 alpha and beta versions have no
/// dedicated server worth running and are left out.
pub fn versions(manifest: &Manifest) -> Vec<VersionEntry> {
    let mut list: Vec<&ManifestVersion> = manifest
        .versions
        .iter()
        .filter(|v| v.kind == "release" || v.kind == "snapshot")
        .collect();
    list.sort_by(|a, b| b.release_time.cmp(&a.release_time));
    list.into_iter()
        .map(|v| VersionEntry {
            id: v.id.clone(),
            kind: if v.kind == "release" { VersionKind::Release } else { VersionKind::Snapshot },
            release_time: Some(v.release_time.clone()),
        })
        .collect()
}

#[derive(Clone, Debug)]
pub struct ServerDownload {
    pub url: String,
    pub sha1: String,
}

#[derive(Clone, Debug)]
pub struct VersionDetails {
    pub java_major: u32,
    pub server: Option<ServerDownload>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawDetails {
    java_version: Option<RawJava>,
    downloads: Option<RawDownloads>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawJava {
    major_version: u32,
}

#[derive(Deserialize)]
struct RawDownloads {
    server: Option<RawFile>,
}

#[derive(Deserialize)]
struct RawFile {
    url: String,
    sha1: String,
}

/// Per-version JSON. Versions without `javaVersion` predate it and run on Java 8.
pub fn parse_version_details(text: &str) -> Result<VersionDetails> {
    let raw: RawDetails = serde_json::from_str(text).context("Mojang's version details have an unexpected format")?;
    Ok(VersionDetails {
        java_major: raw.java_version.map(|j| j.major_version).unwrap_or(8),
        server: raw.downloads.and_then(|d| d.server).map(|s| ServerDownload { url: s.url, sha1: s.sha1 }),
    })
}
