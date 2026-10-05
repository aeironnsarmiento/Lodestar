//! Fabric meta API.

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;

use super::{VersionEntry, VersionKind};

#[derive(Deserialize)]
struct GameVersion {
    version: String,
    stable: bool,
}

/// Game versions Fabric supports, newest first (the API's order).
pub fn parse_game_versions(text: &str) -> Result<Vec<VersionEntry>> {
    let list: Vec<GameVersion> = serde_json::from_str(text).context("Fabric's version list has an unexpected format")?;
    Ok(list
        .into_iter()
        .map(|g| VersionEntry {
            id: g.version,
            kind: if g.stable { VersionKind::Release } else { VersionKind::Snapshot },
            release_time: None,
        })
        .collect())
}

#[derive(Deserialize)]
struct Component {
    version: String,
    stable: bool,
}

/// The newest stable loader or installer version.
pub fn pick_stable(text: &str) -> Result<String> {
    let list: Vec<Component> = serde_json::from_str(text).context("Fabric's metadata has an unexpected format")?;
    list.into_iter()
        .find(|c| c.stable)
        .map(|c| c.version)
        .ok_or_else(|| anyhow!("Fabric has no stable release listed"))
}
