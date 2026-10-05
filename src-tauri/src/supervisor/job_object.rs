//! A Windows Job Object with KILL_ON_JOB_CLOSE (KTD4). Every child process is put in
//! it, so when Glasscraft exits for any reason (including a crash) Windows kills the
//! Java servers and the playit agent and no port stays held by an orphan.

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
