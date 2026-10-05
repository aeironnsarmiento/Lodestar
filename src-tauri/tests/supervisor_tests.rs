use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use glasscraft_lib::core::events::{MemorySink, CONSOLE_BATCH, INSTANCE_STATE};
use glasscraft_lib::providers::LaunchSpec;
use glasscraft_lib::supervisor::console::log_path;
use glasscraft_lib::supervisor::job_object::process_alive;
use glasscraft_lib::supervisor::{ServerState, StopReason, Supervisor};

const WAIT: Duration = Duration::from_secs(10);

fn fake_spec(dir: &Path, flags: &[&str]) -> LaunchSpec {
    LaunchSpec {
        program: PathBuf::from(env!("CARGO_BIN_EXE_fake_mc")),
        args: flags.iter().map(|s| s.to_string()).collect(),
        working_dir: dir.to_path_buf(),
    }
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

fn states(sink: &MemorySink, id: &str) -> Vec<String> {
    sink.named(INSTANCE_STATE)
        .into_iter()
        .filter(|v| v["id"] == id)
        .map(|v| v["state"].as_str().unwrap().to_string())
        .collect()
}

async fn start_fake(sup: &Arc<Supervisor>, id: &str, dir: &Path, port: u16, flags: &[&str]) {
    sup.prepare(id, id, port).unwrap();
    sup.start(id, &fake_spec(dir, flags), &log_path(dir)).unwrap();
}

async fn eventually(mut check: impl FnMut() -> bool) {
    for _ in 0..200 {
        if check() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("condition not met in time");
}

#[tokio::test]
async fn starts_through_preparing_and_starting_to_online_on_done() {
    let sink = MemorySink::new();
    let sup = Supervisor::new(sink.clone());
    let dir = tempfile::tempdir().unwrap();

    start_fake(&sup, "a", dir.path(), free_port(), &["--delay-ms", "150"]).await;
    assert_eq!(sup.state("a"), ServerState::Starting);
    sup.wait_for("a", WAIT, |s| s == ServerState::Online).await.unwrap();
    assert_eq!(states(&sink, "a"), vec!["preparing", "starting", "online"]);
    assert!(sup.snapshot("a").pid.is_some());

    sup.stop_and_wait("a", StopReason::User).await.unwrap();
}

#[tokio::test]
async fn stop_sends_stop_and_ends_stopped_with_exit_zero() {
    let sink = MemorySink::new();
    let sup = Supervisor::new(sink.clone());
    let dir = tempfile::tempdir().unwrap();
    start_fake(&sup, "a", dir.path(), free_port(), &[]).await;
    sup.wait_for("a", WAIT, |s| s == ServerState::Online).await.unwrap();

    let end = sup.stop_and_wait("a", StopReason::User).await.unwrap();
    assert_eq!(end, ServerState::Stopped);
    let lines: Vec<String> = sup.console("a").into_iter().map(|l| l.text).collect();
    assert!(lines.iter().any(|l| l.contains("Stopping the server")));
    assert!(lines.iter().any(|l| l.contains("exit code 0")));
    assert_eq!(states(&sink, "a"), vec!["preparing", "starting", "online", "stopping", "stopped"]);
    assert_eq!(sup.snapshot("a").pid, None);
}

#[tokio::test]
async fn join_and_leave_lines_track_players_and_commands_reach_stdin() {
    let sup = Supervisor::new(MemorySink::new());
    let dir = tempfile::tempdir().unwrap();
    start_fake(&sup, "a", dir.path(), free_port(), &[]).await;
    sup.wait_for("a", WAIT, |s| s == ServerState::Online).await.unwrap();

    sup.send_command("a", "join Steve").unwrap();
    sup.send_command("a", "join Alex").unwrap();
    eventually(|| sup.players("a") == vec!["Alex".to_string(), "Steve".to_string()]).await;
    sup.send_command("a", "leave Steve").unwrap();
    eventually(|| sup.players("a") == vec!["Alex".to_string()]).await;

    sup.send_command("a", "op Alex").unwrap();
    sup.send_command("a", "kick Alex").unwrap();
    eventually(|| {
        let text: Vec<String> = sup.console("a").into_iter().map(|l| l.text).collect();
        text.iter().any(|l| l.contains("Made Alex a server operator")) && text.iter().any(|l| l.contains("Kicked Alex"))
    })
    .await;

    // Chat that merely contains the words does not count as a join.
    sup.send_command("a", "say <Bob> Herobrine joined the game").unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(sup.players("a"), vec!["Alex".to_string()]);

    sup.stop_and_wait("a", StopReason::User).await.unwrap();
    assert!(sup.players("a").is_empty(), "players clear when the server stops");
}

#[tokio::test]
async fn same_port_is_refused_naming_the_running_instance_and_other_ports_run_together() {
    // Covers AE6.
    let sup = Supervisor::new(MemorySink::new());
    let (da, db) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let port = free_port();
    sup.prepare("a", "Speedrun A", port).unwrap();
    sup.start("a", &fake_spec(da.path(), &[]), &log_path(da.path())).unwrap();
    sup.wait_for("a", WAIT, |s| s == ServerState::Online).await.unwrap();

    let err = sup.prepare("b", "Practice B", port).unwrap_err().to_string();
    assert!(err.contains("Speedrun A"), "{err}");
    assert!(err.contains(&port.to_string()));
    assert_eq!(sup.state("b"), ServerState::Stopped);

    let other = free_port();
    sup.prepare("b", "Practice B", other).unwrap();
    sup.start("b", &fake_spec(db.path(), &[]), &log_path(db.path())).unwrap();
    sup.wait_for("b", WAIT, |s| s == ServerState::Online).await.unwrap();
    assert_eq!(sup.state("a"), ServerState::Online);
    assert_eq!(sup.active_ids().len(), 2);

    sup.stop_all(StopReason::Quit).await;
    assert_eq!(sup.state("a"), ServerState::Stopped);
    assert_eq!(sup.state("b"), ServerState::Stopped);
}

#[tokio::test]
async fn a_port_held_by_another_program_is_explained() {
    let sup = Supervisor::new(MemorySink::new());
    let holder = std::net::TcpListener::bind("0.0.0.0:0").unwrap();
    let port = holder.local_addr().unwrap().port();
    let err = sup.prepare("a", "A", port).unwrap_err().to_string();
    assert!(err.contains("another program"), "{err}");
}

#[tokio::test]
async fn comma_decimal_done_line_also_means_online() {
    let sup = Supervisor::new(MemorySink::new());
    let dir = tempfile::tempdir().unwrap();
    start_fake(&sup, "a", dir.path(), free_port(), &["--comma"]).await;
    sup.wait_for("a", WAIT, |s| s == ServerState::Online).await.unwrap();
    sup.stop_and_wait("a", StopReason::User).await.unwrap();
}

#[tokio::test]
async fn ten_thousand_lines_arrive_in_batches_and_the_ring_keeps_five_thousand() {
    let sink = MemorySink::new();
    let sup = Supervisor::new(sink.clone());
    let dir = tempfile::tempdir().unwrap();
    start_fake(&sup, "a", dir.path(), free_port(), &[]).await;
    sup.wait_for("a", WAIT, |s| s == ServerState::Online).await.unwrap();

    sup.send_command("a", "spam 10000").unwrap();
    eventually(|| sup.console("a").last().map(|l| l.text == "spam line 10000").unwrap_or(false)).await;
    tokio::time::sleep(Duration::from_millis(150)).await;

    let ring = sup.console("a");
    assert_eq!(ring.len(), 5000);
    assert_eq!(ring.last().unwrap().text, "spam line 10000");
    assert_eq!(ring.first().unwrap().text, "spam line 5001");

    let batches = sink.named(CONSOLE_BATCH);
    assert!(batches.len() > 1, "output should be batched, not one event per line");
    assert!(batches.len() < 2000, "too many events: {}", batches.len());
    let last_line = batches
        .iter()
        .flat_map(|b| b["lines"].as_array().unwrap().iter())
        .last()
        .unwrap()["text"]
        .clone();
    assert_eq!(last_line, "spam line 10000");

    sup.stop_and_wait("a", StopReason::User).await.unwrap();
}

#[tokio::test]
async fn a_process_that_ignores_stop_is_killed_after_the_timeout() {
    let sup = Supervisor::with_stop_timeout(MemorySink::new(), Duration::from_millis(400));
    let dir = tempfile::tempdir().unwrap();
    start_fake(&sup, "a", dir.path(), free_port(), &["--ignore-stop"]).await;
    sup.wait_for("a", WAIT, |s| s == ServerState::Online).await.unwrap();
    let pid = sup.snapshot("a").pid.unwrap();

    let started = std::time::Instant::now();
    let end = sup.stop_and_wait("a", StopReason::User).await.unwrap();
    assert_eq!(end, ServerState::Stopped, "a requested stop is never a crash");
    assert!(started.elapsed() >= Duration::from_millis(350));
    assert!(!process_alive(pid));
    assert!(sup.console("a").iter().any(|l| l.text.contains("did not stop in time")));
}

#[tokio::test]
async fn explicit_kill_ends_the_process_immediately() {
    let sup = Supervisor::new(MemorySink::new());
    let dir = tempfile::tempdir().unwrap();
    start_fake(&sup, "a", dir.path(), free_port(), &["--ignore-stop"]).await;
    sup.wait_for("a", WAIT, |s| s == ServerState::Online).await.unwrap();
    sup.kill("a").unwrap();
    let end = sup.wait_for("a", WAIT, |s| !s.has_process()).await.unwrap();
    assert_eq!(end, ServerState::Stopped);
}

#[tokio::test]
async fn an_invalid_program_reports_a_launch_failure_and_returns_to_stopped() {
    let sink = MemorySink::new();
    let sup = Supervisor::new(sink.clone());
    let dir = tempfile::tempdir().unwrap();
    sup.prepare("a", "A", free_port()).unwrap();
    let spec = LaunchSpec {
        program: dir.path().join("no-such-java.exe"),
        args: vec![],
        working_dir: dir.path().to_path_buf(),
    };
    let err = sup.start("a", &spec, &log_path(dir.path())).unwrap_err().to_string();
    assert!(err.contains("Launch failed"), "{err}");
    assert_eq!(sup.state("a"), ServerState::Stopped);
    assert!(sup.snapshot("a").message.unwrap().contains("Launch failed"));
    assert_eq!(states(&sink, "a"), vec!["preparing", "stopped"]);
}

#[tokio::test]
async fn an_unrequested_exit_is_a_crash() {
    let sup = Supervisor::new(MemorySink::new());
    let dir = tempfile::tempdir().unwrap();
    start_fake(&sup, "a", dir.path(), free_port(), &[]).await;
    sup.wait_for("a", WAIT, |s| s == ServerState::Online).await.unwrap();
    sup.send_command("a", "crash").unwrap();
    assert_eq!(sup.wait_for("a", WAIT, |s| !s.has_process()).await.unwrap(), ServerState::Crashed);
    assert!(sup.snapshot("a").message.unwrap().contains("unexpectedly"));
}

#[tokio::test]
async fn the_console_log_persists_and_the_previous_session_tail_is_readable() {
    let dir = tempfile::tempdir().unwrap();
    {
        let sup = Supervisor::new(MemorySink::new());
        start_fake(&sup, "a", dir.path(), free_port(), &[]).await;
        sup.wait_for("a", WAIT, |s| s == ServerState::Online).await.unwrap();
        sup.send_command("a", "say remember me").unwrap();
        eventually(|| sup.console("a").iter().any(|l| l.text.contains("remember me") && !l.text.starts_with('>'))).await;
        sup.stop_and_wait("a", StopReason::User).await.unwrap();
    }
    assert!(log_path(dir.path()).is_file());

    // A fresh supervisor (app restart) shows the old session read-only.
    let sup = Supervisor::new(MemorySink::new());
    sup.load_history("a", &log_path(dir.path()));
    let history = sup.console("a");
    assert!(history.iter().all(|l| l.history));
    assert!(history.iter().any(|l| l.text.contains("[Server] remember me")));
    assert!(history.iter().any(|l| l.text.contains("Glasscraft session")));
}

#[tokio::test]
async fn killing_the_parent_kills_children_in_its_job_object() {
    use tokio::io::AsyncBufReadExt;
    let mut parent = tokio::process::Command::new(env!("CARGO_BIN_EXE_fake_mc"))
        .arg("--job-parent")
        .stdout(std::process::Stdio::piped())
        .stdin(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let mut lines = tokio::io::BufReader::new(parent.stdout.take().unwrap()).lines();
    let first = tokio::time::timeout(WAIT, lines.next_line()).await.unwrap().unwrap().unwrap();
    let child_pid: u32 = first.strip_prefix("CHILD ").unwrap().parse().unwrap();
    assert!(process_alive(child_pid));

    parent.kill().await.unwrap();
    let mut gone = false;
    for _ in 0..100 {
        if !process_alive(child_pid) {
            gone = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(gone, "the child outlived its job-owning parent");
}
