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

    assert!(worker.state_file(project).is_none());
    assert_eq!(worker.pids(project).len(), 0);
}

#[test]
fn test_stop_success_one_still_running() {
    let worker = WorkerTestConfig::new();
    let project2 = WorkerTestProject::Two;
    let project3 = WorkerTestProject::Three;

    // Start the project
    let mut cmd = worker.start(&[project2, project3]);
    cmd.assert().success();

    // Stop the project
    let mut cmd = worker.stop(&[project2]);
    cmd.assert().success();

    assert!(worker.state_file(project2).is_none());
    assert_eq!(worker.pids(project2).len(), 0);

    // Assert that project 3 is still running
    assert!(worker.state_file(project3).is_some());
    assert_eq!(worker.pids(project3).len(), 1);
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

    assert!(worker.state_file(project2).is_none());
    assert_eq!(worker.pids(project2).len(), 0);

    assert!(worker.state_file(project3).is_none());
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

    assert!(worker.state_file(project2).is_none());
    assert_eq!(worker.pids(project2).len(), 0);

    // Stop the projects
    let mut cmd = worker.stop(&[project2, project3]);
    cmd.assert().success();

    // Assert that the project is still stopped
    assert!(worker.state_file(project2).is_none());
    assert_eq!(worker.pids(project2).len(), 0);

    assert!(worker.state_file(project3).is_none());
    assert_eq!(worker.pids(project3).len(), 0);
}

#[test]
fn test_stop_group_success() {
    let worker = WorkerTestConfig::new();
    let group1 = WorkerTestProject::GroupOne;

    // Start the project
    let mut cmd = worker.start(&[group1]);
    cmd.assert().success();

    let mut cmd = worker.stop(&[group1]);
    cmd.assert().success();

    let projects = worker.group_projects(&group1);

    // Verify that the state file exists
    assert!(worker.state_file(projects[0]).is_none());
    assert!(worker.state_file(projects[1]).is_none());
    assert_eq!(worker.pids(projects[0]).len(), 0);
    assert_eq!(worker.pids(projects[1]).len(), 0);
}

#[test]
fn test_stop_multiple_groups_success() {
    let worker = WorkerTestConfig::new();
    let group1 = WorkerTestProject::GroupOne;
    let group2 = WorkerTestProject::GroupTwo;

    // Start the project
    let mut cmd = worker.start(&[group1, group2]);
    cmd.assert().success();

    let mut cmd = worker.stop(&[group1, group2]);
    cmd.assert().success();

    let projects1 = worker.group_projects(&group1);
    let projects2 = worker.group_projects(&group2);

    // Verify that the state file exists
    assert!(worker.state_file(projects1[0]).is_none());
    assert!(worker.state_file(projects1[1]).is_none());
    assert!(worker.state_file(projects2[0]).is_none());
    assert!(worker.state_file(projects2[1]).is_none());
    assert_eq!(worker.pids(projects1[0]).len(), 0);
    assert_eq!(worker.pids(projects1[1]).len(), 0);
    assert_eq!(worker.pids(projects2[0]).len(), 0);
    assert_eq!(worker.pids(projects2[1]).len(), 0);
}

#[test]
fn test_stop_groups_and_project_success() {
    let worker = WorkerTestConfig::new();
    let group1 = WorkerTestProject::GroupOne;
    let project3 = WorkerTestProject::Three;

    // Start the project
    let mut cmd = worker.start(&[group1, project3]);
    cmd.assert().success();

    let mut cmd = worker.stop(&[group1, project3]);
    cmd.assert().success();

    let projects1 = worker.group_projects(&group1);

    // Verify that the state file exists
    assert!(worker.state_file(projects1[0]).is_none());
    assert!(worker.state_file(projects1[1]).is_none());
    assert!(worker.state_file(project3).is_none());
    assert_eq!(worker.pids(projects1[0]).len(), 0);
    assert_eq!(worker.pids(projects1[1]).len(), 0);
    assert_eq!(worker.pids(project3).len(), 0);
}
