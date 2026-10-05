mod common;

use std::sync::Arc;
use std::time::Duration;

use chrono::{Local, NaiveDate, NaiveDateTime, TimeZone};
use common::{console_text, fixture_server, ready_instance, test_app_with};
use lodestar_lib::core::instance::{RestartMode, RestartSchedule};
use lodestar_lib::core::paths::Paths;
use lodestar_lib::lifecycle::crash::{CrashDecision, CrashPolicy, BACKOFF};
use lodestar_lib::lifecycle::scheduler::{tick, Action, ScheduleState};
use lodestar_lib::lifecycle::ManualClock;
use lodestar_lib::supervisor::job_object::process_alive;
use lodestar_lib::supervisor::{ServerState, StopReason};

const WAIT: Duration = Duration::from_secs(10);
const FAST: [Duration; 3] = [Duration::from_millis(20), Duration::from_millis(40), Duration::from_millis(80)];

fn at(h: u32, m: u32, s: u32) -> NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 10, 5).unwrap().and_hms_opt(h, m, s).unwrap()
}

fn sched(mode: RestartMode) -> RestartSchedule {
    RestartSchedule { enabled: true, times: vec!["04:00".into()], mode }
}

async fn eventually(mut check: impl FnMut() -> bool) {
    for _ in 0..400 {
        if check() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("condition not met in time");
}

#[test]
fn crash_policy_backs_off_5_15_45_then_gives_up() {
    let mut p = CrashPolicy::default();
    let t0 = Local.with_ymd_and_hms(2026, 10, 5, 20, 0, 0).unwrap();
    let delays: Vec<CrashDecision> = (0..4).map(|i| p.on_crash("a", t0 + chrono::Duration::seconds(i * 30), &BACKOFF)).collect();
    assert_eq!(
        delays,
        vec![
            CrashDecision::RestartAfter { delay: Duration::from_secs(5), attempt: 1 },
            CrashDecision::RestartAfter { delay: Duration::from_secs(15), attempt: 2 },
            CrashDecision::RestartAfter { delay: Duration::from_secs(45), attempt: 3 },
            CrashDecision::GiveUp,
        ]
    );
}

#[test]
fn crashes_more_than_ten_minutes_apart_reset_the_counter() {
    let mut p = CrashPolicy::default();
    let t0 = Local.with_ymd_and_hms(2026, 10, 5, 20, 0, 0).unwrap();
    for i in 0..6 {
        let d = p.on_crash("a", t0 + chrono::Duration::minutes(11 * i), &BACKOFF);
        assert_eq!(d, CrashDecision::RestartAfter { delay: Duration::from_secs(5), attempt: 1 });
    }
    // Other instances keep their own history.
    assert!(matches!(p.on_crash("b", t0, &BACKOFF), CrashDecision::RestartAfter { attempt: 1, .. }));
}

#[test]
fn warn_mode_sends_countdown_warnings_in_order_then_restarts_on_time() {
    // Covers AE1 (two players online at the due time).
    let s = sched(RestartMode::Warn);
    let mut st = ScheduleState::default();
    let mut log = Vec::new();
    let mut t = at(3, 50, 0);
    while t <= at(4, 1, 0) {
        for a in tick(&s, &mut st, t, 2, true) {
            log.push((t, a));
        }
        t += chrono::Duration::seconds(5);
    }
    let said: Vec<String> = log
        .iter()
        .filter_map(|(_, a)| if let Action::Say(m) = a { Some(m.clone()) } else { None })
        .collect();
    assert_eq!(said, vec!["Server restarting in 5 minutes.", "Server restarting in 1 minute.", "Server restarting in 10 seconds!"]);
    let restarts: Vec<NaiveDateTime> = log.iter().filter(|(_, a)| *a == Action::Restart).map(|(t, _)| *t).collect();
    assert_eq!(restarts, vec![at(4, 0, 0)], "exactly one restart, on time");
    // Warnings came at the right moments.
    assert_eq!(log[0].0, at(3, 55, 0));
    assert_eq!(log[1].0, at(3, 59, 0));
    assert_eq!(log[2].0, at(3, 59, 50));
}

#[test]
fn postpone_mode_waits_until_the_last_player_leaves() {
    // Covers AE2.
    let s = sched(RestartMode::Postpone);
    let mut st = ScheduleState::default();
    assert!(tick(&s, &mut st, at(3, 59, 0), 2, true).is_empty(), "no countdown in postpone mode");
    let at_due = tick(&s, &mut st, at(4, 0, 0), 2, true);
    assert!(matches!(&at_due[..], [Action::Say(m)] if m.contains("postponed")));
    for minute in 1..30 {
        assert!(tick(&s, &mut st, at(4, minute, 0), 1, true).is_empty(), "still players at 04:{minute:02}");
    }
    assert_eq!(tick(&s, &mut st, at(4, 30, 0), 0, true), vec![Action::Restart]);
    assert!(tick(&s, &mut st, at(4, 30, 5), 0, true).is_empty(), "fires once");
}

#[test]
fn schedules_skip_stopped_servers_and_fire_again_the_next_day() {
    let s = sched(RestartMode::Warn);
    let mut st = ScheduleState::default();
    assert!(tick(&s, &mut st, at(3, 58, 0), 0, false).is_empty());
    assert!(tick(&s, &mut st, at(4, 0, 0), 0, false).is_empty());
    // The server came up after the due time: nothing queued.
    assert!(tick(&s, &mut st, at(4, 5, 0), 0, true).is_empty());
    let next = NaiveDate::from_ymd_opt(2026, 10, 6).unwrap().and_hms_opt(4, 0, 0).unwrap();
    assert_eq!(tick(&s, &mut st, next, 0, true), vec![Action::Restart]);
}

#[tokio::test]
async fn a_server_that_keeps_crashing_is_restarted_with_backoff_then_left_stopped() {
    // Covers AE7.
    let server = fixture_server();
    let dir = tempfile::tempdir().unwrap();
    let app = test_app_with(&server, dir.path(), |c| c.crash_backoff = FAST);
    let inst = ready_instance(&app, "Crashy");
    std::fs::write(Paths::new(dir.path()).server_dir(&inst.id).join("fake_mc_crash"), b"").unwrap();

    app.launch(&inst.id).await.unwrap();
    eventually(|| {
        let s = app.supervisor.snapshot(&inst.id);
        s.state == ServerState::Stopped && s.message.as_deref().is_some_and(|m| m.contains("stopped restarting"))
    })
    .await;

    let lines = console_text(&app, &inst.id);
    let restarts: Vec<&String> = lines.iter().filter(|l| l.contains("Restarting in")).collect();
    assert_eq!(restarts.len(), 3, "three automatic restarts: {restarts:?}");
    assert!(restarts[0].contains("attempt 1 of 3") && restarts[2].contains("attempt 3 of 3"));
    assert_eq!(lines.iter().filter(|l| l.contains("exited unexpectedly")).count(), 4);
    // It stays stopped: no loop.
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(app.supervisor.state(&inst.id), ServerState::Stopped);
}

#[tokio::test]
async fn user_reset_and_scheduled_stops_are_not_crashes() {
    let server = fixture_server();
    let dir = tempfile::tempdir().unwrap();
    let clock = ManualClock::new(Local.with_ymd_and_hms(2026, 10, 5, 3, 59, 55).unwrap());
    let c2 = clock.clone();
    let app = test_app_with(&server, dir.path(), move |c| {
        c.crash_backoff = FAST;
        c.clock = c2;
    });
    let inst = ready_instance(&app, "Calm");
    app.store
        .modify(&inst.id, |i| i.restart = RestartSchedule { enabled: true, times: vec!["04:00".into()], mode: RestartMode::Warn })
        .unwrap();

    app.launch(&inst.id).await.unwrap();
    app.supervisor.wait_for(&inst.id, WAIT, |s| s == ServerState::Online).await.unwrap();
    // User stop.
    app.supervisor.stop_and_wait(&inst.id, StopReason::User).await.unwrap();
    app.launch(&inst.id).await.unwrap();
    app.supervisor.wait_for(&inst.id, WAIT, |s| s == ServerState::Online).await.unwrap();
    // Reset stop.
    app.reset_world(&inst.id, None).await.unwrap();
    app.supervisor.wait_for(&inst.id, WAIT, |s| s == ServerState::Online).await.unwrap();
    // Scheduled stop: the 10-second warning, then the restart at 04:00.
    let first = app.scheduler_tick();
    assert!(matches!(&first[..], [(_, a)] if a == &vec![Action::Say("Server restarting in 10 seconds!".into())]));
    clock.set(Local.with_ymd_and_hms(2026, 10, 5, 4, 0, 0).unwrap());
    assert!(matches!(&app.scheduler_tick()[..], [(_, a)] if a == &vec![Action::Restart]));
    eventually(|| console_text(&app, &inst.id).iter().filter(|l| l.contains("Done (")).count() >= 4).await;
    app.supervisor.wait_for(&inst.id, WAIT, |s| s == ServerState::Online).await.unwrap();

    let lines = console_text(&app, &inst.id);
    assert!(lines.iter().any(|l| l.contains("[Server] Server restarting in 10 seconds!")));
    assert!(!lines.iter().any(|l| l.contains("Restarting in") || l.contains("unexpectedly")), "no crash handling");
    app.supervisor.stop_and_wait(&inst.id, StopReason::User).await.unwrap();
}

#[tokio::test]
async fn keep_awake_holds_while_any_server_runs() {
    let server = fixture_server();
    let dir = tempfile::tempdir().unwrap();
    let app = test_app_with(&server, dir.path(), |_| {});
    let a = ready_instance(&app, "A");
    let b = ready_instance(&app, "B");
    assert!(!app.keep_awake.is_active());

    app.launch(&a.id).await.unwrap();
    assert!(app.keep_awake.is_active(), "set when the first instance starts");
    app.launch(&b.id).await.unwrap();
    app.supervisor.wait_for(&a.id, WAIT, |s| s == ServerState::Online).await.unwrap();
    app.supervisor.wait_for(&b.id, WAIT, |s| s == ServerState::Online).await.unwrap();

    app.supervisor.stop_and_wait(&a.id, StopReason::User).await.unwrap();
    assert!(app.keep_awake.is_active(), "B still runs");
    app.supervisor.stop_and_wait(&b.id, StopReason::User).await.unwrap();
    assert!(!app.keep_awake.is_active(), "cleared when the last one stops");
}

#[tokio::test]
async fn quit_stops_every_running_server_before_exiting() {
    let server = fixture_server();
    let dir = tempfile::tempdir().unwrap();
    let app = test_app_with(&server, dir.path(), |_| {});
    let a = ready_instance(&app, "A");
    let b = ready_instance(&app, "B");
    for id in [&a.id, &b.id] {
        app.launch(id).await.unwrap();
        app.supervisor.wait_for(id, WAIT, |s| s == ServerState::Online).await.unwrap();
    }
    let pids: Vec<u32> = [&a.id, &b.id].iter().map(|id| app.supervisor.snapshot(id).pid.unwrap()).collect();

    app.shutdown().await;
    for id in [&a.id, &b.id] {
        assert_eq!(app.supervisor.state(id), ServerState::Stopped);
        assert!(console_text(&app, id).iter().any(|l| l.contains("Stopping the server")), "graceful stop for {id}");
    }
    assert!(pids.iter().all(|p| !process_alive(*p)));
    assert!(!app.keep_awake.is_active());
}

#[tokio::test]
async fn servers_flagged_to_start_with_the_app_are_launched() {
    let server = fixture_server();
    let dir = tempfile::tempdir().unwrap();
    let app = test_app_with(&server, dir.path(), |_| {});
    let auto = ready_instance(&app, "Auto");
    let manual = ready_instance(&app, "Manual");
    app.store.modify(&auto.id, |i| i.auto_start = true).unwrap();

    Arc::clone(&app).autostart_instances().await;
    app.supervisor.wait_for(&auto.id, WAIT, |s| s == ServerState::Online).await.unwrap();
    assert_eq!(app.supervisor.state(&manual.id), ServerState::Stopped);
    app.shutdown().await;
}
