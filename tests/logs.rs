use common::{WorkerTestConfig, WorkerTestProject};
use predicates::prelude::predicate;

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

    // Need to sleep a little bit for the program to be able to start
    std::thread::sleep(std::time::Duration::from_secs(1));

    let mut cmd = worker.logs(WorkerTestProject::One);
    cmd.assert().success();
    cmd.assert().stdout(predicate::str::contains(
        "Hello from program started from worker!",
    ));
}
