use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsInfo {
    pub hostname: String,
    pub distro: String,
    pub kernel_version: String,
    pub arch: String,
    pub cpu_count: usize,
    pub total_memory: u64,
    pub used_memory: u64,
    pub uptime_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemInfo {
    pub total: u64,
    pub used: u64,
    pub available: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskInfo {
    pub mount: String,
    pub total: u64,
    pub used: u64,
    pub fs_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NetStats {
    pub rx_bytes: u64,
    pub tx_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub description: String,
    pub installed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessEntry {
    pub pid: u32,
    pub name: String,
    pub cpu_pct: f32,
    pub mem_bytes: u64,
    pub user: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

impl Severity {
    pub fn label(&self) -> &'static str {
        match self {
            Severity::Critical => "CRITICAL",
            Severity::High => "HIGH",
            Severity::Medium => "MEDIUM",
            Severity::Low => "LOW",
            Severity::Info => "INFO",
        }
    }

    pub fn color(&self) -> ratatui::style::Color {
        use ratatui::style::Color;
        match self {
            Severity::Critical => Color::Red,
            Severity::High => Color::LightRed,
            Severity::Medium => Color::Yellow,
            Severity::Low => Color::Blue,
            Severity::Info => Color::DarkGray,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityFinding {
    pub id: String,
    pub title: String,
    pub severity: Severity,
    pub description: String,
    pub file_path: Option<String>,
    pub fix_description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route {
    pub domain: String,
    pub port: u16,
    pub tls: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tunnel {
    pub name: String,
    pub id: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirewallRule {
    /// Rule number as reported by the backend (used for deletion).
    pub num: usize,
    /// Destination / port spec, e.g. "22/tcp", "80/tcp (v6)", "Anywhere".
    pub to: String,
    /// Action string, e.g. "ALLOW IN", "DENY OUT".
    pub action: String,
    /// Source, e.g. "Anywhere", "192.168.1.0/24".
    pub from: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelRoute {
    pub tunnel_id: String,    // UUID used in config + credentials-file path
    pub tunnel_name: String,  // human name used for display
    pub hostname: String,
    pub service: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerContainer {
    pub id: String,       // short container ID
    pub name: String,
    pub image: String,
    pub status: String,   // "running", "exited", "paused", etc.
    pub ports: String,    // human-readable port bindings
    pub created: String,
    pub cpu_pct: f64,
    pub mem_usage: String, // e.g. "45.2MiB / 1GiB"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerImage {
    pub id: String,        // short image ID
    pub repository: String,
    pub tag: String,
    pub size: String,      // human-readable, e.g. "142MB"
    pub created: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerComposeService {
    pub name: String,
    pub status: String,
    pub image: String,
    pub ports: String,
}

/// A currently-banned IP as reported by fail2ban.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JailedIp {
    /// The IP address that was banned.
    pub ip: String,
    /// The fail2ban jail name (e.g. "sshd", "nginx-http-auth").
    pub jail: String,
    /// Total failures recorded in the jail at the time of the query.
    pub total_failures: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshKey {
    pub name: String,         // filename or comment
    pub fingerprint: String,
    pub key_type: String,     // e.g. ssh-rsa
    pub content: String,      // the public key string
    pub is_local: bool,       // true if in ~/.ssh, false if in authorized_keys
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmCloudHost {
    pub id: String,
    pub friendly_name: String,
    pub uptime_secs: u64,
    pub labels: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmCloudComponent {
    pub id: String,
    pub name: String,
    pub image_ref: String,
    pub component_type: String, // "actor" or "provider"
    pub host_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmCloudLink {
    pub source_id: String,
    pub target_id: String,
    pub name: String,
    pub wit_namespace: String,
    pub wit_package: String,
    pub wit_interface: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmCloudApp {
    pub name: String,
    pub version: String,
    pub status: String,
    pub description: String,
}

// ── Ghost Services Hunter ──────────────────────────────────────────────────

/// Why a process was classified as a ghost.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum GhostReason {
    /// Process was reparented to PID 1 (parent died) and is not a systemd service.
    Orphan,
    /// Not tracked by any systemd service and exceeds the memory-leak threshold.
    MemLeak,
    /// Zombie (defunct) process — should have been reaped.
    Zombie,
}

impl GhostReason {
    pub fn label(&self) -> &'static str {
        match self {
            GhostReason::Orphan  => "ORPHAN",
            GhostReason::MemLeak => "MEM-LEAK",
            GhostReason::Zombie  => "ZOMBIE",
        }
    }

    pub fn color(&self) -> ratatui::style::Color {
        use ratatui::style::Color;
        match self {
            GhostReason::Zombie  => Color::Red,
            GhostReason::Orphan  => Color::Yellow,
            GhostReason::MemLeak => Color::LightRed,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhostProcess {
    pub pid: u32,
    pub ppid: u32,
    pub name: String,
    /// Space-joined argv (may be empty if not readable).
    pub cmdline: String,
    pub user: String,
    pub cpu_pct: f32,
    pub mem_bytes: u64,
    /// Raw cgroup string from /proc/<pid>/cgroup (Linux only).
    pub cgroup: String,
    pub reason: GhostReason,
}
