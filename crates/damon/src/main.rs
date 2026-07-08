mod commands;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "damon", version, about = "Orchestrator for persistent coding agents")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Scaffold ~/damon plus config.toml and models.toml
    Init,
}

fn run(cmd: Cmd) -> anyhow::Result<()> {
    match cmd {
        Cmd::Init => commands::init::run(),
    }
}

fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli.cmd) {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}
