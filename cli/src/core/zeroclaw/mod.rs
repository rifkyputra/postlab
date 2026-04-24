use std::path::PathBuf;

use anyhow::Result;
use tokio::process::Command;

// ── Data types ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ZeroclawInfo {
    pub installed: bool,
    pub version: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ComponentHealth {
    pub name: String,
    pub status: String, // "ok" | "error" | "unknown"
}

#[derive(Debug, Clone, Default)]
pub struct ZeroclawStatus {
    pub daemon_running: bool,
    pub gateway_port: u16,
    pub components: Vec<ComponentHealth>,
    pub raw: String,
}

#[derive(Debug, Clone)]
pub struct ZeroclawChannel {
    pub name: String,
    pub platform: String,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct CronJob {
    pub id: String,
    pub schedule: String,
    pub command: String,
    pub last_run: Option<String>,
    pub next_run: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MemoryEntry {
    pub key: String,
    pub preview: String,
    pub created_at: String,
}

// ── Helpers ───────────────────────────────────────────────────────────────

fn config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
    PathBuf::from(home).join(".zeroclaw").join("config.toml")
}

/// Resolve the full path to the zeroclaw binary by searching PATH and common
/// installation locations (cargo, homebrew, local).  Returns None if not found.
async fn find_zeroclaw() -> Option<String> {
    // First: honour the user-set env variable
    if let Ok(p) = std::env::var("ZEROCLAW_BIN") {
        if tokio::fs::metadata(&p).await.is_ok() {
            return Some(p);
        }
    }

    // Second: try common hardcoded locations before falling back to PATH
    let home = std::env::var("HOME").unwrap_or_default();
    let candidates = [
        format!("{home}/.cargo/bin/zeroclaw"),
        format!("{home}/.local/bin/zeroclaw"),
        "/usr/local/bin/zeroclaw".to_string(),
        "/opt/homebrew/bin/zeroclaw".to_string(),
        "/usr/bin/zeroclaw".to_string(),
    ];
    for path in &candidates {
        if tokio::fs::metadata(path).await.is_ok() {
            return Some(path.clone());
        }
    }

    // Last: ask the shell (handles aliases / shims / nix / asdf etc.)
    if let Ok(out) = Command::new("sh")
        .args(["-c", "which zeroclaw 2>/dev/null || command -v zeroclaw 2>/dev/null"])
        .output()
        .await
    {
        let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !path.is_empty() && out.status.success() {
            return Some(path);
        }
    }

    None
}

async fn run(args: &[&str]) -> Result<String> {
    let bin = find_zeroclaw()
        .await
        .ok_or_else(|| anyhow::anyhow!("zeroclaw not found — install it first"))?;

    // Ensure PATH includes cargo bin in case zeroclaw itself spawns subprocesses
    let home = std::env::var("HOME").unwrap_or_default();
    let current_path = std::env::var("PATH").unwrap_or_default();
    let extended_path = format!(
        "{home}/.cargo/bin:{home}/.local/bin:/usr/local/bin:/opt/homebrew/bin:{current_path}"
    );

    let out = Command::new(&bin)
        .args(args)
        .env("PATH", extended_path)
        .output()
        .await?;
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    if out.status.success() {
        Ok(stdout)
    } else {
        let msg = if stderr.is_empty() { stdout } else { stderr };
        Err(anyhow::anyhow!("{}", msg.trim()))
    }
}

// ── Public API ────────────────────────────────────────────────────────────

pub async fn get_info() -> ZeroclawInfo {
    let bin = find_zeroclaw().await;
    let installed = bin.is_some();

    let version = if let Some(ref path) = bin {
        let home = std::env::var("HOME").unwrap_or_default();
        let current_path = std::env::var("PATH").unwrap_or_default();
        let extended_path = format!(
            "{home}/.cargo/bin:{home}/.local/bin:/usr/local/bin:/opt/homebrew/bin:{current_path}"
        );
        Command::new(path)
            .arg("--version")
            .env("PATH", extended_path)
            .output()
            .await
            .ok()
            .and_then(|o| {
                let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                if s.is_empty() { None } else { Some(s) }
            })
    } else {
        None
    };

    ZeroclawInfo { installed, version }
}

pub async fn get_status() -> ZeroclawStatus {
    let bin = match find_zeroclaw().await {
        Some(b) => b,
        None => return ZeroclawStatus::default(),
    };
    let out = match Command::new(&bin)
        .args(["status"])
        .output()
        .await
    {
        Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
        Err(_) => return ZeroclawStatus::default(),
    };

    let daemon_running = out.contains("running") || out.contains("Running");

    // Parse component lines — zeroclaw status often outputs lines like:
    //   gateway      ok
    //   channels     error
    let mut components: Vec<ComponentHealth> = Vec::new();
    for line in out.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            let name = parts[0].to_string();
            let status = parts[1].to_string();
            // Only add lines that look like component rows
            if matches!(status.as_str(), "ok" | "error" | "degraded" | "unknown" | "stopped") {
                components.push(ComponentHealth { name, status });
            }
        }
    }

    ZeroclawStatus {
        daemon_running,
        gateway_port: 42617,
        components,
        raw: out,
    }
}

pub async fn daemon_start() -> Result<String> {
    run(&["daemon"]).await
}

pub async fn daemon_stop() -> Result<String> {
    // zeroclaw doesn't have a direct stop command; send SIGTERM to the daemon process
    let out = Command::new("pkill")
        .args(["-f", "zeroclaw daemon"])
        .output()
        .await?;
    if out.status.success() {
        Ok("Daemon stopped".to_string())
    } else {
        // pkill returns 1 if no process found
        Ok("No daemon process found".to_string())
    }
}

pub async fn service_install() -> Result<String> {
    run(&["service", "install"]).await
}

pub async fn service_start() -> Result<String> {
    run(&["service", "start"]).await
}

pub async fn service_stop() -> Result<String> {
    run(&["service", "stop"]).await
}

pub async fn update_check() -> Result<String> {
    run(&["update", "--check"]).await
}

pub async fn update_apply() -> Result<String> {
    run(&["update", "--force"]).await
}

pub async fn run_doctor() -> Result<String> {
    run(&["doctor"]).await
}

/// Read ~/.zeroclaw/config.toml and mask sensitive values.
pub async fn get_config() -> String {
    let path = config_path();
    let content = match tokio::fs::read_to_string(&path).await {
        Ok(c) => c,
        Err(e) => return format!("# Config not found at {}\n# {}", path.display(), e),
    };
    mask_secrets(&content)
}

fn mask_secrets(content: &str) -> String {
    content
        .lines()
        .map(|line| {
            let lower = line.to_lowercase();
            // Mask lines that assign sensitive values
            if lower.contains("api_key") || lower.contains("bot_token")
                || lower.contains("secret") || lower.contains("password")
                || lower.contains("token") || lower.contains("webhook")
                || lower.contains("private_key")
            {
                // Keep the key name, replace the value with ***
                if let Some(eq) = line.find('=') {
                    let key_part = &line[..eq + 1];
                    return format!("{} \"***\"", key_part.trim_end());
                }
            }
            line.to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Parse configured channels from config.toml.
pub async fn list_channels() -> Vec<ZeroclawChannel> {
    let path = config_path();
    let content = match tokio::fs::read_to_string(&path).await {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let table: toml::Value = match toml::from_str(&content) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let channels_table = match table.get("channels").and_then(|v| v.as_table()) {
        Some(t) => t,
        None => return Vec::new(),
    };

    let platform_map: &[(&str, &str)] = &[
        ("telegram", "Telegram"),
        ("discord", "Discord"),
        ("slack", "Slack"),
        ("whatsapp", "WhatsApp"),
        ("signal", "Signal"),
        ("email", "Email"),
        ("imessage", "iMessage"),
        ("matrix", "Matrix"),
        ("irc", "IRC"),
        ("bluesky", "Bluesky"),
        ("nostr", "Nostr"),
        ("twitter", "Twitter"),
        ("mqtt", "MQTT"),
    ];

    channels_table
        .iter()
        .map(|(key, val)| {
            let enabled = val
                .get("enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let platform = platform_map
                .iter()
                .find(|(k, _)| key.starts_with(k))
                .map(|(_, p)| p.to_string())
                .unwrap_or_else(|| key.to_uppercase());
            ZeroclawChannel {
                name: key.clone(),
                platform,
                enabled,
            }
        })
        .collect()
}

pub async fn list_cron() -> Vec<CronJob> {
    let bin = match find_zeroclaw().await {
        Some(b) => b,
        None => return Vec::new(),
    };
    let out = match Command::new(&bin)
        .args(["cron", "list"])
        .output()
        .await
    {
        Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
        Err(_) => return Vec::new(),
    };

    // Parse tabular output: lines like "abc123  * * * * *  command  last_run  next_run"
    out.lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with("ID") && !l.starts_with('-'))
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(6, '\t').collect();
            if parts.len() < 3 {
                // Fallback: space-split, treat first token as id, next 5 as cron, rest as cmd
                let sp: Vec<&str> = line.splitn(8, ' ').collect();
                if sp.len() >= 7 {
                    return Some(CronJob {
                        id: sp[0].to_string(),
                        schedule: format!("{} {} {} {} {}", sp[1], sp[2], sp[3], sp[4], sp[5]),
                        command: sp[6..].join(" "),
                        last_run: None,
                        next_run: None,
                    });
                }
                return None;
            }
            Some(CronJob {
                id: parts[0].trim().to_string(),
                schedule: parts[1].trim().to_string(),
                command: parts[2].trim().to_string(),
                last_run: parts.get(3).map(|s| s.trim().to_string()),
                next_run: parts.get(4).map(|s| s.trim().to_string()),
            })
        })
        .collect()
}

pub async fn add_cron(schedule: &str, command: &str) -> Result<()> {
    run(&["cron", "add", schedule, command]).await.map(|_| ())
}

pub async fn delete_cron(id: &str) -> Result<()> {
    run(&["cron", "remove", id]).await.map(|_| ())
}

pub async fn list_memory() -> Vec<MemoryEntry> {
    let bin = match find_zeroclaw().await {
        Some(b) => b,
        None => return Vec::new(),
    };
    let out = match Command::new(&bin)
        .args(["memory", "list"])
        .output()
        .await
    {
        Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
        Err(_) => return Vec::new(),
    };

    out.lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with("Key") && !l.starts_with('-'))
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(3, '\t').collect();
            if parts.len() >= 2 {
                let preview = parts.get(1).unwrap_or(&"").trim().to_string();
                let preview = if preview.len() > 60 {
                    format!("{}…", &preview[..60])
                } else {
                    preview
                };
                return Some(MemoryEntry {
                    key: parts[0].trim().to_string(),
                    preview,
                    created_at: parts.get(2).unwrap_or(&"").trim().to_string(),
                });
            }
            // Single-column fallback
            Some(MemoryEntry {
                key: line.trim().to_string(),
                preview: String::new(),
                created_at: String::new(),
            })
        })
        .collect()
}

pub async fn delete_memory(key: &str) -> Result<()> {
    run(&["memory", "clear", "--key", key]).await.map(|_| ())
}
