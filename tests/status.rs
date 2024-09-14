use common::{WorkerTestConfig, WorkerTestProject};

mod common;

#[test]
fn test_status_none_running() {
    let worker = WorkerTestConfig::new();
    let mut cmd = worker.status();
    cmd.assert().success();
    cmd.assert().stdout("");
}

#[test]
fn test_status_one_running() {
    let worker = WorkerTestConfig::new();
    let project1 = WorkerTestProject::One;
    let project2 = WorkerTestProject::Two;

    let project1_name = worker.project_name(&project1);
    let project2_name = worker.project_name(&project2);

    let mut cmd = worker.start(&[project1]);
    cmd.assert().success();

    let mut cmd = worker.status();
    cmd.assert().success();

    let output = &cmd.output().unwrap().stdout;
    let stdout = std::str::from_utf8(output).unwrap();

    assert!(stdout.contains(&project1_name));
    assert!(!stdout.contains(&project2_name));
}

#[test]
fn test_status_multiple_running() {
    let worker = WorkerTestConfig::new();
    let project1 = WorkerTestProject::One;
    let project2 = WorkerTestProject::Two;
    let project3 = WorkerTestProject::Three;

    let project1_name = worker.project_name(&project1);
    let project2_name = worker.project_name(&project2);
    let project3_name = worker.project_name(&project3);

    let mut cmd = worker.start(&[project1, project2]);
    cmd.assert().success();

    let mut cmd = worker.status();
    cmd.assert().success();

    let output = &cmd.output().unwrap().stdout;
    let stdout = std::str::from_utf8(output).unwrap();

    assert!(stdout.contains(&project1_name));
    assert!(stdout.contains(&project2_name));
    assert!(!stdout.contains(&project3_name));
}
