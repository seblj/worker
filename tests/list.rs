use common::{WorkerTestConfig, WorkerTestProject};
use predicates::prelude::predicate;

mod common;

#[test]
fn test_list_projects() {
    let worker = WorkerTestConfig::new();
    let mut cmd = worker.list();

    let project1_name = worker.project_name(&WorkerTestProject::One);
    let project2_name = worker.project_name(&WorkerTestProject::Two);
    let project3_name = worker.project_name(&WorkerTestProject::Three);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains(project1_name))
        .stdout(predicate::str::contains(project2_name))
        .stdout(predicate::str::contains(project3_name));
}
