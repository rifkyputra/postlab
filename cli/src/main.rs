mod core;
mod db;
mod tui;

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::core::platform::detect;
use crate::db::init_db;

#[derive(Parser)]
#[command(name = "postlab")]
#[command(about = "Interactive bare metal server manager", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// SQLite database path
    #[arg(short, long, default_value = "~/.postlab/data.db")]
    database: String,
}

#[derive(Subcommand)]
enum Commands {
    /// Print OS / system information
    Info,
    /// List installed packages
    List,
    /// Launch interactive TUI (default)
    Tui,
}

#[tokio::main]
async fn main() -> Result<()> {
    // System operations (package install, caddy, systemctl) require root.
    // No TTY is available for sudo prompts inside the TUI render loop.
    if !nix::unistd::Uid::effective().is_root() {
        eprintln!("postlab must run as root. Try: sudo postlab");
        std::process::exit(1);
    }

    let cli = Cli::parse();

    let db_path = expand_tilde(&cli.database);
    if let Some(parent) = std::path::Path::new(&db_path).parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let pool = init_db(&format!("sqlite:{}", db_path)).await?;
    let platform = detect()?;

    match cli.command {
        Some(Commands::Info) => {
            let info = platform.system.info().await?;
            println!("Hostname:  {}", info.hostname);
            println!("OS:        {}", info.distro);
            println!("Kernel:    {}", info.kernel_version);
            println!("Arch:      {}", info.arch);
            println!("CPUs:      {} cores", info.cpu_count);
            println!(
                "Memory:    {:.1} / {:.1} GB",
                info.used_memory as f64 / 1_073_741_824.0,
                info.total_memory as f64 / 1_073_741_824.0
            );
            println!("Uptime:    {}s", info.uptime_secs);
        }
        Some(Commands::List) => {
            let packages = platform.packages.list_installed().await?;
            for pkg in &packages {
                println!("{:<30} {}", pkg.name, pkg.version);
            }
            println!("\n{} packages installed", packages.len());
        }
        Some(Commands::Tui) | None => {
            tui::run(platform, pool).await?;
        }
    }

    Ok(())
}

fn expand_tilde(path: &str) -> String {
    if path.starts_with("~/") {
        let home = std::env::var("HOME").unwrap_or_default();
        path.replacen("~", &home, 1)
    } else {
        path.to_string()
    }
}
