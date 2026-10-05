//! The private `playitd` agent: the signed v1.0.10 binary, verified by SHA-256 and
//! run with its own secret file and named pipe so it never collides with a
//! user-installed playit service (KTD10). It runs in the app's Job Object.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::{Context, Result};
use tokio::process::{Child, ChildStdin, Command};

use crate::download::{Downloader, Hash, ProgressFn};
use crate::supervisor::job_object::JobObject;

pub const AGENT_URL: &str =
    "https://github.com/playit-cloud/playit-agent/releases/download/v1.0.10/playit-windows-x86_64-signed.exe";
pub const AGENT_SHA256: &str = "2dbdaad119844cbbc062cc9774b8b462afa5f1b4b7832a9fc5ef4676cae887cf";
pub const SOCKET_PATH: &str = r"\\.\pipe\lodestar-playitd";

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn agent_path(playit_dir: &Path) -> PathBuf {
    playit_dir.join("playitd.exe")
}

pub fn secret_path(playit_dir: &Path) -> PathBuf {
    playit_dir.join("playit.toml")
}

/// Downloads the pinned agent unless a verified copy is already there. A copy with
/// the wrong hash is deleted and refused.
pub async fn install(downloader: &Downloader, url: &str, sha256: &str, playit_dir: &Path, progress: ProgressFn<'_>) -> Result<PathBuf> {
    let dest = agent_path(playit_dir);
    downloader
        .download(url, &dest, Some(&Hash::Sha256(sha256.to_string())), progress)
        .await
        .context("Downloading the playit.gg agent")
}

/// A running agent. Its stdin is held open (the agent ignores it).
pub struct AgentProcess {
    pub child: Child,
    _stdin: Option<ChildStdin>,
}

/// Starts the agent with the app's private secret file, pipe and log.
pub fn spawn(program: &Path, playit_dir: &Path, job: Option<&JobObject>) -> Result<AgentProcess> {
    std::fs::create_dir_all(playit_dir.join("logs"))?;
    let mut cmd = Command::new(program);
    cmd.arg("--secret-path")
        .arg(secret_path(playit_dir))
        .arg("--socket-path")
        .arg(SOCKET_PATH)
        .arg("--log-path")
        .arg(playit_dir.join("logs").join("playitd.log"))
        .current_dir(playit_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW);
    let mut child = cmd.spawn().with_context(|| format!("Could not start {}", program.display()))?;
    if let Some(job) = job {
        if let Err(e) = job.assign(&child) {
            eprintln!("lodestar: could not add playitd to the job object: {e:#}");
        }
    }
    let stdin = child.stdin.take();
    Ok(AgentProcess { child, _stdin: stdin })
}
