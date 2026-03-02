use anyhow::Result;
use clap::Subcommand;
use console::style;

use crate::api;

#[derive(Subcommand)]
pub enum TaskAction {
    /// List recent tasks
    List {
        /// Filter by server ID
        #[arg(long)]
        server: Option<String>,
    },
    /// Show task details and output
    Show {
        /// Task ID
        id: String,
    },
}

pub async fn run(action: TaskAction) -> Result<()> {
    match action {
        TaskAction::List { server } => list_tasks(server.as_deref()).await,
        TaskAction::Show { id } => show_task(&id).await,
    }
}

async fn list_tasks(server_id: Option<&str>) -> Result<()> {
    let spinner = indicatif::ProgressBar::new_spinner();
    spinner.set_message("Fetching tasks…");
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));

    let result = api::list_tasks(server_id).await;
    spinner.finish_and_clear();

    match result {
        Ok(tasks) if tasks.is_empty() => {
            println!("{}", style("No tasks yet.").dim());
        }
        Ok(tasks) => {
            println!(
                "\n{:<36}  {:<12}  {:<10}  {:<22}  {}",
                style("ID").bold(),
                style("KIND").bold(),
                style("STATUS").bold(),
                style("CREATED").bold(),
                style("SERVER").bold(),
            );
            println!("{}", style("─".repeat(100)).dim());
            for t in &tasks {
                let status_styled = match t.status.as_str() {
                    "success" => style(t.status.as_str()).green(),
                    "failed"  => style(t.status.as_str()).red(),
                    "running" => style(t.status.as_str()).cyan(),
                    _         => style(t.status.as_str()).dim(),
                };
                println!(
                    "{:<36}  {:<12}  {:<10}  {:<22}  {}",
                    style(&t.id).dim(),
                    style(&t.kind).cyan(),
                    status_styled,
                    &t.created_at[..19], // trim to YYYY-MM-DDTHH:MM:SS
                    style(&t.server_id).dim(),
                );
            }
            println!("\n{} task(s)", tasks.len());
        }
        Err(e) => {
            println!("{} {e:#}", style("Error:").red().bold());
        }
    }

    Ok(())
}

async fn show_task(id: &str) -> Result<()> {
    let spinner = indicatif::ProgressBar::new_spinner();
    spinner.set_message(format!("Fetching task {}…", style(id).cyan()));
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));

    let result = api::get_task(id).await;
    spinner.finish_and_clear();

    match result {
        Err(e) => {
            println!("{} {e}", style("Error:").red().bold());
        }
        Ok(task) => {
            let status_styled = match task.status.as_str() {
                "success" => style(task.status.as_str()).green().bold(),
                "failed"  => style(task.status.as_str()).red().bold(),
                "running" => style(task.status.as_str()).cyan().bold(),
                _         => style(task.status.as_str()).dim().bold(),
            };

            println!("\n{}", style("Task").bold().underlined());
            println!("  ID        : {}", style(&task.id).dim());
            println!("  Server    : {}", task.server_id);
            println!("  Kind      : {}", style(&task.kind).cyan());
            println!("  Status    : {status_styled}");
            println!("  Created   : {}", task.created_at);
            if let Some(ref started) = task.started_at {
                println!("  Started   : {started}");
            }
            if let Some(ref done) = task.completed_at {
                println!("  Completed : {done}");
            }
            if let Some(ref input) = task.input_json {
                println!("  Input     : {}", style(input).dim());
            }
            if let Some(ref out) = task.output {
                println!("\n{}", style("Output:").bold().underlined());
                for line in out.lines() {
                    println!("  {line}");
                }
            }
            if let Some(ref err) = task.error {
                println!("\n{}", style("Error:").red().bold());
                for line in err.lines() {
                    println!("  {}", style(line).red());
                }
            }
            println!();
        }
    }

    Ok(())
}
