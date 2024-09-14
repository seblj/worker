use std::fs::DirEntry;

use assert_cmd::{cargo::cargo_bin, Command};
use sysinfo::{Pid, System};
use tempfile::TempDir;
use uuid::Uuid;

#[derive(Clone, Copy)]
pub enum WorkerTestProject {
    Project1,
    Project2,
    Project3,
    Unknown,
}

pub struct WorkerTestConfig {
    path: TempDir,
    project1: String,
    project2: String,
    project3: String,
    unique_id: String,
}

impl Default for WorkerTestConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkerTestConfig {
    pub fn new() -> Self {
        let path = TempDir::new().unwrap();
        let unique_id = Uuid::new_v4().to_string();

        let mock_worker_path = cargo_bin("mock_worker").to_string_lossy().to_string();

        let project1 = format!("{} {} 5", mock_worker_path, unique_id);
        let project2 = format!("{} {} 6", mock_worker_path, unique_id);
        let project3 = format!("{} {} 7", mock_worker_path, unique_id);

        // Create the .worker.toml file
        std::fs::write(
            path.path().join(".worker.toml"),
            format!(
                r#"
            [[project]]
            name = "project-1-{unique_id}"
            command = "{project1}"
            cwd = "/"

            [[project]]
            name = "project-2-{unique_id}"
            command = "{project2}"
            cwd = "/"

            [[project]]
            name = "project-3-{unique_id}"
            command = "{project3}"
            cwd = "/"
            "#,
            ),
        )
        .unwrap();

        WorkerTestConfig {
            path,
            project1,
            project2,
            project3,
            unique_id,
        }
    }

    fn run(&self, command: &str, projects: &[WorkerTestProject]) -> Command {
        let mut cmd = Command::cargo_bin("worker").unwrap();
        let projects = projects
            .iter()
            .map(|p| self.project_name(p))
            .collect::<Vec<_>>();
        cmd.current_dir(&self.path)
            .arg("--base-dir")
            .arg(self.path.path())
            .arg(command)
            .args(&projects);
        cmd
    }

    pub fn start(&self, projects: &[WorkerTestProject]) -> Command {
        self.run("start", projects)
    }

    pub fn restart(&self, projects: &[WorkerTestProject]) -> Command {
        self.run("restart", projects)
    }

    pub fn stop(&self, projects: &[WorkerTestProject]) -> Command {
        self.run("stop", projects)
    }

    pub fn get_state_file(
        &self,
        project: WorkerTestProject,
    ) -> Option<Result<DirEntry, std::io::Error>> {
        self.path
            .path()
            .join(".worker/state")
            .read_dir()
            .unwrap()
            .find(|entry| {
                entry
                    .as_ref()
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .contains(&self.project_name(&project))
            })
    }

    pub fn project_name(&self, project: &WorkerTestProject) -> String {
        match project {
            WorkerTestProject::Project1 => format!("project-1-{}", self.unique_id),
            WorkerTestProject::Project2 => format!("project-2-{}", self.unique_id),
            WorkerTestProject::Project3 => format!("project-3-{}", self.unique_id),
            WorkerTestProject::Unknown => "unknown".into(),
        }
    }

    pub fn pids(&self, project: WorkerTestProject) -> Vec<Pid> {
        // Verify that the process is running using sysinfo
        let mut system = System::new_all();
        system.refresh_all();

        let processes = system.processes();

        let cmd = match project {
            WorkerTestProject::Project1 => self.project1.split_whitespace(),
            WorkerTestProject::Project2 => self.project2.split_whitespace(),
            WorkerTestProject::Project3 => self.project3.split_whitespace(),
            WorkerTestProject::Unknown => unreachable!(),
        }
        .collect::<Vec<_>>();

        processes
            .values()
            .filter_map(|p| if p.cmd() == cmd { Some(p.pid()) } else { None })
            .collect()
    }
}
