use anyhow::Result;
use console::style;

pub async fn run(server: Option<&str>) -> Result<()> {
    // TODO: POST /api/servers/:id/harden
    let target = server.unwrap_or("<select interactively>");
    println!(
        "{} Security hardening on {} {}",
        style("→").dim(),
        style(target).yellow(),
        style("— not yet implemented").dim()
    );
    Ok(())
}
