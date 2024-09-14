use common::{WorkerTestConfig, WorkerTestProject};

mod common;

#[test]
fn test_stop_unknown_project() {
    let worker = WorkerTestConfig::new();

    let mut cmd = worker.stop(&[WorkerTestProject::Unknown]);
    cmd.assert().failure();
}

#[test]
fn test_stop_command_not_running() {
    let worker = WorkerTestConfig::new();
    let project = WorkerTestProject::Two;

    let mut cmd = worker.stop(&[project]);
    cmd.assert().stderr(format!(
        "Cannot stop project not running: {}\n",
        worker.project_name(&project)
    ));
}

#[test]
fn test_stop_success() {
    let worker = WorkerTestConfig::new();
    let project = WorkerTestProject::Two;

    // Start the project
    let mut cmd = worker.start(&[project]);
    cmd.assert().success();

    // Stop the project
    let mut cmd = worker.stop(&[project]);
    cmd.assert().success();

    let state_file = worker.get_state_file(project);
    assert!(state_file.is_none());
    assert_eq!(worker.pids(project).len(), 0);
}

#[test]
fn test_stop_multiple_success() {
    let worker = WorkerTestConfig::new();
    let project2 = WorkerTestProject::Two;
    let project3 = WorkerTestProject::Three;

    // Start the projects
    let mut cmd = worker.start(&[project2, project3]);
    cmd.assert().success();

    // Stop the projects
    let mut cmd = worker.stop(&[project2, project3]);
    cmd.assert().success();

    let state_file = worker.get_state_file(project2);
    assert!(state_file.is_none());
    assert_eq!(worker.pids(project2).len(), 0);

    let state_file = worker.get_state_file(project3);
    assert!(state_file.is_none());
    assert_eq!(worker.pids(project3).len(), 0);
}

#[test]
fn test_stop_multiple_one_already_stopped() {
    let worker = WorkerTestConfig::new();
    let project2 = WorkerTestProject::Two;
    let project3 = WorkerTestProject::Three;

    // Start the projects
    let mut cmd = worker.start(&[project2, project3]);
    cmd.assert().success();

    // Stop the projects
    let mut cmd = worker.stop(&[project2]);
    cmd.assert().success();

    let state_file = worker.get_state_file(project2);
    assert!(state_file.is_none());
    assert_eq!(worker.pids(project2).len(), 0);

    // Stop the projects
    let mut cmd = worker.stop(&[project2, project3]);
    cmd.assert().success();

    // Assert that the project is still stopped
    let state_file = worker.get_state_file(project2);
    assert!(state_file.is_none());
    assert_eq!(worker.pids(project2).len(), 0);

    let state_file = worker.get_state_file(project3);
    assert!(state_file.is_none());
    assert_eq!(worker.pids(project3).len(), 0);
}
