#![allow(dead_code)]
use std::fs::DirEntry;

use assert_cmd::{cargo::cargo_bin, Command};
use sysinfo::{Pid, System};
use tempfile::TempDir;
use uuid::Uuid;

pub enum WorkerTestArg {
    Project(WorkerTestProject),
    Group(WorkerTestGroup),
}

#[derive(Clone, Copy)]
pub enum WorkerTestProject {
    One,
    Two,
    Three,
    GroupOne,
    GroupTwo,
    Unknown,
}

#[derive(Clone, Copy)]
pub enum WorkerTestGroup {
    One,
    Two,
}

impl From<WorkerTestGroup> for WorkerTestArg {
    fn from(val: WorkerTestGroup) -> Self {
        WorkerTestArg::Group(val)
    }
}

impl From<WorkerTestProject> for WorkerTestArg {
    fn from(val: WorkerTestProject) -> Self {
        WorkerTestArg::Project(val)
    }
}

pub struct WorkerTestConfig {
    path: TempDir,
    projects: [String; 4],
    names: [Uuid; 4],
    groups: [Uuid; 2],
}

impl Default for WorkerTestConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkerTestConfig {
    pub fn new() -> Self {
        let path = TempDir::with_prefix(Uuid::new_v4().to_string()).unwrap();

        let mock_path = cargo_bin("mock").to_string_lossy().to_string();

        let names = [
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
        ];

        let groups = [Uuid::new_v4(), Uuid::new_v4()];

        let projects = [
            format!("{} {}", mock_path, names[0]),
            format!("{} {}", mock_path, names[1]),
            format!("{} {}", mock_path, names[2]),
            format!("{} {}", mock_path, names[3]),
        ];

        // Create the .worker.toml file
        std::fs::write(
            path.path().join(".worker.toml"),
            format!(
                r#"
            [[project]]
            name = "{}"
            command = "{}"
            cwd = "/"
            group = [ "{}", "{}" ]

            [[project]]
            name = "{}"
            command = "{}"
            cwd = "/"
            group = [ "{}" ]

            [[project]]
            name = "{}"
            command = "{}"
            cwd = "/"
            group = [ "{}" ]

            [[project]]
            name = "{}"
            command = "{}"
            cwd = "/"
            "#,
                names[0],
                projects[0],
                groups[0],
                groups[1],
                names[1],
                projects[1],
                groups[0],
                names[2],
                projects[2],
                groups[1],
                names[3],
                projects[3],
            ),
        )
        .unwrap();

        WorkerTestConfig {
            path,
            projects,
            names,
            groups,
        }
    }

    fn run(&self, command: &str, projects: Option<&[WorkerTestProject]>) -> Command {
        let mut cmd = Command::cargo_bin("worker").unwrap();
        cmd.current_dir(&self.path).arg(command);
        println!("{}", self.path.path().display());

        if let Some(projects) = projects {
            let projects = projects
                .iter()
                .map(|p| self.project_name(p))
                .collect::<Vec<_>>();
            cmd.args(&projects);
        }

        cmd
    }

    pub fn start(&self, projects: &[WorkerTestProject]) -> Command {
        self.run("start", Some(projects))
    }

    pub fn logs(&self, project: WorkerTestProject) -> Command {
        self.run("logs", Some(&[project]))
    }

    pub fn restart(&self, projects: &[WorkerTestProject]) -> Command {
        self.run("restart", Some(projects))
    }

    pub fn stop(&self, projects: &[WorkerTestProject]) -> Command {
        self.run("stop", Some(projects))
    }

    pub fn list(&self) -> Command {
        self.run("list", None)
    }

    pub fn status(&self) -> Command {
        self.run("status", None)
    }

    // Depends on `new()`. Used for asserting that the projects have actually started
    pub fn group_projects(&self, group: &WorkerTestProject) -> &[WorkerTestProject; 2] {
        match group {
            WorkerTestProject::GroupOne => &[WorkerTestProject::One, WorkerTestProject::Two],
            WorkerTestProject::GroupTwo => &[WorkerTestProject::Two, WorkerTestProject::Three],
            WorkerTestProject::One => unreachable!(),
            WorkerTestProject::Two => unreachable!(),
            WorkerTestProject::Three => unreachable!(),
            WorkerTestProject::Unknown => unreachable!(),
        }
    }

    pub fn state_file(&self, project: WorkerTestProject) -> Option<DirEntry> {
        let dir = self.path.path().join(".worker/state").read_dir().unwrap();

        for entry in dir.into_iter() {
            let entry = entry.unwrap();
            let name = &self.project_name(&project);
            if entry.file_name().to_string_lossy().contains(name) {
                return Some(entry);
            }
        }

        None
    }

    pub fn project_name(&self, project: &WorkerTestProject) -> String {
        match project {
            WorkerTestProject::One => self.names[0].to_string(),
            WorkerTestProject::Two => self.names[1].to_string(),
            WorkerTestProject::Three => self.names[2].to_string(),
            WorkerTestProject::Unknown => "unknown".into(),
            WorkerTestProject::GroupOne => self.groups[0].to_string(),
            WorkerTestProject::GroupTwo => self.groups[1].to_string(),
        }
    }

    pub fn pids(&self, project: WorkerTestProject) -> Vec<Pid> {
        // Verify that the process is running using sysinfo
        let cmd = match project {
            WorkerTestProject::One => self.projects[0].split_whitespace(),
            WorkerTestProject::Two => self.projects[1].split_whitespace(),
            WorkerTestProject::Three => self.projects[2].split_whitespace(),
            WorkerTestProject::Unknown => unreachable!(),
            WorkerTestProject::GroupOne => unreachable!(),
            WorkerTestProject::GroupTwo => unreachable!(),
        }
        .collect::<Vec<_>>();

        System::new_all()
            .processes()
            .values()
            .filter_map(|p| if p.cmd() == cmd { Some(p.pid()) } else { None })
            .collect()
    }
}
