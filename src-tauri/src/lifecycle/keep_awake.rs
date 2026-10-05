//! Keeps Windows from sleeping while any server runs (R7, KTD14).
//!
//! `SetThreadExecutionState` applies to the calling thread, so a dedicated thread
//! holds `ES_CONTINUOUS | ES_SYSTEM_REQUIRED` while asked to and clears it otherwise.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Mutex;

use windows_sys::Win32::System::Power::{SetThreadExecutionState, ES_CONTINUOUS, ES_SYSTEM_REQUIRED};

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
            .name("glasscraft-keep-awake".into())
            .spawn(move || {
                for awake in rx {
                    unsafe {
                        if awake {
                            SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED);
                        } else {
                            SetThreadExecutionState(ES_CONTINUOUS);
                        }
                    }
                }
                // Channel closed (app exiting): release the request.
                unsafe {
                    SetThreadExecutionState(ES_CONTINUOUS);
                }
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
