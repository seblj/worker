use common::{WorkerTestConfig, WorkerTestProject};

mod common;

#[test]
fn test_restart_unknown_project() {
    let worker = WorkerTestConfig::new();
    let mut cmd = worker.restart(&[WorkerTestProject::Unknown]);
    cmd.assert().failure();
}

#[test]
fn test_restart_success() {
    let worker = WorkerTestConfig::new();
    let project = WorkerTestProject::One;

    let mut cmd = worker.start(&[project]);
    cmd.assert().success();

    // Verify that the state file exists
    assert!(worker.state_file(project).is_some());

    let pid = worker.pids(project)[0];

    let mut cmd = worker.restart(&[project]);
    cmd.assert().success();

    // Verify that the state file exists
    assert!(worker.state_file(project).is_some());

    let new_pid = worker.pids(project)[0];

    assert_ne!(pid, new_pid);
}

#[test]
fn test_restart_multiple_success() {
    let worker = WorkerTestConfig::new();
    let project1 = WorkerTestProject::One;
    let project2 = WorkerTestProject::Two;

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

#[test]
fn test_restart_multiple_only_one_running() {
    let worker = WorkerTestConfig::new();
    let project1 = WorkerTestProject::One;
    let project2 = WorkerTestProject::Two;

    let mut cmd = worker.start(&[project1]);
    cmd.assert().success();

    let pid1 = worker.pids(project1)[0];

    let mut cmd = worker.restart(&[project1, project2]);
    cmd.assert().success();

    let new_pid1 = worker.pids(project1)[0];
    assert_ne!(pid1, new_pid1);

    // Verify that the project that wasn't running is not started
    assert!(worker.state_file(project2).is_none());
    assert_eq!(worker.pids(project2).len(), 0);
}

#[test]
fn test_restart_group_success() {
    let worker = WorkerTestConfig::new();
    let group = WorkerTestProject::GroupOne;

    let mut cmd = worker.start(&[group]);
    cmd.assert().success();

    let projects = worker.group_projects(&group);

    assert!(worker.state_file(projects[0]).is_some());
    assert!(worker.state_file(projects[1]).is_some());

    let pid1 = worker.pids(projects[0])[0];
    let pid2 = worker.pids(projects[1])[0];

    let mut cmd = worker.restart(&[group]);
    cmd.assert().success();

    // Verify that the state file exists
    assert!(worker.state_file(projects[0]).is_some());
    assert!(worker.state_file(projects[1]).is_some());

    let new_pid1 = worker.pids(projects[0])[0];
    let new_pid2 = worker.pids(projects[1])[0];

    assert_ne!(pid1, new_pid1);
    assert_ne!(pid2, new_pid2);
}

#[test]
fn test_restart_multiple_group_success() {
    let worker = WorkerTestConfig::new();
    let group1 = WorkerTestProject::GroupOne;
    let group2 = WorkerTestProject::GroupTwo;

    let mut cmd = worker.start(&[group1, group2]);
    cmd.assert().success();

    let projects1 = worker.group_projects(&group1);
    let projects2 = worker.group_projects(&group2);

    assert!(worker.state_file(projects1[0]).is_some());
    assert!(worker.state_file(projects1[1]).is_some());

    let pid1 = worker.pids(projects1[0])[0];
    let pid2 = worker.pids(projects1[1])[0];
    let pid3 = worker.pids(projects2[0])[0];
    let pid4 = worker.pids(projects2[1])[0];

    let mut cmd = worker.restart(&[group1, group2]);
    cmd.assert().success();

    // Verify that the state file exists
    assert!(worker.state_file(projects1[0]).is_some());
    assert!(worker.state_file(projects1[0]).is_some());
    assert!(worker.state_file(projects2[1]).is_some());
    assert!(worker.state_file(projects2[1]).is_some());

    let new_pid1 = worker.pids(projects1[0])[0];
    let new_pid2 = worker.pids(projects1[1])[0];
    let new_pid3 = worker.pids(projects2[0])[0];
    let new_pid4 = worker.pids(projects2[1])[0];

    assert_ne!(pid1, new_pid1);
    assert_ne!(pid2, new_pid2);
    assert_ne!(pid3, new_pid3);
    assert_ne!(pid4, new_pid4);
}

#[test]
fn test_restart_group_one_project_running() {
    let worker = WorkerTestConfig::new();
    let group1 = WorkerTestProject::GroupOne;
    let projects = worker.group_projects(&group1);

    let mut cmd = worker.start(&[projects[0]]);
    cmd.assert().success();

    let pid1 = worker.pids(projects[0])[0];

    let mut cmd = worker.restart(&[group1]);
    cmd.assert().success();

    // Verify that the state file exists
    assert!(worker.state_file(projects[0]).is_some());
    assert!(worker.state_file(projects[0]).is_some());

    let new_pid1 = worker.pids(projects[0])[0];
    assert_ne!(pid1, new_pid1);

    // Verify that the project that wasn't running is not started
    assert!(worker.state_file(projects[1]).is_none());
    assert_eq!(worker.pids(projects[1]).len(), 0);
}
