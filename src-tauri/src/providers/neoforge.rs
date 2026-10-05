//! NeoForge (1.20.2+ under this artifact; installed the same way as Forge).

use anyhow::{Context, Result};
use serde::Deserialize;

use super::{compare_versions, mc_at_least, VersionEntry, VersionKind, MODDED_MIN_MC};

#[derive(Deserialize)]
struct Versions {
    versions: Vec<String>,
}

pub fn parse_versions(text: &str) -> Result<Vec<String>> {
    let v: Versions = serde_json::from_str(text).context("NeoForge's version list has an unexpected format")?;
    Ok(v.versions)
}

/// The Minecraft version a NeoForge version targets.
///
/// - `21.1.219` → `1.21.1`; `20.2.3-beta` → `1.20.2`; `21.0.x` → `1.21`
/// - year-based: `26.3.0.48-beta` → `26.3`; `26.3.1.4` → `26.3.1`
///
/// Special builds (April Fools `0.25w14craftmine...`) map to nothing.
pub fn mc_for(neo: &str) -> Option<String> {
    let core = neo.split('-').next()?;
    let parts: Vec<u64> = core.split('.').map(|p| p.parse().ok()).collect::<Option<_>>()?;
    let (a, b) = (*parts.first()?, *parts.get(1)?);
    if a == 0 {
        return None;
    }
    if a >= 25 {
        // Year-based Minecraft versions: <year>.<drop>.<hotfix>.<build>
        let c = *parts.get(2)?;
        parts.get(3)?;
        return Some(if c == 0 { format!("{a}.{b}") } else { format!("{a}.{b}.{c}") });
    }
    Some(if b == 0 { format!("1.{a}") } else { format!("1.{a}.{b}") })
}

/// Minecraft versions with NeoForge builds, newest first.
pub fn mc_versions(all: &[String]) -> Vec<VersionEntry> {
    let mut mcs: Vec<String> = all
        .iter()
        .filter_map(|v| mc_for(v))
        .filter(|mc| mc_at_least(mc, MODDED_MIN_MC))
        .collect();
    mcs.sort_by(|a, b| compare_versions(b, a));
    mcs.dedup();
    mcs.into_iter()
        .map(|id| VersionEntry { id, kind: VersionKind::Release, release_time: None })
        .collect()
}

/// Newest stable build for the Minecraft version, or the newest beta when no stable
/// build exists yet.
pub fn pick_version(all: &[String], mc: &str) -> Option<String> {
    let mut matching: Vec<&String> = all.iter().filter(|v| mc_for(v).as_deref() == Some(mc)).collect();
    matching.sort_by(|a, b| compare_versions(b, a));
    matching
        .iter()
        .find(|v| !v.contains("beta"))
        .or_else(|| matching.first())
        .map(|v| v.to_string())
}

pub fn installer_url(maven_base: &str, version: &str) -> String {
    format!("{maven_base}/{version}/neoforge-{version}-installer.jar")
}
