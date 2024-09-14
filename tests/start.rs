use common::{WorkerTestConfig, WorkerTestProject};

mod common;

#[test]
fn test_start_project_already_running() {
    let worker = WorkerTestConfig::new();
    let project = WorkerTestProject::Project3;

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
    let project = WorkerTestProject::Project1;

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
    let project1 = WorkerTestProject::Project1;
    let project2 = WorkerTestProject::Project2;

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
