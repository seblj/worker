use std::{fmt::Display, fs::File, os::unix::process::CommandExt, process::Stdio, str::FromStr};

use anyhow::bail;
use clap::{command, Parser};

fn log(log_args: LogArgs) -> Result<(), anyhow::Error> {
    todo!("Log output here");

    Ok(())
}

pub enum Fork {
    Parent(libc::pid_t),
    Child,
}

fn fork() -> Result<Fork, i32> {
    let res = unsafe { libc::fork() };
    match res {
        -1 => Err(-1),
        0 => Ok(Fork::Child),
        res => Ok(Fork::Parent(res)),
    }
}

pub fn setsid() -> Result<libc::pid_t, i32> {
    let res = unsafe { libc::setsid() };
    match res {
        -1 => Err(-1),
        res => Ok(res),
    }
}

fn daemon() -> Result<Fork, i32> {
    match fork() {
        Ok(Fork::Child) => setsid().and_then(|_| fork()),
        x => x,
    }
}

// Sends SIGTERM process to `pid`, terminating the entire process tree
fn kill(pid: i32) -> Result<(), anyhow::Error> {
    let result = unsafe { libc::kill(pid, libc::SIGTERM) };
    if result == 0 {
        Ok(())
    } else {
        bail!("Got an error")
    }
}

fn run(args: RunArgs) -> Result<(), anyhow::Error> {
    match daemon().unwrap() {
        Fork::Parent(p) => {
            // TODO: Store the two pids in a structured file. Can read it to be able to `worker stop <project>`
            println!("pid is: {:?}", p);
        }
        Fork::Child => {
            // TODO: Should I use `duct` or something to combine stderr and stdout to get all
            // output to the file maybe?
            let f = File::create("/Users/sebastian/foobar.txt").unwrap();
            std::process::Command::new("yarn")
                .args(["serve"])
                .current_dir("/Users/sebastian/work/smartdok-bff")
                .stdout(f)
                .stderr(Stdio::null())
                .stdin(Stdio::null())
                .exec();
        }
    }

    Ok(())
}

#[derive(Parser, Debug, Clone)]
enum Projects {
    SmartDokUI,
    SmartApi,
    MobileApi,
    SmartDokBFF,
    SmartDokWeb,
}

// TODO: Dependant on ngrok config
impl Display for Projects {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Projects::SmartDokUI => write!(f, "ui"),
            Projects::SmartApi => write!(f, "smartapi"),
            Projects::MobileApi => write!(f, "mobileapi"),
            Projects::SmartDokBFF => write!(f, "bff"),
            Projects::SmartDokWeb => write!(f, "web"),
        }
    }
}

impl FromStr for Projects {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "ui" | "smartdokui" | "smartdok-ui" => Ok(Self::SmartDokUI),
            "bff" => Ok(Self::SmartDokBFF),
            "smartapi" => Ok(Self::SmartApi),
            "mobileapi" => Ok(Self::MobileApi),
            "web" => Ok(Self::SmartDokWeb),
            _ => Err(anyhow::anyhow!("Unknown project")),
        }
    }
}

#[derive(Debug, Parser)]
struct RunArgs {
    projects: Vec<Projects>,
}

#[derive(Debug, Parser)]
struct LogArgs {
    project: Projects,
    #[arg(short, long)]
    follow: bool,
}

#[derive(Parser, Debug)]
enum SubCommands {
    Run(RunArgs),
    Log(LogArgs),
}

#[derive(Parser, Debug)]
struct Cli {
    #[command(subcommand)]
    subcommand: SubCommands,
}

fn main() -> Result<(), anyhow::Error> {
    // TODO: Dedup the projects passed as arg to run maybe
    let args = Cli::parse();

    match args.subcommand {
        SubCommands::Run(run_args) => run(run_args)?,
        SubCommands::Log(log_args) => log(log_args)?,
    }

    Ok(())
}
