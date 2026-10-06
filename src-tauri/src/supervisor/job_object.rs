//! Child processes die with Lodestar (KTD4). When Lodestar exits for any reason
//! (including a crash) the Java servers and the playit agent are killed, so no port
//! stays held by an orphan.
//!
//! On Windows this is a Job Object with KILL_ON_JOB_CLOSE. macOS has no equivalent,
//! so a tiny `/bin/sh` watchdog reads child pids from a pipe; when Lodestar exits the
//! kernel closes the pipe, the watchdog sees EOF and kills every pid still listed.

pub use imp::{process_alive, JobObject};

#[cfg(windows)]
mod imp {
    use std::mem::{size_of, zeroed};
    use std::ptr::null;

    use anyhow::{bail, Result};
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, WAIT_TIMEOUT};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, WaitForSingleObject, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
    };

    pub struct JobObject {
        handle: HANDLE,
    }

    // The handle is only used through thread-safe Win32 calls.
    unsafe impl Send for JobObject {}
    unsafe impl Sync for JobObject {}

    impl JobObject {
        pub fn new_kill_on_close() -> Result<Self> {
            unsafe {
                let handle = CreateJobObjectW(null(), null());
                if handle.is_null() {
                    bail!("CreateJobObjectW failed: {}", std::io::Error::last_os_error());
                }
                let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = zeroed();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                let ok = SetInformationJobObject(
                    handle,
                    JobObjectExtendedLimitInformation,
                    &info as *const _ as *const core::ffi::c_void,
                    size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                );
                if ok == 0 {
                    let e = std::io::Error::last_os_error();
                    CloseHandle(handle);
                    bail!("SetInformationJobObject failed: {e}");
                }
                Ok(Self { handle })
            }
        }

        /// Puts a spawned child in the job.
        pub fn assign(&self, child: &tokio::process::Child) -> Result<()> {
            let Some(process) = child.raw_handle() else {
                bail!("the process already exited");
            };
            unsafe {
                if AssignProcessToJobObject(self.handle, process) == 0 {
                    bail!("AssignProcessToJobObject failed: {}", std::io::Error::last_os_error());
                }
            }
            Ok(())
        }

        /// A child has exited. Windows tracks this itself.
        pub fn release(&self, _pid: Option<u32>) {}
    }

    impl Drop for JobObject {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.handle);
            }
        }
    }

    pub fn process_alive(pid: u32) -> bool {
        unsafe {
            let h = OpenProcess(PROCESS_SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if h.is_null() {
                return false;
            }
            let alive = WaitForSingleObject(h, 0) == WAIT_TIMEOUT;
            CloseHandle(h);
            alive
        }
    }
}

#[cfg(unix)]
mod imp {
    use std::io::Write;
    use std::os::unix::process::CommandExt;
    use std::process::{Child, ChildStdin, Command, Stdio};
    use std::sync::Mutex;

    use anyhow::{bail, Context, Result};

    /// `+pid` adds a pid, `-pid` removes one. On EOF every listed pid is killed,
    /// along with its process group (servers are spawned as group leaders).
    const WATCHDOG: &str = r#"trap '' INT HUP TERM
pids=""
while read -r line; do
  case "$line" in
    +*) pids="$pids ${line#+}" ;;
    -*) pids=$(for p in $pids; do [ "$p" = "${line#-}" ] || printf '%s ' "$p"; done) ;;
  esac
done
for p in $pids; do kill -KILL -"$p" 2>/dev/null; kill -KILL "$p" 2>/dev/null; done"#;

    pub struct JobObject {
        stdin: Mutex<ChildStdin>,
        // Never waited on: it must outlive us, and exits on its own once we are gone.
        _watchdog: Child,
    }

    impl JobObject {
        pub fn new_kill_on_close() -> Result<Self> {
            let mut watchdog = Command::new("/bin/sh")
                .arg("-c")
                .arg(WATCHDOG)
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                // Its own group, so Ctrl+C in a dev terminal does not take it down first.
                .process_group(0)
                .spawn()
                .context("could not start the child-process watchdog")?;
            let stdin = watchdog.stdin.take().context("watchdog has no stdin")?;
            Ok(Self { stdin: Mutex::new(stdin), _watchdog: watchdog })
        }

        /// Hands a spawned child to the watchdog.
        pub fn assign(&self, child: &tokio::process::Child) -> Result<()> {
            let Some(pid) = child.id() else {
                bail!("the process already exited");
            };
            self.send(&format!("+{pid}")).context("could not reach the child-process watchdog")
        }

        /// A child has exited: forget its pid so a later process reusing it is safe.
        pub fn release(&self, pid: Option<u32>) {
            if let Some(pid) = pid {
                let _ = self.send(&format!("-{pid}"));
            }
        }

        fn send(&self, line: &str) -> std::io::Result<()> {
            let mut stdin = self.stdin.lock().unwrap();
            writeln!(stdin, "{line}")?;
            stdin.flush()
        }
    }

    pub fn process_alive(pid: u32) -> bool {
        let Ok(pid) = libc::pid_t::try_from(pid) else { return false };
        // SAFETY: signal 0 only checks that the process exists and may be signalled.
        let rc = unsafe { libc::kill(pid, 0) };
        rc == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }
}
