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
    pub tunnel_id: String,   // UUID used in config + credentials-file path
    pub tunnel_name: String, // human name used for display
    pub hostname: String,
    pub service: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerContainer {
    pub id: String, // short container ID
    pub name: String,
    pub image: String,
    pub status: String, // "running", "exited", "paused", etc.
    pub ports: String,  // human-readable port bindings
    pub created: String,
    pub cpu_pct: f64,
    pub mem_usage: String, // e.g. "45.2MiB / 1GiB"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerImage {
    pub id: String, // short image ID
    pub repository: String,
    pub tag: String,
    pub size: String, // human-readable, e.g. "142MB"
    pub created: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerComposeService {
    pub name: String,
    pub status: String,
    pub image: String,
    pub ports: String,
}

/// A predefined, managed Docker service for local development (e.g. Redis, Postgres).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedDockerService {
    /// Display name (e.g. "Redis")
    pub name: String,
    /// Container name used in `docker run --name`
    pub container_name: String,
    /// Docker image (e.g. "redis:7-alpine")
    pub image: String,
    /// Human-readable port mapping (e.g. "6379:6379")
    pub ports: String,
    /// Current running status — "running", "stopped", "not found", etc.
    pub status: String,
    /// Optional description of what this service is for
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ManagedWorkloadBackend {
    PodmanQuadlet,
    DockerComposeSystemd,
}

impl ManagedWorkloadBackend {
    pub fn label(&self) -> &'static str {
        match self {
            ManagedWorkloadBackend::PodmanQuadlet => "Podman Quadlet",
            ManagedWorkloadBackend::DockerComposeSystemd => "Docker Compose + systemd",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ManagedWorkloadState {
    Running,
    Stopped,
    Failed,
    NotInstalled,
    Unknown,
}

impl ManagedWorkloadState {
    pub fn label(&self) -> &'static str {
        match self {
            ManagedWorkloadState::Running => "Running",
            ManagedWorkloadState::Stopped => "Stopped",
            ManagedWorkloadState::Failed => "Failed",
            ManagedWorkloadState::NotInstalled => "Not installed",
            ManagedWorkloadState::Unknown => "Unknown",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ManagedWorkloadSpec {
    pub name: String,
    pub image: String,
    pub command: Option<Vec<String>>,
    pub env: Vec<(String, String)>,
    pub ports: Vec<String>,
    pub volumes: Vec<String>,
    pub restart_policy: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedWorkload {
    pub name: String,
    pub backend: ManagedWorkloadBackend,
    pub unit_name: String,
    pub engine: String,
    pub image: String,
    pub ports_summary: String,
    pub status: ManagedWorkloadState,
    pub owned_by_postlab: bool,
    pub spec_path: String,
    pub compose_path: Option<String>,
    pub spec: Option<ManagedWorkloadSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedWorkloadCapabilities {
    pub supported: bool,
    pub engine: Option<String>,
    pub backend: Option<ManagedWorkloadBackend>,
    pub reason: Option<String>,
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
    pub name: String, // filename or comment
    pub fingerprint: String,
    pub key_type: String, // e.g. ssh-rsa
    pub content: String,  // the public key string
    pub is_local: bool,   // true if in ~/.ssh, false if in authorized_keys
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

#[allow(dead_code)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageDevice {
    pub device: String,
    pub mount: String,
    pub fs_type: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub avail_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartInfo {
    pub device: String,
    pub model: String,
    pub healthy: bool,
    pub temp_celsius: u32,
    pub power_on_hours: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapEntry {
    pub path: String,
    pub kind: String,
    pub size_bytes: u64,
    pub used_bytes: u64,
    pub priority: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SwapStatus {
    pub total: u64,
    pub used: u64,
    pub free: u64,
    pub entries: Vec<SwapEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub username: String,
    pub uid: u32,
    pub gid: u32,
    pub home: String,
    pub shell: String,
    pub groups: Vec<String>,
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
            GhostReason::Orphan => "ORPHAN",
            GhostReason::MemLeak => "MEM-LEAK",
            GhostReason::Zombie => "ZOMBIE",
        }
    }

    pub fn color(&self) -> ratatui::style::Color {
        use ratatui::style::Color;
        match self {
            GhostReason::Zombie => Color::Red,
            GhostReason::Orphan => Color::Yellow,
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

// ── Git Deployments ────────────────────────────────────────────────────────

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DeploymentType {
    DockerCompose,
    WasmCloud,
    Unknown,
}

impl DeploymentType {
    #[allow(dead_code)]
    pub fn label(&self) -> &'static str {
        match self {
            DeploymentType::DockerCompose => "Docker Compose",
            DeploymentType::WasmCloud => "wasmCloud",
            DeploymentType::Unknown => "Unknown",
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DeploymentStatus {
    Cloning,
    Deploying,
    Running,
    Stopped,
    Failed(String),
}

impl DeploymentStatus {
    #[allow(dead_code)]
    pub fn label(&self) -> &'static str {
        match self {
            DeploymentStatus::Cloning => "Cloning",
            DeploymentStatus::Deploying => "Deploying",
            DeploymentStatus::Running => "Running",
            DeploymentStatus::Stopped => "Stopped",
            DeploymentStatus::Failed(_) => "Failed",
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Deployment {
    pub id: String, // UUID or short hash
    pub repo_url: String,
    pub path: String, // Local clone path
    pub deploy_type: DeploymentType,
    pub status: DeploymentStatus,
    pub last_updated: String,
}

#[cfg(test)]
mod tests {
    use super::{DeploymentStatus, DeploymentType, GhostReason, ManagedWorkloadState, Severity};

    #[test]
    fn severity_labels_are_uppercase() {
        assert_eq!(Severity::Critical.label(), "CRITICAL");
        assert_eq!(Severity::High.label(), "HIGH");
        assert_eq!(Severity::Medium.label(), "MEDIUM");
        assert_eq!(Severity::Low.label(), "LOW");
        assert_eq!(Severity::Info.label(), "INFO");
    }

    #[test]
    fn severity_ordering_critical_is_highest() {
        assert!(Severity::Critical < Severity::High);
        assert!(Severity::High < Severity::Medium);
        assert!(Severity::Medium < Severity::Low);
        assert!(Severity::Low < Severity::Info);
    }

    #[test]
    fn ghost_reason_labels() {
        assert_eq!(GhostReason::Orphan.label(), "ORPHAN");
        assert_eq!(GhostReason::MemLeak.label(), "MEM-LEAK");
        assert_eq!(GhostReason::Zombie.label(), "ZOMBIE");
    }

    #[test]
    fn workload_state_labels() {
        assert_eq!(ManagedWorkloadState::Running.label(), "Running");
        assert_eq!(ManagedWorkloadState::Stopped.label(), "Stopped");
        assert_eq!(ManagedWorkloadState::Failed.label(), "Failed");
        assert_eq!(ManagedWorkloadState::NotInstalled.label(), "Not installed");
        assert_eq!(ManagedWorkloadState::Unknown.label(), "Unknown");
    }

    #[test]
    fn deployment_type_labels() {
        assert_eq!(DeploymentType::DockerCompose.label(), "Docker Compose");
        assert_eq!(DeploymentType::WasmCloud.label(), "wasmCloud");
        assert_eq!(DeploymentType::Unknown.label(), "Unknown");
    }

    #[test]
    fn deployment_status_labels() {
        assert_eq!(DeploymentStatus::Cloning.label(), "Cloning");
        assert_eq!(DeploymentStatus::Deploying.label(), "Deploying");
        assert_eq!(DeploymentStatus::Running.label(), "Running");
        assert_eq!(DeploymentStatus::Stopped.label(), "Stopped");
        assert_eq!(DeploymentStatus::Failed("oops".to_string()).label(), "Failed");
    }
}
