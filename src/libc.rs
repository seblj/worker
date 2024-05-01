use std::fs::File;

use sysinfo::System;

use crate::{Project, STATE_DIR};

pub enum Fork {
    Parent(libc::pid_t),
    Child,
}

fn fork() -> Result<Fork, i32> {
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

// TODO: I need to capture the return output from `setsid` or something, and then later kill all
// processes inside that group. This should fix my problems
pub fn daemon(project: &Project) -> Result<Fork, i32> {
    match fork() {
        Ok(Fork::Child) => setsid().and_then(|sid| {
            let filename = format!("{}-{}", project.name, sid);
            let state_file = STATE_DIR.join(filename);

            let file = File::create(state_file).expect("Couldn't create state file");
            serde_json::to_writer(file, project).expect("Couldn't write to state file");
            fork()
        }),
        x => x,
    }
}

// Sends SIGKILL to the process group provided, killing all processes
pub fn kill(sid: i32) -> Result<(), i32> {
    match unsafe { libc::killpg(sid, libc::SIGKILL) } {
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
