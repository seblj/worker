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
    assert!(worker.state_file(project).is_some());
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
    assert!(worker.state_file(project1).is_some());
    assert_eq!(worker.pids(project1).len(), 1);

    // Verify that project 2 is running
    assert!(worker.state_file(project2).is_some());
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
    assert!(worker.state_file(project2).is_some());
    assert_eq!(worker.pids(project2).len(), 1);
}

#[test]
fn test_start_group_success() {
    let worker = WorkerTestConfig::new();
    let group1 = WorkerTestProject::GroupOne;

    let mut cmd = worker.start(&[group1]);
    cmd.assert().success();

    let projects = worker.group_projects(&group1);

    // Verify that the state file exists
    assert!(worker.state_file(projects[0]).is_some());
    assert!(worker.state_file(projects[1]).is_some());
    assert_eq!(worker.pids(projects[0]).len(), 1);
    assert_eq!(worker.pids(projects[1]).len(), 1);
}

#[test]
fn test_start_group_and_project_success() {
    let worker = WorkerTestConfig::new();
    let group1 = WorkerTestProject::GroupOne;
    let project3 = WorkerTestProject::Three;

    let mut cmd = worker.start(&[group1, project3]);
    cmd.assert().success();

    let projects = worker.group_projects(&group1);

    // Verify that the state file exists
    assert!(worker.state_file(projects[0]).is_some());
    assert!(worker.state_file(projects[1]).is_some());
    assert!(worker.state_file(project3).is_some());
    assert_eq!(worker.pids(projects[0]).len(), 1);
    assert_eq!(worker.pids(projects[1]).len(), 1);
    assert_eq!(worker.pids(project3).len(), 1);
}

#[test]
fn test_start_multiple_groups() {
    let worker = WorkerTestConfig::new();
    let group1 = WorkerTestProject::GroupOne;
    let group2 = WorkerTestProject::GroupTwo;

    let mut cmd = worker.start(&[group1, group2]);
    cmd.assert().success();

    let projects1 = worker.group_projects(&group1);
    let projects2 = worker.group_projects(&group1);

    // Verify that the state file exists
    assert!(worker.state_file(projects1[0]).is_some());
    assert!(worker.state_file(projects1[1]).is_some());
    assert!(worker.state_file(projects2[0]).is_some());
    assert!(worker.state_file(projects2[1]).is_some());

    assert_eq!(worker.pids(projects1[0]).len(), 1);
    assert_eq!(worker.pids(projects1[1]).len(), 1);
    assert_eq!(worker.pids(projects2[0]).len(), 1);
    assert_eq!(worker.pids(projects2[1]).len(), 1);
}
