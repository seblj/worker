use std::{collections::HashMap, hash::Hash, path::PathBuf, str::FromStr};

use anyhow::Context;
use serde::{Deserialize, Serialize};

use crate::libc::Signal;

const CONFIG_FILE: &str = ".worker.toml";

#[derive(Deserialize, Debug)]
pub struct Config {
    pub project: Vec<Project>,
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

impl_display!(Project);
impl_display!(RunningProject);

fn find_project(name: &str) -> Result<Project, anyhow::Error> {
    let config = WorkerConfig::new()?;
    let projects: Vec<String> = config.projects.iter().map(|p| p.name.clone()).collect();

    config
        .projects
        .into_iter()
        .find(|it| it.name == name)
        .with_context(|| format!("Valid projects are {:#?}", projects))
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

pub struct WorkerConfig {
    pub projects: Vec<Project>,
    pub state_dir: PathBuf,
    pub log_dir: PathBuf,
}

impl WorkerConfig {
    pub fn new() -> Result<Self, anyhow::Error> {
        let base_dir = find_config_dir()?.context("Couldn't find config dir")?;
        Self::new_with_base_dir(base_dir)
    }

    pub fn new_with_base_dir(base_dir: PathBuf) -> Result<Self, anyhow::Error> {
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
