//! HTTP downloads with retry, hash verification and zip extraction (R11).
//!
//! Files stream to `<dest>.part` and are renamed into place only after the hash
//! checks out, so a failed or corrupt download never leaves a usable-looking file.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use futures_util::StreamExt;
use serde::de::DeserializeOwned;
use sha1::Digest;

pub const USER_AGENT: &str = concat!("Glasscraft/", env!("CARGO_PKG_VERSION"), " (Minecraft server manager)");

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Hash {
    Sha1(String),
    Sha256(String),
}

impl Hash {
    fn expected(&self) -> &str {
        match self {
            Hash::Sha1(h) | Hash::Sha256(h) => h,
        }
    }

    fn label(&self) -> &'static str {
        match self {
            Hash::Sha1(_) => "SHA-1",
            Hash::Sha256(_) => "SHA-256",
        }
    }
}

/// Hex digest of a file using the same algorithm as `like`.
pub fn file_hash(path: &Path, like: &Hash) -> Result<String> {
    let mut f = fs::File::open(path)?;
    let mut buf = vec![0u8; 1 << 16];
    match like {
        Hash::Sha1(_) => {
            let mut h = sha1::Sha1::new();
            loop {
                let n = f.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                h.update(&buf[..n]);
            }
            Ok(hex::encode(h.finalize()))
        }
        Hash::Sha256(_) => {
            let mut h = sha2::Sha256::new();
            loop {
                let n = f.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                h.update(&buf[..n]);
            }
            Ok(hex::encode(h.finalize()))
        }
    }
}

pub fn hash_matches(path: &Path, hash: &Hash) -> bool {
    file_hash(path, hash)
        .map(|h| h.eq_ignore_ascii_case(hash.expected()))
        .unwrap_or(false)
}

/// Bytes received so far and the total size when the server reports it.
pub type ProgressFn<'a> = &'a (dyn Fn(u64, Option<u64>) + Send + Sync);

pub fn no_progress(_done: u64, _total: Option<u64>) {}

#[derive(Clone)]
pub struct Downloader {
    client: reqwest::Client,
    attempts: u32,
    backoff: Duration,
}

impl Default for Downloader {
    fn default() -> Self {
        Self::new()
    }
}

/// Failures worth another attempt (network trouble, 5xx, rate limits) versus ones
/// that will not fix themselves (404, hash mismatch).
enum Failure {
    Retry(anyhow::Error),
    Fatal(anyhow::Error),
}

impl Downloader {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(Duration::from_secs(15))
            .build()
            .expect("HTTP client");
        Self {
            client,
            attempts: 3,
            backoff: Duration::from_secs(1),
        }
    }

    /// Base delay between attempts; it triples after each failure.
    pub fn with_backoff(mut self, backoff: Duration) -> Self {
        self.backoff = backoff;
        self
    }

    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }

    async fn retrying<T, F, Fut>(&self, what: &str, mut op: F) -> Result<T>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = std::result::Result<T, Failure>>,
    {
        let mut delay = self.backoff;
        let mut last = None;
        for attempt in 0..self.attempts {
            if attempt > 0 {
                tokio::time::sleep(delay).await;
                delay *= 3;
            }
            match op().await {
                Ok(v) => return Ok(v),
                Err(Failure::Fatal(e)) => return Err(e),
                Err(Failure::Retry(e)) => last = Some(e),
            }
        }
        Err(last
            .unwrap_or_else(|| anyhow!("no attempts made"))
            .context(format!("{what} failed after {} attempts", self.attempts)))
    }

    async fn send(&self, url: &str) -> std::result::Result<reqwest::Response, Failure> {
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| Failure::Retry(anyhow!("could not reach {url}: {e}")))?;
        let status = resp.status();
        if status.is_success() {
            Ok(resp)
        } else if status.is_server_error() || status.as_u16() == 429 || status.as_u16() == 408 {
            Err(Failure::Retry(anyhow!("{url} answered {status}")))
        } else {
            Err(Failure::Fatal(anyhow!("{url} answered {status}")))
        }
    }

    pub async fn get_text(&self, url: &str) -> Result<String> {
        self.retrying(&format!("Fetching {url}"), || async {
            let resp = self.send(url).await?;
            resp.text()
                .await
                .map_err(|e| Failure::Retry(anyhow!("reading {url}: {e}")))
        })
        .await
    }

    pub async fn get_json<T: DeserializeOwned>(&self, url: &str) -> Result<T> {
        let text = self.get_text(url).await?;
        serde_json::from_str(&text).with_context(|| format!("unexpected response from {url}"))
    }

    /// Downloads `url` to `dest`. An existing `dest` whose hash matches is reused
    /// without touching the network.
    pub async fn download(&self, url: &str, dest: &Path, hash: Option<&Hash>, progress: ProgressFn<'_>) -> Result<PathBuf> {
        if dest.is_file() {
            match hash {
                Some(h) if hash_matches(dest, h) => return Ok(dest.to_path_buf()),
                None => return Ok(dest.to_path_buf()),
                Some(_) => {
                    fs::remove_file(dest).ok();
                }
            }
        }
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        let part = part_path(dest);
        let name = dest.file_name().unwrap_or_default().to_string_lossy().into_owned();

        let result = self
            .retrying(&format!("Downloading {name}"), || async {
                fs::remove_file(&part).ok();
                let resp = self.send(url).await?;
                let total = resp.content_length();
                let mut file = fs::File::create(&part).map_err(|e| Failure::Fatal(e.into()))?;
                let mut done = 0u64;
                let mut stream = resp.bytes_stream();
                while let Some(chunk) = stream.next().await {
                    let chunk = chunk.map_err(|e| Failure::Retry(anyhow!("download of {name} was interrupted: {e}")))?;
                    file.write_all(&chunk).map_err(|e| Failure::Fatal(e.into()))?;
                    done += chunk.len() as u64;
                    progress(done, total);
                }
                file.sync_all().map_err(|e| Failure::Fatal(e.into()))?;
                drop(file);
                if let Some(h) = hash {
                    let got = file_hash(&part, h).map_err(Failure::Fatal)?;
                    if !got.eq_ignore_ascii_case(h.expected()) {
                        fs::remove_file(&part).ok();
                        return Err(Failure::Fatal(anyhow!(
                            "{name} failed its {} check (expected {}, got {got}). The file was deleted; try again.",
                            h.label(),
                            h.expected()
                        )));
                    }
                }
                Ok(())
            })
            .await;

        match result {
            Ok(()) => {
                fs::rename(&part, dest).with_context(|| format!("saving {}", dest.display()))?;
                Ok(dest.to_path_buf())
            }
            Err(e) => {
                fs::remove_file(&part).ok();
                Err(e)
            }
        }
    }
}

fn part_path(dest: &Path) -> PathBuf {
    let mut name = dest.file_name().unwrap_or_default().to_os_string();
    name.push(".part");
    dest.with_file_name(name)
}

/// Extracts a zip into `dest_dir`. When the archive has a single top-level folder
/// (as JDK zips do), its contents land directly in `dest_dir`. Extraction happens in
/// a sibling temp folder that is removed on failure.
pub fn extract_zip_flatten(zip_path: &Path, dest_dir: &Path) -> Result<()> {
    let parent = dest_dir.parent().ok_or_else(|| anyhow!("no parent for {}", dest_dir.display()))?;
    fs::create_dir_all(parent)?;
    let tmp = parent.join(format!(
        ".extract-{}",
        dest_dir.file_name().unwrap_or_default().to_string_lossy()
    ));
    if tmp.exists() {
        fs::remove_dir_all(&tmp)?;
    }
    let result = (|| -> Result<()> {
        let file = fs::File::open(zip_path)?;
        let mut archive = zip::ZipArchive::new(file).context("the archive is not a valid zip")?;
        archive.extract(&tmp).context("could not extract the archive")?;
        let entries: Vec<_> = fs::read_dir(&tmp)?.collect::<std::io::Result<_>>()?;
        let source = if entries.len() == 1 && entries[0].file_type()?.is_dir() {
            entries[0].path()
        } else {
            tmp.clone()
        };
        if dest_dir.exists() {
            fs::remove_dir_all(dest_dir)?;
        }
        fs::rename(&source, dest_dir)?;
        Ok(())
    })();
    if tmp.exists() {
        fs::remove_dir_all(&tmp).ok();
    }
    if result.is_err() && dest_dir.exists() {
        fs::remove_dir_all(dest_dir).ok();
    }
    result.map_err(|e| e.context(format!("extracting {}", zip_path.display())))
}
