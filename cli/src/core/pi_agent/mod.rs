pub mod rpc;

use std::path::{Path, PathBuf};

use anyhow::Result;
use tokio::process::Command;

// ── Data types ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct PiAgentInfo {
    pub installed: bool,
    pub version: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PiSession {
    pub name: String,
    pub path: String,
    pub modified: String,
}

#[derive(Debug, Clone)]
pub struct PiSkill {
    pub name: String,
    #[allow(dead_code)]
    pub source: String,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct PiAuthEntry {
    pub provider: String,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct LibrarySkill {
    pub name: String,
    pub description: String,
    pub installed: bool,
}

const SKILLS_MANIFEST: &str =
    include_str!("../../../../skills_library/manifest.json");
const UBUNTU_SYSADMIN_SKILL: &str =
    include_str!("../../../../skills_library/ubuntu-sysadmin/SKILL.md");
const FEDORA_SYSADMIN_SKILL: &str =
    include_str!("../../../../skills_library/fedora-sysadmin/SKILL.md");

// ── Path helpers ──────────────────────────────────────────────────────────

/// Real (non-root) home: prefer SUDO_USER-derived path so postlab running
/// under sudo still finds files in the invoking user's home directory.
pub(super) fn real_home() -> String {
    std::env::var("SUDO_USER")
        .ok()
        .map(|u| format!("/home/{}", u))
        .unwrap_or_else(|| std::env::var("HOME").unwrap_or_else(|_| "/root".into()))
}

fn pi_dir() -> PathBuf {
    PathBuf::from(real_home()).join(".pi")
}

fn config_path() -> PathBuf {
    pi_dir().join("agent").join("settings.json")
}

fn auth_path() -> PathBuf {
    pi_dir().join("agent").join("auth.json")
}

fn sessions_dir() -> PathBuf {
    pi_dir().join("agent").join("sessions")
}

/// Locate the `pi` binary by checking PI_BIN env var, common install locations,
/// then PATH via shell.
pub(super) async fn find_pi() -> Option<String> {
    if let Ok(p) = std::env::var("PI_BIN") {
        if tokio::fs::metadata(&p).await.is_ok() {
            return Some(p);
        }
    }

    let home = real_home();
    let candidates = [
        format!("{home}/.local/bin/pi"),
        "/usr/local/bin/pi".to_string(),
        format!("{home}/.npm-global/bin/pi"),
        "/usr/bin/pi".to_string(),
    ];
    for path in &candidates {
        if tokio::fs::metadata(path).await.is_ok() {
            return Some(path.clone());
        }
    }

    // pi-node installs under ~/.local/share/pi-node/<node-version>/bin/pi
    let pi_node_dir = PathBuf::from(&home).join(".local/share/pi-node");
    if let Ok(mut entries) = tokio::fs::read_dir(&pi_node_dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let candidate = entry.path().join("bin/pi");
            if tokio::fs::metadata(&candidate).await.is_ok() {
                return Some(candidate.display().to_string());
            }
        }
    }

    // Shell PATH fallback — run as the real user if possible
    let shell_cmd = if let Ok(user) = std::env::var("SUDO_USER") {
        format!("su -s /bin/sh -c 'which pi 2>/dev/null || command -v pi 2>/dev/null' {user}")
    } else {
        "which pi 2>/dev/null || command -v pi 2>/dev/null".to_string()
    };
    if let Ok(out) = Command::new("sh").args(["-c", &shell_cmd]).output().await {
        let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !path.is_empty() && out.status.success() {
            return Some(path);
        }
    }

    None
}

async fn run(args: &[&str]) -> Result<String> {
    let bin = find_pi()
        .await
        .ok_or_else(|| anyhow::anyhow!("pi not found — install it first"))?;

    let home = real_home();
    let current_path = std::env::var("PATH").unwrap_or_default();
    let extended_path =
        format!("{home}/.local/bin:{home}/.npm-global/bin:/usr/local/bin:{current_path}");

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

pub async fn get_info() -> PiAgentInfo {
    let bin = find_pi().await;
    let installed = bin.is_some();

    let version = if let Some(ref path) = bin {
        let home = real_home();
        let current_path = std::env::var("PATH").unwrap_or_default();
        let extended_path = format!(
            "{home}/.local/bin:{home}/.npm-global/bin:/usr/local/bin:{current_path}"
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

    PiAgentInfo { installed, version }
}

pub async fn list_sessions() -> Vec<PiSession> {
    let dir = sessions_dir();
    let mut top = match tokio::fs::read_dir(&dir).await {
        Ok(rd) => rd,
        Err(_) => return Vec::new(),
    };

    let mut sessions = Vec::new();

    // Sessions live in per-project subdirectories, e.g. --home-ubuntu-postlab--/
    while let Ok(Some(project_entry)) = top.next_entry().await {
        let subdir = project_entry.path();
        if !subdir.is_dir() {
            continue;
        }
        let project = subdir
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let mut sub = match tokio::fs::read_dir(&subdir).await {
            Ok(rd) => rd,
            Err(_) => continue,
        };
        while let Ok(Some(entry)) = sub.next_entry().await {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();
            let name = format!("{}/{}", project, stem);
            let modified = entry
                .metadata()
                .await
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| {
                    let s = t.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs();
                    let min = (s / 60) % 60;
                    let hr = (s / 3600) % 24;
                    let days = s / 86400;
                    let yr = 1970 + days / 365;
                    let mo = (days % 365) / 30 + 1;
                    let dy = (days % 365) % 30 + 1;
                    Some(format!("{yr:04}-{mo:02}-{dy:02} {hr:02}:{min:02}"))
                })
                .unwrap_or_else(|| "unknown".to_string());
            sessions.push(PiSession {
                name,
                path: path.display().to_string(),
                modified,
            });
        }
    }

    sessions.sort_by(|a, b| b.modified.cmp(&a.modified));
    sessions
}

pub async fn get_config() -> String {
    let path = config_path();
    match tokio::fs::read_to_string(&path).await {
        Ok(c) => mask_secrets(&c),
        Err(e) => format!("// Config not found at {}\n// {}", path.display(), e),
    }
}

pub async fn default_provider_model() -> (String, String) {
    let path = config_path();
    let provider = String::from("openrouter");
    let model = String::from("claude-sonnet-4-5");
    if let Ok(content) = tokio::fs::read_to_string(&path).await {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
            let p = v.get("defaultProvider")
                .and_then(|v| v.as_str())
                .unwrap_or(&provider)
                .to_string();
            let m = v.get("defaultModel")
                .and_then(|v| v.as_str())
                .unwrap_or(&model)
                .to_string();
            return (p, m);
        }
    }
    (provider, model)
}

fn mask_secrets(content: &str) -> String {
    // JSON: mask values for lines containing key-like tokens
    content
        .lines()
        .map(|line| {
            let lower = line.to_lowercase();
            if lower.contains("api_key")
                || lower.contains("apikey")
                || lower.contains("secret")
                || lower.contains("password")
                || lower.contains("token")
                || lower.contains("webhook")
            {
                if let Some(colon) = line.find(':') {
                    let key_part = &line[..colon + 1];
                    return format!("{} \"***\"", key_part.trim_end());
                }
            }
            line.to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub async fn get_auth() -> Vec<PiAuthEntry> {
    let path = auth_path();
    let content = match tokio::fs::read_to_string(&path).await {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let v: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let obj = match v.as_object() {
        Some(o) => o,
        None => return Vec::new(),
    };

    obj.iter()
        .map(|(provider, val)| {
            let has_key = ["key", "apiKey", "api_key", "token"]
                .iter()
                .any(|&f| val.get(f).and_then(|v| v.as_str()).map(|s| !s.is_empty()).unwrap_or(false));
            PiAuthEntry {
                provider: provider.clone(),
                status: if has_key { "configured".to_string() } else { "missing key".to_string() },
            }
        })
        .collect()
}

pub async fn list_skills() -> Vec<PiSkill> {
    // Skills are npm packages installed under ~/.pi/agent/npm/node_modules/
    // that contain a skills/ subdirectory.
    let npm_dir = pi_dir().join("agent").join("npm").join("node_modules");
    let mut read_dir = match tokio::fs::read_dir(&npm_dir).await {
        Ok(rd) => rd,
        Err(_) => return Vec::new(),
    };

    let mut skills = Vec::new();
    while let Ok(Some(entry)) = read_dir.next_entry().await {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        // Only include packages that expose a skills/ directory
        if !path.join("skills").is_dir() {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
        let description = try_read_skill_description(&path).await;
        skills.push(PiSkill {
            name,
            source: path.display().to_string(),
            description,
        });
    }

    skills.sort_by(|a, b| a.name.cmp(&b.name));
    skills
}

async fn try_read_skill_description(path: &Path) -> String {
    // Try package.json description field
    let pkg = path.join("package.json");
    if let Ok(content) = tokio::fs::read_to_string(&pkg).await {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(desc) = v.get("description").and_then(|d| d.as_str()) {
                return desc.to_string();
            }
        }
    }
    String::new()
}

pub async fn remove_skill(name: &str) -> Result<()> {
    let pkg_dir = pi_dir()
        .join("agent")
        .join("npm")
        .join("node_modules")
        .join(name);
    if pkg_dir.exists() {
        tokio::fs::remove_dir_all(&pkg_dir).await?;
        Ok(())
    } else {
        Err(anyhow::anyhow!("skill '{}' not found", name))
    }
}

pub async fn get_logs(lines: usize) -> Vec<String> {
    // Read the most recently modified session file
    let sessions = list_sessions().await;
    if let Some(session) = sessions.first() {
        if let Ok(content) = tokio::fs::read_to_string(&session.path).await {
            let all: Vec<String> = content.lines().map(|l| l.to_string()).collect();
            let start = all.len().saturating_sub(lines);
            return all[start..].to_vec();
        }
    }

    // Fallback: journalctl for pi service
    if let Ok(out) = Command::new("journalctl")
        .args(["-u", "pi", "--no-pager", "-n", &lines.to_string()])
        .output()
        .await
    {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).to_string();
            if !s.trim().is_empty() {
                return s.lines().map(|l| l.to_string()).collect();
            }
        }
    }

    Vec::new()
}

pub async fn update_check() -> Result<String> {
    run(&["update", "--self", "--check"]).await
}

pub async fn update_apply() -> Result<String> {
    run(&["update", "--self"]).await
}

/// Install pi via the official install script.
/// Streams progress lines into `tx`.
pub async fn install_pi(tx: tokio::sync::mpsc::UnboundedSender<String>) -> Result<String> {
    macro_rules! emit {
        ($msg:expr) => {
            let _ = tx.send($msg.to_string());
        };
    }

    // Pre-flight: curl required
    let curl_check = Command::new("sh")
        .args(["-c", "command -v curl"])
        .output()
        .await?;
    if !curl_check.status.success() {
        return Err(anyhow::anyhow!(
            "curl is required. Install it: sudo apt install curl"
        ));
    }

    emit!("Downloading pi install script from pi.dev…");

    // Run the official installer; stream stderr (install.sh logs there)
    let home = real_home();
    let current_path = std::env::var("PATH").unwrap_or_default();
    let extended_path =
        format!("{home}/.local/bin:{home}/.npm-global/bin:/usr/local/bin:{current_path}");

    let mut child = tokio::process::Command::new("sh")
        .args(["-c", "curl -fsSL https://pi.dev/install.sh | sh"])
        .env("PATH", extended_path)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    // Forward stdout lines as progress
    if let Some(stdout) = child.stdout.take() {
        let tx2 = tx.clone();
        tokio::spawn(async move {
            use tokio::io::AsyncBufReadExt;
            let mut lines = tokio::io::BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = tx2.send(line);
            }
        });
    }
    if let Some(stderr) = child.stderr.take() {
        let tx3 = tx.clone();
        tokio::spawn(async move {
            use tokio::io::AsyncBufReadExt;
            let mut lines = tokio::io::BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = tx3.send(line);
            }
        });
    }

    let status = child.wait().await?;
    if status.success() {
        let version = get_info().await.version.unwrap_or_else(|| "unknown".into());
        emit!(format!("pi {version} installed successfully."));
        Ok(format!("pi {version} installed"))
    } else {
        Err(anyhow::anyhow!(
            "Install script exited with non-zero status. Check the log above."
        ))
    }
}

// ── Skills library ────────────────────────────────────────────────────────

fn skills_dir() -> PathBuf {
    PathBuf::from(real_home()).join(".pi").join("agent").join("skills")
}

pub async fn list_library_skills() -> Vec<LibrarySkill> {
    let manifest: serde_json::Value = match serde_json::from_str(SKILLS_MANIFEST) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let skills_array = match manifest.get("skills").and_then(|s| s.as_array()) {
        Some(a) => a,
        None => return Vec::new(),
    };

    let mut result = Vec::new();
    for entry in skills_array {
        let name = entry
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let description = entry
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let installed = skills_dir().join(&name).join("SKILL.md").exists();
        result.push(LibrarySkill {
            name,
            description,
            installed,
        });
    }

    result.sort_by(|a, b| a.name.cmp(&b.name));
    result
}

fn get_skill_content(name: &str) -> Option<&'static str> {
    match name {
        "ubuntu-sysadmin" => Some(UBUNTU_SYSADMIN_SKILL),
        "fedora-sysadmin" => Some(FEDORA_SYSADMIN_SKILL),
        _ => None,
    }
}

pub async fn install_library_skill(name: &str) -> Result<()> {
    let content = get_skill_content(name)
        .ok_or_else(|| anyhow::anyhow!("skill '{}' not found in library", name))?;

    let skill_dir = skills_dir().join(name);
    tokio::fs::create_dir_all(&skill_dir).await?;
    tokio::fs::write(skill_dir.join("SKILL.md"), content).await?;
    Ok(())
}
