use anyhow::{bail, Result};
use serde::Deserialize;
use tokio::process::Command;

// ── Status ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum PortStatus {
    Unknown,
    Checking,
    Open,
    Closed,
    Error(String),
}

impl PortStatus {
    pub fn label(&self) -> &str {
        match self {
            PortStatus::Unknown  => "?",
            PortStatus::Checking => "…",
            PortStatus::Open     => "OPEN",
            PortStatus::Closed   => "CLOSED",
            PortStatus::Error(_) => "ERR",
        }
    }

    pub fn color(&self) -> ratatui::style::Color {
        use ratatui::style::Color;
        match self {
            PortStatus::Open     => Color::Green,
            PortStatus::Closed   => Color::Red,
            PortStatus::Checking => Color::Yellow,
            PortStatus::Error(_) => Color::Magenta,
            PortStatus::Unknown  => Color::DarkGray,
        }
    }
}

// ── Port entry (label + port + status) ───────────────────────────────────

#[derive(Debug, Clone)]
pub struct PortEntry {
    pub port: u16,
    pub label: String,
    pub status: PortStatus,
}

impl PortEntry {
    pub fn new(port: u16, label: impl Into<String>) -> Self {
        Self { port, label: label.into(), status: PortStatus::Unknown }
    }
}

/// Common homelab port presets.
pub fn default_entries() -> Vec<PortEntry> {
    vec![
        PortEntry::new(22,   "SSH"),
        PortEntry::new(80,   "HTTP"),
        PortEntry::new(443,  "HTTPS"),
        PortEntry::new(3000, "Postlab API"),
        PortEntry::new(8080, "HTTP Alt"),
        PortEntry::new(8443, "HTTPS Alt"),
    ]
}

// ── Public IP ─────────────────────────────────────────────────────────────

/// Fetches the machine's current public IP by calling api.ipify.org.
pub async fn fetch_public_ip() -> Result<String> {
    let out = Command::new("curl")
        .args(["-s", "--connect-timeout", "5", "-m", "8", "https://api.ipify.org"])
        .output()
        .await?;

    let ip = String::from_utf8_lossy(&out.stdout).trim().to_string();

    if ip.is_empty() {
        bail!("empty response from ipify.org — check internet connectivity");
    }

    // Rough IP validation: digits, dots, colons (IPv6)
    if !ip.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == ':') {
        bail!("unexpected response: {}", ip);
    }

    Ok(ip)
}

// ── External port check (portchecker.co) ─────────────────────────────────

#[derive(Deserialize)]
struct CheckResponse {
    ports: Vec<ApiPortResult>,
}

#[derive(Deserialize)]
struct ApiPortResult {
    port: u16,
    status: String,
}

/// Checks the given ports on `ip` via the portchecker.co JSON API.
/// Returns one `(port, PortStatus)` per requested port.
pub async fn check_ports_external(ip: &str, ports: &[u16]) -> Result<Vec<(u16, PortStatus)>> {
    if ports.is_empty() {
        return Ok(vec![]);
    }

    // Build JSON array: [80,443,22]
    let ports_arr: Vec<String> = ports.iter().map(|p| p.to_string()).collect();
    let body = format!(r#"{{"host":"{}","ports":[{}]}}"#, ip, ports_arr.join(","));

    let out = Command::new("curl")
        .args([
            "-s",
            "--connect-timeout", "10",
            "-m", "30",
            "-X", "POST",
            "-H", "Content-Type: application/json",
            "-d", &body,
            "https://portchecker.co/api/v1/query",
        ])
        .output()
        .await?;

    let raw = String::from_utf8_lossy(&out.stdout);
    if raw.trim().is_empty() {
        bail!("no response from portchecker.co");
    }

    let resp: CheckResponse = serde_json::from_str(&raw)
        .map_err(|e| anyhow::anyhow!("portchecker.co parse error: {} — raw: {}", e, raw))?;

    let results = resp.ports.into_iter().map(|r| {
        let status = match r.status.as_str() {
            "open"   => PortStatus::Open,
            "closed" => PortStatus::Closed,
            other    => PortStatus::Error(other.to_string()),
        };
        (r.port, status)
    }).collect();

    Ok(results)
}
