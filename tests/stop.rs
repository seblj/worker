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
    let project = WorkerTestProject::Project2;

    let mut cmd = worker.stop(&[project]);
    cmd.assert().stderr(format!(
        "Cannot stop project not running: {}\n",
        worker.project_name(&project)
    ));
}

#[test]
fn test_stop_success() {
    let worker = WorkerTestConfig::new();
    let project = WorkerTestProject::Project2;

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
    let project2 = WorkerTestProject::Project2;
    let project3 = WorkerTestProject::Project3;

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
