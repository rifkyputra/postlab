use anyhow::Result;
use clap::{Parser, Subcommand};

mod api;
mod commands;
mod wizard;

#[derive(Parser)]
#[command(
    name = "postlab",
    version = "0.1.0",
    about = "Bare-metal VPS management — for developers"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Manage servers (add, list, info, remove)
    Server {
        #[command(subcommand)]
        action: commands::server::ServerAction,
    },

    /// Install an application on a server
    Install {
        /// Application name (e.g. docker, nginx, postgres)
        app: String,
        /// Target server ID or name
        #[arg(short, long)]
        server: Option<String>,
    },

    /// Upgrade OS packages on a server
    Upgrade {
        /// Target server ID or name
        #[arg(short, long)]
        server: Option<String>,
    },

    /// Run security hardening on a server
    Harden {
        /// Target server ID or name
        #[arg(short, long)]
        server: Option<String>,
    },

    /// Show status dashboard for all servers
    Status,

    /// Manage tasks (list, show)
    Task {
        #[command(subcommand)]
        action: commands::task::TaskAction,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        None => wizard::run_wizard().await,
        Some(Commands::Server { action }) => commands::server::run(action).await,
        Some(Commands::Install { app, server }) => commands::install::run(&app, server.as_deref()).await,
        Some(Commands::Upgrade { server }) => commands::upgrade::run(server.as_deref()).await,
        Some(Commands::Harden { server }) => commands::harden::run(server.as_deref()).await,
        Some(Commands::Status) => commands::status::run().await,
        Some(Commands::Task { action }) => commands::task::run(action).await,
    }
}
