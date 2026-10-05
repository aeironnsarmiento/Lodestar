//! Worlds: one folder per run under `server/worlds/`, each with its `seed.txt`
//! (KTD7). The server is started with `--universe worlds/<run>` and
//! `level-name=world`, so switching or resetting never moves files under a running
//! server, and old runs can be deleted in the background.
//!
//! `--universe` support: the vanilla dedicated server parses `--universe` (world
//! container) itself; Fabric's launcher and the Forge/NeoForge bootstraps pass
//! unrecognised arguments through to it; Paper accepts `--universe` (alias `-W`,
//! `--world-container`). So one mechanism covers every supported type.

pub mod properties;
pub mod reset;

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Local, NaiveDateTime, TimeZone};
use serde::Serialize;

/// Worlds kept per instance (R19). Fixed in v1 (KTD13).
pub const KEEP_WORLDS: usize = 10;

const PREFIX: &str = "run_";
const STAMP: &str = "%Y-%m-%d_%H-%M-%S";

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorldInfo {
    pub name: String,
    pub seed: Option<String>,
    pub created_at: Option<String>,
    pub size_bytes: u64,
    pub current: bool,
}

pub fn worlds_dir(server_dir: &Path) -> PathBuf {
    server_dir.join("worlds")
}

/// The `--universe` argument for a run, relative to the server folder.
pub fn universe_arg(run: &str) -> String {
    format!("worlds/{run}")
}

/// A random 64-bit seed, like Minecraft picks for an empty seed field.
pub fn random_seed() -> String {
    rand::random::<i64>().to_string()
}

/// Ordering key: timestamp, then the `_N` suffix of resets within the same second.
fn sort_key(name: &str) -> Option<(NaiveDateTime, u32)> {
    let rest = name.strip_prefix(PREFIX)?;
    let (stamp, n) = match rest.get(19..) {
        Some("") => (rest, 1),
        Some(suffix) if suffix.starts_with('_') => (&rest[..19], suffix[1..].parse().ok()?),
        _ => return None,
    };
    Some((NaiveDateTime::parse_from_str(stamp, STAMP).ok()?, n))
}

/// Creates `worlds/run_<timestamp>[_N]/seed.txt`. An empty or missing seed gets a
/// random one. Returns the run name and the seed.
pub fn new_world(server_dir: &Path, seed: Option<&str>, now: DateTime<Local>) -> Result<(String, String)> {
    let seed = match seed.map(str::trim) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => random_seed(),
    };
    let dir = worlds_dir(server_dir);
    fs::create_dir_all(&dir)?;
    let base = format!("{PREFIX}{}", now.format(STAMP));
    let mut name = base.clone();
    let mut n = 2;
    while dir.join(&name).exists() {
        name = format!("{base}_{n}");
        n += 1;
    }
    let path = dir.join(&name);
    fs::create_dir(&path).with_context(|| format!("creating {}", path.display()))?;
    fs::write(path.join("seed.txt"), &seed)?;
    Ok((name, seed))
}

pub fn world_seed(server_dir: &Path, run: &str) -> Option<String> {
    let s = fs::read_to_string(worlds_dir(server_dir).join(run).join("seed.txt")).ok()?;
    let s = s.trim().to_string();
    (!s.is_empty()).then_some(s)
}

pub fn world_exists(server_dir: &Path, run: &str) -> bool {
    sort_key(run).is_some() && worlds_dir(server_dir).join(run).is_dir()
}

fn dir_size(path: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(path) else { return 0 };
    entries
        .flatten()
        .map(|e| match e.file_type() {
            Ok(t) if t.is_dir() => dir_size(&e.path()),
            Ok(_) => e.metadata().map(|m| m.len()).unwrap_or(0),
            Err(_) => 0,
        })
        .sum()
}

/// Run names, newest first.
fn run_names(server_dir: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(worlds_dir(server_dir)) else { return Vec::new() };
    let mut names: Vec<(String, (NaiveDateTime, u32))> = entries
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            sort_key(&name).map(|k| (name, k))
        })
        .collect();
    names.sort_by_key(|n| std::cmp::Reverse(n.1));
    names.into_iter().map(|(n, _)| n).collect()
}

/// Kept worlds, newest first.
pub fn list_worlds(server_dir: &Path, current: Option<&str>) -> Vec<WorldInfo> {
    run_names(server_dir)
        .into_iter()
        .map(|name| {
            let created_at = sort_key(&name)
                .and_then(|(t, _)| Local.from_local_datetime(&t).single())
                .map(|t| t.to_rfc3339());
            WorldInfo {
                seed: world_seed(server_dir, &name),
                size_bytes: dir_size(&worlds_dir(server_dir).join(&name)),
                current: current == Some(name.as_str()),
                created_at,
                name,
            }
        })
        .collect()
}

/// Deletes runs beyond the newest `keep`. The current world is never deleted, even
/// when it is the oldest. Returns the deleted run names.
pub fn prune(server_dir: &Path, keep: usize, current: Option<&str>) -> Vec<String> {
    let mut deleted = Vec::new();
    for name in run_names(server_dir).into_iter().skip(keep) {
        if current == Some(name.as_str()) {
            continue;
        }
        let path = worlds_dir(server_dir).join(&name);
        match fs::remove_dir_all(&path) {
            Ok(()) => deleted.push(name),
            // A file still held open (e.g. by an antivirus scan) is retried next time.
            Err(e) => eprintln!("glasscraft: could not delete {}: {e}", path.display()),
        }
    }
    deleted
}

/// Validates a run name coming from the UI.
pub fn check_run_name(server_dir: &Path, run: &str) -> Result<()> {
    if !world_exists(server_dir, run) {
        bail!("There is no kept world named \"{run}\".");
    }
    Ok(())
}
