//! Spawning a launch spec: piped stdio, no console window, inside the Job Object.

use std::process::Stdio;

use anyhow::{Context, Result};
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command};

use super::job_object::JobObject;
use crate::providers::LaunchSpec;

/// Keeps a child from opening a console window (Windows) and makes it the leader of
/// its own process group (macOS), so the watchdog can kill it with anything it starts.
pub fn detach(cmd: &mut Command) {
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    #[cfg(unix)]
    cmd.process_group(0);
}

pub struct Spawned {
    pub child: Child,
    pub stdin: ChildStdin,
    pub stdout: ChildStdout,
    pub stderr: ChildStderr,
}

pub fn spawn(spec: &LaunchSpec, job: Option<&JobObject>) -> Result<Spawned> {
    let mut cmd = Command::new(&spec.program);
    cmd.args(&spec.args)
        .current_dir(&spec.working_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    detach(&mut cmd);
    let mut child = cmd
        .spawn()
        .with_context(|| format!("Could not start {}", spec.program.display()))?;
    if let Some(job) = job {
        // Not fatal: the server still runs, it just would not die with the app.
        if let Err(e) = job.assign(&child) {
            eprintln!("lodestar: could not add process to job object: {e:#}");
        }
    }
    let stdin = child.stdin.take().context("no stdin")?;
    let stdout = child.stdout.take().context("no stdout")?;
    let stderr = child.stderr.take().context("no stderr")?;
    Ok(Spawned { child, stdin, stdout, stderr })
}

/// Reads lines until EOF, decoding invalid UTF-8 lossily and trimming line endings.
pub async fn read_lines<R: AsyncRead + Unpin>(reader: R, mut on_line: impl FnMut(String)) {
    let mut reader = BufReader::new(reader);
    let mut buf = Vec::with_capacity(256);
    loop {
        buf.clear();
        match reader.read_until(b'\n', &mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                while matches!(buf.last(), Some(b'\n' | b'\r')) {
                    buf.pop();
                }
                on_line(String::from_utf8_lossy(&buf).into_owned());
            }
        }
    }
}
