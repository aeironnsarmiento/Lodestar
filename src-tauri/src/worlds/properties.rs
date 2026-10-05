//! `server.properties` merge writer: keeps the user's own keys, comments and order,
//! and sets the keys Lodestar manages.

use std::fs;
use std::path::Path;

use anyhow::Result;

use crate::core::instance::Instance;

/// Keys Lodestar owns, with values from the instance and the world seed.
pub fn managed_properties(inst: &Instance, seed: &str) -> Vec<(&'static str, String)> {
    let p = &inst.properties;
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
        ("pvp", p.pvp.to_string()),
        ("allow-nether", p.allow_nether.to_string()),
        ("generate-structures", p.generate_structures.to_string()),
        ("level-type", p.level_type.as_str().into()),
        ("spawn-protection", p.spawn_protection.to_string()),
        ("force-gamemode", p.force_gamemode.to_string()),
        ("enable-command-block", p.enable_command_block.to_string()),
        ("allow-flight", p.allow_flight.to_string()),
        ("player-idle-timeout", p.player_idle_timeout.to_string()),
        ("enforce-secure-profile", p.enforce_secure_profile.to_string()),
        ("hide-online-players", p.hide_online_players.to_string()),
        ("white-list", p.white_list.to_string()),
        ("enforce-whitelist", p.enforce_whitelist.to_string()),
        ("sync-chunk-writes", p.sync_chunk_writes.to_string()),
        ("entity-broadcast-range-percentage", p.entity_broadcast_range_percentage.to_string()),
        ("resource-pack", p.resource_pack.clone()),
        ("require-resource-pack", p.require_resource_pack.to_string()),
    ]
}

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
        lines.push("#Minecraft server properties - managed by Lodestar (change settings in the app)".into());
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
    let merged = merge(&existing, &managed_properties(inst, seed), &[]);
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

/// Removes players from a vanilla player list file (`whitelist.json`, `ops.json`) while
/// the server is stopped. Entries are matched by name, ignoring case; a missing or
/// unreadable file is left alone.
pub fn remove_from_player_file(path: &Path, names: &[String]) -> Result<()> {
    if names.is_empty() {
        return Ok(());
    }
    let Ok(text) = fs::read_to_string(path) else {
        return Ok(());
    };
    let Ok(serde_json::Value::Array(entries)) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Ok(());
    };
    let kept: Vec<serde_json::Value> = entries
        .into_iter()
        .filter(|e| {
            let name = e.get("name").and_then(|n| n.as_str()).unwrap_or_default();
            !names.iter().any(|r| r.eq_ignore_ascii_case(name))
        })
        .collect();
    fs::write(path, serde_json::to_string_pretty(&kept)?)?;
    Ok(())
}
