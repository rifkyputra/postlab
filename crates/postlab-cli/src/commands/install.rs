use anyhow::Result;
use console::style;

pub async fn run(app: &str, server: Option<&str>) -> Result<()> {
    // TODO: POST /api/servers/:id/install
    let target = server.unwrap_or("<select interactively>");
    println!(
        "{} Install {} on {} {}",
        style("→").dim(),
        style(app).cyan(),
        style(target).yellow(),
        style("— not yet implemented").dim()
    );
    Ok(())
}
