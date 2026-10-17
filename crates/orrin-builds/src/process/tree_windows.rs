use std::ffi::c_void;

use orrin_core::{Error, Result};
use tokio::process::{Child, Command};
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Console::GetConsoleWindow;
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_CPU_RATE_CONTROL_ENABLE,
    JOB_OBJECT_CPU_RATE_CONTROL_HARD_CAP, JOB_OBJECT_LIMIT_JOB_MEMORY, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_CPU_RATE_CONTROL_INFORMATION, JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectCpuRateControlInformation,
    JobObjectExtendedLimitInformation, SetInformationJobObject, TerminateJobObject,
};
use windows::Win32::System::Threading::{CREATE_NO_WINDOW, CREATE_SUSPENDED, PROCESS_CREATION_FLAGS};
use windows::core::PCWSTR;

use super::Limits;

#[link(name = "ntdll", kind = "raw-dylib")]
unsafe extern "system" {
    fn NtResumeProcess(process: HANDLE) -> i32;
}

pub struct Tree(HANDLE);

unsafe impl Send for Tree {}
unsafe impl Sync for Tree {}

impl Tree {
    pub fn spawn(command: &mut Command, limits: Limits) -> Result<(Child, Tree)> {
        let tree = Tree::create(limits)?;
        command.creation_flags((CREATE_SUSPENDED | own_console()).0);
        let mut child = command.spawn()?;
        let started = match child.raw_handle().map(HANDLE) {
            Some(process) => tree.adopt(process),
            None => Err(Error::internal("the job's process has no handle")),
        };
        if let Err(err) = started {
            let _ = child.start_kill();
            return Err(err);
        }
        Ok((child, tree))
    }

    pub fn kill(&self) {
        unsafe {
            let _ = TerminateJobObject(self.0, 1);
        }
    }

    fn adopt(&self, process: HANDLE) -> Result<()> {
        unsafe { AssignProcessToJobObject(self.0, process) }
            .map_err(|_| Error::internal("could not put the job's process in a job object"))?;
        match unsafe { NtResumeProcess(process) } {
            0 => Ok(()),
            status => Err(Error::internal(format!("could not resume the job's process ({status:#x})"))),
        }
    }

    fn create(limits: Limits) -> Result<Tree> {
        let tree = Tree(unsafe { CreateJobObjectW(None, PCWSTR::null()) }.map_err(win)?);
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if let Some(mb) = limits.memory_mb {
            info.BasicLimitInformation.LimitFlags |= JOB_OBJECT_LIMIT_JOB_MEMORY;
            info.JobMemoryLimit = (mb * 1024 * 1024) as usize;
        }
        tree.set(JobObjectExtendedLimitInformation, &info)?;
        if let Some(percent) = limits.cpu_percent.filter(|p| *p < 100) {
            let mut cpu = JOBOBJECT_CPU_RATE_CONTROL_INFORMATION {
                ControlFlags: JOB_OBJECT_CPU_RATE_CONTROL_ENABLE | JOB_OBJECT_CPU_RATE_CONTROL_HARD_CAP,
                ..Default::default()
            };
            cpu.Anonymous.CpuRate = percent.max(1) * 100;
            tree.set(JobObjectCpuRateControlInformation, &cpu)?;
        }
        Ok(tree)
    }

    fn set<T>(&self, class: windows::Win32::System::JobObjects::JOBOBJECTINFOCLASS, info: &T) -> Result<()> {
        let size = std::mem::size_of::<T>() as u32;
        unsafe { SetInformationJobObject(self.0, class, info as *const T as *const c_void, size) }.map_err(win)
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

fn own_console() -> PROCESS_CREATION_FLAGS {
    match unsafe { GetConsoleWindow() }.is_invalid() {
        true => CREATE_NO_WINDOW,
        false => PROCESS_CREATION_FLAGS(0),
    }
}

fn win(err: windows::core::Error) -> Error {
    Error::internal(err.message())
}
