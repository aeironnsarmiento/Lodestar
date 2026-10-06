//! Keep-awake on macOS.

/// On macOS the keep-awake request is a `caffeinate` child tied to our pid. This lives
/// in its own test binary because other tests' apps would start one for the same pid.
#[cfg(target_os = "macos")]
#[test]
fn keep_awake_runs_caffeinate_only_while_held() {
    use std::time::Duration;

    use lodestar_lib::lifecycle::keep_awake::KeepAwake;
    let pattern = format!("caffeinate -i -w {}", std::process::id());
    let running = || std::process::Command::new("pgrep").args(["-f", &pattern]).output().unwrap().status.success();
    let wait_for = |want: bool| {
        for _ in 0..100 {
            if running() == want {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        false
    };
    let keep = KeepAwake::new();
    assert!(!running());
    keep.set(true);
    assert!(wait_for(true), "caffeinate starts");
    keep.set(false);
    assert!(wait_for(false), "caffeinate stops");
}
