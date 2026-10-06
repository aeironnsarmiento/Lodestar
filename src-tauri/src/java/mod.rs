//! Portable Temurin Java runtimes, shared between instances (R10, R11, KTD9).
//! The system Java on PATH is never used.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::download::{extract_tar_gz_flatten, extract_zip_flatten, Downloader, Hash, ProgressFn};

/// Temurin long-term-support feature versions.
pub const LTS: [u32; 4] = [11, 17, 21, 25];

/// The runtime to use for a Minecraft version's Java requirement: exactly 8 for
/// versions that need 8, otherwise the smallest LTS at or above the requirement.
/// A requirement newer than the newest known LTS maps to itself.
pub fn target_major(required: u32) -> u32 {
    if required <= 8 {
        return 8;
    }
    LTS.iter().copied().find(|&lts| lts >= required).unwrap_or(required)
}

/// Feature version from `java -version` output (`1.8.0_412` → 8, `25.0.1` → 25).
pub fn parse_java_version(output: &str) -> Option<u32> {
    let re = Regex::new(r#"version "(\d+)(?:\.(\d+))?"#).unwrap();
    let c = re.captures(output)?;
    let first: u32 = c[1].parse().ok()?;
    if first == 1 {
        c.get(2)?.as_str().parse().ok()
    } else {
        Some(first)
    }
}

/// The Java home inside an extracted runtime: the folder itself, or `Contents/Home`
/// in a macOS JDK bundle.
fn java_home(runtime_dir: &Path) -> PathBuf {
    if cfg!(target_os = "macos") {
        runtime_dir.join("Contents").join("Home")
    } else {
        runtime_dir.to_path_buf()
    }
}

/// The `java` executable inside an extracted runtime.
pub fn java_binary(runtime_dir: &Path) -> PathBuf {
    let name = if cfg!(windows) { "java.exe" } else { "java" };
    java_home(runtime_dir).join("bin").join(name)
}

/// Adoptium's name for this OS.
const ADOPTIUM_OS: &str = if cfg!(windows) {
    "windows"
} else if cfg!(target_os = "macos") {
    "mac"
} else {
    "linux"
};

/// Architectures to try, best first. Windows always uses x64 (it runs on Arm under
/// emulation). Apple Silicon falls back to the Intel build under Rosetta, because
/// Temurin has no Arm build of Java 8.
fn adoptium_architectures() -> &'static [&'static str] {
    match (ADOPTIUM_OS, std::env::consts::ARCH) {
        ("windows", _) => &["x64"],
        ("mac", "aarch64") => &["aarch64", "x64"],
        (_, "aarch64") => &["aarch64"],
        _ => &["x64"],
    }
}

/// Feature version from a JDK `release` file (`JAVA_VERSION="25.0.4"`).
fn release_file_version(dir: &Path) -> Option<u32> {
    let text = fs::read_to_string(java_home(dir).join("release")).ok()?;
    let line = text.lines().find(|l| l.starts_with("JAVA_VERSION="))?;
    parse_java_version(&format!("version {}", line.trim_start_matches("JAVA_VERSION=")))
}

/// Runs `java -version` and returns the feature version.
pub async fn probe_version(java_exe: &Path) -> Option<u32> {
    let mut cmd = tokio::process::Command::new(java_exe);
    cmd.arg("-version").stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    crate::supervisor::process::detach(&mut cmd);
    let out = tokio::time::timeout(Duration::from_secs(20), cmd.output()).await.ok()?.ok()?;
    let text = format!("{}{}", String::from_utf8_lossy(&out.stderr), String::from_utf8_lossy(&out.stdout));
    parse_java_version(&text)
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

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InstalledRuntime {
    pub major: u32,
    pub path: String,
    pub size_bytes: u64,
}

#[derive(Deserialize)]
struct Asset {
    binary: Binary,
}

#[derive(Deserialize)]
struct Binary {
    package: Package,
}

#[derive(Deserialize)]
struct Package {
    link: String,
    checksum: String,
    name: String,
}

#[derive(Clone, Copy)]
enum ArchiveKind {
    Zip,
    TarGz,
}

impl ArchiveKind {
    fn extension(self) -> &'static str {
        match self {
            ArchiveKind::Zip => "zip",
            ArchiveKind::TarGz => "tar.gz",
        }
    }
}

fn archive_kind(name: &str) -> Option<ArchiveKind> {
    let name = name.to_ascii_lowercase();
    if name.ends_with(".zip") {
        Some(ArchiveKind::Zip)
    } else if name.ends_with(".tar.gz") {
        Some(ArchiveKind::TarGz)
    } else {
        None
    }
}

/// An Intel build on an Apple Silicon Mac.
fn needs_rosetta(arch: &str) -> bool {
    cfg!(target_os = "macos") && std::env::consts::ARCH == "aarch64" && arch == "x64"
}

pub struct JavaManager {
    runtimes_dir: PathBuf,
    downloader: Downloader,
    adoptium_api: String,
    install_lock: tokio::sync::Mutex<()>,
}

impl JavaManager {
    pub fn new(runtimes_dir: PathBuf, downloader: Downloader, adoptium_api: impl Into<String>) -> Self {
        Self {
            runtimes_dir,
            downloader,
            adoptium_api: adoptium_api.into(),
            install_lock: tokio::sync::Mutex::new(()),
        }
    }

    pub const DEFAULT_API: &'static str = "https://api.adoptium.net/v3";

    pub fn runtime_dir(&self, major: u32) -> PathBuf {
        self.runtimes_dir.join(format!("jre-{major}"))
    }

    pub fn java_exe(&self, major: u32) -> PathBuf {
        java_binary(&self.runtime_dir(major))
    }

    /// Installed runtimes (folders with a `java` executable), lowest version first.
    pub fn installed(&self) -> Vec<InstalledRuntime> {
        let Ok(entries) = fs::read_dir(&self.runtimes_dir) else { return Vec::new() };
        let mut list: Vec<InstalledRuntime> = entries
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                let major: u32 = name.strip_prefix("jre-")?.parse().ok()?;
                let path = e.path();
                java_binary(&path).is_file().then(|| InstalledRuntime {
                    major,
                    size_bytes: dir_size(&path),
                    path: path.to_string_lossy().into_owned(),
                })
            })
            .collect();
        list.sort_by_key(|r| r.major);
        list
    }

    /// `java` for a Minecraft version's requirement, downloading the runtime first if
    /// needed. A folder without `java` counts as missing and is replaced.
    pub async fn ensure(&self, required: u32, progress: ProgressFn<'_>) -> Result<PathBuf> {
        let major = target_major(required);
        let exe = self.java_exe(major);
        if exe.is_file() {
            return Ok(exe);
        }
        let _guard = self.install_lock.lock().await;
        if exe.is_file() {
            return Ok(exe);
        }
        self.install(major, progress).await?;
        Ok(exe)
    }

    /// The package to install and the architecture it was built for.
    async fn find_package(&self, major: u32) -> Result<(Package, &'static str)> {
        for &arch in adoptium_architectures() {
            for image in ["jre", "jdk"] {
                let url = format!(
                    "{}/assets/latest/{major}/hotspot?architecture={arch}&image_type={image}&os={ADOPTIUM_OS}&vendor=eclipse",
                    self.adoptium_api
                );
                let assets: Vec<Asset> = self.downloader.get_json(&url).await?;
                // macOS also lists a .pkg installer; only archives are usable here.
                if let Some(a) = assets.into_iter().find(|a| archive_kind(&a.binary.package.name).is_some()) {
                    return Ok((a.binary.package, arch));
                }
            }
        }
        bail!("Eclipse Temurin has no {ADOPTIUM_OS} build of Java {major}.")
    }

    async fn install(&self, major: u32, progress: ProgressFn<'_>) -> Result<()> {
        fs::create_dir_all(&self.runtimes_dir)?;
        let (package, arch) = self.find_package(major).await?;
        let Some(kind) = archive_kind(&package.name) else {
            bail!("Unexpected Java package {}", package.name);
        };
        let archive = self.runtimes_dir.join(format!("jre-{major}.{}", kind.extension()));
        self.downloader
            .download(&package.link, &archive, Some(&Hash::Sha256(package.checksum)), progress)
            .await
            .with_context(|| format!("Downloading Java {major}"))?;
        let dir = self.runtime_dir(major);
        let extracted = match kind {
            ArchiveKind::Zip => extract_zip_flatten(&archive, &dir),
            ArchiveKind::TarGz => extract_tar_gz_flatten(&archive, &dir),
        };
        fs::remove_file(&archive).ok();
        extracted.with_context(|| format!("Installing Java {major}"))?;

        let exe = self.java_exe(major);
        if !exe.is_file() {
            fs::remove_dir_all(&dir).ok();
            bail!("The Java {major} download did not contain {}.", exe.strip_prefix(&dir).unwrap_or(&exe).display());
        }
        let found = match probe_version(&exe).await {
            Some(v) => Some(v),
            None if needs_rosetta(arch) => {
                fs::remove_dir_all(&dir).ok();
                bail!(
                    "Java {major} is only available for Intel Macs, so it needs Rosetta. \
                     Install it with `softwareupdate --install-rosetta` in Terminal, then try again."
                );
            }
            None => release_file_version(&dir),
        };
        if found != Some(major) {
            fs::remove_dir_all(&dir).ok();
            return Err(anyhow!("The downloaded runtime is not Java {major} (found {found:?})."));
        }
        Ok(())
    }

    /// Deletes a runtime. Refused while a running server uses it.
    pub fn remove(&self, major: u32, in_use: bool) -> Result<()> {
        if in_use {
            bail!("Java {major} is in use by a running server.");
        }
        let dir = self.runtime_dir(major);
        if !dir.exists() {
            bail!("Java {major} is not installed.");
        }
        fs::remove_dir_all(&dir).with_context(|| format!("removing {}", dir.display()))
    }
}
