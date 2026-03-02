use anyhow::Result;
use console::style;

pub async fn run(server: Option<&str>) -> Result<()> {
    // TODO: POST /api/servers/:id/upgrade
    let target = server.unwrap_or("<all servers>");
    println!(
        "{} Upgrade OS on {} {}",
        style("→").dim(),
        style(target).yellow(),
        style("— not yet implemented").dim()
    );
    Ok(())
}
