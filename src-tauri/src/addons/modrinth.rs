//! Modrinth API v2: search, versions, hash lookups and update checks.

use std::collections::HashMap;

use anyhow::Result;
use serde::Deserialize;
use serde_json::json;

use super::http::{send_json, ProjectVersion, RemoteFile, SearchHit, SearchPage, SearchQuery};
use super::{AddonSource, ProjectKind};

const SITE: &str = "Modrinth";

#[derive(Deserialize, Clone, Debug)]
pub struct Hit {
    pub project_id: String,
    pub slug: String,
    pub title: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub description: String,
    pub icon_url: Option<String>,
    #[serde(default)]
    pub downloads: u64,
}

#[derive(Deserialize)]
struct SearchResponse {
    hits: Vec<Hit>,
    total_hits: u64,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Dependency {
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub dependency_type: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Hashes {
    pub sha1: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct File {
    pub url: String,
    pub filename: String,
    #[serde(default)]
    pub primary: bool,
    pub hashes: Hashes,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Version {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub version_number: String,
    #[serde(default)]
    pub game_versions: Vec<String>,
    #[serde(default)]
    pub loaders: Vec<String>,
    #[serde(default)]
    pub version_type: String,
    #[serde(default)]
    pub date_published: String,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
    pub files: Vec<File>,
}

impl Version {
    /// The file to download: the primary one, else the first.
    pub fn primary_file(&self) -> Option<&File> {
        self.files.iter().find(|f| f.primary).or_else(|| self.files.first())
    }

    pub fn remote_file(&self) -> Option<RemoteFile> {
        self.primary_file().map(|f| RemoteFile {
            url: Some(f.url.clone()),
            file_name: f.filename.clone(),
            sha1: Some(f.hashes.sha1.clone()),
        })
    }

    pub fn to_project_version(&self) -> ProjectVersion {
        ProjectVersion {
            source: AddonSource::Modrinth,
            project_id: self.project_id.clone(),
            version_id: self.id.clone(),
            name: self.name.clone(),
            version_number: self.version_number.clone(),
            game_versions: self.game_versions.clone(),
            loaders: self.loaders.clone(),
            channel: if self.version_type.is_empty() { "release".into() } else { self.version_type.clone() },
            published: self.date_published.clone(),
            file_name: self.primary_file().map(|f| f.filename.clone()).unwrap_or_default(),
        }
    }

    pub fn required_projects(&self) -> Vec<(String, Option<String>)> {
        self.dependencies
            .iter()
            .filter(|d| d.dependency_type == "required")
            .filter_map(|d| d.project_id.clone().map(|p| (p, d.version_id.clone())))
            .collect()
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct Project {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub icon_url: Option<String>,
    #[serde(default)]
    pub project_type: String,
}

impl Project {
    pub fn page_url(&self) -> String {
        page_url(&self.project_type, &self.slug)
    }
}

pub fn page_url(project_type: &str, slug: &str) -> String {
    let kind = if project_type.is_empty() { "mod" } else { project_type };
    format!("https://modrinth.com/{kind}/{slug}")
}

/// The `facets` search parameter for a kind of project and the filters.
pub fn facets(kind: ProjectKind, q: &SearchQuery) -> String {
    let mut groups: Vec<Vec<String>> = Vec::new();
    let loader_group = |names: &[String]| names.iter().map(|l| format!("categories:{l}")).collect::<Vec<_>>();
    match kind {
        ProjectKind::Mod => {
            groups.push(vec!["project_type:mod".into()]);
            groups.push(vec!["server_side:required".into(), "server_side:optional".into()]);
        }
        ProjectKind::Modpack => {
            groups.push(vec!["project_type:modpack".into()]);
            groups.push(vec!["server_side:required".into(), "server_side:optional".into()]);
        }
        ProjectKind::Plugin => {}
    }
    let loaders: Vec<String> = if q.loaders.is_empty() {
        match kind {
            ProjectKind::Plugin => ["paper", "spigot", "bukkit", "purpur"].map(String::from).to_vec(),
            ProjectKind::Modpack => ["fabric", "forge", "neoforge"].map(String::from).to_vec(),
            ProjectKind::Mod => Vec::new(),
        }
    } else {
        q.loaders.clone()
    };
    if !loaders.is_empty() {
        groups.push(loader_group(&loaders));
    }
    if let Some(v) = &q.game_version {
        groups.push(vec![format!("versions:{v}")]);
    }
    serde_json::to_string(&groups).unwrap_or_else(|_| "[]".into())
}

fn kind_path(kind: ProjectKind) -> &'static str {
    match kind {
        ProjectKind::Mod => "mod",
        ProjectKind::Plugin => "plugin",
        ProjectKind::Modpack => "modpack",
    }
}

pub async fn search(client: &reqwest::Client, base: &str, kind: ProjectKind, q: &SearchQuery) -> Result<SearchPage> {
    let index = if q.text.trim().is_empty() { "downloads" } else { "relevance" };
    let limit = if q.limit == 0 { 20 } else { q.limit.min(50) };
    let req = client.get(format!("{base}/search")).query(&[
        ("query", q.text.trim().to_string()),
        ("facets", facets(kind, q)),
        ("index", index.to_string()),
        ("offset", q.offset.to_string()),
        ("limit", limit.to_string()),
    ]);
    let r: SearchResponse = send_json(req, SITE).await?;
    Ok(SearchPage {
        total: r.total_hits,
        hits: r
            .hits
            .into_iter()
            .map(|h| SearchHit {
                source: AddonSource::Modrinth,
                kind,
                page_url: page_url(kind_path(kind), &h.slug),
                project_id: h.project_id,
                slug: h.slug,
                title: h.title,
                author: h.author,
                description: h.description,
                icon_url: h.icon_url.filter(|u| !u.is_empty()),
                downloads: h.downloads,
            })
            .collect(),
    })
}

/// Versions of a project, newest first, filtered by loader and game version.
pub async fn versions(
    client: &reqwest::Client,
    base: &str,
    project: &str,
    loaders: &[&str],
    game_version: Option<&str>,
) -> Result<Vec<Version>> {
    let mut params: Vec<(&str, String)> = Vec::new();
    if !loaders.is_empty() {
        params.push(("loaders", serde_json::to_string(loaders)?));
    }
    if let Some(v) = game_version {
        params.push(("game_versions", serde_json::to_string(&[v])?));
    }
    let req = client.get(format!("{base}/project/{project}/version")).query(&params);
    send_json(req, SITE).await
}

pub async fn version(client: &reqwest::Client, base: &str, id: &str) -> Result<Version> {
    send_json(client.get(format!("{base}/version/{id}")), SITE).await
}

pub async fn projects(client: &reqwest::Client, base: &str, ids: &[String]) -> Result<Vec<Project>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let req = client.get(format!("{base}/projects")).query(&[("ids", serde_json::to_string(ids)?)]);
    send_json(req, SITE).await
}

/// Versions matching file hashes (SHA-1), keyed by hash.
pub async fn by_hashes(client: &reqwest::Client, base: &str, hashes: &[String]) -> Result<HashMap<String, Version>> {
    if hashes.is_empty() {
        return Ok(HashMap::new());
    }
    let req = client
        .post(format!("{base}/version_files"))
        .json(&json!({ "hashes": hashes, "algorithm": "sha1" }));
    send_json(req, SITE).await
}

/// The newest compatible version for each file hash, keyed by hash.
pub async fn updates(
    client: &reqwest::Client,
    base: &str,
    hashes: &[String],
    loaders: &[&str],
    game_version: &str,
) -> Result<HashMap<String, Version>> {
    if hashes.is_empty() {
        return Ok(HashMap::new());
    }
    let req = client.post(format!("{base}/version_files/update")).json(&json!({
        "hashes": hashes,
        "algorithm": "sha1",
        "loaders": loaders,
        "game_versions": [game_version],
    }));
    send_json(req, SITE).await
}

/// The version to install by default: the newest release, else the newest of any kind.
pub fn pick_best(versions: &[Version]) -> Option<&Version> {
    versions.iter().find(|v| v.version_type == "release").or_else(|| versions.first())
}
