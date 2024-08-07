use std::{collections::HashMap, fmt::Display, hash::Hash, path::PathBuf, str::FromStr};

use anyhow::Context;
use serde::{Deserialize, Serialize};

use crate::libc::Signal;

const CONFIG_FILE: &str = ".worker.toml";

#[derive(Deserialize, Debug)]
pub struct Config {
    pub project: Vec<Project>,
}

#[derive(Deserialize, Clone, Debug, Serialize, Eq, PartialEq)]
pub struct Project {
    pub name: String,
    pub command: String,
    pub cwd: String,
    pub display: Option<String>,
    pub stop_signal: Option<Signal>,
    pub envs: Option<HashMap<String, String>>,

    #[serde(skip_deserializing)]
    pub pid: Option<i32>,
}

impl Hash for Project {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state)
    }
}

impl Display for Project {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(ref display) = self.display {
            write!(f, "{} ({})", display, self.name)
        } else {
            write!(f, "{}", self.name)
        }
    }
}

impl FromStr for Project {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let config = WorkerConfig::new()?;
        let projects: Vec<String> = config.projects.iter().map(|p| p.name.clone()).collect();

        config
            .projects
            .into_iter()
            .find(|it| it.name == s)
            .with_context(|| format!("Valid projects are {:#?}", projects))
    }
}

pub struct WorkerConfig {
    pub projects: Vec<Project>,
    pub state_dir: PathBuf,
    pub log_dir: PathBuf,
}

impl WorkerConfig {
    pub fn new() -> Result<Self, anyhow::Error> {
        let config_dir = find_config_dir()
            .expect("Couldn't get current dir")
            .expect("Couldn't find config dir");

        let config_string = std::fs::read_to_string(config_dir.join(CONFIG_FILE))?;

        let state_dir = config_dir.join(".worker/state");
        let log_dir = config_dir.join(".worker/log");

        std::fs::create_dir_all(state_dir.as_path())?;
        std::fs::create_dir_all(log_dir.as_path())?;

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
