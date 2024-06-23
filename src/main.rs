use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    str::FromStr,
    thread::sleep,
    time::{Duration, Instant},
};

use anyhow::{anyhow, bail, Context};
use clap::{command, Parser};
use lazy_static::lazy_static;
use libc::{daemon, has_processes_running, killpg, Fork};
use serde::{Deserialize, Serialize};

pub mod libc;

const CONFIG_FILE: &str = ".worker.toml";

lazy_static! {
    static ref STATE_DIR: PathBuf = CONFIG_DIR.join(".worker/state");
    static ref LOG_DIR: PathBuf = CONFIG_DIR.join(".worker/log");
    static ref CONFIG_DIR: PathBuf = find_config_file()
        .expect("Couldn't get current dir")
        .expect("Couldn't find config dir");
}

fn get_running_projects() -> Result<Vec<(String, i32)>, anyhow::Error> {
    let projects = std::fs::read_dir(STATE_DIR.as_path())?
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            parse_state_filename(&path).ok()
        })
        .collect::<Vec<_>>();

    Ok(projects)
}

// TODO: Should not read the entire file. Should only read last x lines or something
fn log(log_args: LogsArgs) -> Result<(), anyhow::Error> {
    let log_file = LOG_DIR.join(&log_args.project.name);
    let file = File::open(log_file).map_err(|_| {
        // If the log doesn't exist, it should mean that the project isn't running
        anyhow!(
            "{} is not running",
            log_args.project.display.unwrap_or(log_args.project.name)
        )
    })?;

    let mut reader = BufReader::new(file);
    let mut buffer = String::new();

    if log_args.follow {
        loop {
            match reader.read_line(&mut buffer) {
                Ok(0) => {
                    // No new data, so wait before trying again
                    sleep(Duration::from_secs(1));
                }
                Ok(_) => {
                    print!("{}", buffer);
                    buffer.clear(); // Clear the buffer after printing
                }
                Err(e) => {
                    eprintln!("Error reading from file: {}", e);
                    bail!(e)
                }
            }
        }
    } else {
        reader.read_to_string(&mut buffer)?;
        println!("{}", buffer);
    }

    Ok(())
}

fn parse_state_filename(path: &Path) -> anyhow::Result<(String, i32)> {
    let (name, pid) = path
        .file_name()
        .context("No filename")?
        .to_str()
        .context("Invalid unicode filename")?
        .rsplit_once('-')
        .context("File doesn't contain -")?;

    let pid = pid.parse::<i32>().context("Couldn't parse pid to i32")?;
    Ok((name.to_string(), pid))
}

fn status() -> Result<(), anyhow::Error> {
    let mut set: HashSet<String> = HashSet::new();

    for entry in std::fs::read_dir(STATE_DIR.as_path())? {
        let path = entry?.path();

        let f = File::open(&path)?;
        let reader = BufReader::new(f);
        let project: Project = serde_json::from_reader(reader)?;

        let (_, sid) = parse_state_filename(&path)?;

        if has_processes_running(sid) {
            set.insert(project.display.unwrap_or(project.name));
        } else {
            // If the process isn't running, then there is no need to keep the file
            std::fs::remove_file(path)?;
        }
    }

    for project in set {
        println!("{} is running", project);
    }

    Ok(())
}

fn stop(projects: Vec<Project>) -> Result<(), anyhow::Error> {
    // Try to terminate all processes that the user wants to stop
    let running_projects = get_running_projects()?;

    for project in projects.iter() {
        if let Some(p) = running_projects.iter().find(|p| p.0 == project.name) {
            let _ = killpg(p.1);
        } else {
            eprintln!("Cannot stop project not running: {}", project.name);
        }
    }

    let timeout = Duration::new(5, 0);
    let start = Instant::now();

    let mut not_deleted: HashSet<MinimalProject> = HashSet::new();
    let mut deleted: HashSet<MinimalProject> = HashSet::new();
    let mut finished = true;
    while Instant::now().duration_since(start) < timeout {
        finished = true;
        not_deleted.clear();
        for entry in std::fs::read_dir(STATE_DIR.as_path())? {
            let path = entry?.path();

            let (project, pid) = parse_state_filename(&path)?;

            if let Some(p) = projects.iter().find(|p| p.name == project) {
                let minimal_project = MinimalProject {
                    name: p.name.clone(),
                    display: p.display.clone(),
                };
                if has_processes_running(pid) {
                    finished = false;
                    not_deleted.insert(minimal_project);
                } else {
                    deleted.insert(minimal_project);
                    std::fs::remove_file(path)?;
                }
            };
        }

        if finished {
            break;
        }
    }

    // Only delete logs for the processes we were actually able to delete
    for project in deleted.difference(&not_deleted) {
        let log_file = LOG_DIR.join(&project.name);
        let _ = std::fs::remove_file(log_file);
    }

    if !finished {
        for project in not_deleted {
            println!(
                "Was not able to stop {}",
                project.display.unwrap_or(project.name)
            );
        }
    }

    Ok(())
}

fn start(projects: Vec<Project>) -> Result<(), anyhow::Error> {
    let master_pid = sysinfo::get_current_pid().unwrap();
    for project in projects {
        if let Fork::Child =
            daemon(&project).map_err(|e| anyhow!("Error: {} on daemon: {:?}", e, project))?
        {
            let tmp_file = LOG_DIR.join(&project.name);
            let f = File::create(tmp_file)?;

            let parts = shlex::split(&project.command)
                .context(format!("Couldn't parse command: {}", project.command))?;

            let mut env_map: HashMap<_, _> = std::env::vars().collect();
            env_map.extend(project.envs.unwrap_or_default());

            duct::cmd(&parts[0], &parts[1..])
                .full_env(env_map)
                .dir(project.cwd)
                .stderr_to_stdout()
                .stdout_file(f)
                .run()?;
        }

        // Prevent trying to start a project multiple times
        let current_pid = sysinfo::get_current_pid().unwrap();
        if current_pid != master_pid {
            break;
        }
    }

    Ok(())
}

fn restart(projects: Vec<Project>) -> Result<(), anyhow::Error> {
    let running_projects = get_running_projects()?;

    let projects = projects
        .into_iter()
        .filter(|p| {
            if running_projects.iter().any(|rp| rp.0 == *p.name) {
                true
            } else {
                eprintln!("Cannot restart project not running: {}", p.name);
                false
            }
        })
        .collect::<Vec<_>>();

    stop(projects.clone())?;
    start(projects)?;

    Ok(())
}

#[derive(Deserialize, Debug)]
struct Config {
    project: Vec<Project>,
}

#[derive(Eq, PartialEq, Hash)]
struct MinimalProject {
    name: String,
    display: Option<String>,
}

#[derive(Deserialize, Clone, Debug, Serialize)]
pub struct Project {
    name: String,
    command: String,
    cwd: String,
    display: Option<String>,
    envs: Option<HashMap<String, String>>,
}

impl FromStr for Project {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let config_file = CONFIG_DIR.join(CONFIG_FILE);
        let config_string = std::fs::read_to_string(config_file)?;

        // Deserialize the TOML string into the Config struct
        let config: Config = toml::from_str(&config_string)?;

        let projects = config
            .project
            .iter()
            .map(|p| p.name.clone())
            .collect::<Vec<String>>();

        config
            .project
            .clone()
            .into_iter()
            .find(|it| it.name == s)
            .context(format!("Valid projects are {:?}", projects))
    }
}

#[derive(Debug, Parser)]
struct ActionArgs {
    projects: Vec<Project>,
}

#[derive(Debug, Parser)]
struct LogsArgs {
    project: Project,
    #[arg(short, long)]
    follow: bool,
}

#[derive(Parser, Debug)]
enum SubCommands {
    /// Starts the specified project(s). E.g. `worker start foo bar`
    Start(ActionArgs),
    /// Stops the specified project(s). E.g. `worker stop foo bar`
    Stop(ActionArgs),
    /// Restarts the specified project(s). E.g. `worker restart foo bar` (Same as running stop and then start)
    Restart(ActionArgs),
    /// Print out logs for the specified project.
    /// Additionally accepts `-f` to follow the log. E.g. `worker logs foo`
    Logs(LogsArgs),
    /// Prints out a status of which projects is running. Accepts no additional flags or project(s)
    Status,
}

#[derive(Parser, Debug)]
struct Cli {
    #[command(subcommand)]
    subcommand: SubCommands,
}

// Scan root directories until we hopefully find `.worker.toml` or `worker.toml`
pub fn find_config_file() -> Result<Option<PathBuf>, anyhow::Error> {
    let mut dir = std::env::current_dir()?;
    loop {
        if dir.join(".worker.toml").exists() {
            return Ok(Some(dir));
        }
        if let Some(parent) = dir.parent() {
            dir = parent.to_path_buf();
        } else {
            return Ok(None);
        }
    }
}

fn main() -> Result<(), anyhow::Error> {
    // TODO: Maybe dedup the projects passed as arg to run maybe
    let args = Cli::parse();

    // CONFIG_DIR is evaluated at runtime and panics if not found. If found, make sure that the
    // directories needed to store the log and state files are existing
    std::fs::create_dir_all(STATE_DIR.as_path())?;
    std::fs::create_dir_all(LOG_DIR.as_path())?;

    match args.subcommand {
        SubCommands::Start(args) => start(args.projects)?,
        SubCommands::Stop(args) => stop(args.projects)?,
        SubCommands::Restart(args) => restart(args.projects)?,
        SubCommands::Logs(log_args) => log(log_args)?,
        SubCommands::Status => status()?,
    }

    Ok(())
}
