//! Which Forge or NeoForge builds the mods in a server's folder accept, read from
//! each jar's `mods.toml` (`[[dependencies.<mod>]]` entries on `forge`/`neoforge`).

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use serde::Serialize;

use crate::core::instance::ServerType;
use crate::providers::compare_versions;

/// One mod's requirement on the loader.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LoaderRequirement {
    pub file_name: String,
    pub mod_name: String,
    /// Maven version range as the mod wrote it, e.g. `[47.4.20,)`.
    pub range: String,
    /// The range in words: "47.4.20 or newer".
    pub summary: String,
}

/// Requirements of every enabled jar in `mods_dir`, by file name. Jars that cannot
/// be read, or that accept any build, are left out.
pub fn scan(mods_dir: &Path, server_type: ServerType) -> Vec<LoaderRequirement> {
    let (loader, files) = match server_type {
        ServerType::Forge => ("forge", &["META-INF/mods.toml"][..]),
        ServerType::Neoforge => ("neoforge", &["META-INF/neoforge.mods.toml", "META-INF/mods.toml"][..]),
        _ => return Vec::new(),
    };
    let Ok(entries) = std::fs::read_dir(mods_dir) else { return Vec::new() };
    let mut jars: Vec<_> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("jar")))
        .collect();
    jars.sort();
    let mut out = Vec::new();
    for jar in jars {
        let file_name = jar.file_name().unwrap_or_default().to_string_lossy().into_owned();
        let Some(text) = read_entry(&jar, files) else { continue };
        for (mod_name, range) in parse_mods_toml(&text, loader) {
            if is_bounded(&range) {
                let summary = summarize(&range);
                out.push(LoaderRequirement { file_name: file_name.clone(), mod_name, range, summary });
            }
        }
    }
    out
}

fn read_entry(jar: &Path, names: &[&str]) -> Option<String> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(jar).ok()?).ok()?;
    for name in names {
        if let Ok(mut f) = zip.by_name(name) {
            let mut text = String::new();
            if f.read_to_string(&mut text).is_ok() {
                return Some(text);
            }
        }
    }
    None
}

#[derive(Default)]
struct Block {
    section: String,
    keys: HashMap<String, String>,
}

/// `(mod display name, version range)` for each required dependency on `loader`.
///
/// A small line reader rather than a TOML parser: Forge reads these files leniently,
/// and plenty of published jars would fail a strict parser (duplicate keys and the like).
pub fn parse_mods_toml(text: &str, loader: &str) -> Vec<(String, String)> {
    let mut blocks = vec![Block::default()];
    let mut multiline: Option<&str> = None;
    for line in text.lines() {
        if let Some(delim) = multiline {
            if line.contains(delim) {
                multiline = None;
            }
            continue;
        }
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        if t.starts_with('[') {
            let section = t.trim_start_matches('[').split(']').next().unwrap_or("").trim().to_string();
            blocks.push(Block { section, keys: HashMap::new() });
            continue;
        }
        let Some((key, value)) = t.split_once('=') else { continue };
        let value = value.trim();
        for delim in ["'''", "\"\"\""] {
            if value.starts_with(delim) && value.matches(delim).count() == 1 {
                multiline = Some(delim);
            }
        }
        if multiline.is_some() {
            continue;
        }
        let key = key.trim().trim_matches('"').to_string();
        blocks.last_mut().unwrap().keys.insert(key, unquote(value));
    }

    let names: HashMap<&str, &str> = blocks
        .iter()
        .filter(|b| b.section == "mods")
        .filter_map(|b| Some((b.keys.get("modId")?.as_str(), b.keys.get("displayName").map_or("", |s| s.as_str()))))
        .collect();
    let first_name = blocks
        .iter()
        .find(|b| b.section == "mods")
        .and_then(|b| b.keys.get("displayName").or_else(|| b.keys.get("modId")))
        .cloned()
        .unwrap_or_default();

    blocks
        .iter()
        .filter_map(|b| {
            let owner = b.section.strip_prefix("dependencies.")?.trim().trim_matches('"');
            if !b.keys.get("modId").is_some_and(|m| m == loader) {
                return None;
            }
            let required = match (b.keys.get("mandatory"), b.keys.get("type")) {
                (Some(m), _) => m == "true",
                (None, Some(t)) => t.eq_ignore_ascii_case("required"),
                (None, None) => true,
            };
            if !required {
                return None;
            }
            let range = b.keys.get("versionRange")?.clone();
            let name = match names.get(owner) {
                Some(n) if !n.is_empty() => n.to_string(),
                Some(_) => owner.to_string(),
                None if !first_name.is_empty() => first_name.clone(),
                None => owner.to_string(),
            };
            Some((strip_formatting(&name), range))
        })
        .collect()
}

/// Drops Minecraft `§` colour and style codes: `§6§lTizio§r` → `Tizio`.
fn strip_formatting(name: &str) -> String {
    let mut out = String::new();
    let mut chars = name.chars();
    while let Some(c) = chars.next() {
        if c == '§' {
            chars.next();
        } else {
            out.push(c);
        }
    }
    out.trim().to_string()
}

fn unquote(value: &str) -> String {
    let v = value.trim();
    for q in ['"', '\''] {
        if let Some(rest) = v.strip_prefix(q) {
            return rest.split(q).next().unwrap_or("").to_string();
        }
    }
    v.split('#').next().unwrap_or("").trim().to_string()
}

/// A range end: the version and whether it is included.
type Bound = Option<(String, bool)>;

/// `[low,high)`-style restrictions; a bare version is Maven's "soft" requirement
/// and accepts anything.
fn restrictions(range: &str) -> Option<Vec<(Bound, Bound)>> {
    let mut out = Vec::new();
    let mut rest = range.trim();
    if !(rest.starts_with('[') || rest.starts_with('(')) {
        return None;
    }
    while !rest.is_empty() {
        let open = rest.chars().next()?;
        if open != '[' && open != '(' {
            return None;
        }
        let end = rest.find([']', ')'])?;
        let close = rest[end..].chars().next()?;
        let inner = &rest[1..end];
        let (lo_inc, hi_inc) = (open == '[', close == ']');
        let bound = |s: &str, inc: bool| {
            let s = s.trim();
            (!s.is_empty()).then(|| (s.to_string(), inc))
        };
        match inner.split_once(',') {
            Some((lo, hi)) => out.push((bound(lo, lo_inc), bound(hi, hi_inc))),
            None => {
                let exact = bound(inner, true);
                out.push((exact.clone(), exact));
            }
        }
        rest = rest[end + 1..].trim_start().trim_start_matches(',').trim_start();
    }
    Some(out)
}

/// True when `version` is inside a Maven range. Unreadable ranges accept anything.
pub fn range_allows(range: &str, version: &str) -> bool {
    use std::cmp::Ordering::*;
    let Some(rs) = restrictions(range) else { return true };
    rs.iter().any(|(lo, hi)| {
        let lo_ok = lo.as_ref().is_none_or(|(v, inc)| match compare_versions(version, v) {
            Greater => true,
            Equal => *inc,
            Less => false,
        });
        let hi_ok = hi.as_ref().is_none_or(|(v, inc)| match compare_versions(version, v) {
            Less => true,
            Equal => *inc,
            Greater => false,
        });
        lo_ok && hi_ok
    })
}

/// A range that rules some builds out (not `[0,)`, `*` or a bare version).
fn is_bounded(range: &str) -> bool {
    restrictions(range).is_some_and(|rs| {
        !rs.iter().any(|(lo, hi)| hi.is_none() && lo.as_ref().is_none_or(|(v, _)| crate::providers::version_key(v).iter().all(|n| *n == 0)))
    })
}

/// The range in words, for the UI.
pub fn summarize(range: &str) -> String {
    let Some(rs) = restrictions(range) else { return range.to_string() };
    let parts: Vec<String> = rs
        .iter()
        .map(|(lo, hi)| match (lo, hi) {
            (Some((a, _)), Some((b, _))) if a == b => format!("exactly {a}"),
            (Some((a, true)), None) => format!("{a} or newer"),
            (Some((a, false)), None) => format!("newer than {a}"),
            (None, Some((b, true))) => format!("{b} or older"),
            (None, Some((b, false))) => format!("older than {b}"),
            (Some((a, _)), Some((b, true))) => format!("{a} to {b}"),
            (Some((a, _)), Some((b, false))) => format!("{a} up to (not including) {b}"),
            (None, None) => "any version".to_string(),
        })
        .collect();
    parts.join(" or ")
}
