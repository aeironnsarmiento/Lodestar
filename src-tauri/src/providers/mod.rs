//! Server software providers: version lists and installed server files for each
//! server type, plus the launch spec abstraction (KTD5).

pub mod fabric;
pub mod forge;
pub mod modrinth;
pub mod mojang;
pub mod neoforge;
pub mod paper;

use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{anyhow, bail, Result};
use serde::Serialize;

use crate::core::instance::{LaunchInfo, ServerType};
use crate::core::paths::Paths;
use crate::download::{Downloader, ProgressFn};

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum VersionKind {
    Release,
    Snapshot,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VersionEntry {
    pub id: String,
    pub kind: VersionKind,
    /// RFC 3339 release time when known (from Mojang's manifest).
    pub release_time: Option<String>,
}

/// What to run and where. The supervisor runs any spec (KTD5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaunchSpec {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub working_dir: PathBuf,
}

/// Numeric parts of a Minecraft version before any suffix: `1.21.11-rc3` → [1, 21, 11].
pub fn version_key(v: &str) -> Vec<u64> {
    let core = v.split(['-', '_', '+', ' ']).next().unwrap_or("");
    core.split('.').map_while(|p| p.parse::<u64>().ok()).collect()
}

/// Orders versions numerically; a pre-release sorts below its release.
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    let (ka, kb) = (version_key(a), version_key(b));
    let len = ka.len().max(kb.len());
    for i in 0..len {
        let (x, y) = (ka.get(i).copied().unwrap_or(0), kb.get(i).copied().unwrap_or(0));
        match x.cmp(&y) {
            Ordering::Equal => {}
            o => return o,
        }
    }
    let (sa, sb) = (is_prerelease(a), is_prerelease(b));
    match (sa, sb) {
        (false, true) => Ordering::Greater,
        (true, false) => Ordering::Less,
        _ => a.cmp(b),
    }
}

/// Snapshots, pre-releases and release candidates.
pub fn is_prerelease(v: &str) -> bool {
    let lower = v.to_ascii_lowercase();
    lower.contains("-pre") || lower.contains("-rc") || lower.contains("snapshot") || lower.contains('w') && !lower.contains('.')
}

/// True when `v` is at least `min` (both Minecraft versions). Year-based versions
/// (26.x) sort above 1.x.
pub fn mc_at_least(v: &str, min: &str) -> bool {
    let k = version_key(v);
    !k.is_empty() && compare_versions(v, min) != Ordering::Less
}

/// Forge and NeoForge are supported from this Minecraft version on.
pub const MODDED_MIN_MC: &str = "1.17";

/// Base URLs, overridable so tests can point every provider at a local server.
#[derive(Clone, Debug)]
pub struct Endpoints {
    pub mojang_manifest: String,
    pub paper: String,
    pub fabric_meta: String,
    pub forge_promotions: String,
    pub forge_maven: String,
    pub neoforge_api: String,
    pub neoforge_maven: String,
    pub modrinth: String,
    pub curseforge: String,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self {
            mojang_manifest: "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json".into(),
            paper: "https://fill.papermc.io/v3/projects/paper".into(),
            fabric_meta: "https://meta.fabricmc.net/v2".into(),
            forge_promotions: "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json".into(),
            forge_maven: "https://maven.minecraftforge.net/net/minecraftforge/forge".into(),
            neoforge_api: "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge".into(),
            neoforge_maven: "https://maven.neoforged.net/releases/net/neoforged/neoforge".into(),
            modrinth: "https://api.modrinth.com/v2".into(),
            curseforge: "https://api.curseforge.com/v1".into(),
        }
    }
}

impl Endpoints {
    /// Every endpoint under one base URL (tests).
    pub fn local(base: &str) -> Self {
        Self {
            mojang_manifest: format!("{base}/mojang/version_manifest_v2.json"),
            paper: format!("{base}/paper"),
            fabric_meta: format!("{base}/fabric"),
            forge_promotions: format!("{base}/forge/promotions_slim.json"),
            forge_maven: format!("{base}/forge/maven"),
            neoforge_api: format!("{base}/neoforge/api"),
            neoforge_maven: format!("{base}/neoforge/maven"),
            modrinth: format!("{base}/modrinth"),
            curseforge: format!("{base}/curseforge"),
        }
    }
}

/// Fetches version lists (cached per session) and installs server software.
pub struct Providers {
    pub downloader: Downloader,
    pub endpoints: Endpoints,
    paths: Paths,
    manifest: tokio::sync::Mutex<Option<mojang::Manifest>>,
    versions: Mutex<HashMap<ServerType, Vec<VersionEntry>>>,
}

impl Providers {
    pub fn new(paths: Paths, downloader: Downloader, endpoints: Endpoints) -> Self {
        Self {
            downloader,
            endpoints,
            paths,
            manifest: tokio::sync::Mutex::new(None),
            versions: Mutex::new(HashMap::new()),
        }
    }

    pub async fn manifest(&self) -> Result<mojang::Manifest> {
        let mut guard = self.manifest.lock().await;
        if let Some(m) = guard.as_ref() {
            return Ok(m.clone());
        }
        let text = self.downloader.get_text(&self.endpoints.mojang_manifest).await?;
        let m = mojang::parse_manifest(&text)?;
        *guard = Some(m.clone());
        Ok(m)
    }

    /// Every version for a type, newest first, releases and snapshots tagged. The UI
    /// filters snapshots. Lists are cached for the session.
    pub async fn versions(&self, server_type: ServerType) -> Result<Vec<VersionEntry>> {
        if let Some(v) = self.versions.lock().unwrap().get(&server_type) {
            return Ok(v.clone());
        }
        let manifest = self.manifest().await.ok();
        let mut list = match server_type {
            ServerType::Vanilla => {
                let m = match manifest.clone() {
                    Some(m) => m,
                    None => self.manifest().await?,
                };
                mojang::versions(&m)
            }
            ServerType::Paper => {
                let text = self.downloader.get_text(&self.endpoints.paper).await?;
                paper::parse_project(&text)?
            }
            ServerType::Fabric => {
                let text = self.downloader.get_text(&format!("{}/versions/game", self.endpoints.fabric_meta)).await?;
                fabric::parse_game_versions(&text)?
            }
            ServerType::Forge => {
                let xml = self.downloader.get_text(&format!("{}/maven-metadata.xml", self.endpoints.forge_maven)).await?;
                forge::mc_versions(&forge::parse_maven_metadata(&xml))
            }
            ServerType::Neoforge => {
                let text = self.downloader.get_text(&self.endpoints.neoforge_api).await?;
                neoforge::mc_versions(&neoforge::parse_versions(&text)?)
            }
        };
        if let Some(m) = manifest {
            m.annotate_release_times(&mut list);
        }
        self.versions.lock().unwrap().insert(server_type, list.clone());
        Ok(list)
    }

    /// The Java feature version Mojang says a Minecraft version needs (KTD9).
    pub async fn java_requirement(&self, mc_version: &str) -> Result<u32> {
        let manifest = self.manifest().await?;
        let entry = manifest.find(mc_version).ok_or_else(|| anyhow!("Minecraft {mc_version} is not in Mojang's version list."))?;
        let text = self.downloader.get_text(&entry.url).await?;
        Ok(mojang::parse_version_details(&text)?.java_major)
    }

    /// Installs the server software into the shared jar cache or the instance's server
    /// folder and returns how to launch it. Forge and NeoForge need `java` to run their
    /// installer.
    pub async fn install(
        &self,
        server_type: ServerType,
        mc_version: &str,
        loader_version: Option<&str>,
        server_dir: &Path,
        java: Option<&Path>,
        progress: ProgressFn<'_>,
    ) -> Result<LaunchInfo> {
        let cache = self.paths.jar_cache_dir();
        match server_type {
            ServerType::Vanilla => {
                let manifest = self.manifest().await?;
                let entry = manifest
                    .find(mc_version)
                    .ok_or_else(|| anyhow!("Minecraft {mc_version} is not in Mojang's version list."))?;
                let details = mojang::parse_version_details(&self.downloader.get_text(&entry.url).await?)?;
                let server = details
                    .server
                    .ok_or_else(|| anyhow!("Mojang has no server download for {mc_version}."))?;
                let dest = cache.join(format!("vanilla-{mc_version}.jar"));
                self.downloader
                    .download(&server.url, &dest, Some(&crate::download::Hash::Sha1(server.sha1)), progress)
                    .await?;
                Ok(LaunchInfo::Jar { jar: dest.to_string_lossy().into_owned() })
            }
            ServerType::Paper => {
                let url = format!("{}/versions/{mc_version}/builds/latest", self.endpoints.paper);
                let build = paper::parse_build(&self.downloader.get_text(&url).await?)?;
                let dest = cache.join(&build.name);
                self.downloader
                    .download(&build.url, &dest, Some(&crate::download::Hash::Sha256(build.sha256)), progress)
                    .await?;
                Ok(LaunchInfo::Jar { jar: dest.to_string_lossy().into_owned() })
            }
            ServerType::Fabric => {
                let meta = &self.endpoints.fabric_meta;
                let loader = match loader_version {
                    Some(v) => v.to_string(),
                    None => fabric::pick_stable(&self.downloader.get_text(&format!("{meta}/versions/loader")).await?)?,
                };
                let installer = fabric::pick_stable(&self.downloader.get_text(&format!("{meta}/versions/installer")).await?)?;
                let url = format!("{meta}/versions/loader/{mc_version}/{loader}/{installer}/server/jar");
                let dest = cache.join(format!("fabric-{mc_version}-loader{loader}-launcher{installer}.jar"));
                self.downloader.download(&url, &dest, None, progress).await?;
                Ok(LaunchInfo::Jar { jar: dest.to_string_lossy().into_owned() })
            }
            ServerType::Forge | ServerType::Neoforge => {
                if !mc_at_least(mc_version, MODDED_MIN_MC) {
                    bail!("{} needs Minecraft {MODDED_MIN_MC} or newer.", server_type.label());
                }
                let java = java.ok_or_else(|| anyhow!("Java is needed to run the {} installer.", server_type.label()))?;
                let (loader_version, url) = if let Some(pinned) = loader_version {
                    // Forge's maven names builds `<mc>-<forge>`; modpacks give just `<forge>`.
                    let v = if server_type == ServerType::Forge && !pinned.contains('-') {
                        format!("{mc_version}-{pinned}")
                    } else {
                        pinned.to_string()
                    };
                    let url = if server_type == ServerType::Forge {
                        forge::installer_url(&self.endpoints.forge_maven, &v)
                    } else {
                        neoforge::installer_url(&self.endpoints.neoforge_maven, &v)
                    };
                    (v, url)
                } else if server_type == ServerType::Forge {
                    let promos = forge::parse_promotions(&self.downloader.get_text(&self.endpoints.forge_promotions).await?)?;
                    let xml = self.downloader.get_text(&format!("{}/maven-metadata.xml", self.endpoints.forge_maven)).await?;
                    let v = forge::pick_version(&forge::parse_maven_metadata(&xml), &promos, mc_version)
                        .ok_or_else(|| anyhow!("Forge has no build for Minecraft {mc_version}."))?;
                    let url = forge::installer_url(&self.endpoints.forge_maven, &v);
                    (v, url)
                } else {
                    let all = neoforge::parse_versions(&self.downloader.get_text(&self.endpoints.neoforge_api).await?)?;
                    let v = neoforge::pick_version(&all, mc_version)
                        .ok_or_else(|| anyhow!("NeoForge has no build for Minecraft {mc_version}."))?;
                    let url = neoforge::installer_url(&self.endpoints.neoforge_maven, &v);
                    (v, url)
                };
                let installer = cache.join(format!("{}-{loader_version}-installer.jar", server_type.label().to_lowercase()));
                self.downloader.download(&url, &installer, None, progress).await?;
                forge::run_installer(java, &installer, server_dir).await?;
                forge::detect_launch(server_dir)
            }
        }
    }
}

/// JVM flags used for every server (Aikar's G1 set, minus ones that slow startup).
fn jvm_flags(ram_mb: u32) -> Vec<String> {
    let mut v = vec![format!("-Xms{ram_mb}M"), format!("-Xmx{ram_mb}M")];
    // The console is read through pipes; make Java write it as UTF-8.
    v.extend(["-Dfile.encoding=UTF-8", "-Dstdout.encoding=UTF-8", "-Dstderr.encoding=UTF-8"].map(String::from));
    v.extend(
        [
            "-XX:+UseG1GC",
            "-XX:+ParallelRefProcEnabled",
            "-XX:MaxGCPauseMillis=200",
            "-XX:+UnlockExperimentalVMOptions",
            "-XX:+DisableExplicitGC",
            "-XX:G1NewSizePercent=30",
            "-XX:G1MaxNewSizePercent=40",
            "-XX:G1HeapRegionSize=8M",
            "-XX:G1ReservePercent=20",
            "-XX:G1HeapWastePercent=5",
            "-XX:G1MixedGCCountTarget=4",
            "-XX:InitiatingHeapOccupancyPercent=15",
            "-XX:G1MixedGCLiveThresholdPercent=90",
            "-XX:SurvivorRatio=32",
            "-XX:+PerfDisableSharedMem",
            "-XX:MaxTenuringThreshold=1",
        ]
        .map(String::from),
    );
    v
}

/// Builds the command line for an installed server. `extra` goes after `nogui`
/// (for example `--universe worlds/<run>`).
pub fn launch_spec(java: &Path, server_dir: &Path, launch: &LaunchInfo, ram_mb: u32, extra: &[String]) -> LaunchSpec {
    let mut args = jvm_flags(ram_mb);
    match launch {
        LaunchInfo::Jar { jar } => {
            args.push("-jar".into());
            args.push(jar.clone());
        }
        LaunchInfo::ArgsFile { args_file } => {
            if server_dir.join("user_jvm_args.txt").exists() {
                args.push("@user_jvm_args.txt".into());
            }
            args.push(format!("@{args_file}"));
        }
    }
    args.push("nogui".into());
    args.extend(extra.iter().cloned());
    LaunchSpec {
        program: java.to_path_buf(),
        args,
        working_dir: server_dir.to_path_buf(),
    }
}
