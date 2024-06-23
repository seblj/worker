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

pub fn killpg(sid: i32) -> Result<(), i32> {
    match unsafe { libc::killpg(sid, libc::SIGINT) } {
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
