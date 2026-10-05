//! CurseForge API v1. Every request needs the user's API key (Settings → CurseForge);
//! CurseForge does not allow apps to ship one. Some authors turn off third-party
//! downloads, in which case a file has no download link and must be fetched by hand.

use anyhow::{bail, Result};
use serde::Deserialize;
use serde_json::json;

use super::http::{send_json, ProjectVersion, RemoteFile, SearchHit, SearchPage, SearchQuery};
use super::{AddonSource, ProjectKind};

const SITE: &str = "CurseForge";
const MINECRAFT: u32 = 432;

fn class_id(kind: ProjectKind) -> u32 {
    match kind {
        ProjectKind::Mod => 6,
        ProjectKind::Modpack => 4471,
        ProjectKind::Plugin => 5,
    }
}

/// CurseForge's mod loader ids, from Modrinth-style names.
pub fn loader_type(name: &str) -> Option<u32> {
    match name {
        "forge" => Some(1),
        "fabric" => Some(4),
        "quilt" => Some(5),
        "neoforge" => Some(6),
        _ => None,
    }
}

const LOADER_NAMES: [(&str, &str); 4] = [("Forge", "forge"), ("Fabric", "fabric"), ("Quilt", "quilt"), ("NeoForge", "neoforge")];

#[derive(Deserialize)]
struct Data<T> {
    data: T,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Pagination {
    total_count: u64,
}

#[derive(Deserialize)]
struct SearchResponse {
    data: Vec<Mod>,
    pagination: Pagination,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Logo {
    #[serde(rename = "thumbnailUrl")]
    pub thumbnail_url: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Author {
    pub name: String,
}

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Links {
    pub website_url: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Mod {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub download_count: f64,
    pub logo: Option<Logo>,
    #[serde(default)]
    pub authors: Vec<Author>,
    #[serde(default)]
    pub links: Links,
}

impl Mod {
    pub fn icon(&self) -> Option<String> {
        self.logo.as_ref().and_then(|l| l.thumbnail_url.clone()).filter(|u| !u.is_empty())
    }

    pub fn page_url(&self) -> String {
        self.links
            .website_url
            .clone()
            .unwrap_or_else(|| format!("https://www.curseforge.com/minecraft/mc-mods/{}", self.slug))
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct FileHash {
    pub value: String,
    pub algo: u32,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FileDependency {
    pub mod_id: u64,
    pub relation_type: u32,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct File {
    pub id: u64,
    pub mod_id: u64,
    pub display_name: String,
    pub file_name: String,
    #[serde(default)]
    pub file_date: String,
    #[serde(default)]
    pub release_type: u32,
    pub download_url: Option<String>,
    #[serde(default)]
    pub hashes: Vec<FileHash>,
    #[serde(default)]
    pub game_versions: Vec<String>,
    #[serde(default)]
    pub dependencies: Vec<FileDependency>,
}

impl File {
    pub fn sha1(&self) -> Option<String> {
        self.hashes.iter().find(|h| h.algo == 1).map(|h| h.value.clone())
    }

    pub fn remote_file(&self) -> RemoteFile {
        RemoteFile { url: self.download_url.clone().filter(|u| !u.is_empty()), file_name: self.file_name.clone(), sha1: self.sha1() }
    }

    pub fn loaders(&self) -> Vec<String> {
        LOADER_NAMES
            .iter()
            .filter(|(cf, _)| self.game_versions.iter().any(|g| g == cf))
            .map(|(_, ours)| ours.to_string())
            .collect()
    }

    pub fn minecraft_versions(&self) -> Vec<String> {
        self.game_versions
            .iter()
            .filter(|g| g.chars().next().is_some_and(|c| c.is_ascii_digit()))
            .cloned()
            .collect()
    }

    pub fn to_project_version(&self) -> ProjectVersion {
        ProjectVersion {
            source: AddonSource::Curseforge,
            project_id: self.mod_id.to_string(),
            version_id: self.id.to_string(),
            name: self.display_name.clone(),
            version_number: self.display_name.clone(),
            game_versions: self.minecraft_versions(),
            loaders: self.loaders(),
            channel: match self.release_type {
                2 => "beta",
                3 => "alpha",
                _ => "release",
            }
            .into(),
            published: self.file_date.clone(),
            file_name: self.file_name.clone(),
        }
    }

    /// Required dependencies (relation type 3).
    pub fn required_mods(&self) -> Vec<u64> {
        self.dependencies.iter().filter(|d| d.relation_type == 3).map(|d| d.mod_id).collect()
    }
}

fn key(api_key: &str) -> Result<&str> {
    let k = api_key.trim();
    if k.is_empty() {
        bail!("Add a CurseForge API key in Settings → CurseForge to browse and install from CurseForge.");
    }
    Ok(k)
}

pub async fn search(client: &reqwest::Client, base: &str, api_key: &str, kind: ProjectKind, q: &SearchQuery) -> Result<SearchPage> {
    let limit = if q.limit == 0 { 20 } else { q.limit.min(50) };
    let mut params: Vec<(&str, String)> = vec![
        ("gameId", MINECRAFT.to_string()),
        ("classId", class_id(kind).to_string()),
        ("searchFilter", q.text.trim().to_string()),
        ("sortField", "2".into()),
        ("sortOrder", "desc".into()),
        ("index", q.offset.to_string()),
        ("pageSize", limit.to_string()),
    ];
    if let Some(v) = &q.game_version {
        params.push(("gameVersion", v.clone()));
    }
    // CurseForge filters by one loader at a time.
    if let Some(l) = q.loaders.iter().find_map(|l| loader_type(l)) {
        if kind != ProjectKind::Plugin {
            params.push(("modLoaderType", l.to_string()));
        }
    }
    let req = client.get(format!("{base}/mods/search")).header("x-api-key", key(api_key)?).query(&params);
    let r: SearchResponse = send_json(req, SITE).await?;
    Ok(SearchPage {
        total: r.pagination.total_count,
        hits: r
            .data
            .into_iter()
            .map(|m| SearchHit {
                source: AddonSource::Curseforge,
                kind,
                project_id: m.id.to_string(),
                icon_url: m.icon(),
                page_url: m.page_url(),
                slug: m.slug,
                title: m.name,
                author: m.authors.first().map(|a| a.name.clone()).unwrap_or_default(),
                description: m.summary,
                downloads: m.download_count.max(0.0) as u64,
            })
            .collect(),
    })
}

/// Files of a project, newest first, filtered by game version and loader.
pub async fn files(
    client: &reqwest::Client,
    base: &str,
    api_key: &str,
    mod_id: &str,
    game_version: Option<&str>,
    loader: Option<&str>,
) -> Result<Vec<File>> {
    let mut params: Vec<(&str, String)> = vec![("pageSize", "50".into())];
    if let Some(v) = game_version {
        params.push(("gameVersion", v.to_string()));
    }
    if let Some(l) = loader.and_then(loader_type) {
        params.push(("modLoaderType", l.to_string()));
    }
    let req = client
        .get(format!("{base}/mods/{mod_id}/files"))
        .header("x-api-key", key(api_key)?)
        .query(&params);
    let r: Data<Vec<File>> = send_json(req, SITE).await?;
    let mut files = r.data;
    files.sort_by(|a, b| b.file_date.cmp(&a.file_date));
    Ok(files)
}

pub async fn file(client: &reqwest::Client, base: &str, api_key: &str, mod_id: &str, file_id: &str) -> Result<File> {
    let req = client.get(format!("{base}/mods/{mod_id}/files/{file_id}")).header("x-api-key", key(api_key)?);
    let r: Data<File> = send_json(req, SITE).await?;
    Ok(r.data)
}

/// Many files by id (modpack manifests).
pub async fn files_by_id(client: &reqwest::Client, base: &str, api_key: &str, ids: &[u64]) -> Result<Vec<File>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let req = client
        .post(format!("{base}/mods/files"))
        .header("x-api-key", key(api_key)?)
        .json(&json!({ "fileIds": ids }));
    let r: Data<Vec<File>> = send_json(req, SITE).await?;
    Ok(r.data)
}

pub async fn mods_by_id(client: &reqwest::Client, base: &str, api_key: &str, ids: &[u64]) -> Result<Vec<Mod>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let req = client
        .post(format!("{base}/mods"))
        .header("x-api-key", key(api_key)?)
        .json(&json!({ "modIds": ids }));
    let r: Data<Vec<Mod>> = send_json(req, SITE).await?;
    Ok(r.data)
}

/// The file to install by default: the newest release, else the newest of any kind.
pub fn pick_best(files: &[File]) -> Option<&File> {
    files.iter().find(|f| f.release_type == 1).or_else(|| files.first())
}
