use common::{WorkerTestConfig, WorkerTestProject};

mod common;

#[test]
fn test_start_project_already_running() {
    let worker = WorkerTestConfig::new();
    let project = WorkerTestProject::Three;

    let mut cmd = worker.start(&[project]);
    cmd.assert().success();

    let mut cmd = worker.start(&[project]);
    cmd.assert().stderr(format!(
        "{} is already running\n",
        worker.project_name(&project)
    ));

    assert_eq!(worker.pids(project).len(), 1);
}

#[test]
fn test_start_unknown_project() {
    let worker = WorkerTestConfig::new();

    let mut cmd = worker.start(&[WorkerTestProject::Unknown]);
    cmd.assert().failure();
}

#[test]
fn test_start_success() {
    let worker = WorkerTestConfig::new();
    let project = WorkerTestProject::One;

    let mut cmd = worker.start(&[project]);
    cmd.assert().success();

    // Verify that the state file exists
    let state_file = worker.get_state_file(project);
    assert!(state_file.is_some());
    assert_eq!(worker.pids(project).len(), 1);
}

#[test]
fn test_start_multiple_success() {
    let worker = WorkerTestConfig::new();
    let project1 = WorkerTestProject::One;
    let project2 = WorkerTestProject::Two;

    let mut cmd = worker.start(&[project1, project2]);
    cmd.assert().success();

    // Verify that project 1 is running
    let state_file = worker.get_state_file(project1);
    assert!(state_file.is_some());
    assert_eq!(worker.pids(project1).len(), 1);

    // Verify that project 2 is running
    let state_file = worker.get_state_file(project2);
    assert!(state_file.is_some());
    assert_eq!(worker.pids(project2).len(), 1);
}

#[test]
fn test_start_multiple_one_already_running() {
    let worker = WorkerTestConfig::new();
    let project1 = WorkerTestProject::One;
    let project2 = WorkerTestProject::Two;

    let mut cmd = worker.start(&[project1]);
    cmd.assert().success();

    let pid1 = worker.pids(project1)[0];

    let mut cmd = worker.start(&[project1, project2]);
    cmd.assert().success();

    // Should not start the already running project
    let new_pids1 = worker.pids(project1);
    assert_eq!(pid1, new_pids1[0]);
    assert_eq!(new_pids1.len(), 1);

    // Verify that project 2 is running
    let state_file = worker.get_state_file(project2);
    assert!(state_file.is_some());
    assert_eq!(worker.pids(project2).len(), 1);
}
