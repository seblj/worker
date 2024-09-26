use std::time::{Duration, Instant};

use common::{WorkerTestConfig, WorkerTestProject};

mod common;

#[test]
fn test_logs_project_not_running() {
    let worker = WorkerTestConfig::new();
    let mut cmd = worker.logs(WorkerTestProject::One);
    cmd.assert().failure();
}

#[test]
fn test_logs_success() {
    let worker = WorkerTestConfig::new();
    let mut cmd = worker.start(&[WorkerTestProject::One]);
    cmd.assert().success();

    let timeout = Duration::new(1, 0);
    let start = Instant::now();

    // Try multiple times since it may not output immediately
    while Instant::now().duration_since(start) < timeout {
        let mut cmd = worker.logs(WorkerTestProject::One);
        cmd.assert().success();

        let output = &cmd.output().unwrap().stdout;
        let stdout = std::str::from_utf8(output).unwrap();
        if stdout.contains("Hello from mock!") {
            return;
        }
    }
    unreachable!("Couldn't find output in 1 second")
}
