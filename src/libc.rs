use sysinfo::{Pid, System};

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

pub fn daemon() -> Result<Fork, i32> {
    match fork() {
        Ok(Fork::Child) => setsid().and_then(|_| fork()),
        x => x,
    }
}

// Sends SIGTERM process to `pid`, terminating the entire process tree
pub fn terminate(pid: i32) -> Result<(), i32> {
    match unsafe { libc::kill(pid, libc::SIGTERM) } {
        0 => Ok(()),
        e => Err(e),
    }
}

pub fn is_process_running(pid: libc::pid_t) -> bool {
    let mut sys = System::new();
    sys.refresh_all();
    sys.process(Pid::from(pid as usize)).is_some()
}
