//! `server.properties` merge writer: keeps the user's own keys, comments and order,
//! and sets the keys Glasscraft manages.

use std::fs;
use std::path::Path;

use anyhow::Result;

use crate::core::instance::Instance;

/// Keys Glasscraft owns, with values from the instance and the world seed.
pub fn managed_properties(inst: &Instance, seed: &str) -> Vec<(&'static str, String)> {
    vec![
        ("server-port", inst.port.to_string()),
        ("level-name", "world".into()),
        ("level-seed", seed.to_string()),
        ("gamemode", inst.game_mode.as_str().into()),
        ("difficulty", inst.difficulty.as_str().into()),
        ("hardcore", inst.hardcore.to_string()),
        ("max-players", inst.max_players.to_string()),
        ("view-distance", inst.view_distance.to_string()),
        ("simulation-distance", inst.simulation_distance.to_string()),
        ("motd", inst.motd.clone()),
        ("online-mode", inst.online_mode.to_string()),
    ]
}

/// Defaults that make a fresh speedrun server friendlier; written only when the key
/// is missing, so the user can change them in the file.
const FIRST_RUN_DEFAULTS: [(&str, &str); 4] = [
    ("spawn-protection", "0"),
    ("allow-flight", "true"),
    ("sync-chunk-writes", "false"),
    ("enforce-secure-profile", "false"),
];

/// Escapes characters that `java.util.Properties` treats specially in values.
fn escape_value(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    for c in v.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            c if (c as u32) > 0x7e => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn key_of(line: &str) -> Option<&str> {
    let t = line.trim_start();
    if t.is_empty() || t.starts_with('#') || t.starts_with('!') {
        return None;
    }
    let end = t.find(['=', ':']).unwrap_or(t.len());
    Some(t[..end].trim())
}

/// Merges `set` into existing properties text. Existing managed keys are replaced in
/// place; unknown keys and comments are kept; new keys are appended.
pub fn merge(existing: &str, set: &[(&str, String)], defaults: &[(&str, &str)]) -> String {
    let mut lines: Vec<String> = existing.lines().map(str::to_string).collect();
    let mut seen = vec![false; set.len()];
    for line in lines.iter_mut() {
        if let Some(key) = key_of(line) {
            if let Some(i) = set.iter().position(|(k, _)| *k == key) {
                *line = format!("{}={}", set[i].0, escape_value(&set[i].1));
                seen[i] = true;
            }
        }
    }
    let present: Vec<String> = lines.iter().filter_map(|l| key_of(l).map(str::to_string)).collect();
    if lines.is_empty() {
        lines.push("#Minecraft server properties - managed by Glasscraft (change settings in the app)".into());
    }
    for (i, (k, v)) in set.iter().enumerate() {
        if !seen[i] {
            lines.push(format!("{k}={}", escape_value(v)));
        }
    }
    for (k, v) in defaults {
        if !present.iter().any(|p| p == k) && !set.iter().any(|(s, _)| s == k) {
            lines.push(format!("{k}={v}"));
        }
    }
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

/// Writes `server.properties` for the next start.
pub fn write_server_properties(server_dir: &Path, inst: &Instance, seed: &str) -> Result<()> {
    let path = server_dir.join("server.properties");
    let existing = fs::read_to_string(&path).unwrap_or_default();
    let merged = merge(&existing, &managed_properties(inst, seed), &FIRST_RUN_DEFAULTS);
    fs::create_dir_all(server_dir)?;
    fs::write(path, merged)?;
    Ok(())
}

/// Reads one value (tests and diagnostics).
pub fn read_value(server_dir: &Path, key: &str) -> Option<String> {
    let text = fs::read_to_string(server_dir.join("server.properties")).ok()?;
    text.lines().find_map(|l| {
        (key_of(l) == Some(key)).then(|| l.split_once('=').map(|(_, v)| v.to_string()).unwrap_or_default())
    })
}
