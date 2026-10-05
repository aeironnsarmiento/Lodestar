//! Minecraft Forge (1.17+): version lists, headless install and launch detection
//! (KTD8). The installer output is shared with NeoForge, which uses the same layout.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use regex::Regex;
use serde::Deserialize;

use super::{compare_versions, mc_at_least, VersionEntry, VersionKind, MODDED_MIN_MC};
use crate::core::instance::LaunchInfo;

#[derive(Deserialize)]
struct Promotions {
    promos: HashMap<String, String>,
}

/// `promos` from `promotions_slim.json`: `"<mc>-latest"` / `"<mc>-recommended"` → Forge version.
pub fn parse_promotions(text: &str) -> Result<HashMap<String, String>> {
    let p: Promotions = serde_json::from_str(text).context("Forge's promotions have an unexpected format")?;
    Ok(p.promos)
}

/// All `(mc_version, full_version)` pairs from `maven-metadata.xml` for MC ≥ 1.17.
/// Every entry is parsed because the file's `<latest>` tag points at an older line.
pub fn parse_maven_metadata(xml: &str) -> Vec<(String, String)> {
    let re = Regex::new(r"<version>\s*([^<\s]+)\s*</version>").unwrap();
    re.captures_iter(xml)
        .filter_map(|c| {
            let full = c[1].to_string();
            let (mc, _forge) = full.split_once('-')?;
            if mc.contains('_') || !mc_at_least(mc, MODDED_MIN_MC) {
                return None;
            }
            Some((mc.to_string(), full))
        })
        .collect()
}

/// Minecraft versions that have Forge builds, newest first.
pub fn mc_versions(meta: &[(String, String)]) -> Vec<VersionEntry> {
    let mut mcs: Vec<String> = meta.iter().map(|(mc, _)| mc.clone()).collect();
    mcs.sort_by(|a, b| compare_versions(b, a));
    mcs.dedup();
    mcs.into_iter()
        .map(|id| VersionEntry { id, kind: VersionKind::Release, release_time: None })
        .collect()
}

/// The Forge build to install: recommended, then latest promotion, then the
/// numerically newest build in the maven metadata.
pub fn pick_version(meta: &[(String, String)], promos: &HashMap<String, String>, mc: &str) -> Option<String> {
    for key in [format!("{mc}-recommended"), format!("{mc}-latest")] {
        if let Some(v) = promos.get(&key) {
            return Some(format!("{mc}-{v}"));
        }
    }
    let mut builds: Vec<&String> = meta.iter().filter(|(m, _)| m == mc).map(|(_, full)| full).collect();
    builds.sort_by(|a, b| {
        let fa = a.split_once('-').map(|x| x.1).unwrap_or(a);
        let fb = b.split_once('-').map(|x| x.1).unwrap_or(b);
        compare_versions(fb, fa)
    });
    builds.first().map(|s| s.to_string())
}

pub fn installer_url(maven_base: &str, full_version: &str) -> String {
    format!("{maven_base}/{full_version}/forge-{full_version}-installer.jar")
}

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// `java -jar <installer> --installServer <dir>` with no console window.
pub async fn run_installer(java: &Path, installer: &Path, server_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(server_dir)?;
    let mut cmd = tokio::process::Command::new(java);
    cmd.arg("-jar")
        .arg(installer)
        .arg("--installServer")
        .arg(server_dir)
        .current_dir(server_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    let child = cmd.spawn().with_context(|| format!("could not start {}", java.display()))?;
    let out = tokio::time::timeout(Duration::from_secs(15 * 60), child.wait_with_output())
        .await
        .map_err(|_| anyhow!("The installer took longer than 15 minutes and was stopped."))??;
    if !out.status.success() {
        let text = String::from_utf8_lossy(&out.stdout);
        let tail: Vec<&str> = text.lines().rev().take(8).collect();
        let tail: Vec<&str> = tail.into_iter().rev().collect();
        bail!("The installer failed ({}):\n{}", out.status, tail.join("\n"));
    }
    Ok(())
}

fn find_named(dir: &Path, name: &str, depth: usize, out: &mut Vec<PathBuf>) {
    if depth == 0 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            find_named(&p, name, depth - 1, out);
        } else if p.file_name().is_some_and(|n| n == name) {
            out.push(p);
        }
    }
}

/// Works out how to launch an installed Forge/NeoForge server: the generated
/// `win_args.txt` (used with `@user_jvm_args.txt`), or a shim jar when the installer
/// produced one instead. Never `run.bat`.
pub fn detect_launch(server_dir: &Path) -> Result<LaunchInfo> {
    let mut found = Vec::new();
    find_named(&server_dir.join("libraries"), "win_args.txt", 12, &mut found);
    // Prefer the loader's own args file if several exist.
    found.sort_by_key(|p| {
        let s = p.to_string_lossy().replace('\\', "/");
        !(s.contains("/net/minecraftforge/forge/") || s.contains("/net/neoforged/neoforge/"))
    });
    if let Some(p) = found.first() {
        let rel = p.strip_prefix(server_dir).unwrap_or(p);
        let rel = rel.to_string_lossy().replace('\\', "/");
        return Ok(LaunchInfo::ArgsFile { args_file: rel });
    }

    let mut jars: BTreeMap<String, PathBuf> = BTreeMap::new();
    if let Ok(entries) = std::fs::read_dir(server_dir) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.ends_with("-shim.jar") {
                jars.insert(name, e.path());
            }
        }
    }
    if let Some((_, jar)) = jars.into_iter().next_back() {
        return Ok(LaunchInfo::Jar { jar: jar.to_string_lossy().into_owned() });
    }
    bail!("The installer finished but left no launch files (win_args.txt or a shim jar) in {}", server_dir.display())
}
