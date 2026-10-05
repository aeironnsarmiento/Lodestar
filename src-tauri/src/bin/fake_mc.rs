//! Test helper that behaves like a Minecraft server on stdin/stdout, so supervisor
//! tests never download Minecraft or accept the EULA (KTD5).
//!
//! Flags:
//!   --delay-ms N     wait before printing the startup lines
//!   --comma          print `Done (1,23s)!` (comma decimal separator)
//!   --ignore-stop    keep running after `stop`
//!   --crash          exit 1 right after starting
//!   --universe DIR   create DIR/<level-name>/level.dat like a real server would
//!   --job-parent     put a second fake_mc in a kill-on-close job, print its PID, wait
//!
//! The files `fake_mc_ignore_stop` and `fake_mc_crash` in the working directory act
//! like `--ignore-stop` and `--crash`, so app-level tests can steer one instance.
//!
//! Commands on stdin: stop, crash, join <name>, leave <name>, spam <n>, say <text>,
//! op <name>, kick <name>; anything else is echoed.

use std::io::{BufRead, Write};
use std::path::Path;
use std::time::Duration;

fn ts() -> String {
    let now = chrono::Local::now();
    now.format("[%H:%M:%S]").to_string()
}

fn info(msg: &str) {
    println!("{} [Server thread/INFO]: {msg}", ts());
}

fn read_properties(key: &str) -> Option<String> {
    let text = std::fs::read_to_string("server.properties").ok()?;
    text.lines()
        .find_map(|l| l.strip_prefix(&format!("{key}=")).map(|v| v.to_string()))
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag_file = |f: &str| Path::new(&format!("fake_mc_{}", f.trim_start_matches("--").replace('-', "_"))).exists();
    let has = |f: &str| args.iter().any(|a| a == f) || flag_file(f);
    let value = |f: &str| args.iter().position(|a| a == f).and_then(|i| args.get(i + 1)).cloned();

    if has("--job-parent") {
        job_parent();
        return;
    }

    println!("fake_mc args: {}", args.join(" "));
    if let Some(ms) = value("--delay-ms").and_then(|v| v.parse().ok()) {
        std::thread::sleep(Duration::from_millis(ms));
    }
    info("Starting minecraft server version fake");
    let level = read_properties("level-name").unwrap_or_else(|| "world".into());
    if let Some(seed) = read_properties("level-seed").filter(|s| !s.is_empty()) {
        info(&format!("Using seed {seed}"));
    }
    if let Some(universe) = value("--universe") {
        let dir = Path::new(&universe).join(&level);
        std::fs::create_dir_all(&dir).ok();
        std::fs::write(dir.join("level.dat"), b"fake level data").ok();
        info(&format!("Preparing level \"{}\"", dir.display()));
    }
    if has("--crash") {
        eprintln!("Exception in server tick loop: fake crash");
        std::process::exit(1);
    }
    if has("--comma") {
        info("Done (1,23s)! For help, type \"help\"");
    } else {
        info("Done (1.23s)! For help, type \"help\"");
    }

    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let line = line.trim().to_string();
        let (cmd, rest) = line.split_once(' ').unwrap_or((line.as_str(), ""));
        match cmd {
            "stop" => {
                if has("--ignore-stop") {
                    info("Ignoring stop");
                    continue;
                }
                info("Stopping the server");
                info("Saving worlds");
                std::process::exit(0);
            }
            "crash" => {
                eprintln!("Exception in server tick loop: requested crash");
                std::process::exit(1);
            }
            "join" => info(&format!("{rest} joined the game")),
            "leave" => info(&format!("{rest} left the game")),
            "spam" => {
                let n: usize = rest.parse().unwrap_or(0);
                let out = std::io::stdout();
                let mut out = out.lock();
                for i in 1..=n {
                    writeln!(out, "spam line {i}").ok();
                }
                out.flush().ok();
            }
            "say" => info(&format!("[Server] {rest}")),
            "op" => info(&format!("Made {rest} a server operator")),
            "kick" => info(&format!("Kicked {rest}: Kicked by an operator")),
            _ => info(&format!("Unknown or incomplete command: {line}")),
        }
    }
}

/// Spawns a child fake_mc inside a kill-on-close Job Object and waits forever. When
/// this process is killed, Windows must kill the child too.
fn job_parent() {
    let job = glasscraft_lib::supervisor::job_object::JobObject::new_kill_on_close().expect("job");
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    rt.block_on(async {
        let exe = std::env::current_exe().unwrap();
        let mut cmd = tokio::process::Command::new(exe);
        cmd.stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::null());
        let child = cmd.spawn().expect("child");
        job.assign(&child).expect("assign");
        println!("CHILD {}", child.id().unwrap());
        std::io::stdout().flush().ok();
        // Keep the job (and the child's stdin) open until we are killed.
        std::mem::forget(child);
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    });
}
