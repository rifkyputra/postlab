use std::path::PathBuf;

use anyhow::Result;
use tokio::process::Command;

// ── Data types ────────────────────────────────────────────────────────────

/// Kind of a permission field — controls how it is rendered and edited.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PermFieldKind {
    /// Boolean — toggled with Space/Enter, rendered as [ON]/[OFF].
    Bool,
    /// Free-form text — edited inline.
    Text,
    /// Comma-separated list — edited as "a, b, c", saved as TOML array ["a","b","c"].
    TextList,
}

/// A single row shown in the Permission tab.
#[derive(Debug, Clone)]
pub struct PermissionField {
    pub path: &'static str,
    pub label: &'static str,
    pub desc: &'static str,
    pub kind: PermFieldKind,
    pub value: String,
}

/// Ordered list of permission fields (toml_path, label, description, kind).
/// These map directly to [`autonomy`] and [`browser`] in config.toml — the
/// fields that control what zeroclaw's tool-calling can actually execute.
pub static PERMISSION_DEFS: &[(&str, &str, &str, PermFieldKind)] = &[
    // ── Autonomy / shell execution ────────────────────────────────────────
    (
        "autonomy.level",
        "Autonomy Level",
        "read_only | supervised | full — overall agent permission tier",
        PermFieldKind::Text,
    ),
    (
        "autonomy.block_high_risk_commands",
        "Block High-Risk Cmds",
        "Block dangerous shell commands even when allowlisted",
        PermFieldKind::Bool,
    ),
    (
        "autonomy.require_approval_for_medium_risk",
        "Approval: Medium Risk",
        "Require user approval before running medium-risk commands",
        PermFieldKind::Bool,
    ),
    (
        "autonomy.workspace_only",
        "Workspace Only",
        "Restrict filesystem access to workspace-relative paths only",
        PermFieldKind::Bool,
    ),
    (
        "autonomy.allowed_commands",
        "Allowed Commands",
        "Commands zeroclaw may run, comma-separated (e.g. uname, echo, git, curl)",
        PermFieldKind::TextList,
    ),
    (
        "autonomy.shell_env_passthrough",
        "Env Passthrough",
        "Env vars forwarded to shell subprocesses (e.g. USER, TERM, LANG)",
        PermFieldKind::TextList,
    ),
    (
        "autonomy.shell_timeout_secs",
        "Shell Timeout (secs)",
        "Max seconds a shell subprocess may run before being killed (default: 60)",
        PermFieldKind::Text,
    ),
    // ── Browser ───────────────────────────────────────────────────────────
    (
        "browser.enabled",
        "Browser Tool",
        "Enable the browser_open tool (open URLs in the system browser)",
        PermFieldKind::Bool,
    ),
    (
        "browser.backend",
        "Browser Backend",
        "agent_browser | rust_native | computer_use | auto",
        PermFieldKind::Text,
    ),
    (
        "browser.native_chrome_path",
        "Chrome Binary Path",
        "Path to Chrome/Chromium (e.g. /usr/bin/chromium-browser, /usr/bin/google-chrome)",
        PermFieldKind::Text,
    ),
];

/// A single editable field shown in the Easy Config tab.
#[derive(Debug, Clone)]
pub struct EasyConfigField {
    pub path: &'static str,
    pub label: &'static str,
    pub desc: &'static str,
    pub value: String,
}

/// Ordered list of fields exposed in the Easy Config tab.
/// Each entry is (toml_path, display_label, description).
pub static EASY_CONFIG_DEFS: &[(&str, &str, &str)] = &[
    (
        "ai.model",
        "AI Model",
        "LLM model (e.g. openai/gpt-4o-mini, anthropic/claude-3-5-sonnet)",
    ),
    (
        "ai.provider",
        "AI Provider",
        "openrouter | openai | anthropic | ollama",
    ),
    (
        "gateway.port",
        "Gateway Port",
        "Port the zeroclaw daemon listens on (default: 42617)",
    ),
    (
        "log_level",
        "Log Level",
        "trace | debug | info | warn | error",
    ),
    (
        "providers.models.openrouter.api_key",
        "OpenRouter API Key",
        "API key for the OpenRouter model provider",
    ),
    (
        "providers.models.openrouter.merge_system_into_user",
        "OpenRouter Merge System",
        "true | false — merge system prompts into the user message",
    ),
    (
        "providers.models.openrouter.model",
        "OpenRouter Model",
        "OpenRouter model id (e.g. nvidia/nemotron-3-super-120b-a12b:free)",
    ),
];

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
        .args([
            "-c",
            "which zeroclaw 2>/dev/null || command -v zeroclaw 2>/dev/null",
        ])
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
                if s.is_empty() {
                    None
                } else {
                    Some(s)
                }
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
    let out = match Command::new(&bin).args(["status"]).output().await {
        Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
        Err(_) => return ZeroclawStatus::default(),
    };

    // Check the gateway port directly — `zeroclaw status` reports the *system service*
    // state ("Service: stopped/running"), not whether the daemon process is alive.
    // A TCP connect to the gateway is the ground truth.
    let daemon_running = tokio::net::TcpStream::connect("127.0.0.1:42617")
        .await
        .is_ok();

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
            if matches!(
                status.as_str(),
                "ok" | "error" | "degraded" | "unknown" | "stopped"
            ) {
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

/// Download and install zeroclaw from GitHub Releases pre-built binaries.
/// Streams status lines into `tx` for TUI progress display.
/// Installs to `~/.local/bin/zeroclaw` (created if absent).
pub async fn install_from_release(
    tx: tokio::sync::mpsc::UnboundedSender<String>,
) -> Result<String> {
    use tokio::process::Command;

    macro_rules! emit {
        ($msg:expr) => {
            let _ = tx.send($msg.to_string());
        };
    }

    // ── Detect platform ──────────────────────────────────────────
    let arch_out = Command::new("uname").arg("-m").output().await?;
    let arch = String::from_utf8_lossy(&arch_out.stdout).trim().to_string();

    let os_out = Command::new("uname").arg("-s").output().await?;
    let os = String::from_utf8_lossy(&os_out.stdout).trim().to_string();

    let target = match (os.as_str(), arch.as_str()) {
        ("Linux", "x86_64") => "x86_64-unknown-linux-gnu",
        ("Linux", "aarch64") => "aarch64-unknown-linux-gnu",
        ("Linux", "armv7l") => "armv7-unknown-linux-gnueabihf",
        ("Linux", "arm") => "arm-unknown-linux-gnueabihf",
        ("Darwin", "arm64") => "aarch64-apple-darwin",
        ("Darwin", "x86_64") => {
            emit!("Intel Mac: no standalone CLI binary in releases — use: brew install zeroclaw");
            return Err(anyhow::anyhow!(
                "Intel macOS not in releases — install via: brew install zeroclaw"
            ));
        }
        _ => {
            return Err(anyhow::anyhow!("Unsupported platform: {os}/{arch}"));
        }
    };

    emit!(format!("Platform: {os}/{arch} → {target}"));

    let url = format!(
        "https://github.com/zeroclaw-labs/zeroclaw/releases/latest/download/zeroclaw-{target}.tar.gz"
    );

    // ── Download to temp file ────────────────────────────────────
    let tmp_dir = std::env::temp_dir().join("zeroclaw-install");
    tokio::fs::create_dir_all(&tmp_dir).await?;
    let archive = tmp_dir.join("zeroclaw.tar.gz");

    emit!(format!("Downloading from GitHub Releases…"));
    let dl = Command::new("curl")
        .args([
            "-fsSL",
            "--retry",
            "3",
            &url,
            "-o",
            archive.to_str().unwrap(),
        ])
        .output()
        .await?;
    if !dl.status.success() {
        let err = String::from_utf8_lossy(&dl.stderr).to_string();
        return Err(anyhow::anyhow!("Download failed: {}", err.trim()));
    }
    emit!("Download complete.");

    // ── Extract binary ───────────────────────────────────────────
    emit!("Extracting…");
    let extract = Command::new("tar")
        .args([
            "xzf",
            archive.to_str().unwrap(),
            "-C",
            tmp_dir.to_str().unwrap(),
            "zeroclaw",
        ])
        .output()
        .await?;
    if !extract.status.success() {
        let err = String::from_utf8_lossy(&extract.stderr).to_string();
        return Err(anyhow::anyhow!("Extraction failed: {}", err.trim()));
    }

    // ── Install binary ───────────────────────────────────────────
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
    let install_dir = PathBuf::from(&home).join(".local").join("bin");
    tokio::fs::create_dir_all(&install_dir).await?;
    let dest = install_dir.join("zeroclaw");

    let src = tmp_dir.join("zeroclaw");
    tokio::fs::copy(&src, &dest).await?;

    // chmod +x
    let chmod = Command::new("chmod")
        .args(["+x", dest.to_str().unwrap()])
        .output()
        .await?;
    if !chmod.status.success() {
        return Err(anyhow::anyhow!("chmod failed"));
    }

    // ── Cleanup ──────────────────────────────────────────────────
    let _ = tokio::fs::remove_dir_all(&tmp_dir).await;

    let installed_path = dest.display().to_string();
    emit!(format!("Installed → {installed_path}"));
    emit!(format!(
        "Add to PATH if needed:  export PATH=\"$HOME/.local/bin:$PATH\""
    ));

    Ok(format!("zeroclaw installed to {installed_path}"))
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
            if lower.contains("api_key")
                || lower.contains("bot_token")
                || lower.contains("secret")
                || lower.contains("password")
                || lower.contains("token")
                || lower.contains("webhook")
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
            let enabled = val.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);
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
    let out = match Command::new(&bin).args(["cron", "list"]).output().await {
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
    let out = match Command::new(&bin).args(["memory", "list"]).output().await {
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

// ── Easy Config ───────────────────────────────────────────────────────────

/// Read the known easy-config fields from ~/.zeroclaw/config.toml.
pub async fn get_easy_config() -> Vec<EasyConfigField> {
    let path = config_path();
    let content = tokio::fs::read_to_string(&path).await.unwrap_or_default();
    let table: toml::Value =
        toml::from_str(&content).unwrap_or(toml::Value::Table(toml::map::Map::new()));

    EASY_CONFIG_DEFS
        .iter()
        .map(|(toml_path, label, desc)| {
            let value = read_toml_path(&table, toml_path).unwrap_or_default();
            EasyConfigField {
                path: toml_path,
                label,
                desc,
                value,
            }
        })
        .collect()
}

/// Write a single field back to ~/.zeroclaw/config.toml preserving existing
/// formatting and comments.  Creates the file (and `[section]` header) if absent.
pub async fn set_config_field(toml_path: &str, new_value: &str) -> Result<()> {
    let file = config_path();
    let content = tokio::fs::read_to_string(&file).await.unwrap_or_default();
    let path_parts: Vec<&str> = toml_path.split('.').collect();
    let (section_parts, key) = match path_parts.split_last() {
        Some((key, [])) => (Vec::new(), *key),
        Some((key, sections)) => (sections.to_vec(), *key),
        None => return Err(anyhow::anyhow!("invalid config path: {}", toml_path)),
    };
    let updated = patch_toml_text(&content, &section_parts, key, new_value);
    // Ensure parent directory exists
    if let Some(parent) = file.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(&file, updated).await?;
    Ok(())
}

fn read_toml_path(table: &toml::Value, path: &str) -> Option<String> {
    let mut current = table;
    for part in path.split('.') {
        current = current.get(part)?;
    }
    Some(match current {
        toml::Value::String(s) => s.clone(),
        toml::Value::Integer(i) => i.to_string(),
        toml::Value::Float(f) => f.to_string(),
        toml::Value::Boolean(b) => b.to_string(),
        toml::Value::Array(arr) => arr
            .iter()
            .map(|v| match v {
                toml::Value::String(s) => s.clone(),
                other => other.to_string(),
            })
            .collect::<Vec<_>>()
            .join(", "),
        other => other.to_string(),
    })
}

/// Convert a comma-separated user string into a TOML inline array literal.
/// "git, cargo, uname" → ["git", "cargo", "uname"]
pub fn comma_list_to_toml_array(s: &str) -> String {
    let items: Vec<String> = s
        .split(',')
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .map(|item| format!("\"{}\"", item))
        .collect();
    format!("[{}]", items.join(", "))
}

/// Read the known permission fields from ~/.zeroclaw/config.toml.
pub async fn get_permissions() -> Vec<PermissionField> {
    let path = config_path();
    let content = tokio::fs::read_to_string(&path).await.unwrap_or_default();
    let table: toml::Value =
        toml::from_str(&content).unwrap_or(toml::Value::Table(toml::map::Map::new()));

    PERMISSION_DEFS
        .iter()
        .map(|(toml_path, label, desc, kind)| {
            let value = read_toml_path(&table, toml_path).unwrap_or_else(|| match kind {
                PermFieldKind::Bool => "false".to_string(),
                PermFieldKind::Text | PermFieldKind::TextList => String::new(),
            });
            PermissionField {
                path: toml_path,
                label,
                desc,
                kind: *kind,
                value,
            }
        })
        .collect()
}

/// In-place line replacement that preserves comments and ordering.
/// Values that are "true", "false", or valid numbers are written unquoted.
fn patch_toml_text(content: &str, sections: &[&str], key: &str, value: &str) -> String {
    let needs_quotes = !matches!(value, "true" | "false")
        && value.parse::<f64>().is_err()
        && !value.starts_with('[');
    let new_line = if needs_quotes {
        format!("{} = \"{}\"", key, value)
    } else {
        format!("{} = {}", key, value)
    };
    let raw: Vec<&str> = content.lines().collect();
    let mut out: Vec<String> = Vec::with_capacity(raw.len());
    let mut in_target = sections.is_empty();
    let mut replaced = false;
    let mut section_header_out_idx: Option<usize> = None;
    let mut section_end_out_idx: Option<usize> = None;
    let wanted_section = (!sections.is_empty()).then(|| sections.join("."));
    let mut i = 0;
    while i < raw.len() {
        let line = raw[i];
        let trimmed = line.trim();
        if trimmed.starts_with('[') && !trimmed.starts_with("[[") {
            let sec = trimmed.trim_matches(|c: char| c == '[' || c == ']').trim();
            if let Some(wanted) = wanted_section.as_deref() {
                if sec == wanted {
                    in_target = true;
                    out.push(line.to_string());
                    section_header_out_idx = Some(out.len());
                    i += 1;
                    continue;
                } else if in_target && section_end_out_idx.is_none() {
                    section_end_out_idx = Some(out.len());
                    in_target = false;
                }
            } else if in_target && section_end_out_idx.is_none() {
                section_end_out_idx = Some(out.len());
                in_target = false;
            }
        }
        if in_target && !replaced {
            if let Some((k, rest)) = line.split_once('=') {
                if k.trim() == key {
                    out.push(new_line.clone());
                    replaced = true;
                    let rest_t = rest.trim();
                    // Multi-line array: opening `[` without closing `]` on same line
                    if rest_t.starts_with('[') && !rest_t.trim_end_matches(',').ends_with(']') {
                        i += 1;
                        while i < raw.len() {
                            if raw[i].trim().starts_with(']') {
                                i += 1;
                                break;
                            }
                            i += 1;
                        }
                    } else {
                        i += 1;
                    }
                    continue;
                }
            }
        }
        out.push(line.to_string());
        i += 1;
    }
    if !replaced {
        if let Some(sec) = wanted_section.as_deref() {
            if section_header_out_idx.is_some() {
                let at = section_end_out_idx.unwrap_or(out.len());
                out.insert(at, new_line);
            } else {
                if out.last().map(|l| !l.is_empty()).unwrap_or(false) {
                    out.push(String::new());
                }
                out.push(format!("[{}]", sec));
                out.push(new_line);
            }
        } else {
            let at = section_end_out_idx.unwrap_or(out.len());
            out.insert(at, new_line);
        }
    }
    let joined = out.join("\n");
    if content.ends_with('\n') && !joined.ends_with('\n') {
        format!("{}\n", joined)
    } else {
        joined
    }
}

#[cfg(test)]
mod tests {
    use super::{patch_toml_text, read_toml_path};

    #[test]
    fn reads_nested_toml_paths() {
        let table: toml::Value = toml::from_str(
            r#"
[providers.models.openrouter]
api_key = "***"
merge_system_into_user = false
model = "nvidia/nemotron-3-super-120b-a12b:free"
"#,
        )
        .expect("valid toml");

        assert_eq!(
            read_toml_path(&table, "providers.models.openrouter.api_key").as_deref(),
            Some("***")
        );
        assert_eq!(
            read_toml_path(&table, "providers.models.openrouter.merge_system_into_user").as_deref(),
            Some("false")
        );
        assert_eq!(
            read_toml_path(&table, "providers.models.openrouter.model").as_deref(),
            Some("nvidia/nemotron-3-super-120b-a12b:free")
        );
    }

    #[test]
    fn updates_existing_nested_section_field() {
        let content = r#"[providers.models.openrouter]
api_key = "***"
merge_system_into_user = false
"#;

        let updated = patch_toml_text(
            content,
            &["providers", "models", "openrouter"],
            "merge_system_into_user",
            "true",
        );

        assert!(updated.contains("merge_system_into_user = true"));
        assert!(!updated.contains("merge_system_into_user = false"));
    }

    #[test]
    fn creates_missing_nested_section_for_field() {
        let updated = patch_toml_text(
            "",
            &["providers", "models", "openrouter"],
            "model",
            "nvidia/nemotron-3-super-120b-a12b:free",
        );

        assert!(updated.contains("[providers.models.openrouter]"));
        assert!(updated.contains("model = \"nvidia/nemotron-3-super-120b-a12b:free\""));
    }
}
