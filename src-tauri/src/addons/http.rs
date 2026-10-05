//! JSON requests to the mod sites, with readable errors, plus the result shapes the
//! UI shows for either site.

use anyhow::{anyhow, Result};
use serde::de::DeserializeOwned;
use serde::Serialize;

use super::{AddonSource, ProjectKind};

/// Sends a request and decodes JSON. `site` names the service in errors.
pub async fn send_json<T: DeserializeOwned>(req: reqwest::RequestBuilder, site: &str) -> Result<T> {
    let resp = req.send().await.map_err(|e| anyhow!("Could not reach {site}: {e}"))?;
    let status = resp.status();
    if status.as_u16() == 401 || status.as_u16() == 403 {
        if site == "CurseForge" {
            return Err(anyhow!("CurseForge refused the API key. Check it in Settings → CurseForge."));
        }
        return Err(anyhow!("{site} refused the request ({status})."));
    }
    if status.as_u16() == 404 {
        return Err(anyhow!("{site} could not find that project or version."));
    }
    if !status.is_success() {
        return Err(anyhow!("{site} answered {status}. Try again in a moment."));
    }
    let text = resp.text().await.map_err(|e| anyhow!("Reading {site}'s answer failed: {e}"))?;
    serde_json::from_str(&text).map_err(|e| anyhow!("{site}'s answer has an unexpected format ({e})."))
}

/// One search result.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub source: AddonSource,
    pub kind: ProjectKind,
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub author: String,
    pub description: String,
    pub icon_url: Option<String>,
    pub downloads: u64,
    pub page_url: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SearchPage {
    pub hits: Vec<SearchHit>,
    pub total: u64,
}

/// A downloadable version of a project.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectVersion {
    pub source: AddonSource,
    pub project_id: String,
    pub version_id: String,
    pub name: String,
    pub version_number: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    /// "release", "beta" or "alpha".
    pub channel: String,
    pub published: String,
    pub file_name: String,
}

/// A file to fetch: where from, where to, and how to check it.
#[derive(Clone, Debug, PartialEq)]
pub struct RemoteFile {
    pub url: Option<String>,
    pub file_name: String,
    pub sha1: Option<String>,
}

/// Search parameters shared by both sites.
#[derive(Clone, Debug, Default)]
pub struct SearchQuery {
    pub text: String,
    pub game_version: Option<String>,
    /// Loader names (Modrinth spelling); empty means any supported loader.
    pub loaders: Vec<String>,
    pub offset: u32,
    pub limit: u32,
}
