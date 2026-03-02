use anyhow::Result;
use console::style;

pub async fn run() -> Result<()> {
    // TODO: GET /api/servers + parallel /api/servers/:id/status
    println!(
        "\n{}\n",
        style("=== Postlab Server Dashboard ===").bold().cyan()
    );
    println!(
        "{}",
        style("Dashboard — not yet implemented (API integration pending)").dim()
    );
    Ok(())
}
