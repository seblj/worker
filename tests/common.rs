#![allow(dead_code)]
use std::fs::DirEntry;

use assert_cmd::{cargo::cargo_bin, Command};
use sysinfo::{Pid, System};
use tempfile::TempDir;
use uuid::Uuid;

#[derive(Clone, Copy)]
pub enum WorkerTestProject {
    One,
    Two,
    Three,
    Unknown,
}

pub struct WorkerTestConfig {
    path: TempDir,
    projects: [String; 3],
    unique_id: String,
}

impl Default for WorkerTestConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkerTestConfig {
    pub fn new() -> Self {
        let unique_id = Uuid::new_v4().to_string();
        let path = TempDir::with_prefix(&unique_id).unwrap();

        let mock_worker_path = cargo_bin("mock_worker").to_string_lossy().to_string();

        let projects = [
            format!("{} 5 {}", mock_worker_path, unique_id),
            format!("{} 6 {}", mock_worker_path, unique_id),
            format!("{} 7 {}", mock_worker_path, unique_id),
        ];

        // Create the .worker.toml file
        std::fs::write(
            path.path().join(".worker.toml"),
            format!(
                r#"
            [[project]]
            name = "project-1-{unique_id}"
            command = "{}"
            cwd = "/"

            [[project]]
            name = "project-2-{unique_id}"
            command = "{}"
            cwd = "/"

            [[project]]
            name = "project-3-{unique_id}"
            command = "{}"
            cwd = "/"
            "#,
                projects[0], projects[1], projects[2],
            ),
        )
        .unwrap();

        WorkerTestConfig {
            path,
            projects,
            unique_id,
        }
    }

    fn run(&self, command: &str, projects: Option<&[WorkerTestProject]>) -> Command {
        let mut cmd = Command::cargo_bin("worker").unwrap();
        cmd.current_dir(&self.path)
            .arg("--base-dir")
            .arg(self.path.path())
            .arg(command);

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

    pub fn logs(&self, projects: WorkerTestProject) -> Command {
        self.run("logs", Some(&[projects]))
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
            WorkerTestProject::One => format!("project-1-{}", self.unique_id),
            WorkerTestProject::Two => format!("project-2-{}", self.unique_id),
            WorkerTestProject::Three => format!("project-3-{}", self.unique_id),
            WorkerTestProject::Unknown => "unknown".into(),
        }
    }

    pub fn pids(&self, project: WorkerTestProject) -> Vec<Pid> {
        // Verify that the process is running using sysinfo
        let cmd = match project {
            WorkerTestProject::One => self.projects[0].split_whitespace(),
            WorkerTestProject::Two => self.projects[1].split_whitespace(),
            WorkerTestProject::Three => self.projects[2].split_whitespace(),
            WorkerTestProject::Unknown => unreachable!(),
        }
        .collect::<Vec<_>>();

        System::new_all()
            .processes()
            .values()
            .filter_map(|p| if p.cmd() == cmd { Some(p.pid()) } else { None })
            .collect()
    }
}
