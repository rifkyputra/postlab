use anyhow::Result;
use console::style;
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};

/// Interactive wizard — launched when postlab is run with no subcommand.
pub async fn run_wizard() -> Result<()> {
    let theme = ColorfulTheme::default();

    println!(
        "\n{}\n",
        style("=== Postlab Setup Wizard ===").bold().cyan()
    );
    println!("{}", style("What would you like to do?").dim());

    let actions = &[
        "Add a server",
        "View server status",
        "Install an app",
        "Upgrade OS packages",
        "Harden security",
        "View recent tasks",
        "Exit",
    ];

    let idx = Select::with_theme(&theme)
        .with_prompt("Action")
        .items(actions)
        .default(0)
        .interact()?;

    match idx {
        0 => prompt_add_server(&theme).await,
        1 => {
            println!("{}", style("→ run: postlab status").yellow());
            Ok(())
        }
        2 => {
            let app: String = Input::with_theme(&theme)
                .with_prompt("App name")
                .interact_text()?;
            println!("{}", style(format!("→ run: postlab install {app}")).yellow());
            Ok(())
        }
        3 => {
            println!("{}", style("→ run: postlab upgrade").yellow());
            Ok(())
        }
        4 => {
            println!("{}", style("→ run: postlab harden").yellow());
            Ok(())
        }
        5 => {
            println!("{}", style("→ run: postlab task list").yellow());
            Ok(())
        }
        _ => {
            println!("{}", style("Bye!").dim());
            Ok(())
        }
    }
}

async fn prompt_add_server(theme: &ColorfulTheme) -> Result<()> {
    let name: String = Input::with_theme(theme)
        .with_prompt("Server name")
        .interact_text()?;

    let host: String = Input::with_theme(theme)
        .with_prompt("Host / IP")
        .interact_text()?;

    let port: String = Input::with_theme(theme)
        .with_prompt("SSH port")
        .default("22".to_string())
        .interact_text()?;

    let user: String = Input::with_theme(theme)
        .with_prompt("SSH user")
        .default("root".to_string())
        .interact_text()?;

    let proceed = Confirm::with_theme(theme)
        .with_prompt(format!(
            "Add server '{}' at {}@{}:{}?",
            style(&name).cyan(),
            style(&user).yellow(),
            style(&host).yellow(),
            style(&port).yellow()
        ))
        .default(true)
        .interact()?;

    if proceed {
        println!(
            "{}  {} run: postlab server add --name {} --host {} --port {} --user {}",
            style("✔").green(),
            style("Next,").dim(),
            name,
            host,
            port,
            user
        );
    } else {
        println!("{}", style("Aborted.").red());
    }

    Ok(())
}
