use std::{
    fs::OpenOptions,
    os::{
        fd::{FromRawFd, IntoRawFd},
        unix::process::CommandExt,
    },
    process::Stdio,
    time::{Duration, Instant},
};

use anyhow::{anyhow, Context};
use clap::{command, Parser};
use config::{Project, WorkerConfig};
use itertools::Itertools;
use libc::{fork, setsid, waitpid, Fork};

pub mod config;
pub mod libc;

fn logs(config: &WorkerConfig, log_args: LogsArgs) -> Result<(), anyhow::Error> {
    if !config.is_running(&log_args.project)? {
        return Err(anyhow!("{} is not running", log_args.project));
    }

    let mut cmd = std::process::Command::new("tail");

    if log_args.follow {
        cmd.arg("-f");
    }

    let mut child = cmd
        .args(["-n", &log_args.number.to_string()])
        .arg(config.log_file(&log_args.project))
        .spawn()?;

    child.wait()?;

    Ok(())
}

fn status(config: &WorkerConfig, status_args: StatusArgs) -> Result<(), anyhow::Error> {
    for project in config.running()? {
        if status_args.quiet {
            println!("{}", project.name);
        } else {
            println!("{} is running", project);
        }
    }

    Ok(())
}

fn stop(config: &WorkerConfig, projects: Vec<Project>) -> Result<(), anyhow::Error> {
    let (running, not_running) = config.partition_projects(projects)?;

    for project in running.iter() {
        project.stop()?;
    }

    for project in not_running {
        eprintln!("Cannot stop project not running: {}", project);
    }

    let timeout = Duration::new(5, 0);
    let start = Instant::now();

    while Instant::now().duration_since(start) < timeout {
        // Get all running projects and filter them on projects we are trying to stop.
        // If some of them are still running, we should print out a message that we failed to stop them
        let num_running = config
            .running()?
            .iter()
            .filter(|rp| running.contains(rp))
            .count();

        if num_running == 0 {
            return Ok(());
        }
    }

    config.running()?.iter().for_each(|rp| {
        if running.iter().any(|p| rp.name == p.name) {
            eprintln!("Was not able to stop {}", rp);
        }
    });

    Ok(())
}

fn start(config: &WorkerConfig, projects: Vec<Project>) -> Result<(), anyhow::Error> {
    let (running, not_running) = config.partition_projects(projects)?;

    for project in running {
        eprintln!("{} is already running", project);
    }

    for project in not_running {
        match fork().expect("Couldn't fork") {
            Fork::Parent(p) => {
                waitpid(p).unwrap();
            }
            Fork::Child => {
                let sid = setsid().expect("Couldn't setsid");
                config.store_state(sid, &project)?;

                match fork().expect("Couldn't fork inner") {
                    Fork::Parent(_) => std::process::exit(0),
                    Fork::Child => {
                        // Create a raw filedescriptor to use to merge stdout and stderr
                        let fd = OpenOptions::new()
                            .write(true)
                            .truncate(true)
                            .create(true)
                            .open(config.log_file(&project))?
                            .into_raw_fd();

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
                };
            }
        };
    }

    Ok(())
}

fn restart(config: &WorkerConfig, projects: Vec<Project>) -> Result<(), anyhow::Error> {
    let (projects, filtered) = config.partition_projects(projects)?;
    let projects: Vec<Project> = projects.into_iter().map(|p| p.into()).collect();

    for project in filtered {
        eprintln!("Cannot restart project not running: {}", project);
    }

    stop(config, projects.clone())?;
    start(config, projects)?;

    Ok(())
}

fn list(config: &WorkerConfig, list_args: ListArgs) -> Result<(), anyhow::Error> {
    for p in config.projects.iter() {
        if list_args.quiet {
            println!("{}", p.name)
        } else {
            println!("{}", p)
        }
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

    #[arg(short, long = "lines", default_value = "10")]
    number: i32,
}

#[derive(Debug, Parser)]
struct StatusArgs {
    #[arg(short, long, help = "Only print name of the project")]
    quiet: bool,
}

#[derive(Debug, Parser)]
struct ListArgs {
    #[arg(short, long, help = "Only print name of the project")]
    quiet: bool,
}

#[derive(Parser, Debug)]
enum SubCommands {
    /// Start the specified project(s). E.g. `worker start foo bar`
    Start(ActionArgs),
    /// Stop the specified project(s). E.g. `worker stop foo bar`
    Stop(ActionArgs),
    /// Restart the specified project(s). E.g. `worker restart foo bar` (Same as running stop and then start)
    Restart(ActionArgs),
    /// Print out logs for the specified project.
    Logs(LogsArgs),
    /// Print out a status of which projects is running
    Status(StatusArgs),
    /// Print out a list of available projects to run
    List(ListArgs),
}

#[derive(Parser, Debug)]
struct Cli {
    #[command(subcommand)]
    subcommand: SubCommands,
}

fn main() -> Result<(), anyhow::Error> {
    let args = Cli::parse();

    let config = WorkerConfig::new()?;

    let unique = |projects: Vec<Project>| projects.into_iter().unique().collect();

    match args.subcommand {
        SubCommands::Start(args) => start(&config, unique(args.projects))?,
        SubCommands::Stop(args) => stop(&config, unique(args.projects))?,
        SubCommands::Restart(args) => restart(&config, unique(args.projects))?,
        SubCommands::Logs(log_args) => logs(&config, log_args)?,
        SubCommands::Status(status_args) => status(&config, status_args)?,
        SubCommands::List(list_args) => list(&config, list_args)?,
    }

    Ok(())
}
