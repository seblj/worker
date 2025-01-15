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
    GroupOne,
    GroupTwo,
    Unknown,
}

#[derive(Clone, Copy)]
pub enum WorkerTestGroup {
    One,
    Two,
}

pub struct WorkerTestConfig {
    dir: TempDir,
    cmds: [String; 4],
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
        let dir = TempDir::with_prefix(Uuid::new_v4().to_string()).unwrap();

        let mock_path = cargo_bin("mock").to_string_lossy().to_string();

        let name1 = Uuid::new_v4();
        let name2 = Uuid::new_v4();
        let name3 = Uuid::new_v4();
        let name4 = Uuid::new_v4();

        let group1 = Uuid::new_v4();
        let group2 = Uuid::new_v4();

        let cmd1 = format!("{} {}", mock_path, name1);
        let cmd2 = format!("{} {}", mock_path, name2);
        let cmd3 = format!("{} {}", mock_path, name3);
        let cmd4 = format!("{} {}", mock_path, name4);

        // Create the .worker.toml file
        std::fs::write(
            dir.path().join(".worker.toml"),
            format!(
                r#"
            [[project]]
            name = "{name1}"
            command = "{cmd1}"
            cwd = "/"
            group = [ "{group1}", "{group2}" ]

            [[project]]
            name = "{name2}"
            command = "{cmd2}"
            cwd = "/"
            group = [ "{group1}" ]

            [[project]]
            name = "{name3}"
            command = "{cmd3}"
            cwd = "/"
            group = [ "{group2}" ]

            [[project]]
            name = "{name4}"
            command = "{cmd4}"
            cwd = "/"
            "#
            ),
        )
        .unwrap();

        WorkerTestConfig {
            dir,
            cmds: [cmd1, cmd2, cmd3, cmd4],
            names: [name1, name2, name3, name4],
            groups: [group1, group2],
        }
    }

    fn run(&self, command: &str, projects: Option<&[WorkerTestProject]>) -> Command {
        let mut cmd = Command::cargo_bin("worker").unwrap();
        cmd.current_dir(&self.dir).arg(command);

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
        let dir = self.dir.path().join(".worker/state").read_dir().unwrap();

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
            WorkerTestProject::One => self.cmds[0].split_whitespace(),
            WorkerTestProject::Two => self.cmds[1].split_whitespace(),
            WorkerTestProject::Three => self.cmds[2].split_whitespace(),
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
