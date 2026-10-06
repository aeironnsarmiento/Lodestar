//! Keeps the computer from sleeping while any server runs (R7, KTD14).
//!
//! A dedicated thread holds the request while asked to and clears it otherwise:
//! on Windows `SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED)`, which
//! applies to the calling thread; on macOS a `caffeinate -i` child, which also
//! watches Lodestar's pid so it never outlives the app.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Mutex;

pub struct KeepAwake {
    tx: Mutex<mpsc::Sender<bool>>,
    active: AtomicBool,
}

impl Default for KeepAwake {
    fn default() -> Self {
        Self::new()
    }
}

impl KeepAwake {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel::<bool>();
        std::thread::Builder::new()
            .name("lodestar-keep-awake".into())
            .spawn(move || {
                let mut request = imp::Request::new();
                for awake in rx {
                    request.set(awake);
                }
                // Channel closed (app exiting): release the request.
                request.set(false);
            })
            .expect("keep-awake thread");
        Self { tx: Mutex::new(tx), active: AtomicBool::new(false) }
    }

    /// Holds or releases the request. Repeated calls with the same value are free.
    pub fn set(&self, awake: bool) {
        if self.active.swap(awake, Ordering::SeqCst) != awake {
            let _ = self.tx.lock().unwrap().send(awake);
        }
    }

    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::SeqCst)
    }
}

#[cfg(windows)]
mod imp {
    use windows_sys::Win32::System::Power::{SetThreadExecutionState, ES_CONTINUOUS, ES_SYSTEM_REQUIRED};

    pub struct Request;

    impl Request {
        pub fn new() -> Self {
            Self
        }

        pub fn set(&mut self, awake: bool) {
            unsafe {
                if awake {
                    SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED);
                } else {
                    SetThreadExecutionState(ES_CONTINUOUS);
                }
            }
        }
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use std::process::{Child, Command, Stdio};

    pub struct Request(Option<Child>);

    impl Request {
        pub fn new() -> Self {
            Self(None)
        }

        pub fn set(&mut self, awake: bool) {
            if awake && self.0.is_none() {
                let spawned = Command::new("/usr/bin/caffeinate")
                    .arg("-i")
                    .arg("-w")
                    .arg(std::process::id().to_string())
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn();
                match spawned {
                    Ok(child) => self.0 = Some(child),
                    Err(e) => eprintln!("lodestar: could not start caffeinate: {e}"),
                }
            } else if !awake {
                if let Some(mut child) = self.0.take() {
                    let _ = child.kill();
                    let _ = child.wait();
                }
            }
        }
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
mod imp {
    pub struct Request;

    impl Request {
        pub fn new() -> Self {
            Self
        }

        pub fn set(&mut self, _awake: bool) {}
    }
}
