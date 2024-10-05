use std::{collections::HashMap, fs::File, hash::Hash, path::PathBuf, str::FromStr};

use anyhow::{anyhow, Context};
use itertools::{Either, Itertools};
use serde::{Deserialize, Serialize};

use crate::libc::{has_processes_running, stop_pg, Signal};

const CONFIG_FILE: &str = ".worker.toml";

#[derive(Deserialize, Debug)]
pub struct Config {
    pub project: Vec<Project>,
}

pub trait WorkerProject {
    fn name(&self) -> &str;
}

/// Project deserialized from config file
#[derive(Deserialize, Serialize, Clone, Debug, Eq, PartialEq)]
pub struct Project {
    pub name: String,
    pub command: String,
    pub cwd: String,
    pub display: Option<String>,
    pub stop_signal: Option<Signal>,
    pub envs: Option<HashMap<String, String>>,
}

/// Project with process id
#[derive(Deserialize, Serialize, Clone, Debug, Eq, PartialEq)]
pub struct RunningProject {
    pub name: String,
    pub command: String,
    pub cwd: String,
    pub display: Option<String>,
    pub stop_signal: Option<Signal>,
    pub envs: Option<HashMap<String, String>>,
    pub pid: i32,
}

impl Hash for Project {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state)
    }
}

macro_rules! impl_display {
    ($project:tt) => {
        impl std::fmt::Display for $project {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                if let Some(ref display) = self.display {
                    write!(f, "{} ({})", display, self.name)
                } else {
                    write!(f, "{}", self.name)
                }
            }
        }
    };
}

macro_rules! impl_worker_project {
    ($project:tt) => {
        impl WorkerProject for $project {
            fn name(&self) -> &str {
                &self.name
            }
        }
    };
}

impl_display!(Project);
impl_display!(RunningProject);

impl_worker_project!(Project);
impl_worker_project!(RunningProject);

fn find_project(name: &str) -> Result<Project, anyhow::Error> {
    let config = WorkerConfig::new()?;
    let projects: Vec<String> = config.projects.iter().map(|p| p.name.clone()).collect();

    config
        .projects
        .into_iter()
        .find(|it| it.name == name)
        .with_context(|| format!("Valid projects are {:#?}", projects))
}

impl From<RunningProject> for Project {
    fn from(value: RunningProject) -> Self {
        Self {
            name: value.name,
            command: value.command,
            cwd: value.cwd,
            display: value.display,
            stop_signal: value.stop_signal,
            envs: value.envs,
        }
    }
}

impl FromStr for Project {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        find_project(s)
    }
}

impl FromStr for RunningProject {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (name, pid) = s.rsplit_once('-').context("No - in string")?;
        let project = find_project(name)?;

        Ok(RunningProject {
            name: project.name,
            command: project.command,
            cwd: project.cwd,
            display: project.display,
            stop_signal: project.stop_signal,
            envs: project.envs,
            pid: pid.parse().context("Couldn't parse pid")?,
        })
    }
}

impl RunningProject {
    pub fn stop(&self) -> Result<(), anyhow::Error> {
        let signal = self.stop_signal.as_ref().unwrap_or(&Signal::SIGINT);
        stop_pg(self.pid, signal).map_err(|_| anyhow!("Error trying to stop project"))
    }
}

pub struct WorkerConfig {
    pub projects: Vec<Project>,
    state_dir: PathBuf,
    log_dir: PathBuf,
}

impl WorkerConfig {
    pub fn new() -> Result<Self, anyhow::Error> {
        let base_dir = find_config_dir()?.context("Couldn't find config dir")?;
        let config_string = std::fs::read_to_string(base_dir.join(CONFIG_FILE))?;

        let state_dir = base_dir.join(".worker/state");
        let log_dir = base_dir.join(".worker/log");

        std::fs::create_dir_all(&state_dir)?;
        std::fs::create_dir_all(&log_dir)?;

        // Deserialize the TOML string into the Config struct
        let config: Config = toml::from_str(&config_string)?;

        Ok(Self {
            projects: config.project,
            state_dir,
            log_dir,
        })
    }

    pub fn log_file(&self, project: &Project) -> PathBuf {
        self.log_dir.join(&project.name)
    }

    pub fn store_state(&self, pid: i32, project: &Project) -> Result<(), anyhow::Error> {
        let filename = format!("{}-{}", project.name, pid);
        let state_file = self.state_dir.join(filename);

        let file = File::create(state_file).expect("Couldn't create state file");
        serde_json::to_writer(file, &project).expect("Couldn't write to state file");

        Ok(())
    }

    pub fn is_running(&self, project: &Project) -> Result<bool, anyhow::Error> {
        Ok(self.running()?.iter().any(|it| it.name == project.name))
    }

    // Try to get vec of running projects. Try to remove the state file if the process is not running
    pub fn running(&self) -> Result<Vec<RunningProject>, anyhow::Error> {
        let projects = std::fs::read_dir(self.state_dir.as_path())?
            .filter_map(|entry| {
                let path = entry.ok()?.path();
                let project = RunningProject::from_str(path.file_name()?.to_str()?).ok()?;
                if has_processes_running(project.pid) {
                    Some(project)
                } else {
                    let _ = std::fs::remove_file(&path);
                    None
                }
            })
            .collect::<Vec<_>>();

        Ok(projects)
    }

    pub fn partition_projects<T>(
        &self,
        projects: Vec<T>,
    ) -> Result<(Vec<RunningProject>, Vec<Project>), anyhow::Error>
    where
        T: WorkerProject + Into<Project>,
    {
        // Partition map to get project with pid set
        let running_projects = self.running()?;
        let (running, not_running): (Vec<_>, Vec<_>) = projects.into_iter().partition_map(|rp| {
            match running_projects.iter().find(|p| p.name == rp.name()) {
                Some(p) => Either::Left(p.to_owned()),
                None => Either::Right(rp.into()),
            }
        });

        Ok((running, not_running))
    }
}

// Scan root directories until we hopefully find the config file
fn find_config_dir() -> Result<Option<PathBuf>, anyhow::Error> {
    let mut dir = std::env::current_dir()?;
    loop {
        if dir.join(CONFIG_FILE).exists() {
            return Ok(Some(dir));
        }
        if let Some(parent) = dir.parent() {
            dir = parent.to_path_buf();
        } else {
            return Ok(None);
        }
    }
}
