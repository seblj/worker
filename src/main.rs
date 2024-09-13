use std::{
    fs::File,
    io::{BufRead, BufReader, Read},
    os::{
        fd::{FromRawFd, IntoRawFd},
        unix::process::CommandExt,
    },
    process::Stdio,
    str::FromStr,
    thread::sleep,
    time::{Duration, Instant},
};

use anyhow::{anyhow, Context};
use clap::{command, Parser};
use config::{Project, RunningProject, WorkerConfig};
use itertools::{Either, Itertools};
use libc::{daemon, has_processes_running, stop_pg, Fork, Signal};

pub mod config;
pub mod libc;

// Try to get vec of running projects. Try to remove the state file if the process is not running
fn get_running_projects(config: &WorkerConfig) -> Result<Vec<RunningProject>, anyhow::Error> {
    let projects = std::fs::read_dir(config.state_dir.as_path())?
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            let project = RunningProject::from_str(path.file_name()?.to_str()?).ok()?;
            if has_processes_running(project.pid) {
                Some(project)
            } else {
                let _ = std::fs::remove_file(&path);
                let _ = std::fs::remove_file(config.log_dir.join(project.name));
                None
            }
        })
        .collect::<Vec<_>>();

    Ok(projects)
}

// TODO: Should not read the entire file. Should only read last x lines or something
fn log(config: &WorkerConfig, log_args: LogsArgs) -> Result<(), anyhow::Error> {
    if !get_running_projects(config)?
        .iter()
        .any(|it| it.name == log_args.project.name)
    {
        return Err(anyhow!("{} is not running", log_args.project));
    }

    let log_file = config.log_dir.join(&log_args.project.name);
    let file = File::open(log_file).expect("Log file should exist");

    let mut reader = BufReader::new(file);
    let mut buffer = String::new();

    if log_args.follow {
        loop {
            match reader.read_line(&mut buffer)? {
                0 => sleep(Duration::from_secs(1)),
                _ => {
                    print!("{}", buffer);
                    buffer.clear();
                }
            }
        }
    } else {
        reader.read_to_string(&mut buffer)?;
        println!("{}", buffer);
    }

    Ok(())
}

fn status(config: &WorkerConfig) -> Result<(), anyhow::Error> {
    for project in get_running_projects(config)? {
        println!("{} is running", project);
    }

    Ok(())
}

fn stop(config: &WorkerConfig, projects: Vec<Project>) -> Result<(), anyhow::Error> {
    let running_projects = get_running_projects(config)?;

    // Partition map to get project with pid set
    let (running, not_running): (Vec<_>, Vec<_>) = projects.into_iter().partition_map(|rp| {
        match running_projects.iter().find(|p| p.name == rp.name) {
            Some(p) => Either::Left(p.to_owned()),
            None => Either::Right(rp),
        }
    });

    for project in running.iter() {
        let signal = project.stop_signal.as_ref().unwrap_or(&Signal::SIGINT);
        let _ = stop_pg(project.pid, signal);
    }

    for project in not_running {
        eprintln!("Cannot stop project not running: {}", project);
    }

    let timeout = Duration::new(5, 0);
    let start = Instant::now();

    while Instant::now().duration_since(start) < timeout {
        // Get all running projects and filter them on projects we are trying to stop.
        // If some of them are still running, we should print out a message that we failed to stop them
        let num_running = get_running_projects(config)?
            .iter()
            .filter(|rp| running.iter().any(|p| rp.name == p.name))
            .count();

        if num_running == 0 {
            return Ok(());
        }
    }

    get_running_projects(config)?.iter().for_each(|rp| {
        if running.iter().any(|p| rp.name == p.name) {
            eprintln!("Was not able to stop {}", rp);
        }
    });

    Ok(())
}

fn start(config: &WorkerConfig, projects: Vec<Project>) -> Result<(), anyhow::Error> {
    let running_projects = get_running_projects(config)?;
    let (running, not_running): (Vec<_>, Vec<_>) = projects
        .into_iter()
        .partition(|p| running_projects.iter().any(|rp| rp.name == p.name));

    for project in running {
        eprintln!("{} is already running", project);
    }

    let master_pid = sysinfo::get_current_pid().unwrap();
    for project in not_running {
        if let Fork::Child = daemon(config, &project)
            .map_err(|e| anyhow!("Error: {} on daemon: {:?}", e, project))?
        {
            let tmp_file = config.log_dir.join(&project.name);
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

fn restart(config: &WorkerConfig, projects: Vec<Project>) -> Result<(), anyhow::Error> {
    let running_projects = get_running_projects(config)?;

    let (projects, filtered): (Vec<_>, Vec<_>) = projects
        .into_iter()
        .partition(|p| running_projects.iter().any(|rp| rp.name == p.name));

    for project in filtered {
        eprintln!("Cannot restart project not running: {}", project);
    }

    stop(config, projects.clone())?;
    start(config, projects)?;

    Ok(())
}

fn list(config: &WorkerConfig) -> Result<(), anyhow::Error> {
    for p in config.projects.iter() {
        println!("{}", p)
    }

    Ok(())
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
    /// Prints out a list of available projects to run
    List,
}

#[derive(Parser, Debug)]
struct Cli {
    #[command(subcommand)]
    subcommand: SubCommands,
}

fn main() -> Result<(), anyhow::Error> {
    let args = Cli::parse();

    let config = WorkerConfig::new()?;

    match args.subcommand {
        SubCommands::Start(args) => start(&config, args.projects.into_iter().unique().collect())?,
        SubCommands::Stop(args) => stop(&config, args.projects.into_iter().unique().collect())?,
        SubCommands::Restart(args) => {
            restart(&config, args.projects.into_iter().unique().collect())?
        }
        SubCommands::Logs(log_args) => log(&config, log_args)?,
        SubCommands::Status => status(&config)?,
        SubCommands::List => list(&config)?,
    }

    Ok(())
}
