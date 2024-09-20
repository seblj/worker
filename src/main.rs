use std::{
    fs::File,
    io::{BufRead, BufReader, Read},
    os::{
        fd::{FromRawFd, IntoRawFd},
        unix::process::CommandExt,
    },
    path::PathBuf,
    process::Stdio,
    str::FromStr,
    thread::sleep,
    time::{Duration, Instant},
};

use anyhow::{anyhow, Context};
use clap::{command, Parser};
use config::{Project, RunningProject, WorkerConfig};
use itertools::{Either, Itertools};
use libc::{fork, has_processes_running, setsid, stop_pg, waitpid, Fork, Signal};

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
fn logs(config: &WorkerConfig, log_args: LogsArgs) -> Result<(), anyhow::Error> {
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

fn status(config: &WorkerConfig, status_args: StatusArgs) -> Result<(), anyhow::Error> {
    for project in get_running_projects(config)? {
        if status_args.quiet {
            println!("{}", project.name);
        } else {
            println!("{} is running", project);
        }
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

    for project in not_running {
        match fork().expect("Couldn't fork") {
            Fork::Parent(p) => {
                waitpid(p).unwrap();
            }
            Fork::Child => {
                let sid = setsid().expect("Couldn't setsid");
                let filename = format!("{}-{}", project.name, sid);
                let state_file = config.state_dir.join(filename);

                let file = File::create(state_file).expect("Couldn't create state file");
                serde_json::to_writer(file, &project).expect("Couldn't write to state file");

                match fork().expect("Couldn't fork inner") {
                    Fork::Parent(_) => {
                        std::process::exit(0);
                    }
                    Fork::Child => {
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
                };
            }
        };
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
    /// Additionally accepts `-f` to follow the log. E.g. `worker logs foo`
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

    #[arg(long)]
    base_dir: Option<PathBuf>,
}

fn main() -> Result<(), anyhow::Error> {
    let args = Cli::parse();

    let config = if let Some(base_dir) = args.base_dir {
        WorkerConfig::new_with_base_dir(base_dir)?
    } else {
        WorkerConfig::new()?
    };

    match args.subcommand {
        SubCommands::Start(args) => start(&config, args.projects.into_iter().unique().collect())?,
        SubCommands::Stop(args) => stop(&config, args.projects.into_iter().unique().collect())?,
        SubCommands::Restart(args) => {
            restart(&config, args.projects.into_iter().unique().collect())?
        }
        SubCommands::Logs(log_args) => logs(&config, log_args)?,
        SubCommands::Status(status_args) => status(&config, status_args)?,
        SubCommands::List(list_args) => list(&config, list_args)?,
    }

    Ok(())
}
