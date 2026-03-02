use anyhow::Result;
use clap::Subcommand;
use console::style;
use dialoguer::{theme::ColorfulTheme, Confirm, Input};

use crate::api::{self, CreateServerInput};

#[derive(Subcommand)]
pub enum ServerAction {
    /// Add a new server
    Add {
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        host: Option<String>,
        #[arg(long, default_value = "22")]
        port: u16,
        #[arg(long, default_value = "root")]
        user: String,
        /// Auth method: 'key' or 'password'
        #[arg(long, default_value = "key")]
        auth: String,
        /// Path to SSH private key file
        #[arg(long)]
        key: Option<String>,
    },
    /// List all managed servers
    List,
    /// Show details + live system status for a server
    Info {
        /// Server ID or name
        id: String,
    },
    /// Remove a server
    Remove {
        /// Server ID or name
        id: String,
    },
}

pub async fn run(action: ServerAction) -> Result<()> {
    match action {
        ServerAction::Add { name, host, port, user, auth, key } => {
            add_server(name, host, port, user, auth, key).await
        }
        ServerAction::List => list_servers().await,
        ServerAction::Info { id } => server_info(&id).await,
        ServerAction::Remove { id } => remove_server(&id).await,
    }
}

async fn add_server(
    name: Option<String>,
    host: Option<String>,
    port: u16,
    user: String,
    auth: String,
    key: Option<String>,
) -> Result<()> {
    let theme = ColorfulTheme::default();

    let name = match name {
        Some(n) => n,
        None => Input::with_theme(&theme)
            .with_prompt("Server name")
            .interact_text()?,
    };

    let host = match host {
        Some(h) => h,
        None => Input::with_theme(&theme)
            .with_prompt("Host / IP")
            .interact_text()?,
    };

    // Prompt for key path if auth=key and none provided
    let key = if auth == "key" && key.is_none() {
        let default_key = format!(
            "{}/.ssh/id_ed25519",
            std::env::var("HOME").unwrap_or_default()
        );
        let path: String = Input::with_theme(&theme)
            .with_prompt("SSH private key path")
            .default(default_key)
            .interact_text()?;
        Some(path)
    } else {
        key
    };

    let spinner = indicatif::ProgressBar::new_spinner();
    spinner.set_message(format!("Adding {}…", style(&name).cyan()));
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));

    let result = api::create_server(CreateServerInput {
        name: name.clone(),
        host: host.clone(),
        port: Some(port as i64),
        user: Some(user.clone()),
        auth_method: Some(auth),
        ssh_key_path: key,
    })
    .await;

    spinner.finish_and_clear();

    match result {
        Ok(resp) => {
            let id = resp["id"].as_str().unwrap_or("?");
            println!(
                "{} Server {} added ({})",
                style("✔").green().bold(),
                style(&name).cyan().bold(),
                style(id).dim()
            );
            println!("  Host: {}@{}:{}", style(&user).yellow(), style(&host).yellow(), port);
        }
        Err(e) => {
            println!("{} Failed to add server: {e:#}", style("✗").red().bold());
        }
    }

    Ok(())
}

async fn list_servers() -> Result<()> {
    let spinner = indicatif::ProgressBar::new_spinner();
    spinner.set_message("Fetching servers…");
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));

    let result = api::list_servers().await;
    spinner.finish_and_clear();

    match result {
        Ok(servers) if servers.is_empty() => {
            println!("{}", style("No servers registered. Run: postlab server add").dim());
        }
        Ok(servers) => {
            println!(
                "\n{:<36}  {:<18}  {:<24}  {}",
                style("ID").bold(),
                style("NAME").bold(),
                style("HOST").bold(),
                style("OS").bold()
            );
            println!("{}", style("─".repeat(90)).dim());
            for s in &servers {
                println!(
                    "{:<36}  {:<18}  {:<24}  {}",
                    style(&s.id).dim(),
                    style(&s.name).cyan(),
                    format!("{}@{}:{}", s.user, s.host, s.port),
                    s.os_family.as_deref().unwrap_or("—")
                );
            }
            println!("\n{} server(s)", servers.len());
        }
        Err(e) => {
            println!("{} {e:#}", style("Error:").red().bold());
        }
    }

    Ok(())
}

async fn server_info(id: &str) -> Result<()> {
    let spinner = indicatif::ProgressBar::new_spinner();
    spinner.set_message(format!("Fetching info for {}…", style(id).cyan()));
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));

    // Fetch server details and live status concurrently
    let (server_res, status_res) =
        tokio::join!(api::get_server(id), api::get_server_status(id));

    spinner.finish_and_clear();

    let server = match server_res {
        Err(e) => {
            println!("{} {e:#}", style("Error:").red().bold());
            return Ok(());
        }
        Ok(s) => s,
    };

    println!("\n{}", style(&server.name).bold().cyan());
    println!("{}", style("─".repeat(40)).dim());
    println!("  ID      : {}", style(&server.id).dim());
    println!("  Host    : {}@{}:{}", server.user, server.host, server.port);
    println!("  Auth    : {}", server.auth_method);
    println!("  OS      : {}", server.os_family.as_deref().unwrap_or("unknown"));
    println!("  Added   : {}", server.created_at);

    match status_res {
        Ok(status) => {
            println!("\n{}", style("Live Status").bold().underlined());
            println!("  Uptime  : {}", status["uptime"].as_str().unwrap_or("—"));
            println!("  Load    : {}", status["load"].as_str().unwrap_or("—"));
            println!("  Memory  : {}", status["memory"].as_str().unwrap_or("—"));
            println!("  Disk /  : {}", status["disk"].as_str().unwrap_or("—"));
        }
        Err(e) => {
            println!("\n  {} Live status unavailable: {e}", style("⚠").yellow());
        }
    }
    println!();

    Ok(())
}

async fn remove_server(id: &str) -> Result<()> {
    // Fetch name for a human-friendly confirmation prompt
    let name = api::get_server(id)
        .await
        .map(|s| s.name)
        .unwrap_or_else(|_| id.to_string());

    let theme = ColorfulTheme::default();
    let confirm = Confirm::with_theme(&theme)
        .with_prompt(format!(
            "Permanently remove server '{}'?",
            style(&name).red()
        ))
        .default(false)
        .interact()?;

    if !confirm {
        println!("{}", style("Cancelled.").dim());
        return Ok(());
    }

    let spinner = indicatif::ProgressBar::new_spinner();
    spinner.set_message(format!("Removing {}…", style(&name).cyan()));
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));

    let result = api::delete_server(id).await;
    spinner.finish_and_clear();

    match result {
        Ok(_) => println!("{} Removed {}", style("✔").green().bold(), style(&name).cyan()),
        Err(e) => println!("{} {e}", style("Error:").red().bold()),
    }

    Ok(())
}
