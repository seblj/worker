use std::{collections::HashMap, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::libc::Signal;

const CONFIG_FILE: &str = ".worker.toml";

#[derive(Deserialize, Debug)]
pub struct Config {
    pub project: Vec<Project>,
}

#[derive(Deserialize, Clone, Debug, Serialize)]
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
