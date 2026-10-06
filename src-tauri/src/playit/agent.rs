//! The private `playitd` agent, run with its own secret file and IPC socket so it
//! never collides with a user-installed playit service (KTD10). It runs in the app's
//! Job Object.
//!
//! On Windows it is the signed v1.0.10 binary, downloaded and verified by SHA-256.
//! playit.gg publishes no macOS build, so on macOS the same version is built from
//! source (`scripts/build-playitd.sh`) and shipped inside the app as a sidecar.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::{bail, Context, Result};
use tokio::process::{Child, ChildStdin, Command};

use crate::download::{Downloader, Hash, ProgressFn};
use crate::supervisor::job_object::JobObject;

#[cfg(windows)]
pub const AGENT_URL: &str =
    "https://github.com/playit-cloud/playit-agent/releases/download/v1.0.10/playit-windows-x86_64-signed.exe";
#[cfg(windows)]
pub const AGENT_SHA256: &str = "2dbdaad119844cbbc062cc9774b8b462afa5f1b4b7832a9fc5ef4676cae887cf";

/// No download elsewhere: the agent ships with the app (see [`bundled_agent`]).
#[cfg(not(windows))]
pub const AGENT_URL: &str = "";
#[cfg(not(windows))]
pub const AGENT_SHA256: &str = "";

#[cfg(windows)]
const AGENT_FILE: &str = "playitd.exe";
#[cfg(not(windows))]
const AGENT_FILE: &str = "playitd";

pub fn agent_path(playit_dir: &Path) -> PathBuf {
    playit_dir.join(AGENT_FILE)
}

pub fn secret_path(playit_dir: &Path) -> PathBuf {
    playit_dir.join("playit.toml")
}

/// The agent shipped next to the app's own executable (a Tauri sidecar lands in
/// `Lodestar.app/Contents/MacOS/`), if there is one.
pub fn bundled_agent() -> Option<PathBuf> {
    if cfg!(windows) {
        return None;
    }
    let exe = std::env::current_exe().ok()?;
    let path = exe.parent()?.join(AGENT_FILE);
    path.is_file().then_some(path)
}

/// Where the agent listens for its CLI: a named pipe on Windows, a Unix socket in
/// the playit folder elsewhere (or in the temp folder if that path is too long for
/// a socket address).
pub fn socket_path(playit_dir: &Path) -> PathBuf {
    if cfg!(windows) {
        return PathBuf::from(r"\\.\pipe\lodestar-playitd");
    }
    let path = playit_dir.join("playitd.sock");
    // sun_path holds 104 bytes on macOS, including the NUL.
    if path.as_os_str().len() < 100 {
        path
    } else {
        std::env::temp_dir().join("lodestar-playitd.sock")
    }
}

/// Downloads the pinned agent unless a verified copy is already there. A copy with
/// the wrong hash is deleted and refused.
pub async fn install(downloader: &Downloader, url: &str, sha256: &str, playit_dir: &Path, progress: ProgressFn<'_>) -> Result<PathBuf> {
    if url.is_empty() {
        bail!("This build of Lodestar does not include the playit.gg agent.");
    }
    let dest = agent_path(playit_dir);
    downloader
        .download(url, &dest, Some(&Hash::Sha256(sha256.to_string())), progress)
        .await
        .context("Downloading the playit.gg agent")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755))?;
    }
    Ok(dest)
}

/// A running agent. Its stdin is held open (the agent ignores it).
pub struct AgentProcess {
    pub child: Child,
    _stdin: Option<ChildStdin>,
}

/// Starts the agent with the app's private secret file, socket and log.
pub fn spawn(program: &Path, playit_dir: &Path, job: Option<&JobObject>) -> Result<AgentProcess> {
    std::fs::create_dir_all(playit_dir.join("logs"))?;
    let socket = socket_path(playit_dir);
    if cfg!(unix) {
        // A socket left by an agent that was killed would make the new one fail to bind.
        std::fs::remove_file(&socket).ok();
    }
    let mut cmd = Command::new(program);
    cmd.arg("--secret-path")
        .arg(secret_path(playit_dir))
        .arg("--socket-path")
        .arg(&socket)
        .arg("--log-path")
        .arg(playit_dir.join("logs").join("playitd.log"))
        .current_dir(playit_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    crate::supervisor::process::detach(&mut cmd);
    let mut child = cmd.spawn().with_context(|| format!("Could not start {}", program.display()))?;
    if let Some(job) = job {
        if let Err(e) = job.assign(&child) {
            eprintln!("lodestar: could not add playitd to the job object: {e:#}");
        }
    }
    let stdin = child.stdin.take();
    Ok(AgentProcess { child, _stdin: stdin })
}
