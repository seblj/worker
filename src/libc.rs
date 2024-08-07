use std::fs::File;

use serde::{Deserialize, Serialize};
use sysinfo::System;

use crate::{config::WorkerConfig, Project};

pub enum Fork {
    Parent(libc::pid_t),
    Child,
}

pub fn fork() -> Result<Fork, i32> {
    let res = unsafe { libc::fork() };
    match res {
        -1 => Err(-1),
        0 => Ok(Fork::Child),
        res => Ok(Fork::Parent(res)),
    }
}

pub fn setsid() -> Result<libc::pid_t, i32> {
    let res = unsafe { libc::setsid() };
    match res {
        -1 => Err(-1),
        res => Ok(res),
    }
}

pub fn daemon(config: &WorkerConfig, project: &Project) -> Result<Fork, i32> {
    match fork() {
        Ok(Fork::Child) => setsid().and_then(|sid| {
            let filename = format!("{}-{}", project.name, sid);
            let state_file = config.state_dir.join(filename);

            let file = File::create(state_file).expect("Couldn't create state file");
            serde_json::to_writer(file, project).expect("Couldn't write to state file");
            fork()
        }),
        x => x,
    }
}

pub fn stop_pg(sid: i32, signal: &Signal) -> Result<(), i32> {
    match unsafe { libc::killpg(sid, signal.to_owned() as i32) } {
        0 => Ok(()),
        e => Err(e),
    }
}

pub fn has_processes_running(sid: libc::pid_t) -> bool {
    let mut sys = System::new();
    sys.refresh_all();
    sys.processes().iter().any(|(_, p)| {
        p.session_id()
            .is_some_and(|session_id| session_id.as_u32() == sid as u32)
    })
}

#[derive(Deserialize, Clone, Debug, Serialize, Hash, PartialEq, Eq)]
#[non_exhaustive]
#[repr(i32)]
pub enum Signal {
    SIGHUP = 1,
    SIGINT = 2,
    SIGQUIT = 3,
    SIGILL = 4,
    SIGTRAP = 5,
    SIGABRT = 6,
    SIGBUS = 7,
    SIGFPE = 8,
    SIGKILL = 9,
    SIGUSR1 = 10,
    SIGSEGV = 11,
    SIGUSR2 = 12,
    SIGPIPE = 13,
    SIGALRM = 14,
    SIGTERM = 15,
    SIGSTKFLT = 16,
    SIGCHLD = 17,
    SIGCONT = 18,
    SIGSTOP = 19,
    SIGTSTP = 20,
    SIGTTIN = 21,
    SIGTTOU = 22,
    SIGURG = 23,
    SIGXCPU = 24,
    SIGXFSZ = 25,
    SIGVTALRM = 26,
    SIGPROF = 27,
    SIGWINCH = 28,
    SIGIO = 29,
    SIGPWR = 30,
    SIGSYS = 31,
}
