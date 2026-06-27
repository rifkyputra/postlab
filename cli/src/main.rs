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

    #[arg(short, long, default_value = "~/.postlab/data.db")]
    database: String,
}

#[derive(Subcommand)]
enum Commands {
    /// Launch interactive TUI (default)
    Tui,

    /// Print OS / system information
    Info,
    /// List installed packages
    List,

    /// System diagnostics and swap management
    System {
        #[command(subcommand)]
        cmd: SystemCmd,
    },
    /// Package management
    Packages {
        #[command(subcommand)]
        cmd: PackagesCmd,
    },
    /// Process management
    Processes {
        #[command(subcommand)]
        cmd: ProcessesCmd,
    },
    /// Service management (systemd)
    Services {
        #[command(subcommand)]
        cmd: ServicesCmd,
    },
}

#[derive(Subcommand)]
enum SystemCmd {
    /// Print OS, kernel, CPU, memory, and uptime
    Info,
    /// Per-core CPU percentages
    Cpu,
    /// Memory usage
    Mem,
    /// Disk usage by mount point
    Disks,
    /// Network RX/TX bytes
    Net,
    /// Swap management
    #[command(subcommand)]
    Swap(SwapCmd),
}

#[derive(Subcommand)]
enum SwapCmd {
    /// Show swap status and entries
    Status,
    /// Create a swap file and activate it
    Create {
        /// Path to swap file (e.g. /swapfile)
        path: String,
        /// Size in MiB
        size_mb: u64,
    },
    /// Deactivate and delete a swap file
    Delete {
        /// Path to swap file
        path: String,
    },
    /// Activate a swap file (swapon)
    Enable {
        /// Path to swap file
        path: String,
    },
    /// Deactivate a swap file (swapoff)
    Disable {
        /// Path to swap file
        path: String,
    },
    /// Resize an existing swap file
    Resize {
        /// Path to swap file
        path: String,
        /// New size in MiB
        size_mb: u64,
    },
}

#[derive(Subcommand)]
enum PackagesCmd {
    /// List installed packages
    List,
    /// Search the package index
    Search {
        /// Search query
        query: String,
    },
    /// Install a package (streams progress)
    Install {
        /// Package name
        name: String,
    },
    /// Remove a package (streams progress)
    Remove {
        /// Package name
        name: String,
    },
    /// Upgrade all packages
    Upgrade,
    /// Clean the package cache
    CacheClean,
}

#[derive(Subcommand)]
enum ProcessesCmd {
    /// List running processes
    List {
        /// Sort by: cpu, mem, pid, name
        #[arg(short, long, default_value = "cpu")]
        sort: String,
    },
    /// Kill a process by PID
    Kill {
        /// Process ID
        pid: u32,
        /// Send SIGKILL instead of SIGTERM
        #[arg(short, long)]
        force: bool,
    },
}

#[derive(Subcommand)]
enum ServicesCmd {
    /// List all systemd services
    List,
    /// Start a service
    Start {
        /// Service unit name (e.g. nginx.service)
        name: String,
    },
    /// Stop a service
    Stop {
        name: String,
    },
    /// Restart a service
    Restart {
        name: String,
    },
    /// Enable a service to start at boot
    Enable {
        name: String,
    },
    /// Disable a service from starting at boot
    Disable {
        name: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
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
        Some(Commands::Info) => cmd_info(&platform).await,
        Some(Commands::List) => cmd_list(&platform).await,

        Some(Commands::System { cmd }) => match cmd {
            SystemCmd::Info => cmd_info(&platform).await,
            SystemCmd::Cpu => cmd_cpu(&platform).await,
            SystemCmd::Mem => cmd_mem(&platform).await,
            SystemCmd::Disks => cmd_disks(&platform).await,
            SystemCmd::Net => cmd_net(&platform).await,
            SystemCmd::Swap(swap_cmd) => match swap_cmd {
                SwapCmd::Status => cmd_swap_status(&platform).await,
                SwapCmd::Create { path, size_mb } => {
                    platform.system.swap_create(&path, size_mb).await?;
                    println!("Swap file {} created ({} MiB) and activated.", path, size_mb);
                    Ok(())
                }
                SwapCmd::Delete { path } => {
                    platform.system.swap_delete(&path).await?;
                    println!("Swap file {} deactivated and removed.", path);
                    Ok(())
                }
                SwapCmd::Enable { path } => {
                    platform.system.swap_enable(&path).await?;
                    println!("Swap {} activated.", path);
                    Ok(())
                }
                SwapCmd::Disable { path } => {
                    platform.system.swap_disable(&path).await?;
                    println!("Swap {} deactivated.", path);
                    Ok(())
                }
                SwapCmd::Resize { path, size_mb } => {
                    platform.system.swap_resize(&path, size_mb).await?;
                    println!("Swap file {} resized to {} MiB.", path, size_mb);
                    Ok(())
                }
            },
        },

        Some(Commands::Packages { cmd }) => match cmd {
            PackagesCmd::List => cmd_list(&platform).await,
            PackagesCmd::Search { query } => {
                let results = platform.packages.search(&query).await?;
                if results.is_empty() {
                    println!("No packages found for \"{}\"", query);
                } else {
                    for pkg in &results {
                        println!("{:<30} {}  {}", pkg.name, pkg.version, pkg.description);
                    }
                    println!("\n{} packages found", results.len());
                }
                Ok(())
            }
            PackagesCmd::Install { name } => {
                let out = platform.packages.install(&name).await?;
                println!("{}", out);
                Ok(())
            }
            PackagesCmd::Remove { name } => {
                let out = platform.packages.remove(&name).await?;
                println!("{}", out);
                Ok(())
            }
            PackagesCmd::Upgrade => {
                let out = platform.packages.upgrade_all().await?;
                println!("{}", out);
                Ok(())
            }
            PackagesCmd::CacheClean => {
                let out = platform.packages.clean_cache().await?;
                println!("{}", out);
                Ok(())
            }
        },

        Some(Commands::Processes { cmd }) => match cmd {
            ProcessesCmd::List { sort } => {
                let mut entries = platform.processes.list().await?;
                match sort.as_str() {
                    "mem" => entries.sort_by_key(|b| std::cmp::Reverse(b.mem_bytes)),
                    "pid" => entries.sort_by_key(|a| a.pid),
                    "name" => entries.sort_by(|a, b| a.name.cmp(&b.name)),
                    _ => {} // default cpu, already sorted
                }
                println!(
                    "{:>8} {:>6} {:>8}  {:<16} {:<8} NAME",
                    "PID", "CPU%", "MEM", "USER", "STATUS"
                );
                for p in &entries {
                    println!(
                        "{:>8} {:>5.1} {:>7}  {:<16} {:<8} {}",
                        p.pid,
                        p.cpu_pct,
                        format_bytes(p.mem_bytes),
                        truncate_str(&p.user, 16),
                        truncate_str(&p.status, 8),
                        p.name,
                    );
                }
                println!("\n{} processes", entries.len());
                Ok(())
            }
            ProcessesCmd::Kill { pid, force } => {
                if force {
                    let out = tokio::process::Command::new("kill")
                        .args(["-9", &pid.to_string()])
                        .output()
                        .await?;
                    if out.status.success() {
                        println!("Process {} killed (SIGKILL).", pid);
                        Ok(())
                    } else {
                        anyhow::bail!("{}", String::from_utf8_lossy(&out.stderr).trim())
                    }
                } else {
                    platform.processes.kill(pid).await?;
                    println!("Process {} killed (SIGTERM).", pid);
                    Ok(())
                }
            }
        },

        Some(Commands::Services { cmd }) => match cmd {
            ServicesCmd::List => {
                let services = platform.services.list_services().await?;
                println!(
                    "{:<40} {:<12} {:<12} {:<10} DESCRIPTION",
                    "NAME", "LOAD", "ACTIVE", "SUB"
                );
                for s in &services {
                    println!(
                        "{:<40} {:<12} {:<12} {:<10} {}",
                        truncate_str(&s.name, 40),
                        s.load_state,
                        s.active_state,
                        s.sub_state,
                        s.description,
                    );
                }
                println!("\n{} services", services.len());
                Ok(())
            }
            ServicesCmd::Start { name } => {
                platform.services.start(&name).await?;
                println!("Service {} started.", name);
                Ok(())
            }
            ServicesCmd::Stop { name } => {
                platform.services.stop(&name).await?;
                println!("Service {} stopped.", name);
                Ok(())
            }
            ServicesCmd::Restart { name } => {
                platform.services.restart(&name).await?;
                println!("Service {} restarted.", name);
                Ok(())
            }
            ServicesCmd::Enable { name } => {
                platform.services.enable(&name).await?;
                println!("Service {} enabled.", name);
                Ok(())
            }
            ServicesCmd::Disable { name } => {
                platform.services.disable(&name).await?;
                println!("Service {} disabled.", name);
                Ok(())
            }
        },

        Some(Commands::Tui) | None => tui::run(platform, pool).await,
    }
}

// ── Command handlers ───────────────────────────────────────────────────────

async fn cmd_info(platform: &crate::core::Platform) -> Result<()> {
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
    Ok(())
}

async fn cmd_list(platform: &crate::core::Platform) -> Result<()> {
    let packages = platform.packages.list_installed().await?;
    for pkg in &packages {
        println!("{:<30} {}", pkg.name, pkg.version);
    }
    println!("\n{} packages installed", packages.len());
    Ok(())
}

async fn cmd_cpu(platform: &crate::core::Platform) -> Result<()> {
    let cores = platform.system.cpu_pct().await?;
    for (i, pct) in cores.iter().enumerate() {
        println!("CPU {:>2}: {:>5.1}%", i, pct);
    }
    Ok(())
}

async fn cmd_mem(platform: &crate::core::Platform) -> Result<()> {
    let m = platform.system.mem().await?;
    println!(
        "Total:     {:.1} GB",
        m.total as f64 / 1_073_741_824.0
    );
    println!(
        "Used:      {:.1} GB",
        m.used as f64 / 1_073_741_824.0
    );
    println!(
        "Available: {:.1} GB",
        m.available as f64 / 1_073_741_824.0
    );
    Ok(())
}

async fn cmd_disks(platform: &crate::core::Platform) -> Result<()> {
    let disks = platform.system.disks().await?;
    println!("{:<20} {:>10} {:>10} {:>6}  FS", "MOUNT", "TOTAL", "USED", "USE%");
    for d in &disks {
        let pct = if d.total > 0 {
            (d.used as f64 / d.total as f64) * 100.0
        } else {
            0.0
        };
        println!(
            "{:<20} {:>9} {:>9} {:>5.0}%  {}",
            truncate_str(&d.mount, 20),
            format_bytes(d.total),
            format_bytes(d.used),
            pct,
            d.fs_type,
        );
    }
    Ok(())
}

async fn cmd_net(platform: &crate::core::Platform) -> Result<()> {
    let n = platform.system.net().await?;
    println!("RX: {} bytes ({})", n.rx_bytes, format_bytes(n.rx_bytes));
    println!("TX: {} bytes ({})", n.tx_bytes, format_bytes(n.tx_bytes));
    Ok(())
}

async fn cmd_swap_status(platform: &crate::core::Platform) -> Result<()> {
    let s = platform.system.swap_status().await?;
    println!(
        "Total: {}, Used: {}, Free: {}",
        format_bytes(s.total),
        format_bytes(s.used),
        format_bytes(s.free),
    );
    if !s.entries.is_empty() {
        println!(
            "\n{:<24} {:>6} {:>10} {:>10} {:>8}",
            "PATH", "KIND", "SIZE", "USED", "PRIO"
        );
        for e in &s.entries {
            println!(
                "{:<24} {:>6} {:>10} {:>10} {:>8}",
                truncate_str(&e.path, 24),
                e.kind,
                format_bytes(e.size_bytes),
                format_bytes(e.used_bytes),
                e.priority,
            );
        }
    }
    Ok(())
}

// ── Formatting helpers ─────────────────────────────────────────────────────

fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit_idx = 0;
    while value >= 1024.0 && unit_idx < UNITS.len() - 1 {
        value /= 1024.0;
        unit_idx += 1;
    }
    if unit_idx == 0 {
        format!("{} B", bytes)
    } else {
        format!("{:.1} {}", value, UNITS[unit_idx])
    }
}

fn truncate_str(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max - 1])
    }
}

fn expand_tilde(path: &str) -> String {
    if path.starts_with("~/") {
        let home = std::env::var("HOME").unwrap_or_default();
        path.replacen("~", &home, 1)
    } else {
        path.to_string()
    }
}
