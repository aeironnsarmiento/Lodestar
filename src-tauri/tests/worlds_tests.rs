mod common;

use std::time::Duration;

use chrono::{Local, TimeZone};
use common::{console_text, fixture_server, ready_instance, test_app, test_app_with};
use glasscraft_lib::core::paths::Paths;
use glasscraft_lib::supervisor::ServerState;
use glasscraft_lib::worlds::properties::{merge, read_value};
use glasscraft_lib::worlds::{list_worlds, new_world, prune, world_seed, worlds_dir, KEEP_WORLDS};

const WAIT: Duration = Duration::from_secs(10);

async fn eventually(mut check: impl FnMut() -> bool) {
    for _ in 0..200 {
        if check() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("condition not met in time");
}

fn universe_line(lines: &[String], run: &str) -> bool {
    lines.iter().any(|l| l.starts_with("fake_mc args:") && l.contains(&format!("--universe worlds/{run}")))
}

#[tokio::test]
async fn reset_on_an_online_server_stops_it_and_starts_a_new_random_seed_world() {
    // Covers AE3.
    let server = fixture_server();
    let dir = tempfile::tempdir().unwrap();
    let app = test_app(&server, dir.path());
    let inst = ready_instance(&app, "Speedrun");
    app.launch(&inst.id).await.unwrap();
    app.supervisor.wait_for(&inst.id, WAIT, |s| s == ServerState::Online).await.unwrap();
    let first = app.store.get(&inst.id).unwrap().current_world.unwrap();
    app.send_command(&inst.id, "join Friend").unwrap();
    eventually(|| app.supervisor.players(&inst.id).len() == 1).await;

    let run = app.reset_world(&inst.id, None).await.unwrap();
    assert_ne!(run, first);
    app.supervisor.wait_for(&inst.id, WAIT, |s| s == ServerState::Online).await.unwrap();

    let server_dir = Paths::new(dir.path()).server_dir(&inst.id);
    let lines = console_text(&app, &inst.id);
    assert!(lines.iter().any(|l| l.contains("Stopping the server")), "stop was sent");
    assert!(app.supervisor.players(&inst.id).is_empty(), "everyone was disconnected");
    let seed = world_seed(&server_dir, &run).unwrap();
    seed.parse::<i64>().expect("a random 64-bit seed");
    assert_eq!(read_value(&server_dir, "level-seed").unwrap(), seed);
    assert_eq!(read_value(&server_dir, "level-name").unwrap(), "world");
    assert!(universe_line(&lines, &run), "restarted with --universe worlds/{run}");
    assert!(worlds_dir(&server_dir).join(&run).join("world").join("level.dat").is_file());
    assert_eq!(app.store.get(&inst.id).unwrap().current_world.as_deref(), Some(run.as_str()));
    // The previous world is kept with its seed.
    assert!(list_worlds(&server_dir, Some(&run)).iter().any(|w| w.name == first && w.seed.is_some()));

    app.supervisor.stop_and_wait(&inst.id, glasscraft_lib::supervisor::StopReason::User).await.unwrap();
}

#[tokio::test]
async fn reset_with_an_entered_seed_records_and_uses_it() {
    // Covers AE4.
    let server = fixture_server();
    let dir = tempfile::tempdir().unwrap();
    let app = test_app(&server, dir.path());
    let inst = ready_instance(&app, "Practice");

    // Reset on a stopped instance creates the world and starts it.
    let run = app.reset_world(&inst.id, Some("speedrun123".into())).await.unwrap();
    app.supervisor.wait_for(&inst.id, WAIT, |s| s == ServerState::Online).await.unwrap();
    let server_dir = Paths::new(dir.path()).server_dir(&inst.id);
    assert_eq!(std::fs::read_to_string(worlds_dir(&server_dir).join(&run).join("seed.txt")).unwrap(), "speedrun123");
    assert_eq!(read_value(&server_dir, "level-seed").unwrap(), "speedrun123");
    assert!(console_text(&app, &inst.id).iter().any(|l| l.contains("Using seed speedrun123")));
    app.supervisor.stop_and_wait(&inst.id, glasscraft_lib::supervisor::StopReason::User).await.unwrap();
}

#[tokio::test]
async fn a_reset_with_ten_kept_worlds_deletes_the_oldest_and_keeps_ten() {
    // Covers AE5.
    let server = fixture_server();
    let dir = tempfile::tempdir().unwrap();
    let app = test_app(&server, dir.path());
    let inst = ready_instance(&app, "Ten");
    let server_dir = Paths::new(dir.path()).server_dir(&inst.id);
    let mut names = Vec::new();
    for day in 1..=10 {
        let t = Local.with_ymd_and_hms(2026, 9, day, 12, 0, 0).unwrap();
        names.push(new_world(&server_dir, Some(&format!("seed{day}")), t).unwrap().0);
    }
    app.switch_world(&inst.id, names.last().unwrap()).unwrap();
    assert_eq!(list_worlds(&server_dir, None).len(), KEEP_WORLDS);

    let run = app.reset_world(&inst.id, None).await.unwrap();
    app.supervisor.wait_for(&inst.id, WAIT, |s| s == ServerState::Online).await.unwrap();
    eventually(|| list_worlds(&server_dir, None).len() == KEEP_WORLDS).await;

    let kept: Vec<String> = list_worlds(&server_dir, None).into_iter().map(|w| w.name).collect();
    assert!(!kept.contains(&names[0]), "the oldest world was deleted");
    assert!(kept.contains(&run));
    assert_eq!(kept[0], run, "newest first");
    app.supervisor.stop_and_wait(&inst.id, glasscraft_lib::supervisor::StopReason::User).await.unwrap();
}

#[tokio::test]
async fn play_this_world_makes_an_older_run_the_next_start() {
    let server = fixture_server();
    let dir = tempfile::tempdir().unwrap();
    let app = test_app(&server, dir.path());
    let inst = ready_instance(&app, "Switch");
    let server_dir = Paths::new(dir.path()).server_dir(&inst.id);
    let old = new_world(&server_dir, Some("old-seed"), Local.with_ymd_and_hms(2026, 9, 1, 8, 0, 0).unwrap()).unwrap().0;
    let newer = new_world(&server_dir, Some("new-seed"), Local.with_ymd_and_hms(2026, 9, 2, 8, 0, 0).unwrap()).unwrap().0;
    app.switch_world(&inst.id, &newer).unwrap();

    app.switch_world(&inst.id, &old).unwrap();
    assert!(app.worlds(&inst.id).unwrap().iter().any(|w| w.name == old && w.current));
    app.launch(&inst.id).await.unwrap();
    app.supervisor.wait_for(&inst.id, WAIT, |s| s == ServerState::Online).await.unwrap();
    assert!(universe_line(&console_text(&app, &inst.id), &old));
    assert_eq!(read_value(&server_dir, "level-seed").unwrap(), "old-seed");

    assert!(app.switch_world(&inst.id, "run_not-a-world").is_err());
    assert!(app.switch_world(&inst.id, "..\\..\\escape").is_err());
    app.supervisor.stop_and_wait(&inst.id, glasscraft_lib::supervisor::StopReason::User).await.unwrap();
}

#[test]
fn two_resets_in_the_same_second_get_distinct_folders() {
    let dir = tempfile::tempdir().unwrap();
    let now = Local.with_ymd_and_hms(2026, 10, 5, 21, 30, 0).unwrap();
    let a = new_world(dir.path(), None, now).unwrap().0;
    let b = new_world(dir.path(), None, now).unwrap().0;
    let c = new_world(dir.path(), Some("  "), now).unwrap().0;
    assert_eq!(a, "run_2026-10-05_21-30-00");
    assert_eq!(b, "run_2026-10-05_21-30-00_2");
    assert_eq!(c, "run_2026-10-05_21-30-00_3");
    let listed: Vec<String> = list_worlds(dir.path(), None).into_iter().map(|w| w.name).collect();
    assert_eq!(listed, vec![c, b, a], "newest first, including same-second resets");
}

#[test]
fn pruning_never_deletes_the_current_world_even_when_it_is_the_oldest() {
    let dir = tempfile::tempdir().unwrap();
    let mut names = Vec::new();
    for hour in 0..12 {
        let t = Local.with_ymd_and_hms(2026, 10, 1, hour, 0, 0).unwrap();
        names.push(new_world(dir.path(), None, t).unwrap().0);
    }
    let oldest = names[0].clone();
    let deleted = prune(dir.path(), KEEP_WORLDS, Some(&oldest));
    assert_eq!(deleted, vec![names[1].clone()]);
    let kept: Vec<String> = list_worlds(dir.path(), Some(&oldest)).into_iter().map(|w| w.name).collect();
    assert!(kept.contains(&oldest));
    assert_eq!(kept.len(), KEEP_WORLDS + 1);
}

#[test]
fn properties_merge_keeps_user_keys_and_updates_managed_ones() {
    let existing = "#Minecraft server properties\nmotd=Old motd\nallow-nether=false\nlevel-seed=123\nspawn-protection=16\n";
    let set = vec![("motd", "New motd".to_string()), ("level-seed", "456".to_string()), ("server-port", "25570".to_string())];
    let out = merge(existing, &set, &[("spawn-protection", "0"), ("allow-flight", "true")]);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0], "#Minecraft server properties");
    assert_eq!(lines[1], "motd=New motd", "updated in place");
    assert!(lines.contains(&"allow-nether=false"), "user key kept");
    assert!(lines.contains(&"level-seed=456"));
    assert!(lines.contains(&"server-port=25570"), "new managed key appended");
    assert!(lines.contains(&"spawn-protection=16"), "defaults never override the user's value");
    assert!(lines.contains(&"allow-flight=true"), "missing default added");
    // Non-ASCII MOTDs are escaped the way java.util.Properties expects.
    let out = merge("", &[("motd", "Café ✨".to_string())], &[]);
    assert!(out.contains("motd=Caf\\u00e9 \\u2728"));
}

#[tokio::test]
async fn a_server_that_will_not_stop_is_killed_and_the_reset_continues() {
    let server = fixture_server();
    let dir = tempfile::tempdir().unwrap();
    let app = test_app_with(&server, dir.path(), |c| c.stop_timeout = Duration::from_millis(500));
    let inst = ready_instance(&app, "Stubborn");
    let server_dir = Paths::new(dir.path()).server_dir(&inst.id);
    std::fs::write(server_dir.join("fake_mc_ignore_stop"), b"").unwrap();
    app.launch(&inst.id).await.unwrap();
    app.supervisor.wait_for(&inst.id, WAIT, |s| s == ServerState::Online).await.unwrap();

    let run = app.reset_world(&inst.id, None).await.unwrap();
    app.supervisor.wait_for(&inst.id, WAIT, |s| s == ServerState::Online).await.unwrap();
    let lines = console_text(&app, &inst.id);
    assert!(lines.iter().any(|l| l.contains("did not stop in time")));
    assert!(universe_line(&lines, &run));
    app.supervisor.kill(&inst.id).unwrap();
    app.supervisor.wait_for(&inst.id, WAIT, |s| !s.has_process()).await.unwrap();
}

#[tokio::test]
async fn the_host_is_opped_when_the_server_comes_online() {
    let server = fixture_server();
    let dir = tempfile::tempdir().unwrap();
    let app = test_app(&server, dir.path());
    let inst = ready_instance(&app, "Op");
    app.store.modify(&inst.id, |i| i.op_name = "Speedy".into()).unwrap();
    app.launch(&inst.id).await.unwrap();
    app.supervisor.wait_for(&inst.id, WAIT, |s| s == ServerState::Online).await.unwrap();
    eventually(|| console_text(&app, &inst.id).iter().any(|l| l.contains("Made Speedy a server operator"))).await;
    app.supervisor.stop_and_wait(&inst.id, glasscraft_lib::supervisor::StopReason::User).await.unwrap();
}
