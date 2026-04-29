mod commands;
mod config;
mod net;
mod prompt;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "constellation",
    about = "P2P A2A mesh runtime — see docs/setup-prompt.md",
    version
)]
struct Cli {
    #[arg(long, env = "CONSTELLATION_CONFIG")]
    config: Option<PathBuf>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    Init {
        #[arg(long)]
        name: Option<String>,
        #[arg(long, value_delimiter = ',')]
        skills: Option<Vec<String>>,
        #[arg(long)]
        port: Option<u16>,
    },
    Serve,
    Peers {
        #[arg(long)]
        json: bool,
    },
    Send {
        peer: String,
        text: String,
    },
    Wait {
        task_id: String,
        #[arg(long, default_value_t = 60)]
        timeout: u64,
    },
    Inbox {
        #[arg(long)]
        json: bool,
    },
    Respond {
        task_id: String,
        text: String,
    },
    Card,
    InstallService,
}

fn config_path(cli: &Cli) -> PathBuf {
    cli.config
        .clone()
        .unwrap_or_else(config::Config::default_path)
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,constellation=info")),
        )
        .init();
    let cli = Cli::parse();
    let path = config_path(&cli);
    match cli.cmd {
        Cmd::Init { name, skills, port } => commands::init::run(&path, name, skills, port).await,
        Cmd::Serve => commands::serve::run(&path).await,
        Cmd::Peers { json } => commands::peers::run(&path, json).await,
        Cmd::Send { peer, text } => commands::send::run(&path, &peer, &text).await,
        Cmd::Wait { task_id, timeout } => commands::wait::run(&path, &task_id, timeout).await,
        Cmd::Inbox { json } => commands::inbox::run(&path, json).await,
        Cmd::Respond { task_id, text } => commands::respond::run(&path, &task_id, &text).await,
        Cmd::Card => commands::card::run(&path).await,
        Cmd::InstallService => commands::install_service::run().await,
    }
}
