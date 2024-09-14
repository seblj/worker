use common::{WorkerTestConfig, WorkerTestProject};

mod common;

#[test]
fn test_restart_success() {
    let worker = WorkerTestConfig::new();
    let project = WorkerTestProject::Project1;

    let mut cmd = worker.start(&[project]);
    cmd.assert().success();

    // Verify that the state file exists
    let state_file = worker.get_state_file(project);
    assert!(state_file.is_some());

    let pid = worker.pids(project)[0];

    let mut cmd = worker.restart(&[project]);
    cmd.assert().success();

    // Verify that the state file exists
    let state_file = worker.get_state_file(project);
    assert!(state_file.is_some());

    let new_pid = worker.pids(project)[0];

    assert_ne!(pid, new_pid);
}

#[test]
fn test_restart_multiple_success() {
    let worker = WorkerTestConfig::new();
    let project1 = WorkerTestProject::Project1;
    let project2 = WorkerTestProject::Project2;

    let mut cmd = worker.start(&[project1, project2]);
    cmd.assert().success();

    let pid1 = worker.pids(project1)[0];
    let pid2 = worker.pids(project2)[0];

    let mut cmd = worker.restart(&[project1, project2]);
    cmd.assert().success();

    let new_pid1 = worker.pids(project1)[0];
    let new_pid2 = worker.pids(project2)[0];

    assert_ne!(pid1, new_pid1);
    assert_ne!(pid2, new_pid2);
}
