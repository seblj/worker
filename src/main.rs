use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader, Read},
    os::{
        fd::{FromRawFd, IntoRawFd},
        unix::process::CommandExt,
    },
    path::{Path, PathBuf},
    process::Stdio,
    str::FromStr,
    thread::sleep,
    time::{Duration, Instant},
};

use anyhow::{anyhow, bail, Context};
use clap::{command, Parser};
use lazy_static::lazy_static;
use libc::{daemon, has_processes_running, stop_pg, Fork, Signal};
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

fn try_cleanup_state() -> Result<(), anyhow::Error> {
    let running_projects = get_running_projects()?;
    for p in running_projects.iter() {
        let _ = std::fs::remove_file(LOG_DIR.join(&p.name));
    }

    for entry in std::fs::read_dir(STATE_DIR.as_path())? {
        let path = entry?.path();
        let (_, sid) = parse_state_filename(&path)?;
        if !has_processes_running(sid) {
            let _ = std::fs::remove_file(path);
        }
    }

    Ok(())
}

// Try to get vec of running projects. Try to remove the state file if the process is not running
fn get_running_projects() -> Result<Vec<MinimalProject>, anyhow::Error> {
    let projects = std::fs::read_dir(STATE_DIR.as_path())?
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            let (name, sid) = parse_state_filename(&path).ok()?;
            let project = Project::from_str(&name).ok()?;
            has_processes_running(sid).then_some(MinimalProject {
                name,
                display: project.display,
                stop_signal: project.stop_signal,
                pid: sid,
            })
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
    for project in get_running_projects()? {
        println!("{} is running", project.display.unwrap_or(project.name));
    }

    Ok(())
}

fn stop(projects: Vec<Project>) -> Result<(), anyhow::Error> {
    let running_projects = get_running_projects()?;

    let (running, not_running): (Vec<_>, Vec<_>) = projects
        .into_iter()
        .partition(|rp| running_projects.iter().any(|p| p.name == rp.name));

    for project in running.iter() {
        let p = running_projects
            .iter()
            .find(|it| it.name == project.name)
            .unwrap();
        let signal = project.stop_signal.as_ref().unwrap_or(&Signal::SIGINT);
        let _ = stop_pg(p.pid, signal);
    }

    for project in not_running {
        eprintln!("Cannot stop project not running: {}", project.name);
    }

    let timeout = Duration::new(5, 0);
    let start = Instant::now();

    let mut running_projects = Vec::new();
    while Instant::now().duration_since(start) < timeout {
        // Get all running projects and filter them on projects we are trying to stop.
        // If some of them are still running, we should print out a message that we failed to stop
        // them
        running_projects = get_running_projects()?
            .into_iter()
            .filter(|rp| running.iter().any(|p| rp.name == p.name))
            .collect();

        try_cleanup_state()?;

        if running_projects.is_empty() {
            return Ok(());
        }
    }

    try_cleanup_state()?;

    // If neither the kind `SIGINT` or forceful `SIGKILL` didn't work, we need to print out that it
    // failed to stop the projects...
    for project in running_projects {
        println!(
            "Was not able to stop {}",
            project.display.unwrap_or(project.name)
        );
    }

    Ok(())
}

fn start(projects: Vec<Project>) -> Result<(), anyhow::Error> {
    let running_projects = get_running_projects()?;
    let (running, not_running): (Vec<_>, Vec<_>) = projects
        .into_iter()
        .partition(|p| running_projects.iter().any(|rp| rp.name == p.name));

    for project in running {
        eprintln!(
            "{} is already running",
            project.display.unwrap_or(project.name)
        );
    }

    let master_pid = sysinfo::get_current_pid().unwrap();
    for project in not_running {
        if let Fork::Child =
            daemon(&project).map_err(|e| anyhow!("Error: {} on daemon: {:?}", e, project))?
        {
            let tmp_file = LOG_DIR.join(&project.name);
            let f = File::create(tmp_file)?;

            // Create a raw filedescriptor to use to merge stdout and stderr
            let fd = f.into_raw_fd();

            let parts = shlex::split(&project.command)
                .context(format!("Couldn't parse command: {}", project.command))?;

            std::process::Command::new(&parts[0])
                .args(&parts[1..])
                .envs(project.envs.unwrap_or_default())
                .current_dir(project.cwd)
                .stdout(unsafe { Stdio::from_raw_fd(fd) })
                .stderr(unsafe { Stdio::from_raw_fd(fd) })
                .stdin(Stdio::null())
                .exec();
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

    let (projects, filtered): (Vec<_>, Vec<_>) = projects
        .into_iter()
        .partition(|p| running_projects.iter().any(|rp| rp.name == p.name));

    for project in filtered {
        eprintln!("Cannot restart project not running: {}", project.name);
    }

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
    stop_signal: Option<Signal>,
    pid: i32,
}

#[derive(Deserialize, Clone, Debug, Serialize)]
pub struct Project {
    name: String,
    command: String,
    cwd: String,
    display: Option<String>,
    stop_signal: Option<Signal>,
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
