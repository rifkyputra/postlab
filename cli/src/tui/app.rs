use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use std::time::Instant;

use ratatui::widgets::{ListState, TableState};
use sqlx::SqlitePool;
use tokio::sync::mpsc;

use crate::core::{
    models::{
        DiskInfo, DockerComposeService, DockerContainer, DockerImage, FirewallRule, GhostProcess,
        JailedIp, ManagedDockerService, ManagedWorkload, ManagedWorkloadCapabilities,
        ManagedWorkloadSpec, MemInfo, OsInfo, Package, ProcessEntry, Route, SecurityFinding,
        SshKey, SwapStatus, Tunnel, UserInfo, WasmCloudApp, WasmCloudComponent, WasmCloudHost,
    },
    portcheck::{default_entries, PortEntry, PortStatus},
    services::ServiceUnit,
    Platform,
};

// ── Screens ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    Dashboard,
    Packages,
    Security,
    Networking,
    Docker,
    WasmCloud,
    Agent,
    System,
    Projects,
}

impl Screen {
    pub fn all() -> &'static [Screen] {
        &[
            Screen::Dashboard,
            Screen::Packages,
            Screen::Security,
            Screen::Networking,
            Screen::Docker,
            Screen::WasmCloud,
            Screen::Agent,
            Screen::System,
            Screen::Projects,
        ]
    }

    pub fn title(&self) -> &'static str {
        match self {
            Screen::Dashboard => "1. Dashboard",
            Screen::Packages => "2. Packages",
            Screen::Security => "3. Security",
            Screen::Networking => "4. Networking",
            Screen::Docker => "5. Docker",
            Screen::WasmCloud => "6. wasmCloud",
            Screen::Agent => "7. Agent",
            Screen::System => "8. System",
            Screen::Projects => "9. Projects",
        }
    }

    pub fn index(&self) -> usize {
        Screen::all().iter().position(|s| s == self).unwrap_or(0)
    }
}

// ── System tabs ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Default)]
pub enum SystemTab {
    #[default]
    Ghosts,
    Janitor,
    Services,
    Users,
    Swap,
    Storage,
}

impl SystemTab {
    pub fn all() -> &'static [SystemTab] {
        &[
            SystemTab::Ghosts,
            SystemTab::Janitor,
            SystemTab::Services,
            SystemTab::Users,
            SystemTab::Swap,
            SystemTab::Storage,
        ]
    }

    pub fn title(&self) -> &'static str {
        match self {
            SystemTab::Ghosts => "Ghosts",
            SystemTab::Janitor => "Janitor",
            SystemTab::Services => "Services",
            SystemTab::Users => "Users",
            SystemTab::Swap => "Swap",
            SystemTab::Storage => "Storage",
        }
    }

    pub fn index(&self) -> usize {
        Self::all().iter().position(|t| t == self).unwrap_or(0)
    }
}

// ── Networking tabs ───────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Default)]
pub enum NetworkingTab {
    #[default]
    Gateway,
    Tunnel,
    Tailscale,
}

impl NetworkingTab {
    pub fn all() -> &'static [NetworkingTab] {
        &[NetworkingTab::Gateway, NetworkingTab::Tunnel, NetworkingTab::Tailscale]
    }

    pub fn title(&self) -> &'static str {
        match self {
            NetworkingTab::Gateway => "Gateway",
            NetworkingTab::Tunnel => "Tunnel",
            NetworkingTab::Tailscale => "Tailscale",
        }
    }

    pub fn index(&self) -> usize {
        Self::all().iter().position(|t| t == self).unwrap_or(0)
    }
}

// ── Docker tabs ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum DockerTab {
    Containers,
    Images,
    Compose,
    Workloads,
    Managed,
}

impl DockerTab {
    pub fn all() -> &'static [DockerTab] {
        &[
            DockerTab::Containers,
            DockerTab::Images,
            DockerTab::Compose,
            DockerTab::Workloads,
            DockerTab::Managed,
        ]
    }
    pub fn title(&self) -> &'static str {
        match self {
            DockerTab::Containers => "Containers",
            DockerTab::Images => "Images",
            DockerTab::Compose => "Compose",
            DockerTab::Workloads => "Workloads",
            DockerTab::Managed => "Managed",
        }
    }
    pub fn index(&self) -> usize {
        DockerTab::all().iter().position(|t| t == self).unwrap_or(0)
    }
}

// ── Dashboard tabs ────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum DashboardTab {
    Overview,
    Processes,
    Resources,
}

impl DashboardTab {
    pub fn all() -> &'static [DashboardTab] {
        &[
            DashboardTab::Overview,
            DashboardTab::Processes,
            DashboardTab::Resources,
        ]
    }

    pub fn title(&self) -> &'static str {
        match self {
            DashboardTab::Overview => "Overview",
            DashboardTab::Processes => "Processes",
            DashboardTab::Resources => "Resources",
        }
    }

    pub fn index(&self) -> usize {
        DashboardTab::all()
            .iter()
            .position(|t| t == self)
            .unwrap_or(0)
    }
}

// ── Agent tabs ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Default)]
pub enum AgentTab {
    #[default]
    Chat,
    Tools,
    Tasks,
    Status,
    Sessions,
    Config,
    Auth,
    Skills,
    Library,
    Logs,
}

impl AgentTab {
    pub fn all() -> &'static [AgentTab] {
        &[
            AgentTab::Chat,
            AgentTab::Tools,
            AgentTab::Tasks,
            AgentTab::Status,
            AgentTab::Sessions,
            AgentTab::Config,
            AgentTab::Auth,
            AgentTab::Skills,
            AgentTab::Library,
            AgentTab::Logs,
        ]
    }
    pub fn title(&self) -> &'static str {
        match self {
            AgentTab::Chat => "Chat",
            AgentTab::Tools => "Tools",
            AgentTab::Tasks => "Tasks",
            AgentTab::Status => "Status",
            AgentTab::Sessions => "Sessions",
            AgentTab::Config => "Config",
            AgentTab::Auth => "Auth",
            AgentTab::Skills => "Skills",
            AgentTab::Library => "Library",
            AgentTab::Logs => "Logs",
        }
    }
    pub fn index(&self) -> usize {
        AgentTab::all().iter().position(|t| t == self).unwrap_or(0)
    }
}

pub const BTS_FRONTENDS: &[&str] = &[
    "tanstack-router", "react-router", "tanstack-start", "next", "nuxt", "svelte", "solid",
    "astro", "native-bare", "native-uniwind", "native-unistyles", "none",
];
pub const BTS_DATABASES: &[&str] = &["sqlite", "postgres", "mysql", "mongodb", "none"];
pub const BTS_ORMS: &[&str] = &["drizzle", "prisma", "mongoose", "none"];
pub const BTS_AUTHS: &[&str] = &["better-auth", "clerk", "none"];
pub const BTS_BACKENDS: &[&str] = &["hono", "express", "fastify", "elysia", "convex", "self", "none"];
pub const BTS_APIS: &[&str] = &["trpc", "orpc", "none"];
pub const BTS_RUNTIMES: &[&str] = &["bun", "node", "workers", "none"];
pub const BTS_PAYMENTS: &[&str] = &["none", "polar"];
pub const BTS_EXAMPLES: &[&str] = &["none", "todo", "ai"];
pub const BTS_GIT: &[&str] = &["yes", "no"];
pub const BTS_WEB_DEPLOY: &[&str] = &["none", "cloudflare", "docker"];
pub const BTS_SERVER_DEPLOY: &[&str] = &["none", "cloudflare", "docker"];
pub const BTS_ADDONS: &[&str] = &[
    "pwa", "tauri", "electrobun", "starlight", "biome", "lefthook", "husky", "mcp",
    "turborepo", "nx", "vite-plus", "fumadocs", "ultracite", "oxlint", "opentui", "wxt",
    "skills", "evlog",
];

// ── Projects tabs ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Default)]
pub enum ProjectsTab {
    #[default]
    Projects,
    New,
    Clone,
    Settings,
}

impl ProjectsTab {
    pub fn all() -> &'static [ProjectsTab] {
        &[
            ProjectsTab::Projects,
            ProjectsTab::New,
            ProjectsTab::Clone,
            ProjectsTab::Settings,
        ]
    }

    pub fn title(&self) -> &'static str {
        match self {
            ProjectsTab::Projects => "Projects",
            ProjectsTab::New => "New",
            ProjectsTab::Clone => "Clone",
            ProjectsTab::Settings => "Settings",
        }
    }

    pub fn index(&self) -> usize {
        Self::all().iter().position(|t| t == self).unwrap_or(0)
    }
}

// ── Agent message types ───────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum AgentRole {
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone)]
pub struct AgentMessage {
    pub role: AgentRole,
    pub content: String,
}

// ── Background task results ───────────────────────────────────────────────

#[derive(Debug)]
#[allow(dead_code)]
pub enum TaskResult {
    PackageList(Vec<Package>),
    PackagesUpdated(Vec<Package>), // merge/add specific entries without full reload
    SearchResults(Vec<Package>),
    OpProgress {
        op: String,
        target: String,
        line: String,
    },
    OpDone {
        op: String,
        target: String,
        output: String,
        success: bool,
    },
    ProcessList(Vec<ProcessEntry>),
    SecurityScan(Vec<SecurityFinding>),
    SecurityApply {
        id: String,
        output: String,
        success: bool,
    },
    Fail2BanList(Vec<JailedIp>),
    Fail2BanActionDone {
        ip: String,
        jail: String,
        action: String,
        success: bool,
    },
    RouteList(Vec<Route>),
    TunnelList(Vec<Tunnel>),
    TunnelCreated(Tunnel),
    GatewayStatus {
        installed: bool,
        version: Option<String>,
    },
    TailscaleStatus {
        installed: bool,
        version: Option<String>,
        backend_state: String,
        self_ip: Option<String>,
        self_name: Option<String>,
        peers: Vec<crate::core::tailscale::TailscalePeer>,
    },
    TunnelStatus {
        installed: bool,
        version: Option<String>,
    },
    TunnelConfigContent(String),
    TunnelServiceStatus {
        active: bool,
        enabled: bool,
    },
    InstallProgress {
        target: String,
        line: String,
    },
    InstallDone {
        target: String,
        success: bool,
    },
    DockerStatus {
        installed: bool,
        version: Option<String>,
    },
    DockerContainerList(Vec<DockerContainer>),
    DockerImageList(Vec<DockerImage>),
    DockerComposeList(Vec<DockerComposeService>),
    ManagedServiceList(Vec<ManagedDockerService>),
    WorkloadCapabilities(ManagedWorkloadCapabilities),
    WorkloadList(Vec<ManagedWorkload>),
    FirewallStatus {
        enabled: bool,
        backend: String,
    },
    FirewallRules(Vec<FirewallRule>),
    PublicIp(String),
    PortCheckDone {
        results: Vec<(u16, PortStatus)>,
    },
    WasmCloudStatus {
        installed: bool,
        version: Option<String>,
    },
    WasmCloudHostList(Vec<WasmCloudHost>),
    WasmCloudComponentList(Vec<WasmCloudComponent>),
    WasmCloudAppList(Vec<WasmCloudApp>),
    WasmCloudNatsStatus {
        running: bool,
        storage_usage: Option<u64>,
        synced: bool,
    },
    SshLocalKeys(Vec<SshKey>),
    SshAuthorizedKeys(Vec<SshKey>),
    SshOpDone {
        op: String,
        success: bool,
        output: String,
    },
    WasmCloudInspect(String),
    GhostScan(Vec<GhostProcess>),
    UserList(Vec<UserInfo>),
    ServiceList(Vec<ServiceUnit>),
    ServiceOpDone {
        name: String,
        op: String,
        success: bool,
    },
    MaintenanceDone {
        op: String,
        output: String,
        success: bool,
    },
    PiAgentInstallProgress(String),
    PiAgentInstallDone {
        output: String,
        success: bool,
    },
    PiAgentInfo(crate::core::pi_agent::PiAgentInfo),
    PiAgentSessions(Vec<crate::core::pi_agent::PiSession>),
    PiAgentConfig(String),
    PiAgentAuth(Vec<crate::core::pi_agent::PiAuthEntry>),
    PiAgentSkills(Vec<crate::core::pi_agent::PiSkill>),
    PiAgentSkillRemoved { name: String, success: bool },
    PiAgentLibrarySkills(Vec<crate::core::pi_agent::LibrarySkill>),
    PiAgentLibraryInstall { name: String, success: bool },
    PiAgentLogs(Vec<String>),
    PiAgentActionDone {
        action: String,
        output: String,
        success: bool,
    },
    PiAgentTasks(Vec<crate::db::agent_tasks::AgentTask>),
    PiAgentTaskCreated,
    PiAgentTaskDeleted,
    PiAgentTaskToggled,
    PiAgentRpcConnected(crate::core::pi_agent::rpc::RpcHandle),
    PiAgentRpcStarted,
    PiAgentRpcStopped,
    PiAgentTextDelta(String),
    PiAgentAgentEnd,
    PiAgentToolStart(String),
    PiAgentToolEnd { name: String, is_error: bool },
    PiAgentRpcStderr(String),
    PiAgentRpcError(String),
    StorageLoaded(Vec<crate::core::models::StorageDevice>),
    SmartLoaded(Vec<crate::core::models::SmartInfo>),
    StorageOpDone { op: String, success: bool },
    StorageFstabLoaded(String),
    UpdatesList(Vec<crate::core::models::UpgradablePackage>),
    UpdatesOpDone { success: bool },
    SwapLoaded(SwapStatus),
    SwapOpDone { op: String, success: bool },
    ProjectsList(Vec<crate::core::projects::ProjectEntry>),
    ProjectsOpProgress { op: String, line: String },
    ProjectsOpDone { op: String, name: String, success: bool },
    ProjectsDirLoaded(String),
    ProjectsGitLoaded(crate::core::projects::GitStatus),
    ProjectsGitSaved { what: String, success: bool },
    Status(String),
    Error(String),
}

// ── Confirm dialog ────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum ConfirmAction {
    KillProcess { pid: u32, name: String },
    RemovePackage { name: String },
    ApplySecurityFix { id: String, title: String },
    DeleteRoute { domain: String },
    DeleteTunnel { name: String },
    DeleteIngress { tunnel_id: String, hostname: String },
    StopContainer { id: String, name: String },
    RemoveContainer { id: String, name: String },
    RemoveImage { id: String, tag: String },
    DeleteWorkload { name: String },
    DeleteFirewallRule { num: usize },
    Fail2BanForgive { ip: String, jail: String },
    Fail2BanBanish { ip: String, jail: String },
    DeauthorizeKey { fingerprint: String, name: String },
    AuthorizeLocalKey { content: String, name: String },
    KillGhost { pid: u32, name: String },
    ServiceAction { name: String, op: String },
    MaintenanceAction { op: String },
    DeleteSwap { path: String },
    Umount { target: String },
}

#[derive(Debug)]
pub struct ConfirmDialog {
    pub message: String,
    pub action: ConfirmAction,
}

pub struct WorkloadFormState {
    pub input_mode: InputMode,
    pub editing_name: Option<String>,
    pub input_focus: usize,
    pub name: String,
    pub image: String,
    pub command: String,
    pub env: String,
    pub ports: String,
    pub volumes: String,
    pub restart_policy: String,
}

impl Default for WorkloadFormState {
    fn default() -> Self {
        Self {
            input_mode: InputMode::Normal,
            editing_name: None,
            input_focus: 0,
            name: String::new(),
            image: String::new(),
            command: String::new(),
            env: String::new(),
            ports: String::new(),
            volumes: String::new(),
            restart_policy: "unless-stopped".to_string(),
        }
    }
}

impl WorkloadFormState {
    pub fn reset_for_create(&mut self) {
        *self = Self::default();
        self.input_mode = InputMode::Editing;
    }

    pub fn reset_for_edit(&mut self, spec: &ManagedWorkloadSpec) {
        self.input_mode = InputMode::Editing;
        self.editing_name = Some(spec.name.clone());
        self.input_focus = 0;
        self.name = spec.name.clone();
        self.image = spec.image.clone();
        self.command = spec.command.clone().unwrap_or_default().join(", ");
        self.env = spec
            .env
            .iter()
            .map(|(key, value)| format!("{}={}", key, value))
            .collect::<Vec<_>>()
            .join(", ");
        self.ports = spec.ports.join(", ");
        self.volumes = spec.volumes.join(", ");
        self.restart_policy = spec.restart_policy.clone();
    }
}

#[derive(Default)]
pub struct DockerWorkloadsState {
    pub capabilities: Option<ManagedWorkloadCapabilities>,
    pub workloads: Vec<ManagedWorkload>,
    pub table_state: TableState,
    pub form: WorkloadFormState,
}

// ── Input mode ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum InputMode {
    Normal,
    Editing,
    SettingPassword,
    AddingDomain,
    EditingIngress, // editing an existing ingress entry (hostname + service)
}

// ── Tunnel panel focus ────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum TunnelPanel {
    Tunnels,
    Ingress,
}

// ── Package tab ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum PackageTab {
    Installed,
    Search,
    QuickInstall,
    Queue,
    Updates,
}

impl PackageTab {
    pub fn all() -> &'static [PackageTab] {
        &[
            PackageTab::Installed,
            PackageTab::Search,
            PackageTab::QuickInstall,
            PackageTab::Queue,
            PackageTab::Updates,
        ]
    }
    pub fn title(&self) -> &'static str {
        match self {
            PackageTab::Installed => "Installed",
            PackageTab::Search => "Search",
            PackageTab::QuickInstall => "Quick Install",
            PackageTab::Queue => "Queue",
            PackageTab::Updates => "Updates",
        }
    }
    pub fn index(&self) -> usize {
        PackageTab::all()
            .iter()
            .position(|t| t == self)
            .unwrap_or(0)
    }
}

// ── Security tab ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum SecurityTab {
    Findings,
    Firewall,
    Ports,
    Ssh,
    Fail2Ban,
}

impl SecurityTab {
    pub fn all() -> &'static [SecurityTab] {
        &[
            SecurityTab::Findings,
            SecurityTab::Firewall,
            SecurityTab::Ports,
            SecurityTab::Ssh,
            SecurityTab::Fail2Ban,
        ]
    }
    pub fn title(&self) -> &'static str {
        match self {
            SecurityTab::Findings => "Findings",
            SecurityTab::Firewall => "Firewall",
            SecurityTab::Ports => "Ports",
            SecurityTab::Ssh => "SSH",
            SecurityTab::Fail2Ban => "Fail2Ban",
        }
    }
    pub fn index(&self) -> usize {
        SecurityTab::all()
            .iter()
            .position(|t| t == self)
            .unwrap_or(0)
    }
}

// ── wasmCloud tabs ────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum WasmCloudTab {
    Hosts,
    Components,
    Apps,
    Inspector,
}

impl WasmCloudTab {
    pub fn all() -> &'static [WasmCloudTab] {
        &[
            WasmCloudTab::Hosts,
            WasmCloudTab::Components,
            WasmCloudTab::Apps,
            WasmCloudTab::Inspector,
        ]
    }
    pub fn title(&self) -> &'static str {
        match self {
            WasmCloudTab::Hosts => "Hosts",
            WasmCloudTab::Components => "Components",
            WasmCloudTab::Apps => "Apps",
            WasmCloudTab::Inspector => "Inspector",
        }
    }
    pub fn index(&self) -> usize {
        WasmCloudTab::all()
            .iter()
            .position(|t| t == self)
            .unwrap_or(0)
    }
}

#[derive(Debug)]
pub struct QueuedOp {
    pub kind: String,
    pub target: String,
    pub status: OpStatus,
    pub output: String,
}

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum OpStatus {
    Pending,
    Running,
    Done,
    Failed,
}

impl OpStatus {
    pub fn label(&self) -> &'static str {
        match self {
            OpStatus::Pending => "pending",
            OpStatus::Running => "running",
            OpStatus::Done => "done",
            OpStatus::Failed => "failed",
        }
    }
}

// ── Per-screen state ──────────────────────────────────────────────────────

pub struct DashboardState {
    pub active_tab: DashboardTab,
    pub os_info: Option<OsInfo>,
    pub cpu_pct: Vec<f32>,
    pub mem: Option<MemInfo>,
    pub disks: Vec<DiskInfo>,
}

impl Default for DashboardState {
    fn default() -> Self {
        Self {
            active_tab: DashboardTab::Overview,
            os_info: None,
            cpu_pct: Vec::new(),
            mem: None,
            disks: Vec::new(),
        }
    }
}

pub struct PackagesState {
    pub active_tab: PackageTab,
    // Installed tab
    pub installed: Vec<Package>,
    pub installed_state: ListState,
    pub filter: String,
    pub filter_mode: InputMode,
    pub selected: HashSet<String>,
    // Search tab
    pub search_query: String,
    pub search_mode: InputMode,
    pub search_results: Vec<Package>,
    pub search_state: ListState,
    pub search_selected: HashSet<String>,
    // Quick install tab
    pub curated_selected: HashSet<String>, // packages to install
    pub curated_uninstall: HashSet<String>, // installed packages marked for removal
    pub curated_cursor: usize,
    // Queue
    pub queue: VecDeque<QueuedOp>,
    #[allow(dead_code)]
    pub queue_state: ListState,
    pub queue_selected: Option<usize>,
    pub output_scroll: usize,
    // Updates tab
    pub updates: Vec<crate::core::models::UpgradablePackage>,
    pub updates_state: TableState,
    pub updates_selected: HashSet<String>,
    pub updates_loading: bool,
}

impl Default for PackagesState {
    fn default() -> Self {
        Self {
            active_tab: PackageTab::Installed,
            installed: Vec::new(),
            installed_state: ListState::default(),
            filter: String::new(),
            filter_mode: InputMode::Normal,
            selected: HashSet::new(),
            search_query: String::new(),
            search_mode: InputMode::Normal,
            search_results: Vec::new(),
            search_state: ListState::default(),
            search_selected: HashSet::new(),
            curated_selected: HashSet::new(),
            curated_uninstall: HashSet::new(),
            curated_cursor: 0,
            queue: VecDeque::new(),
            queue_state: ListState::default(),
            queue_selected: None,
            output_scroll: 0,
            updates: Vec::new(),
            updates_state: TableState::default(),
            updates_selected: HashSet::new(),
            updates_loading: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ProcessSort {
    Cpu,
    Memory,
    Pid,
}

pub struct ProcessesState {
    pub list: Vec<ProcessEntry>,
    pub table_state: TableState,
    pub sort: ProcessSort,
}

impl Default for ProcessesState {
    fn default() -> Self {
        Self {
            list: Vec::new(),
            table_state: TableState::default(),
            sort: ProcessSort::Cpu,
        }
    }
}

pub struct SecurityState {
    pub active_tab: SecurityTab,
    // Findings tab
    pub findings: Vec<SecurityFinding>,
    pub list_state: ListState,
    pub selected: HashSet<String>,
    pub last_scan: Option<std::time::SystemTime>,
    pub scanning: bool,
    pub output: Option<String>,
    // Fail2Ban tab
    pub jailed: Vec<JailedIp>,
    pub jailed_state: ListState,
    pub f2b_loading: bool,
    pub f2b_installed: bool,
}

impl Default for SecurityState {
    fn default() -> Self {
        Self {
            active_tab: SecurityTab::Findings,
            findings: Vec::new(),
            list_state: ListState::default(),
            selected: HashSet::new(),
            last_scan: None,
            scanning: false,
            output: None,
            jailed: Vec::new(),
            jailed_state: ListState::default(),
            f2b_loading: false,
            f2b_installed: false,
        }
    }
}

#[derive(Default)]
pub struct ResourcesState {
    pub cpu_history: Vec<Vec<u64>>,
    pub mem_history: Vec<u64>,
    pub net_rx_history: Vec<u64>,
    pub net_tx_history: Vec<u64>,
    pub last_net_rx: u64,
    pub last_net_tx: u64,
}

pub struct GatewayState {
    pub installed: bool,
    pub version: Option<String>,
    pub routes: Vec<Route>,
    pub table_state: TableState,
    pub input_mode: InputMode,
    pub input_domain: String,
    pub input_port: String,
    pub input_focus: usize, // 0 = domain, 1 = port
}

impl Default for GatewayState {
    fn default() -> Self {
        Self {
            installed: false,
            version: None,
            routes: Vec::new(),
            table_state: TableState::default(),
            input_mode: InputMode::Normal,
            input_domain: String::new(),
            input_port: String::new(),
            input_focus: 0,
        }
    }
}

pub struct TunnelState {
    pub installed: bool,
    pub version: Option<String>,
    pub tunnels: Vec<Tunnel>,
    pub table_state: TableState,
    pub input_mode: InputMode,
    pub input_name: String,
    pub input_host: String,
    pub input_service: String,
    pub input_focus: usize,
    /// When EditingIngress, the hostname being edited (for removal of old entry).
    pub input_original_host: String,
    // Config + service
    pub config_content: Option<String>, // ~/.cloudflared/config.yaml
    pub service_active: Option<bool>,   // true=active, false=inactive, None=unknown
    pub service_enabled: Option<bool>,
    /// The tunnel UUID that config.yaml is pointing to (set with Enter).
    pub active_tunnel_id: Option<String>,
    /// Parsed ingress entries from config.yaml: Vec<(hostname, service)>
    pub ingress_entries: Vec<(String, String)>,
    pub ingress_state: ListState,
    /// Which panel has keyboard focus on the Tunnel screen.
    pub panel_focus: TunnelPanel,
}

impl Default for TunnelState {
    fn default() -> Self {
        Self {
            installed: false,
            version: None,
            tunnels: Vec::new(),
            table_state: TableState::default(),
            input_mode: InputMode::Normal,
            input_name: String::new(),
            input_host: String::new(),
            input_service: String::new(),
            input_focus: 0,
            input_original_host: String::new(),
            config_content: None,
            service_active: None,
            service_enabled: None,
            active_tunnel_id: None,
            ingress_entries: Vec::new(),
            ingress_state: ListState::default(),
            panel_focus: TunnelPanel::Tunnels,
        }
    }
}

// ── Tailscale state ───────────────────────────────────────────────────────

#[derive(Default)]
pub struct TailscaleState {
    pub installed: bool,
    pub version: Option<String>,
    pub backend_state: String,
    pub self_ip: Option<String>,
    pub self_name: Option<String>,
    pub peers: Vec<crate::core::tailscale::TailscalePeer>,
    pub peers_state: ListState,
    pub loading: bool,
}

// ── Docker state ─────────────────────────────────────────────────────────

pub struct DockerState {
    pub installed: bool,
    pub version: Option<String>,
    pub active_tab: DockerTab,
    // Containers tab
    pub containers: Vec<DockerContainer>,
    pub containers_state: TableState,
    // Images tab
    pub images: Vec<DockerImage>,
    pub images_state: TableState,
    // Compose tab
    pub compose_services: Vec<DockerComposeService>,
    pub compose_state: TableState,
    pub compose_path: String,
    // Workloads tab
    pub workloads: DockerWorkloadsState,
    // Managed dev services tab
    pub managed_services: Vec<ManagedDockerService>,
    pub managed_state: TableState,
    pub loading: bool,
}

impl Default for DockerState {
    fn default() -> Self {
        Self {
            installed: false,
            version: None,
            active_tab: DockerTab::Containers,
            containers: Vec::new(),
            containers_state: TableState::default(),
            images: Vec::new(),
            images_state: TableState::default(),
            compose_services: Vec::new(),
            compose_state: TableState::default(),
            compose_path: String::from("docker-compose.yml"),
            workloads: DockerWorkloadsState::default(),
            managed_services: Vec::new(),
            managed_state: TableState::default(),
            loading: false,
        }
    }
}

// ── Firewall state ────────────────────────────────────────────────────────

pub struct FirewallState {
    pub enabled: Option<bool>,
    pub backend: String,
    pub rules: Vec<FirewallRule>,
    pub table_state: TableState,
    pub input_mode: InputMode,
    /// 0 = port, 1 = proto, 2 = from, 3 = action
    pub input_focus: usize,
    pub input_port: String,
    pub input_proto: usize, // index into PROTOS
    pub input_from: String,
    pub input_action: usize, // index into ACTIONS
}

pub const PROTOS: &[&str] = &["tcp", "udp", "any"];
pub const ACTIONS: &[&str] = &["allow", "deny"];

// ── PortChecker state ─────────────────────────────────────────────────────

pub struct PortCheckerState {
    /// Resolved public IP (fetched from ipify.org)
    pub public_ip: Option<String>,
    /// True while we are fetching the public IP
    pub ip_loading: bool,
    /// True while the portchecker.co check is running
    pub checking: bool,
    /// The port list with their last-known statuses
    pub entries: Vec<PortEntry>,
    pub list_state: ListState,
    /// Text input for adding a custom port
    pub input_mode: InputMode,
    pub input_port: String,
    pub input_label: String,
    /// 0 = port field, 1 = label field
    pub input_focus: usize,
}

impl Default for PortCheckerState {
    fn default() -> Self {
        let entries = default_entries();
        let mut list_state = ListState::default();
        if !entries.is_empty() {
            list_state.select(Some(0));
        }
        Self {
            public_ip: None,
            ip_loading: false,
            checking: false,
            entries,
            list_state,
            input_mode: InputMode::Normal,
            input_port: String::new(),
            input_label: String::new(),
            input_focus: 0,
        }
    }
}

impl Default for FirewallState {
    fn default() -> Self {
        Self {
            enabled: None,
            backend: String::new(),
            rules: Vec::new(),
            table_state: TableState::default(),
            input_mode: InputMode::Normal,
            input_focus: 0,
            input_port: String::new(),
            input_proto: 0,
            input_from: String::new(),
            input_action: 0,
        }
    }
}

// ── SSH state ────────────────────────────────────────────────────────────

pub struct SshState {
    pub local_keys: Vec<SshKey>,
    pub local_state: ListState,
    pub authorized_keys: Vec<SshKey>,
    pub authorized_state: ListState,
    pub loading: bool,
    pub input_mode: InputMode,
    pub input_name: String,
    pub input_type: String, // ed25519 (default), rsa
    pub focus: usize,       // 0 = local, 1 = authorized
}

impl Default for SshState {
    fn default() -> Self {
        Self {
            local_keys: Vec::new(),
            local_state: ListState::default(),
            authorized_keys: Vec::new(),
            authorized_state: ListState::default(),
            loading: false,
            input_mode: InputMode::Normal,
            input_name: String::new(),
            input_type: "ed25519".to_string(),
            focus: 0,
        }
    }
}

// ── wasmCloud state ───────────────────────────────────────────────────────

pub struct WasmCloudState {
    pub installed: bool,
    pub version: Option<String>,
    pub active_tab: WasmCloudTab,
    pub hosts: Vec<WasmCloudHost>,
    pub hosts_state: TableState,
    pub components: Vec<WasmCloudComponent>,
    pub components_state: TableState,
    pub apps: Vec<WasmCloudApp>,
    pub apps_state: TableState,
    pub inspect_target: String,
    pub inspect_output: Option<String>,
    pub loading: bool,
    pub nats_running: bool,
    pub nats_storage_usage: Option<u64>,
    pub nats_synced: bool,
    /// Tick counter for throttled NATS health polls (fires every 20 ticks ≈ 5 s)
    pub nats_poll_counter: u8,
    /// Whether the Inspector text field is active
    pub input_mode: InputMode,
}

impl Default for WasmCloudState {
    fn default() -> Self {
        Self {
            installed: false,
            version: None,
            active_tab: WasmCloudTab::Hosts,
            hosts: Vec::new(),
            hosts_state: TableState::default(),
            components: Vec::new(),
            components_state: TableState::default(),
            apps: Vec::new(),
            apps_state: TableState::default(),
            inspect_target: String::new(),
            inspect_output: None,
            loading: false,
            nats_running: false,
            nats_storage_usage: None,
            nats_synced: false,
            nats_poll_counter: 0,
            input_mode: InputMode::Normal,
        }
    }
}

// ── Ghost Services Hunter state ────────────────────────────────────────────

#[derive(Default)]
pub struct GhostState {
    pub ghosts: Vec<GhostProcess>,
    pub table_state: TableState,
    pub scanning: bool,
}

// ── Swap state ───────────────────────────────────────────────────────────────

pub struct SwapState {
    pub status: Option<SwapStatus>,
    pub loading: bool,
    pub table_state: TableState,
    pub input_mode: InputMode,
    pub input_path: String,
    pub input_size: String,
    pub input_focus: usize, // 0 = path, 1 = size
    pub resize_mode: bool,  // true when resizing an existing entry
}

impl Default for SwapState {
    fn default() -> Self {
        Self {
            status: None,
            loading: false,
            table_state: TableState::default(),
            input_mode: InputMode::Normal,
            input_path: String::from("/swapfile"),
            input_size: String::from("2048"),
            input_focus: 0,
            resize_mode: false,
        }
    }
}

// ── Storage state ───────────────────────────────────────────────────────────
pub struct StorageState {
    pub devices: Vec<crate::core::models::StorageDevice>,
    pub table_state: TableState,
    pub physical: Vec<crate::core::models::SmartInfo>,
    pub loading: bool,
    pub smart_loading: bool,
    pub input_mode: InputMode,
    pub input_device: String,
    pub input_mountpoint: String,
    pub input_focus: usize, // 0 = device, 1 = mountpoint
    pub show_fstab: bool,
    pub fstab_scroll: u16,
    pub fstab_content: String,
}

impl Default for StorageState {
    fn default() -> Self {
        Self {
            devices: Vec::new(),
            table_state: TableState::default(),
            physical: Vec::new(),
            loading: false,
            smart_loading: false,
            input_mode: InputMode::Normal,
            input_device: String::new(),
            input_mountpoint: String::new(),
            input_focus: 0,
            show_fstab: false,
            fstab_scroll: 0,
            fstab_content: String::new(),
        }
    }
}

// ── Users state ─────────────────────────────────────────────────────────────
pub struct UsersState {
    pub users: Vec<UserInfo>,
    pub table_state: TableState,
    pub loading: bool,
    pub input_mode: InputMode,
    /// 0 = username, 1 = shell  (add-user form)
    pub input_focus: usize,
    pub input_username: String,
    pub input_shell: String,
    /// password popup fields: 0 = password, 1 = confirm
    pub pw_focus: usize,
    pub pw_password: String,
    pub pw_confirm: String,
    /// username targeted by the password popup
    pub pw_target: String,
}

impl Default for UsersState {
    fn default() -> Self {
        Self {
            users: Vec::new(),
            table_state: TableState::default(),
            loading: false,
            input_mode: InputMode::Normal,
            input_focus: 0,
            input_username: String::new(),
            input_shell: String::new(),
            pw_focus: 0,
            pw_password: String::new(),
            pw_confirm: String::new(),
            pw_target: String::new(),
        }
    }
}

// ── Services state ────────────────────────────────────────────────────────
pub struct ServicesState {
    pub list: Vec<ServiceUnit>,
    pub table_state: TableState,
    pub loading: bool,
    pub filter: String,
    pub filter_mode: InputMode,
}

impl Default for ServicesState {
    fn default() -> Self {
        Self {
            list: Vec::new(),
            table_state: TableState::default(),
            loading: false,
            filter: String::new(),
            filter_mode: InputMode::Normal,
        }
    }
}

// ── Maintenance state ─────────────────────────────────────────────────────
#[derive(Default)]
pub struct MaintenanceState {
    pub running_op: Option<String>,
    pub last_output: String,
}

// ── Agent screen state ────────────────────────────────────────────────────

pub struct AgentState {
    pub active_tab: AgentTab,
    // Chat
    pub messages: Vec<AgentMessage>,
    pub input: String,
    pub input_mode: InputMode,
    pub streaming: bool,
    pub rpc_active: bool,
    pub tool_log: Vec<String>,
    pub status: String,
    pub rpc_handle: Option<crate::core::pi_agent::rpc::RpcHandle>,
    pub pending_prompt: Option<String>,
    // Tasks
    pub tasks: Vec<crate::db::agent_tasks::AgentTask>,
    pub tasks_state: TableState,
    pub tasks_loading: bool,
    pub task_form_open: bool,
    pub task_form_name: String,
    pub task_form_prompt: String,
    pub task_form_schedule_idx: usize,
    pub task_form_focus: usize,
    pub task_form_mode: InputMode,
    // Status / management
    pub info: crate::core::pi_agent::PiAgentInfo,
    pub sessions: Vec<crate::core::pi_agent::PiSession>,
    pub sessions_state: TableState,
    pub config_text: String,
    pub config_scroll: u16,
    pub config_search: String,
    pub config_search_mode: InputMode,
    pub auth_entries: Vec<crate::core::pi_agent::PiAuthEntry>,
    pub auth_state: TableState,
    pub skills: Vec<crate::core::pi_agent::PiSkill>,
    pub skills_state: TableState,
    pub skills_status: Option<String>,
    pub library_skills: Vec<crate::core::pi_agent::LibrarySkill>,
    pub library_state: TableState,
    pub library_status: Option<String>,
    pub logs: Vec<String>,
    pub logs_scroll: u16,
    pub logs_follow: bool,
    pub loading: bool,
    pub installing: bool,
    pub install_log: Vec<String>,
    pub action_output: Option<String>,
    pub poll_counter: u32,
}

impl Default for AgentState {
    fn default() -> Self {
        Self {
            active_tab: AgentTab::Chat,
            messages: Vec::new(),
            input: String::new(),
            input_mode: InputMode::Normal,
            streaming: false,
            rpc_active: false,
            tool_log: Vec::new(),
            status: "Disconnected".to_string(),
            rpc_handle: None,
            pending_prompt: None,
            tasks: Vec::new(),
            tasks_state: TableState::default(),
            tasks_loading: false,
            task_form_open: false,
            task_form_name: String::new(),
            task_form_prompt: String::new(),
            task_form_schedule_idx: 1,
            task_form_focus: 0,
            task_form_mode: InputMode::Normal,
            info: crate::core::pi_agent::PiAgentInfo {
                installed: false,
                version: None,
            },
            sessions: Vec::new(),
            sessions_state: TableState::default(),
            config_text: String::new(),
            config_scroll: 0,
            config_search: String::new(),
            config_search_mode: InputMode::Normal,
            auth_entries: Vec::new(),
            auth_state: TableState::default(),
            skills: Vec::new(),
            skills_state: TableState::default(),
            skills_status: None,
            library_skills: Vec::new(),
            library_state: TableState::default(),
            library_status: None,
            logs: Vec::new(),
            logs_scroll: 0,
            logs_follow: true,
            loading: false,
            installing: false,
            install_log: Vec::new(),
            action_output: None,
            poll_counter: 0,
        }
    }
}

// ── Projects state ────────────────────────────────────────────────────────

pub struct ProjectsState {
    pub active_tab: ProjectsTab,
    // Projects list tab
    pub list: Vec<crate::core::projects::ProjectEntry>,
    pub list_state: ListState,
    pub loading: bool,
    // New tab
    pub new_name: String,
    pub new_form_focus: usize,  // 0-11 = Frontend..ServerDeploy
    pub new_frontend_idx: usize,
    pub new_database_idx: usize,
    pub new_orm_idx: usize,
    pub new_auth_idx: usize,
    pub new_backend_idx: usize,
    pub new_api_idx: usize,
    pub new_runtime_idx: usize,
    pub new_payments_idx: usize,
    pub new_examples_idx: usize,
    pub new_git_idx: usize,
    pub new_web_deploy_idx: usize,
    pub new_server_deploy_idx: usize,
    pub new_addons_selected: Vec<bool>,
    pub new_addons_cursor: usize,
    pub new_addons_popup: bool,
    pub new_stack_popup: bool,
    pub new_output: Vec<String>,
    pub new_output_scroll: usize,
    pub new_running: bool,
    pub new_input_mode: InputMode,
    // Clone tab
    pub clone_url: String,
    pub clone_output: Vec<String>,
    pub clone_output_scroll: usize,
    pub clone_running: bool,
    pub clone_input_mode: InputMode,
    // Settings tab
    pub dir: String,
    pub dir_input: String,
    pub settings_focus: usize, // 0=dir, 1=git name, 2=git email, 3=github token
    pub settings_edit_mode: InputMode,
    pub git: crate::core::projects::GitStatus,
    pub git_name_input: String,
    pub git_email_input: String,
    pub git_token_input: String,
}

impl Default for ProjectsState {
    fn default() -> Self {
        Self {
            active_tab: ProjectsTab::Projects,
            list: Vec::new(),
            list_state: ListState::default(),
            loading: false,
            new_name: String::new(),
            new_form_focus: 0,
            new_frontend_idx: 0,
            new_database_idx: 0,
            new_orm_idx: 0,
            new_auth_idx: 0,
            new_backend_idx: 0,
            new_api_idx: 0,
            new_runtime_idx: 0,
            new_payments_idx: 0,
            new_examples_idx: 0,
            new_git_idx: 0,
            new_web_deploy_idx: 0,
            new_server_deploy_idx: 0,
            new_addons_selected: vec![false; BTS_ADDONS.len()],
            new_addons_cursor: 0,
            new_addons_popup: false,
            new_stack_popup: false,
            new_output: Vec::new(),
            new_output_scroll: 0,
            new_running: false,
            new_input_mode: InputMode::Normal,
            clone_url: String::new(),
            clone_output: Vec::new(),
            clone_output_scroll: 0,
            clone_running: false,
            clone_input_mode: InputMode::Normal,
            dir: String::new(),
            dir_input: String::new(),
            settings_focus: 0,
            settings_edit_mode: InputMode::Normal,
            git: crate::core::projects::GitStatus::default(),
            git_name_input: String::new(),
            git_email_input: String::new(),
            git_token_input: String::new(),
        }
    }
}

// ── Agent overlay state ───────────────────────────────────────────────────

#[derive(Default)]
pub struct AgentOverlayState {
    pub open: bool,
    pub context_label: String,
    pub context_body: String,
    pub question: String,
}

// ── Main App ──────────────────────────────────────────────────────────────

pub struct App {
    pub screen: Screen,
    pub platform: Arc<Platform>,
    pub pool: SqlitePool,

    // Sub-tabs
    pub networking_tab: NetworkingTab,
    pub system_tab: SystemTab,

    // Screen state
    pub dashboard: DashboardState,
    pub packages: PackagesState,
    pub processes: ProcessesState,
    pub security: SecurityState,
    pub resources: ResourcesState,
    pub gateway: GatewayState,
    pub tailscale: TailscaleState,
    pub tunnel: TunnelState,
    pub docker: DockerState,
    pub firewall: FirewallState,
    pub ssh: SshState,
    pub portchecker: PortCheckerState,
    pub wasm_cloud: WasmCloudState,
    pub ghost: GhostState,
    pub users: UsersState,
    pub services: ServicesState,
    pub maintenance: MaintenanceState,
    pub agent: AgentState,
    pub swap: SwapState,
    pub storage: StorageState,
    pub projects: ProjectsState,

    pub terminal_width: u16,
    pub scheduler_tick: u32,

    // Background task channel
    pub task_tx: mpsc::UnboundedSender<TaskResult>,
    pub task_rx: mpsc::UnboundedReceiver<TaskResult>,

    pub overlay: AgentOverlayState,
    pub confirm: Option<ConfirmDialog>,
    pub status_msg: Option<String>,
    pub last_tick: Instant,
    /// Set to true to suspend the TUI and run `cloudflared tunnel login` in the foreground.
    pub needs_login: bool,
}

impl App {
    pub fn new(platform: Platform, pool: SqlitePool) -> Self {
        let (task_tx, task_rx) = mpsc::unbounded_channel();
        Self {
            screen: Screen::Dashboard,
            networking_tab: NetworkingTab::default(),
            system_tab: SystemTab::default(),
            platform: Arc::new(platform),
            pool,
            dashboard: DashboardState::default(),
            packages: PackagesState::default(),
            processes: ProcessesState::default(),
            security: SecurityState::default(),
            resources: ResourcesState::default(),
            gateway: GatewayState::default(),
            tailscale: TailscaleState::default(),
            tunnel: TunnelState::default(),
            docker: DockerState::default(),
            firewall: FirewallState::default(),
            ssh: SshState::default(),
            portchecker: PortCheckerState::default(),
            wasm_cloud: WasmCloudState::default(),
            ghost: GhostState::default(),
            users: UsersState::default(),
            services: ServicesState::default(),
            maintenance: MaintenanceState::default(),
            agent: AgentState::default(),
            swap: SwapState::default(),
            storage: StorageState::default(),
            projects: ProjectsState::default(),
            task_tx,
            task_rx,
            scheduler_tick: 0,
            overlay: AgentOverlayState::default(),
            confirm: None,
            status_msg: None,
            last_tick: Instant::now(),
            terminal_width: 0,
            needs_login: false,
        }
    }

    pub fn set_screen(&mut self, screen: Screen) {
        self.screen = screen.clone();
        self.status_msg = None;
        // Trigger initial data load for screens that need it
        match &screen {
            Screen::Dashboard => {
                if self.swap.status.is_none() {
                    self.spawn_load_swap();
                }
            }
            Screen::Packages => {
                if self.packages.installed.is_empty() {
                    self.spawn_load_packages();
                }
            }
            Screen::Security => {
                let tab = self.security.active_tab.clone();
                self.spawn_load_security_tab(tab);
            }
            Screen::Networking => {
                self.spawn_load_gateway();
                self.spawn_load_tunnels();
                let id = self.tunnel.active_tunnel_id.clone();
                self.spawn_tunnel_extras(id);
                self.spawn_load_tailscale();
            }
            Screen::Docker => {
                self.spawn_load_docker();
                self.spawn_load_workloads();
            }
            Screen::WasmCloud => {
                self.spawn_load_wasm_cloud();
                self.spawn_poll_nats_status();
            }
            Screen::Agent => {
                self.spawn_load_pi_agent_status();
                self.spawn_load_agent_tasks();
            }
            Screen::System => {
                self.spawn_load_system_tab(self.system_tab.clone());
            }
            Screen::Projects => {
                if self.projects.dir.is_empty() {
                    self.spawn_projects_load_dir();
                } else {
                    self.spawn_load_projects();
                }
            }
        }
    }

    pub fn next_screen(&mut self) {
        let idx = (self.screen.index() + 1) % Screen::all().len();
        let s = Screen::all()[idx].clone();
        self.set_screen(s);
    }

    pub fn prev_screen(&mut self) {
        let idx = self.screen.index();
        let prev = if idx == 0 {
            Screen::all().len() - 1
        } else {
            idx - 1
        };
        let s = Screen::all()[prev].clone();
        self.set_screen(s);
    }

    pub fn set_screen_by_index(&mut self, idx: usize) {
        if let Some(s) = Screen::all().get(idx) {
            let s = s.clone();
            self.set_screen(s);
        }
    }

    pub fn spawn_load_security_tab(&mut self, tab: SecurityTab) {
        match tab {
            SecurityTab::Findings => {
                // Optional: avoid auto-scan if it's slow, but let's keep it consistent
                if self.security.findings.is_empty() && !self.security.scanning {
                    self.spawn_security_scan();
                }
            }
            SecurityTab::Firewall => self.spawn_load_firewall(),
            SecurityTab::Ports => {
                if self.portchecker.public_ip.is_none() && !self.portchecker.ip_loading {
                    self.spawn_fetch_public_ip();
                }
            }
            SecurityTab::Ssh => self.spawn_load_ssh(),
            SecurityTab::Fail2Ban => self.spawn_fail2ban_list(),
        }
    }

    pub fn spawn_load_system_tab(&mut self, tab: SystemTab) {
        match tab {
            SystemTab::Ghosts => {
                if self.ghost.ghosts.is_empty() {
                    self.spawn_ghost_scan();
                }
            }
            SystemTab::Janitor => {}
            SystemTab::Services => self.spawn_load_services(),
            SystemTab::Users => self.spawn_load_users(),
            SystemTab::Swap => self.spawn_load_swap(),
            SystemTab::Storage => self.spawn_load_storage(),
        }
    }

    pub fn spawn_load_networking_tab(&mut self, tab: NetworkingTab) {
        match tab {
            NetworkingTab::Gateway => self.spawn_load_gateway(),
            NetworkingTab::Tunnel => {
                self.spawn_load_tunnels();
                let id = self.tunnel.active_tunnel_id.clone();
                self.spawn_tunnel_extras(id);
            }
            NetworkingTab::Tailscale => self.spawn_load_tailscale(),
        }
    }

    pub fn spawn_load_docker_tab(&mut self, tab: &DockerTab) {
        match tab {
            DockerTab::Containers | DockerTab::Images => self.spawn_load_docker(),
            DockerTab::Compose => self.spawn_load_compose(),
            DockerTab::Workloads => self.spawn_load_workloads(),
            DockerTab::Managed => self.spawn_load_managed_services(),
        }
    }

    // ── async data loaders (spawn background tasks) ───────────────────────

    #[allow(dead_code)]
    pub fn spawn_load_dashboard(&mut self) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            if let Ok(info) = platform.system.info().await {
                let _ = tx.send(TaskResult::PackageList(Vec::new())); // dummy; we update separately
                let _ = tx.send(TaskResult::ProcessList(Vec::new())); // dummy
                drop(info); // suppress unused warning — real dashboard updates via tick
            }
        });
    }

    pub fn spawn_load_packages(&mut self) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            match platform.packages.list_installed().await {
                Ok(pkgs) => {
                    let _ = tx.send(TaskResult::PackageList(pkgs));
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(e.to_string()));
                }
            }
        });
    }

    /// Targeted refresh: query only the given package names and merge results.
    pub fn spawn_check_packages(&mut self, names: Vec<String>) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            let refs: Vec<&str> = names.iter().map(String::as_str).collect();
            match platform.packages.check_packages(&refs).await {
                Ok(pkgs) => {
                    let _ = tx.send(TaskResult::PackagesUpdated(pkgs));
                }
                Err(_) => {
                    // Fall back to full reload if targeted check fails
                    if let Ok(pkgs) = platform.packages.list_installed().await {
                        let _ = tx.send(TaskResult::PackageList(pkgs));
                    }
                }
            }
        });
    }

    pub fn spawn_search(&mut self, query: String) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            match platform.packages.search(&query).await {
                Ok(pkgs) => {
                    let _ = tx.send(TaskResult::SearchResults(pkgs));
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(e.to_string()));
                }
            }
        });
    }

    pub fn spawn_install(&mut self, name: String) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        let pool = self.pool.clone();
        self.packages.queue.push_back(QueuedOp {
            kind: "install".to_string(),
            target: name.clone(),
            status: OpStatus::Running,
            output: String::new(),
        });
        if self.packages.queue_selected.is_none() {
            self.packages.queue_selected = Some(self.packages.queue.len() - 1);
            self.packages.output_scroll = 0;
        }
        tokio::spawn(async move {
            let (ptx, mut prx) = tokio::sync::mpsc::unbounded_channel::<String>();
            let tx_fwd = tx.clone();
            let name_fwd = name.clone();
            let fwd = tokio::spawn(async move {
                while let Some(line) = prx.recv().await {
                    let _ = tx_fwd.send(TaskResult::OpProgress {
                        op: "install".to_string(),
                        target: name_fwd.clone(),
                        line,
                    });
                }
            });
            let result = platform.packages.install_streamed(&name, ptx).await;
            let _ = fwd.await;
            let (output, success) = match result {
                Ok(out) => (out, true),
                Err(e) => (e.to_string(), false),
            };
            let _ =
                crate::db::audit::log_action(&pool, "install", Some(&name), &output, success).await;
            let _ = tx.send(TaskResult::OpDone {
                op: "install".to_string(),
                target: name,
                output,
                success,
            });
        });
    }

    pub fn spawn_remove(&mut self, name: String) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        let pool = self.pool.clone();
        self.packages.queue.push_back(QueuedOp {
            kind: "remove".to_string(),
            target: name.clone(),
            status: OpStatus::Running,
            output: String::new(),
        });
        if self.packages.queue_selected.is_none() {
            self.packages.queue_selected = Some(self.packages.queue.len() - 1);
            self.packages.output_scroll = 0;
        }
        tokio::spawn(async move {
            let (ptx, mut prx) = tokio::sync::mpsc::unbounded_channel::<String>();
            let tx_fwd = tx.clone();
            let name_fwd = name.clone();
            let fwd = tokio::spawn(async move {
                while let Some(line) = prx.recv().await {
                    let _ = tx_fwd.send(TaskResult::OpProgress {
                        op: "remove".to_string(),
                        target: name_fwd.clone(),
                        line,
                    });
                }
            });
            let result = platform.packages.remove_streamed(&name, ptx).await;
            let _ = fwd.await;
            let (output, success) = match result {
                Ok(out) => (out, true),
                Err(e) => (e.to_string(), false),
            };
            let _ =
                crate::db::audit::log_action(&pool, "remove", Some(&name), &output, success).await;
            let _ = tx.send(TaskResult::OpDone {
                op: "remove".to_string(),
                target: name,
                output,
                success,
            });
        });
    }

    pub fn spawn_load_processes(&mut self) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            match platform.processes.list().await {
                Ok(procs) => {
                    let _ = tx.send(TaskResult::ProcessList(procs));
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(e.to_string()));
                }
            }
        });
    }

    pub fn spawn_security_scan(&mut self) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        self.security.scanning = true;
        tokio::spawn(async move {
            match platform.security.scan().await {
                Ok(findings) => {
                    let _ = tx.send(TaskResult::SecurityScan(findings));
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(e.to_string()));
                }
            }
        });
    }

    pub fn spawn_fail2ban_list(&mut self) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        self.security.f2b_loading = true;
        tokio::spawn(async move {
            match platform.fail2ban.list_jailed().await {
                Ok(jailed) => {
                    let _ = tx.send(TaskResult::Fail2BanList(jailed));
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(format!("fail2ban: {}", e)));
                }
            }
        });
    }

    pub fn spawn_fail2ban_unban(&mut self, jail: String, ip: String) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            let success = platform.fail2ban.unban(&jail, &ip).await.is_ok();
            let _ = tx.send(TaskResult::Fail2BanActionDone {
                ip,
                jail,
                action: "forgiven".to_string(),
                success,
            });
        });
    }

    pub fn spawn_fail2ban_banish(&mut self, jail: String, ip: String) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            let success = platform.fail2ban.banish(&jail, &ip).await.is_ok();
            let _ = tx.send(TaskResult::Fail2BanActionDone {
                ip,
                jail,
                action: "banished".to_string(),
                success,
            });
        });
    }

    pub fn spawn_security_apply(&mut self, id: String) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        let pool = self.pool.clone();
        tokio::spawn(async move {
            let result = platform.security.apply(&id).await;
            let (output, success) = match result {
                Ok(out) => (out, true),
                Err(e) => (e.to_string(), false),
            };
            let _ =
                crate::db::audit::log_action(&pool, "harden", Some(&id), &output, success).await;
            let _ = tx.send(TaskResult::SecurityApply {
                id,
                output,
                success,
            });
        });
    }

    pub fn spawn_load_gateway(&mut self) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            let installed = platform.gateway.is_installed().await;
            let version = platform.gateway.version().await;
            let _ = tx.send(TaskResult::GatewayStatus { installed, version });
            if installed {
                match platform.gateway.list_routes().await {
                    Ok(routes) => {
                        let _ = tx.send(TaskResult::RouteList(routes));
                    }
                    Err(e) => {
                        let _ = tx.send(TaskResult::Error(e.to_string()));
                    }
                }
            }
        });
    }

    pub fn spawn_load_tunnels(&mut self) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            let installed = platform.tunnel.is_installed().await;
            let version = platform.tunnel.version().await;
            let _ = tx.send(TaskResult::TunnelStatus { installed, version });
            if installed {
                match platform.tunnel.list_tunnels().await {
                    Ok(tunnels) => {
                        let _ = tx.send(TaskResult::TunnelList(tunnels));
                    }
                    Err(e) => {
                        let _ = tx.send(TaskResult::Error(e.to_string()));
                    }
                }
            }
        });
    }

    pub fn spawn_load_tailscale(&mut self) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        self.tailscale.loading = true;
        tokio::spawn(async move {
            let installed = platform.tailscale.is_installed().await;
            let version = platform.tailscale.version().await;
            let (backend_state, self_ip, self_name, peers) = if installed {
                match platform.tailscale.status().await {
                    Ok(s) => (s.backend_state, s.self_ip, s.self_name, s.peers),
                    Err(_) => (String::new(), None, None, Vec::new()),
                }
            } else {
                (String::new(), None, None, Vec::new())
            };
            let _ = tx.send(TaskResult::TailscaleStatus {
                installed,
                version,
                backend_state,
                self_ip,
                self_name,
                peers,
            });
        });
    }

    pub fn spawn_install_tailscale(&mut self) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        self.status_msg = Some("Installing Tailscale…".to_string());
        tokio::spawn(async move {
            let (ptx, mut prx) = tokio::sync::mpsc::unbounded_channel::<String>();
            let tx_fwd = tx.clone();
            let fwd = tokio::spawn(async move {
                while let Some(line) = prx.recv().await {
                    let _ = tx_fwd.send(TaskResult::InstallProgress {
                        target: "tailscale".to_string(),
                        line,
                    });
                }
            });
            let result = platform.tailscale.install(ptx).await;
            let _ = fwd.await;
            let success = result.is_ok();
            let installed = platform.tailscale.is_installed().await;
            let version = platform.tailscale.version().await;
            let _ = tx.send(TaskResult::TailscaleStatus {
                installed,
                version,
                backend_state: String::new(),
                self_ip: None,
                self_name: None,
                peers: Vec::new(),
            });
            let _ = tx.send(TaskResult::InstallDone {
                target: "tailscale".to_string(),
                success,
            });
        });
    }

    pub fn spawn_tailscale_up(&mut self) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        self.status_msg = Some("Running tailscale up…".to_string());
        tokio::spawn(async move {
            match platform.tailscale.up().await {
                Ok(()) => {
                    if let Ok(s) = platform.tailscale.status().await {
                        let installed = true;
                        let version = platform.tailscale.version().await;
                        let _ = tx.send(TaskResult::TailscaleStatus {
                            installed,
                            version,
                            backend_state: s.backend_state,
                            self_ip: s.self_ip,
                            self_name: s.self_name,
                            peers: s.peers,
                        });
                    }
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(e.to_string()));
                }
            }
        });
    }

    pub fn spawn_tailscale_down(&mut self) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        self.status_msg = Some("Running tailscale down…".to_string());
        tokio::spawn(async move {
            match platform.tailscale.down().await {
                Ok(()) => {
                    if let Ok(s) = platform.tailscale.status().await {
                        let installed = true;
                        let version = platform.tailscale.version().await;
                        let _ = tx.send(TaskResult::TailscaleStatus {
                            installed,
                            version,
                            backend_state: s.backend_state,
                            self_ip: s.self_ip,
                            self_name: s.self_name,
                            peers: s.peers,
                        });
                    }
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(e.to_string()));
                }
            }
        });
    }

    pub fn spawn_tunnel_extras(&mut self, tunnel_id: Option<String>) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            if let Some(ref id) = tunnel_id {
                match platform.tunnel.config_content(id).await {
                    Ok(c) => {
                        let _ = tx.send(TaskResult::TunnelConfigContent(c));
                    }
                    Err(_) => {
                        let _ = tx.send(TaskResult::TunnelConfigContent(String::new()));
                    }
                }
            }
            if let Ok((active, enabled)) = platform.tunnel.service_status().await {
                let _ = tx.send(TaskResult::TunnelServiceStatus { active, enabled });
            }
        });
    }

    pub fn spawn_install_cloudflared(&mut self) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        self.status_msg = Some("Installing cloudflared…".to_string());
        tokio::spawn(async move {
            let (ptx, mut prx) = tokio::sync::mpsc::unbounded_channel::<String>();
            let tx_fwd = tx.clone();
            let fwd = tokio::spawn(async move {
                while let Some(line) = prx.recv().await {
                    let _ = tx_fwd.send(TaskResult::InstallProgress {
                        target: "cloudflared".to_string(),
                        line,
                    });
                }
            });
            let result = platform.tunnel.install_streamed(ptx).await;
            let _ = fwd.await;
            match result {
                Ok(_) => {
                    let installed = platform.tunnel.is_installed().await;
                    let version = platform.tunnel.version().await;
                    let _ = tx.send(TaskResult::TunnelStatus { installed, version });
                    let _ = tx.send(TaskResult::InstallDone {
                        target: "cloudflared".to_string(),
                        success: true,
                    });
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::InstallDone {
                        target: "cloudflared".to_string(),
                        success: false,
                    });
                    let _ = tx.send(TaskResult::Error(format!(
                        "cloudflared install failed: {}",
                        e
                    )));
                }
            }
        });
    }

    pub fn spawn_install_caddy(&mut self) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        self.status_msg = Some("Installing Caddy…".to_string());
        tokio::spawn(async move {
            let (ptx, mut prx) = tokio::sync::mpsc::unbounded_channel::<String>();
            let tx_fwd = tx.clone();
            let fwd = tokio::spawn(async move {
                while let Some(line) = prx.recv().await {
                    let _ = tx_fwd.send(TaskResult::InstallProgress {
                        target: "caddy".to_string(),
                        line,
                    });
                }
            });
            let result = platform.gateway.install_streamed(ptx).await;
            let _ = fwd.await;
            match result {
                Ok(_) => {
                    let installed = platform.gateway.is_installed().await;
                    let version = platform.gateway.version().await;
                    let _ = tx.send(TaskResult::GatewayStatus { installed, version });
                    let _ = tx.send(TaskResult::InstallDone {
                        target: "caddy".to_string(),
                        success: true,
                    });
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::InstallDone {
                        target: "caddy".to_string(),
                        success: false,
                    });
                    let _ = tx.send(TaskResult::Error(format!("Caddy install failed: {}", e)));
                }
            }
        });
    }

    pub fn spawn_install_wash(&mut self) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        self.status_msg = Some("Installing wash CLI…".to_string());
        tokio::spawn(async move {
            let (ptx, mut prx) = tokio::sync::mpsc::unbounded_channel::<String>();
            let tx_fwd = tx.clone();
            let fwd = tokio::spawn(async move {
                while let Some(line) = prx.recv().await {
                    let _ = tx_fwd.send(TaskResult::InstallProgress {
                        target: "wash".to_string(),
                        line,
                    });
                }
            });
            let result = platform.wasm_cloud.install_streamed(ptx).await;
            let _ = fwd.await;
            match result {
                Ok(_) => {
                    let installed = platform.wasm_cloud.is_installed().await;
                    let version = platform.wasm_cloud.version().await;
                    let _ = tx.send(TaskResult::WasmCloudStatus { installed, version });
                    let _ = tx.send(TaskResult::InstallDone {
                        target: "wash".to_string(),
                        success: true,
                    });
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::InstallDone {
                        target: "wash".to_string(),
                        success: false,
                    });
                    let _ = tx.send(TaskResult::Error(format!("wash install failed: {}", e)));
                }
            }
        });
    }

    // ── Docker loaders ────────────────────────────────────────────────────

    pub fn spawn_load_docker(&mut self) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        self.docker.loading = true;
        tokio::spawn(async move {
            let installed = platform.docker.is_installed().await;
            let version = platform.docker.version().await;
            let _ = tx.send(TaskResult::DockerStatus { installed, version });
            if installed {
                match platform.docker.list_containers().await {
                    Ok(containers) => {
                        let _ = tx.send(TaskResult::DockerContainerList(containers));
                    }
                    Err(e) => {
                        let _ = tx.send(TaskResult::Error(e.to_string()));
                    }
                }
                match platform.docker.list_images().await {
                    Ok(images) => {
                        let _ = tx.send(TaskResult::DockerImageList(images));
                    }
                    Err(e) => {
                        let _ = tx.send(TaskResult::Error(e.to_string()));
                    }
                }
            }
        });
    }

    pub fn spawn_docker_container_action(&mut self, action: &'static str, id: String) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        let id_clone = id.clone();
        tokio::spawn(async move {
            let result = match action {
                "start" => platform.docker.start_container(&id_clone).await,
                "stop" => platform.docker.stop_container(&id_clone).await,
                "restart" => platform.docker.restart_container(&id_clone).await,
                "remove" => platform.docker.remove_container(&id_clone).await,
                _ => Ok(()),
            };
            match result {
                Ok(()) => {
                    let _ = tx.send(TaskResult::Status(format!(
                        "{} {} — done",
                        action, id_clone
                    )));
                    match platform.docker.list_containers().await {
                        Ok(containers) => {
                            let _ = tx.send(TaskResult::DockerContainerList(containers));
                        }
                        Err(e) => {
                            let _ = tx.send(TaskResult::Error(e.to_string()));
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(format!("{} failed: {}", action, e)));
                }
            }
        });
    }

    pub fn spawn_docker_image_remove(&mut self, id: String) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            match platform.docker.remove_image(&id).await {
                Ok(()) => {
                    let _ = tx.send(TaskResult::Status(format!("Image {} removed", id)));
                    match platform.docker.list_images().await {
                        Ok(images) => {
                            let _ = tx.send(TaskResult::DockerImageList(images));
                        }
                        Err(e) => {
                            let _ = tx.send(TaskResult::Error(e.to_string()));
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(format!("Remove image failed: {}", e)));
                }
            }
        });
    }

    pub fn spawn_load_compose(&mut self) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        let path = self.docker.compose_path.clone();
        tokio::spawn(async move {
            match platform.docker.list_compose_services(&path).await {
                Ok(services) => {
                    let _ = tx.send(TaskResult::DockerComposeList(services));
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(e.to_string()));
                }
            }
        });
    }

    pub fn spawn_compose_action(&mut self, action: &'static str) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        let path = self.docker.compose_path.clone();
        tokio::spawn(async move {
            let result = match action {
                "up" => platform.docker.compose_up(&path).await,
                "down" => platform.docker.compose_down(&path).await,
                "restart" => platform.docker.compose_restart(&path).await,
                _ => Ok(()),
            };
            match result {
                Ok(()) => {
                    let _ = tx.send(TaskResult::Status(format!("compose {} — done", action)));
                    match platform.docker.list_compose_services(&path).await {
                        Ok(services) => {
                            let _ = tx.send(TaskResult::DockerComposeList(services));
                        }
                        Err(e) => {
                            let _ = tx.send(TaskResult::Error(e.to_string()));
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(format!(
                        "compose {} failed: {}",
                        action, e
                    )));
                }
            }
        });
    }

    pub fn spawn_load_managed_services(&mut self) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            match platform.docker.list_managed_services().await {
                Ok(services) => {
                    let _ = tx.send(TaskResult::ManagedServiceList(services));
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(e.to_string()));
                }
            }
        });
    }

    pub fn spawn_managed_service_action(
        &mut self,
        action: &'static str,
        container_name: String,
        image: String,
        ports: String,
    ) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            let result = match action {
                "start" => {
                    platform
                        .docker
                        .start_managed_service(&container_name, &image, &ports)
                        .await
                }
                "stop" => platform.docker.stop_managed_service(&container_name).await,
                "restart" => {
                    platform
                        .docker
                        .restart_managed_service(&container_name)
                        .await
                }
                _ => Ok(()),
            };
            match result {
                Ok(()) => {
                    let _ = tx.send(TaskResult::Status(format!(
                        "{} {} — done",
                        action, container_name
                    )));
                    // Refresh the list
                    match platform.docker.list_managed_services().await {
                        Ok(services) => {
                            let _ = tx.send(TaskResult::ManagedServiceList(services));
                        }
                        Err(e) => {
                            let _ = tx.send(TaskResult::Error(e.to_string()));
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(format!("{} failed: {}", action, e)));
                }
            }
        });
    }

    pub fn workload_spec_from_form(&self) -> anyhow::Result<ManagedWorkloadSpec> {
        let form = &self.docker.workloads.form;
        let parse_csv = |value: &str| -> Vec<String> {
            value
                .split(',')
                .map(|entry| entry.trim().to_string())
                .filter(|entry| !entry.is_empty())
                .collect()
        };

        let env = parse_csv(&form.env)
            .into_iter()
            .map(|pair| {
                let (key, value) = pair
                    .split_once('=')
                    .ok_or_else(|| anyhow::anyhow!("Env entries must use KEY=VALUE"))?;
                Ok((key.trim().to_string(), value.trim().to_string()))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        let command = {
            let command = parse_csv(&form.command);
            if command.is_empty() {
                None
            } else {
                Some(command)
            }
        };

        Ok(ManagedWorkloadSpec {
            name: form.name.trim().to_string(),
            image: form.image.trim().to_string(),
            command,
            env,
            ports: parse_csv(&form.ports),
            volumes: parse_csv(&form.volumes),
            restart_policy: form.restart_policy.trim().to_string(),
        })
    }

    pub fn spawn_load_workloads(&mut self) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            let capabilities = platform.workloads.capabilities().await;
            let _ = tx.send(TaskResult::WorkloadCapabilities(capabilities.clone()));
            if capabilities.supported {
                match platform.workloads.list_workloads().await {
                    Ok(workloads) => {
                        let _ = tx.send(TaskResult::WorkloadList(workloads));
                    }
                    Err(e) => {
                        let _ = tx.send(TaskResult::Error(format!("workloads: {}", e)));
                    }
                }
            }
        });
    }

    pub fn spawn_create_workload(&mut self, spec: ManagedWorkloadSpec) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            match platform.workloads.create_workload(spec.clone()).await {
                Ok(workload) => {
                    let _ = tx.send(TaskResult::Status(format!(
                        "Created workload {}",
                        workload.name
                    )));
                    match platform.workloads.list_workloads().await {
                        Ok(workloads) => {
                            let _ = tx.send(TaskResult::WorkloadList(workloads));
                        }
                        Err(e) => {
                            let _ = tx.send(TaskResult::Error(format!("workloads: {}", e)));
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(format!("create workload failed: {}", e)));
                }
            }
        });
    }

    pub fn spawn_update_workload(&mut self, name: String, spec: ManagedWorkloadSpec) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            match platform.workloads.update_workload(&name, spec).await {
                Ok(workload) => {
                    let _ = tx.send(TaskResult::Status(format!(
                        "Updated workload {}",
                        workload.name
                    )));
                    match platform.workloads.list_workloads().await {
                        Ok(workloads) => {
                            let _ = tx.send(TaskResult::WorkloadList(workloads));
                        }
                        Err(e) => {
                            let _ = tx.send(TaskResult::Error(format!("workloads: {}", e)));
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(format!("update workload failed: {}", e)));
                }
            }
        });
    }

    pub fn spawn_delete_workload(&mut self, name: String) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            match platform.workloads.delete_workload(&name).await {
                Ok(()) => {
                    let _ = tx.send(TaskResult::Status(format!("Deleted workload {}", name)));
                    match platform.workloads.list_workloads().await {
                        Ok(workloads) => {
                            let _ = tx.send(TaskResult::WorkloadList(workloads));
                        }
                        Err(e) => {
                            let _ = tx.send(TaskResult::Error(format!("workloads: {}", e)));
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(format!("delete workload failed: {}", e)));
                }
            }
        });
    }

    pub fn spawn_workload_action(&mut self, action: &'static str, name: String) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            let result = match action {
                "start" => platform.workloads.start_workload(&name).await,
                "stop" => platform.workloads.stop_workload(&name).await,
                "restart" => platform.workloads.restart_workload(&name).await,
                "enable" => platform.workloads.enable_workload(&name).await,
                "disable" => platform.workloads.disable_workload(&name).await,
                _ => Ok(()),
            };
            match result {
                Ok(()) => {
                    let _ = tx.send(TaskResult::Status(format!("{} {} — done", action, name)));
                    match platform.workloads.list_workloads().await {
                        Ok(workloads) => {
                            let _ = tx.send(TaskResult::WorkloadList(workloads));
                        }
                        Err(e) => {
                            let _ = tx.send(TaskResult::Error(format!("workloads: {}", e)));
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(format!(
                        "{} workload failed: {}",
                        action, e
                    )));
                }
            }
        });
    }

    // ── Firewall loaders ──────────────────────────────────────────────────

    pub fn spawn_load_firewall(&mut self) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            match platform.firewall.status().await {
                Ok((enabled, backend)) => {
                    let _ = tx.send(TaskResult::FirewallStatus { enabled, backend });
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(e.to_string()));
                    return;
                }
            }
            match platform.firewall.list_rules().await {
                Ok(rules) => {
                    let _ = tx.send(TaskResult::FirewallRules(rules));
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(e.to_string()));
                }
            }
        });
    }

    pub fn spawn_firewall_add_rule(
        &mut self,
        port: String,
        proto: String,
        from: String,
        action: String,
    ) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            match platform
                .firewall
                .add_rule(&port, &proto, &from, &action)
                .await
            {
                Ok(()) => {
                    let _ = tx.send(TaskResult::Status(format!(
                        "Rule added: {} {} from {}",
                        action, port, from
                    )));
                    match platform.firewall.list_rules().await {
                        Ok(rules) => {
                            let _ = tx.send(TaskResult::FirewallRules(rules));
                        }
                        Err(e) => {
                            let _ = tx.send(TaskResult::Error(e.to_string()));
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(format!("add rule failed: {}", e)));
                }
            }
        });
    }

    pub fn spawn_firewall_delete_rule(&mut self, num: usize) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            match platform.firewall.delete_rule(num).await {
                Ok(()) => {
                    let _ = tx.send(TaskResult::Status(format!("Rule {} deleted", num)));
                    match platform.firewall.list_rules().await {
                        Ok(rules) => {
                            let _ = tx.send(TaskResult::FirewallRules(rules));
                        }
                        Err(e) => {
                            let _ = tx.send(TaskResult::Error(e.to_string()));
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(format!("delete rule failed: {}", e)));
                }
            }
        });
    }

    pub fn spawn_firewall_set_enabled(&mut self, enabled: bool) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            match platform.firewall.set_enabled(enabled).await {
                Ok(()) => {
                    let label = if enabled { "enabled" } else { "disabled" };
                    let _ = tx.send(TaskResult::FirewallStatus {
                        enabled,
                        backend: String::new(),
                    });
                    let _ = tx.send(TaskResult::Status(format!("Firewall {}", label)));
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(e.to_string()));
                }
            }
        });
    }

    // ── Port Checker spawners ─────────────────────────────────────────────

    pub fn spawn_fetch_public_ip(&mut self) {
        self.portchecker.ip_loading = true;
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            match crate::core::portcheck::fetch_public_ip().await {
                Ok(ip) => {
                    let _ = tx.send(TaskResult::PublicIp(ip));
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(format!("public IP: {}", e)));
                }
            }
        });
    }

    pub fn spawn_check_ports(&mut self) {
        let ip = match &self.portchecker.public_ip {
            Some(ip) => ip.clone(),
            None => {
                self.status_msg = Some("Fetch public IP first ([r])".to_string());
                return;
            }
        };
        self.portchecker.checking = true;
        // Mark all current entries as Checking
        for e in &mut self.portchecker.entries {
            e.status = PortStatus::Checking;
        }
        let ports: Vec<u16> = self.portchecker.entries.iter().map(|e| e.port).collect();
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            match crate::core::portcheck::check_ports_external(&ip, &ports).await {
                Ok(results) => {
                    let _ = tx.send(TaskResult::PortCheckDone { results });
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(format!("port check: {}", e)));
                }
            }
        });
    }

    // ── Ghost Services Hunter ─────────────────────────────────────────────

    pub fn spawn_ghost_scan(&mut self) {
        self.ghost.scanning = true;
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            match crate::core::ghost::scan().await {
                Ok(ghosts) => {
                    let _ = tx.send(TaskResult::GhostScan(ghosts));
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(format!("ghost scan: {}", e)));
                }
            }
        });
    }

    // ── SSH spawners ──────────────────────────────────────────────────────

    pub fn spawn_load_ssh(&mut self) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        self.ssh.loading = true;
        tokio::spawn(async move {
            match platform.ssh.list_local_keys().await {
                Ok(keys) => {
                    let _ = tx.send(TaskResult::SshLocalKeys(keys));
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(e.to_string()));
                }
            }
            match platform.ssh.list_authorized_keys().await {
                Ok(keys) => {
                    let _ = tx.send(TaskResult::SshAuthorizedKeys(keys));
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(e.to_string()));
                }
            }
        });
    }

    pub fn spawn_authorize_key(&mut self, content: String) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            match platform.ssh.authorize_key(&content).await {
                Ok(()) => {
                    let _ = tx.send(TaskResult::SshOpDone {
                        op: "authorize".to_string(),
                        success: true,
                        output: String::new(),
                    });
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::SshOpDone {
                        op: "authorize".to_string(),
                        success: false,
                        output: e.to_string(),
                    });
                }
            }
        });
    }

    pub fn spawn_deauthorize_key(&mut self, fingerprint: String) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            match platform.ssh.deauthorize_key(&fingerprint).await {
                Ok(()) => {
                    let _ = tx.send(TaskResult::SshOpDone {
                        op: "deauthorize".to_string(),
                        success: true,
                        output: String::new(),
                    });
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::SshOpDone {
                        op: "deauthorize".to_string(),
                        success: false,
                        output: e.to_string(),
                    });
                }
            }
        });
    }

    pub fn spawn_generate_key(&mut self, name: String, key_type: String) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            match platform.ssh.generate_key(&name, &key_type).await {
                Ok(fingerprint) => {
                    let _ = tx.send(TaskResult::SshOpDone {
                        op: "generate".to_string(),
                        success: true,
                        output: fingerprint,
                    });
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::SshOpDone {
                        op: "generate".to_string(),
                        success: false,
                        output: e.to_string(),
                    });
                }
            }
        });
    }

    // ── Process incoming task results ─────────────────────────────────────

    pub fn spawn_load_users(&mut self) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        self.users.loading = true;
        tokio::spawn(async move {
            match platform.users.list_users().await {
                Ok(users) => {
                    let _ = tx.send(TaskResult::UserList(users));
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(e.to_string()));
                }
            }
        });
    }

    pub fn spawn_create_user(&mut self, username: String, shell: Option<String>) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            let shell_ref = shell.as_deref();
            match platform.users.create_user(&username, shell_ref).await {
                Ok(()) => {
                    let _ = tx.send(TaskResult::Status(format!("User '{}' created", username)));
                    match platform.users.list_users().await {
                        Ok(users) => {
                            let _ = tx.send(TaskResult::UserList(users));
                        }
                        Err(e) => {
                            let _ = tx.send(TaskResult::Error(e.to_string()));
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(format!("create user failed: {}", e)));
                }
            }
        });
    }

    pub fn spawn_set_password(&mut self, username: String, password: String) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            match platform.users.set_password(&username, &password).await {
                Ok(()) => {
                    let _ = tx.send(TaskResult::Status(format!(
                        "Password set for '{}'",
                        username
                    )));
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(format!("set password failed: {}", e)));
                }
            }
        });
    }

    pub fn spawn_add_to_sudoers(&mut self, username: String) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            match platform.users.add_to_sudoers(&username).await {
                Ok(()) => {
                    let _ = tx.send(TaskResult::Status(format!(
                        "'{}' added to sudoers",
                        username
                    )));
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(format!("add to sudoers failed: {}", e)));
                }
            }
        });
    }

    pub fn spawn_user_action(&mut self, action: String, username: String) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            let result = match action.as_str() {
                "delete" => platform.users.delete_user(&username).await,
                _ => Ok(()),
            };
            match result {
                Ok(()) => {
                    let _ = tx.send(TaskResult::Status(format!(
                        "User {} {} success",
                        username, action
                    )));
                    match platform.users.list_users().await {
                        Ok(users) => {
                            let _ = tx.send(TaskResult::UserList(users));
                        }
                        Err(e) => {
                            let _ = tx.send(TaskResult::Error(e.to_string()));
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(format!("{} failed: {}", action, e)));
                }
            }
        });
    }

    pub fn spawn_load_wasm_cloud(&mut self) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        self.wasm_cloud.loading = true;
        tokio::spawn(async move {
            let installed = platform.wasm_cloud.is_installed().await;
            let version = platform.wasm_cloud.version().await;
            let _ = tx.send(TaskResult::WasmCloudStatus { installed, version });

            if installed {
                if let Ok(hosts) = platform.wasm_cloud.list_hosts().await {
                    let _ = tx.send(TaskResult::WasmCloudHostList(hosts));
                }
                if let Ok(components) = platform.wasm_cloud.list_components().await {
                    let _ = tx.send(TaskResult::WasmCloudComponentList(components));
                }
                if let Ok(apps) = platform.wasm_cloud.list_apps().await {
                    let _ = tx.send(TaskResult::WasmCloudAppList(apps));
                }

                let nats_running = platform.nats.is_running();
                let storage_usage = platform.nats.get_storage_usage();
                let synced = platform.nats.is_synced();
                let _ = tx.send(TaskResult::WasmCloudNatsStatus {
                    running: nats_running,
                    storage_usage,
                    synced,
                });
            }
        });
    }

    #[allow(dead_code)]
    pub fn spawn_wasm_cloud_hosts(&mut self) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            if let Ok(hosts) = platform.wasm_cloud.list_hosts().await {
                let _ = tx.send(TaskResult::WasmCloudHostList(hosts));
            }
        });
    }

    #[allow(dead_code)]
    pub fn spawn_wasm_cloud_components(&mut self) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            if let Ok(components) = platform.wasm_cloud.list_components().await {
                let _ = tx.send(TaskResult::WasmCloudComponentList(components));
            }
        });
    }

    #[allow(dead_code)]
    pub fn spawn_wasm_cloud_apps(&mut self) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            if let Ok(apps) = platform.wasm_cloud.list_apps().await {
                let _ = tx.send(TaskResult::WasmCloudAppList(apps));
            }
        });
    }

    pub fn spawn_inspect_component(&mut self, target: String) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            match platform.wasm_cloud.inspect_component(&target).await {
                Ok(output) => {
                    let _ = tx.send(TaskResult::WasmCloudInspect(output));
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::WasmCloudInspect(format!("Error: {}", e)));
                }
            }
        });
    }

    /// Auto-provision NATS: download if missing → write config → start sidecar → init KC buckets.
    pub fn spawn_nats_provision(&mut self) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        self.status_msg = Some("NATS: provisioning backbone…".to_string());
        tokio::spawn(async move {
            // 1. Auto-download nats-server if not found (spawn_blocking internally)
            if let Err(e) = platform.nats.auto_download().await {
                let _ = tx.send(TaskResult::Status(format!("NATS download failed: {}", e)));
                return;
            }

            // 2 + 3. Write config + start sidecar (spawn_blocking internally)
            if let Err(e) = platform.nats.start_async().await {
                let _ = tx.send(TaskResult::Status(format!("NATS start failed: {}", e)));
                return;
            }

            // 4. Give the server time to bind ports
            tokio::time::sleep(std::time::Duration::from_millis(1200)).await;

            // 5. Initialise JetStream buckets + streams (spawn_blocking internally)
            if let Err(e) = platform.nats.init_wasmcloud_buckets_async().await {
                let _ = tx.send(TaskResult::Status(format!(
                    "NATS bucket init failed: {}",
                    e
                )));
            }

            // 6. Emit health snapshot (TCP probe — fast, non-blocking)
            let nats = Arc::clone(&platform.nats);
            let (running, storage_usage, synced) = tokio::task::spawn_blocking(move || {
                let r = nats.is_running();
                let s = nats.get_storage_usage();
                let y = nats.is_synced();
                (r, s, y)
            })
            .await
            .unwrap_or((false, None, false));

            let _ = tx.send(TaskResult::WasmCloudNatsStatus {
                running,
                storage_usage,
                synced,
            });
            let _ = tx.send(TaskResult::Status(if running {
                "✅ NATS backbone is running (port 4222)".to_string()
            } else {
                "⚠️  NATS started but port 4222 not yet open — retry in a moment".to_string()
            }));
        });
    }

    /// Lightweight NATS health poll — TCP probe, fully non-blocking.
    pub fn spawn_poll_nats_status(&mut self) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            let nats = Arc::clone(&platform.nats);
            let (running, storage_usage, synced) = tokio::task::spawn_blocking(move || {
                let r = nats.is_running();
                let s = nats.get_storage_usage();
                let y = nats.is_synced();
                (r, s, y)
            })
            .await
            .unwrap_or((false, None, false));
            let _ = tx.send(TaskResult::WasmCloudNatsStatus {
                running,
                storage_usage,
                synced,
            });
        });
    }

    pub fn spawn_load_services(&mut self) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        self.services.loading = true;
        tokio::spawn(async move {
            match platform.services.list_services().await {
                Ok(services) => {
                    let _ = tx.send(TaskResult::ServiceList(services));
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(e.to_string()));
                }
            }
        });
    }

    pub fn spawn_load_swap(&mut self) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        self.swap.loading = true;
        tokio::spawn(async move {
            match platform.system.swap_status().await {
                Ok(status) => {
                    let _ = tx.send(TaskResult::SwapLoaded(status));
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(e.to_string()));
                }
            }
        });
    }

    pub fn spawn_swap_create(&mut self, path: String, size_mb: u64) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            match platform.system.swap_create(&path, size_mb).await {
                Ok(()) => {
                    let _ = tx.send(TaskResult::SwapOpDone { op: "create".into(), success: true });
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(e.to_string()));
                    let _ = tx.send(TaskResult::SwapOpDone { op: "create".into(), success: false });
                }
            }
        });
    }

    pub fn spawn_swap_delete(&mut self, path: String) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            match platform.system.swap_delete(&path).await {
                Ok(()) => {
                    let _ = tx.send(TaskResult::SwapOpDone { op: "delete".into(), success: true });
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(e.to_string()));
                    let _ = tx.send(TaskResult::SwapOpDone { op: "delete".into(), success: false });
                }
            }
        });
    }

    pub fn spawn_swap_enable(&mut self, path: String) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            match platform.system.swap_enable(&path).await {
                Ok(()) => {
                    let _ = tx.send(TaskResult::SwapOpDone { op: "enable".into(), success: true });
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(e.to_string()));
                    let _ = tx.send(TaskResult::SwapOpDone { op: "enable".into(), success: false });
                }
            }
        });
    }

    pub fn spawn_swap_disable(&mut self, path: String) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            match platform.system.swap_disable(&path).await {
                Ok(()) => {
                    let _ = tx.send(TaskResult::SwapOpDone { op: "disable".into(), success: true });
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(e.to_string()));
                    let _ = tx.send(TaskResult::SwapOpDone { op: "disable".into(), success: false });
                }
            }
        });
    }

    pub fn spawn_swap_resize(&mut self, path: String, size_mb: u64) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            match platform.system.swap_resize(&path, size_mb).await {
                Ok(()) => {
                    let _ = tx.send(TaskResult::SwapOpDone { op: "resize".into(), success: true });
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(e.to_string()));
                    let _ = tx.send(TaskResult::SwapOpDone { op: "resize".into(), success: false });
                }
            }
        });
    }

    // ── Storage spawn methods ────────────────────────────────────────────

    pub fn spawn_load_storage(&mut self) {
        let tx = self.task_tx.clone();
        self.storage.loading = true;
        self.storage.smart_loading = true;
        tokio::spawn(async move {
            match crate::core::storage::list_filesystems().await {
                Ok(devices) => {
                    let _ = tx.send(TaskResult::StorageLoaded(devices));
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(e.to_string()));
                }
            }
        });
        let tx2 = self.task_tx.clone();
        tokio::spawn(async move {
            match crate::core::storage::list_physical().await {
                Ok(physical) => {
                    let _ = tx2.send(TaskResult::SmartLoaded(physical));
                }
                Err(e) => {
                    let _ = tx2.send(TaskResult::Error(e.to_string()));
                }
            }
        });
    }

    pub fn spawn_storage_mount(&mut self, device: String, mountpoint: String) {
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            match crate::core::storage::mount(&device, &mountpoint).await {
                Ok(_) => {
                    let _ = tx.send(TaskResult::StorageOpDone { op: "mount".into(), success: true });
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(e.to_string()));
                    let _ = tx.send(TaskResult::StorageOpDone { op: "mount".into(), success: false });
                }
            }
        });
    }

    pub fn spawn_storage_umount(&mut self, target: String) {
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            match crate::core::storage::umount(&target).await {
                Ok(_) => {
                    let _ = tx.send(TaskResult::StorageOpDone { op: "umount".into(), success: true });
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(e.to_string()));
                    let _ = tx.send(TaskResult::StorageOpDone { op: "umount".into(), success: false });
                }
            }
        });
    }

    pub fn spawn_storage_read_fstab(&mut self) {
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            match crate::core::storage::read_fstab().await {
                Ok(content) => {
                    let _ = tx.send(TaskResult::StorageFstabLoaded(content));
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(e.to_string()));
                }
            }
        });
    }

    // ── Updates spawn methods ────────────────────────────────────────────

    pub fn spawn_load_updates(&mut self) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        self.packages.updates_loading = true;
        tokio::spawn(async move {
            match platform.packages.list_upgradable().await {
                Ok(list) => {
                    let _ = tx.send(TaskResult::UpdatesList(list));
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(e.to_string()));
                }
            }
        });
    }

    pub fn spawn_upgrade_all(&mut self) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        let pool = self.pool.clone();
        self.packages.queue.push_back(QueuedOp {
            kind: "upgrade".to_string(),
            target: "all".to_string(),
            status: OpStatus::Running,
            output: String::new(),
        });
        if self.packages.queue_selected.is_none() {
            self.packages.queue_selected = Some(self.packages.queue.len() - 1);
            self.packages.output_scroll = 0;
        }
        tokio::spawn(async move {
            let result = platform.packages.upgrade_all().await;
            let (output, success) = match result {
                Ok(out) => (out, true),
                Err(e) => (e.to_string(), false),
            };
            let _ =
                crate::db::audit::log_action(&pool, "upgrade", Some("all"), &output, success).await;
            let _ = tx.send(TaskResult::OpDone {
                op: "upgrade".to_string(),
                target: "all".to_string(),
                output,
                success,
            });
            let _ = tx.send(TaskResult::UpdatesOpDone { success });
        });
    }

    // ── Projects spawn methods ────────────────────────────────────────────

    pub fn spawn_projects_load_dir(&mut self) {
        let tx = self.task_tx.clone();
        let pool = self.pool.clone();
        tokio::spawn(async move {
            let row = sqlx::query_as::<_, (String,)>(
                "SELECT value FROM projects_config WHERE key = 'projects_dir'",
            )
            .fetch_optional(&pool)
            .await;
            let dir = match row {
                Ok(Some((v,))) => v,
                _ => "~/projects".to_string(),
            };
            let _ = tx.send(TaskResult::ProjectsDirLoaded(dir));
        });
    }

    pub fn spawn_projects_save_dir(&mut self, dir: String) {
        let tx = self.task_tx.clone();
        let pool = self.pool.clone();
        self.projects.dir = dir.clone();
        self.projects.dir_input = dir.clone();
        tokio::spawn(async move {
            let _ = sqlx::query(
                "INSERT OR REPLACE INTO projects_config (key, value) VALUES ('projects_dir', ?)",
            )
            .bind(&dir)
            .execute(&pool)
            .await;
            let _ = tx.send(TaskResult::Status("Projects directory saved".to_string()));
        });
    }

    pub fn spawn_projects_load_git(&mut self) {
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            let status = crate::core::projects::ProjectsManager.git_status().await;
            let _ = tx.send(TaskResult::ProjectsGitLoaded(status));
        });
    }

    pub fn spawn_projects_save_git_identity(&mut self, name: String, email: String) {
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            let ok = crate::core::projects::ProjectsManager
                .set_git_identity(&name, &email)
                .await
                .is_ok();
            let _ = tx.send(TaskResult::ProjectsGitSaved {
                what: "identity".to_string(),
                success: ok,
            });
        });
    }

    pub fn spawn_projects_save_github_token(&mut self, token: String) {
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            let ok = crate::core::projects::ProjectsManager
                .set_github_token(&token)
                .await
                .is_ok();
            let _ = tx.send(TaskResult::ProjectsGitSaved {
                what: "token".to_string(),
                success: ok,
            });
        });
    }

    pub fn spawn_load_projects(&mut self) {
        let tx = self.task_tx.clone();
        let dir = self.projects.dir.clone();
        self.projects.loading = true;
        tokio::spawn(async move {
            let mgr = crate::core::projects::ProjectsManager;
            match mgr.list(&dir).await {
                Ok(list) => {
                    let _ = tx.send(TaskResult::ProjectsList(list));
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(e.to_string()));
                }
            }
        });
    }

    pub fn spawn_projects_scaffold(&mut self, name: String) {
        let tx = self.task_tx.clone();
        let dir = self.projects.dir.clone();
        let git_flag = if BTS_GIT[self.projects.new_git_idx] == "yes" { "--git" } else { "--no-git" };
        let addons_flags: String = BTS_ADDONS
            .iter()
            .zip(self.projects.new_addons_selected.iter())
            .filter_map(|(name, &sel)| if sel { Some(format!("--addons {}", name)) } else { None })
            .collect::<Vec<_>>()
            .join(" ");
        let flags = format!(
            "--frontend {} --database {} --orm {} --auth {} --backend {} --api {} --runtime {} --payments {} --examples {} --db-setup none {} --web-deploy {} --server-deploy {} {}",
            BTS_FRONTENDS[self.projects.new_frontend_idx],
            BTS_DATABASES[self.projects.new_database_idx],
            BTS_ORMS[self.projects.new_orm_idx],
            BTS_AUTHS[self.projects.new_auth_idx],
            BTS_BACKENDS[self.projects.new_backend_idx],
            BTS_APIS[self.projects.new_api_idx],
            BTS_RUNTIMES[self.projects.new_runtime_idx],
            BTS_PAYMENTS[self.projects.new_payments_idx],
            BTS_EXAMPLES[self.projects.new_examples_idx],
            git_flag,
            BTS_WEB_DEPLOY[self.projects.new_web_deploy_idx],
            BTS_SERVER_DEPLOY[self.projects.new_server_deploy_idx],
            addons_flags,
        );
        self.projects.new_running = true;
        self.projects.new_output.clear();
        self.projects.new_output_scroll = 0;
        tokio::spawn(async move {
            let mgr = crate::core::projects::ProjectsManager;
            let (ptx, mut prx) = tokio::sync::mpsc::unbounded_channel::<String>();
            let tx_fwd = tx.clone();
            let fwd = tokio::spawn(async move {
                while let Some(line) = prx.recv().await {
                    let _ = tx_fwd.send(TaskResult::ProjectsOpProgress {
                        op: "new".to_string(),
                        line,
                    });
                }
            });
            let result = mgr.scaffold_new(&name, &dir, &flags, ptx).await;
            let _ = fwd.await;
            let success = result.is_ok();
            if let Err(ref e) = result {
                let _ = tx.send(TaskResult::ProjectsOpProgress {
                    op: "new".to_string(),
                    line: format!("Error: {}", e),
                });
            }
            let _ = tx.send(TaskResult::ProjectsOpDone {
                op: "new".to_string(),
                name,
                success,
            });
        });
    }

    pub fn spawn_projects_clone(&mut self, url: String) {
        let tx = self.task_tx.clone();
        let dir = self.projects.dir.clone();
        self.projects.clone_running = true;
        self.projects.clone_output.clear();
        self.projects.clone_output_scroll = 0;
        tokio::spawn(async move {
            let mgr = crate::core::projects::ProjectsManager;
            let (ptx, mut prx) = tokio::sync::mpsc::unbounded_channel::<String>();
            let tx_fwd = tx.clone();
            let fwd = tokio::spawn(async move {
                while let Some(line) = prx.recv().await {
                    let _ = tx_fwd.send(TaskResult::ProjectsOpProgress {
                        op: "clone".to_string(),
                        line,
                    });
                }
            });
            let result = mgr.clone_repo(&url, &dir, ptx).await;
            let _ = fwd.await;
            let (name, success) = match result {
                Ok(n) => (n, true),
                Err(e) => {
                    let _ = tx.send(TaskResult::ProjectsOpProgress {
                        op: "clone".to_string(),
                        line: format!("Error: {}", e),
                    });
                    (String::new(), false)
                }
            };
            let _ = tx.send(TaskResult::ProjectsOpDone {
                op: "clone".to_string(),
                name,
                success,
            });
        });
    }

    pub fn spawn_projects_pull(&mut self, path: String, name: String) {
        let tx = self.task_tx.clone();
        self.status_msg = Some(format!("Pulling '{}'…", name));
        tokio::spawn(async move {
            let mgr = crate::core::projects::ProjectsManager;
            let (ptx, mut prx) = tokio::sync::mpsc::unbounded_channel::<String>();
            let tx_fwd = tx.clone();
            let fwd = tokio::spawn(async move {
                while let Some(line) = prx.recv().await {
                    let _ = tx_fwd.send(TaskResult::ProjectsOpProgress {
                        op: "pull".to_string(),
                        line,
                    });
                }
            });
            let result = mgr.pull_project(&path, ptx).await;
            let _ = fwd.await;
            let success = result.is_ok();
            if let Err(ref e) = result {
                let _ = tx.send(TaskResult::ProjectsOpProgress {
                    op: "pull".to_string(),
                    line: format!("Error: {}", e),
                });
            }
            let _ = tx.send(TaskResult::ProjectsOpDone {
                op: "pull".to_string(),
                name,
                success,
            });
        });
    }

    pub fn spawn_service_action(&mut self, name: String, op: String) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            let result = match op.as_str() {
                "start" => platform.services.start(&name).await,
                "stop" => platform.services.stop(&name).await,
                "restart" => platform.services.restart(&name).await,
                "enable" => platform.services.enable(&name).await,
                "disable" => platform.services.disable(&name).await,
                _ => Ok(()),
            };
            match result {
                Ok(()) => {
                    let _ = tx.send(TaskResult::ServiceOpDone {
                        name,
                        op,
                        success: true,
                    });
                }
                Err(e) => {
                    let _ = tx.send(TaskResult::Error(format!("service {} failed: {}", op, e)));
                }
            }
        });
    }

    pub fn spawn_maintenance_action(&mut self, op: String) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        let pool = self.pool.clone();
        self.maintenance.running_op = Some(op.clone());
        tokio::spawn(async move {
            let result = match op.as_str() {
                "clean_pkg_cache" => platform.packages.clean_cache().await,
                _ => Ok("Unknown action".to_string()),
            };
            let (output, success) = match result {
                Ok(out) => (out, true),
                Err(e) => (e.to_string(), false),
            };
            let _ = crate::db::audit::log_action(&pool, "maintenance", Some(&op), &output, success)
                .await;
            let _ = tx.send(TaskResult::MaintenanceDone {
                op,
                output,
                success,
            });
        });
    }

    // ── Pi Agent spawn methods ────────────────────────────────────────────

    pub fn spawn_load_pi_agent_status(&mut self) {
        let tx = self.task_tx.clone();
        self.agent.loading = true;
        tokio::spawn(async move {
            let info = crate::core::pi_agent::get_info().await;
            let _ = tx.send(TaskResult::PiAgentInfo(info));
        });
    }

    pub fn spawn_load_pi_agent_sessions(&mut self) {
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            let sessions = crate::core::pi_agent::list_sessions().await;
            let _ = tx.send(TaskResult::PiAgentSessions(sessions));
        });
    }

    pub fn spawn_load_pi_agent_config(&mut self) {
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            let config = crate::core::pi_agent::get_config().await;
            let _ = tx.send(TaskResult::PiAgentConfig(config));
        });
    }

    pub fn spawn_load_pi_agent_auth(&mut self) {
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            let entries = crate::core::pi_agent::get_auth().await;
            let _ = tx.send(TaskResult::PiAgentAuth(entries));
        });
    }

    pub fn spawn_load_pi_agent_skills(&mut self) {
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            let skills = crate::core::pi_agent::list_skills().await;
            let _ = tx.send(TaskResult::PiAgentSkills(skills));
        });
    }

    pub fn spawn_pi_agent_remove_skill(&mut self, name: String) {
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            let success = crate::core::pi_agent::remove_skill(&name).await.is_ok();
            let _ = tx.send(TaskResult::PiAgentSkillRemoved { name, success });
        });
    }

    pub fn spawn_load_pi_agent_library(&mut self) {
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            let skills = crate::core::pi_agent::list_library_skills().await;
            let _ = tx.send(TaskResult::PiAgentLibrarySkills(skills));
        });
    }

    pub fn spawn_pi_agent_install_skill(&mut self, name: String) {
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            let success = crate::core::pi_agent::install_library_skill(&name).await.is_ok();
            let _ = tx.send(TaskResult::PiAgentLibraryInstall { name, success });
        });
    }

    pub fn spawn_load_pi_agent_logs(&mut self) {
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            let lines = crate::core::pi_agent::get_logs(200).await;
            let _ = tx.send(TaskResult::PiAgentLogs(lines));
        });
    }

    pub fn spawn_pi_agent_action(&mut self, action: &'static str) {
        let tx = self.task_tx.clone();
        let pool = self.pool.clone();
        tokio::spawn(async move {
            let result: anyhow::Result<String> = match action {
                "update_check" => crate::core::pi_agent::update_check().await,
                "update_apply" => crate::core::pi_agent::update_apply().await,
                _ => Ok("Unknown action".to_string()),
            };
            let (output, success) = match result {
                Ok(o) => (o, true),
                Err(e) => (e.to_string(), false),
            };
            let _ =
                crate::db::audit::log_action(&pool, "pi-agent", Some(action), &output, success)
                    .await;
            let _ = tx.send(TaskResult::PiAgentActionDone {
                action: action.to_string(),
                output,
                success,
            });
        });
    }

    pub fn spawn_pi_agent_install(&mut self) {
        let tx = self.task_tx.clone();
        self.agent.installing = true;
        self.agent.install_log.clear();
        tokio::spawn(async move {
            let (ptx, mut prx) = tokio::sync::mpsc::unbounded_channel::<String>();
            let tx_fwd = tx.clone();
            let fwd = tokio::spawn(async move {
                while let Some(line) = prx.recv().await {
                    let _ = tx_fwd.send(TaskResult::PiAgentInstallProgress(line));
                }
            });
            let result = crate::core::pi_agent::install_pi(ptx).await;
            let _ = fwd.await;
            let (output, success) = match result {
                Ok(o) => (o, true),
                Err(e) => (e.to_string(), false),
            };
            let _ = tx.send(TaskResult::PiAgentInstallDone { output, success });
        });
    }

    // ── Agent RPC methods ─────────────────────────────────────────────────

    pub fn spawn_start_agent_rpc(&mut self) {
        if self.agent.rpc_active {
            return;
        }
        self.agent.status = "Connecting…".to_string();
        let task_tx = self.task_tx.clone();
        tokio::spawn(async move {
            let (event_tx, mut event_rx) =
                tokio::sync::mpsc::unbounded_channel::<crate::core::pi_agent::rpc::PiRpcEvent>();

            // Bridge: PiRpcEvent → TaskResult
            let task_tx2 = task_tx.clone();
            tokio::spawn(async move {
                use crate::core::pi_agent::rpc::PiRpcEvent;
                while let Some(ev) = event_rx.recv().await {
                    let r = match ev {
                        PiRpcEvent::AgentStart => TaskResult::PiAgentRpcStarted,
                        PiRpcEvent::TextDelta(d) => TaskResult::PiAgentTextDelta(d),
                        PiRpcEvent::AgentEnd => TaskResult::PiAgentAgentEnd,
                        PiRpcEvent::ToolStart(n) => TaskResult::PiAgentToolStart(n),
                        PiRpcEvent::ToolEnd { name, is_error } => {
                            TaskResult::PiAgentToolEnd { name, is_error }
                        }
                        PiRpcEvent::Error(e) => TaskResult::PiAgentRpcError(e),
                        PiRpcEvent::Stderr(line) => TaskResult::PiAgentRpcStderr(line),
                        PiRpcEvent::Stopped => {
                            let _ = task_tx2.send(TaskResult::PiAgentRpcStopped);
                            break;
                        }
                    };
                    if task_tx2.send(r).is_err() {
                        break;
                    }
                }
            });

            let (provider, model) = crate::core::pi_agent::default_provider_model().await;
            match crate::core::pi_agent::rpc::spawn_rpc(&provider, &model, event_tx).await {
                Ok(handle) => {
                    let _ = task_tx.send(TaskResult::PiAgentRpcConnected(handle));
                }
                Err(e) => {
                    let _ = task_tx.send(TaskResult::PiAgentRpcError(e.to_string()));
                }
            }
        });
    }

    pub fn send_agent_prompt(&mut self, text: String) {
        self.agent.messages.push(AgentMessage {
            role: AgentRole::User,
            content: text.clone(),
        });
        if let Some(ref handle) = self.agent.rpc_handle {
            let cmd = serde_json::json!({"type": "prompt", "message": text});
            let _ = handle.cmd_tx.send(cmd);
            self.agent.streaming = true;
            self.agent.status = "Streaming…".to_string();
        } else {
            self.agent.messages.push(AgentMessage {
                role: AgentRole::Tool,
                content: "Not connected — press [s] to start a session".to_string(),
            });
        }
    }

    pub fn stop_agent_rpc(&mut self) {
        self.agent.rpc_handle = None;
        self.agent.rpc_active = false;
        self.agent.streaming = false;
        self.agent.status = "Disconnected".to_string();
    }

    // ── Agent task CRUD ───────────────────────────────────────────────────

    pub fn spawn_load_agent_tasks(&mut self) {
        if self.agent.tasks_loading {
            return;
        }
        self.agent.tasks_loading = true;
        let pool = self.pool.clone();
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            let tasks = crate::db::agent_tasks::list_tasks(&pool)
                .await
                .unwrap_or_default();
            let _ = tx.send(TaskResult::PiAgentTasks(tasks));
        });
    }

    pub fn spawn_create_agent_task(&mut self, name: String, prompt: String, schedule: String) {
        let pool = self.pool.clone();
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            let _ = crate::db::agent_tasks::create_task(&pool, &name, &prompt, &schedule).await;
            let tasks = crate::db::agent_tasks::list_tasks(&pool)
                .await
                .unwrap_or_default();
            let _ = tx.send(TaskResult::PiAgentTaskCreated);
            let _ = tx.send(TaskResult::PiAgentTasks(tasks));
        });
    }

    pub fn spawn_delete_agent_task(&mut self, id: i64) {
        let pool = self.pool.clone();
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            let _ = crate::db::agent_tasks::delete_task(&pool, id).await;
            let tasks = crate::db::agent_tasks::list_tasks(&pool)
                .await
                .unwrap_or_default();
            let _ = tx.send(TaskResult::PiAgentTaskDeleted);
            let _ = tx.send(TaskResult::PiAgentTasks(tasks));
        });
    }

    pub fn spawn_toggle_agent_task(&mut self, id: i64, enabled: bool) {
        let pool = self.pool.clone();
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            let _ = crate::db::agent_tasks::toggle_task(&pool, id, enabled).await;
            let tasks = crate::db::agent_tasks::list_tasks(&pool)
                .await
                .unwrap_or_default();
            let _ = tx.send(TaskResult::PiAgentTaskToggled);
            let _ = tx.send(TaskResult::PiAgentTasks(tasks));
        });
    }

    // ── Scheduler ─────────────────────────────────────────────────────────

    pub async fn check_scheduled_tasks(&mut self) {
        if !self.agent.rpc_active || self.agent.streaming || self.agent.tasks.is_empty() {
            return;
        }
        let now = chrono::Utc::now().timestamp();

        let due = self
            .agent
            .tasks
            .iter()
            .find(|t| {
                if !t.enabled {
                    return false;
                }
                let interval = crate::db::agent_tasks::schedule_secs(&t.schedule);
                match t.last_run_at {
                    None => true,
                    Some(last) => now - last >= interval,
                }
            })
            .map(|t| (t.id, t.name.clone(), t.prompt.clone()));

        if let Some((id, name, prompt)) = due {
            let full_prompt = format!("[Scheduled: {}]\n{}", name, prompt);
            self.send_agent_prompt(full_prompt);

            // Optimistic in-memory update
            if let Some(t) = self.agent.tasks.iter_mut().find(|t| t.id == id) {
                t.last_run_at = Some(now);
            }

            let pool = self.pool.clone();
            let tx = self.task_tx.clone();
            tokio::spawn(async move {
                let _ = crate::db::agent_tasks::mark_run(&pool, id, "sent", true).await;
                let tasks = crate::db::agent_tasks::list_tasks(&pool)
                    .await
                    .unwrap_or_default();
                let _ = tx.send(TaskResult::PiAgentTasks(tasks));
            });
        }
    }

    // ── Context overlay ───────────────────────────────────────────────────

    pub fn open_agent_overlay(&mut self) {
        let (label, context, question) = self.build_overlay_context();
        self.overlay.open = true;
        self.overlay.context_label = label;
        self.overlay.context_body = context;
        self.overlay.question = question;
    }

    pub fn close_agent_overlay(&mut self) {
        self.overlay.open = false;
        self.overlay.question.clear();
        self.overlay.context_body.clear();
        self.overlay.context_label.clear();
    }

    pub fn send_overlay_prompt(&mut self) {
        let prompt = if self.overlay.context_body.is_empty() {
            self.overlay.question.trim().to_string()
        } else {
            format!(
                "Context ({}):\n{}\n\nQuestion: {}",
                self.overlay.context_label,
                self.overlay.context_body,
                self.overlay.question.trim()
            )
        };
        self.close_agent_overlay();
        if self.agent.rpc_active {
            self.send_agent_prompt(prompt);
        } else {
            self.agent.pending_prompt = Some(prompt);
            self.spawn_start_agent_rpc();
        }
        self.set_screen(Screen::Agent);
    }

    fn build_overlay_context(&self) -> (String, String, String) {
        match &self.screen {
            Screen::Dashboard => {
                let mut lines = Vec::new();
                if let Some(info) = &self.dashboard.os_info {
                    let h = info.uptime_secs / 3600;
                    let m = (info.uptime_secs % 3600) / 60;
                    lines.push(format!("Host: {}  OS: {}", info.hostname, info.distro));
                    lines.push(format!("Uptime: {}h {}m  CPUs: {}", h, m, info.cpu_count));
                }
                if !self.dashboard.cpu_pct.is_empty() {
                    let avg = self.dashboard.cpu_pct.iter().sum::<f32>()
                        / self.dashboard.cpu_pct.len() as f32;
                    lines.push(format!("CPU: {:.1}%", avg));
                }
                if let Some(mem) = &self.dashboard.mem {
                    let pct = (mem.used * 100).checked_div(mem.total).unwrap_or(0);
                    lines.push(format!(
                        "Memory: {}% ({:.1}/{:.1} GB)",
                        pct,
                        mem.used as f64 / 1_073_741_824.0,
                        mem.total as f64 / 1_073_741_824.0
                    ));
                }
                for disk in &self.dashboard.disks {
                    let pct = (disk.used * 100).checked_div(disk.total).unwrap_or(0);
                    lines.push(format!(
                        "Disk {}: {}% ({:.1}/{:.1} GB)",
                        disk.mount,
                        pct,
                        disk.used as f64 / 1_073_741_824.0,
                        disk.total as f64 / 1_073_741_824.0
                    ));
                }
                let ctx = if lines.is_empty() {
                    "System data loading…".to_string()
                } else {
                    lines.join("\n")
                };
                (
                    "Dashboard".to_string(),
                    ctx,
                    "Analyze my system health and flag anything concerning.".to_string(),
                )
            }
            Screen::Security => {
                let mut lines = Vec::new();
                lines.push(format!(
                    "Findings: {} total",
                    self.security.findings.len()
                ));
                for f in self.security.findings.iter().take(6) {
                    lines.push(format!("  [{}] {}", f.severity.label(), f.title));
                }
                if self.security.findings.len() > 6 {
                    lines.push(format!(
                        "  … and {} more",
                        self.security.findings.len() - 6
                    ));
                }
                if let Some(fw) = self.firewall.enabled {
                    lines.push(format!(
                        "Firewall: {} ({})",
                        if fw { "enabled" } else { "disabled" },
                        self.firewall.backend
                    ));
                }
                let ctx = lines.join("\n");
                (
                    "Security".to_string(),
                    ctx,
                    "Review my security posture and recommend fixes for the most critical issues."
                        .to_string(),
                )
            }
            Screen::Docker => {
                let mut lines = Vec::new();
                let running = self
                    .docker
                    .containers
                    .iter()
                    .filter(|c| c.status.contains("Up"))
                    .count();
                lines.push(format!(
                    "Containers: {} running / {} total",
                    running,
                    self.docker.containers.len()
                ));
                for c in self.docker.containers.iter().take(6) {
                    lines.push(format!("  {} — {} ({})", c.name, c.status, c.image));
                }
                if self.docker.containers.len() > 6 {
                    lines.push(format!("  … and {} more", self.docker.containers.len() - 6));
                }
                let ctx = if lines.is_empty() {
                    "No containers loaded yet.".to_string()
                } else {
                    lines.join("\n")
                };
                (
                    "Docker".to_string(),
                    ctx,
                    "Review my container setup and flag any issues or suggest improvements."
                        .to_string(),
                )
            }
            Screen::Networking => {
                let mut lines = Vec::new();
                let gw = if self.gateway.installed {
                    self.gateway.version.as_deref().unwrap_or("installed")
                } else {
                    "not installed"
                };
                lines.push(format!("Caddy: {} ({} routes)", gw, self.gateway.routes.len()));
                for r in self.gateway.routes.iter().take(5) {
                    lines.push(format!("  {} → :{}", r.domain, r.port));
                }
                lines.push(format!("Tunnels: {}", self.tunnel.tunnels.len()));
                (
                    "Networking".to_string(),
                    lines.join("\n"),
                    "Review my network config and suggest improvements.".to_string(),
                )
            }
            Screen::System => {
                let failed: Vec<_> = self
                    .services
                    .list
                    .iter()
                    .filter(|s| s.sub_state == "failed")
                    .collect();
                let mut lines = vec![format!(
                    "Services: {} total ({} failed)",
                    self.services.list.len(),
                    failed.len()
                )];
                for s in failed.iter().take(4) {
                    lines.push(format!("  ✗ {}", s.name));
                }
                lines.push(format!("Users: {}", self.users.users.len()));
                (
                    "System".to_string(),
                    lines.join("\n"),
                    "Review my services and flag any failures or concerns.".to_string(),
                )
            }
            Screen::Packages => (
                "Packages".to_string(),
                format!("Installed packages: {}", self.packages.installed.len()),
                "What packages should I check or update for security?".to_string(),
            ),
            _ => (
                self.screen.title().trim_start_matches(|c: char| c.is_ascii_digit() || c == '.').trim().to_string(),
                String::new(),
                "What can you help me with on this server?".to_string(),
            ),
        }
    }

    pub fn drain_task_results(&mut self) {
        while let Ok(result) = self.task_rx.try_recv() {
            self.handle_result(result);
        }
    }

    fn handle_result(&mut self, result: TaskResult) {
        match result {
            TaskResult::PackageList(pkgs) => {
                self.packages.installed = pkgs;
                if self.packages.installed_state.selected().is_none()
                    && !self.packages.installed.is_empty()
                {
                    self.packages.installed_state.select(Some(0));
                }
            }
            TaskResult::PackagesUpdated(updated) => {
                // Merge: update existing entries or append new ones
                for pkg in updated {
                    if let Some(existing) = self
                        .packages
                        .installed
                        .iter_mut()
                        .find(|p| p.name == pkg.name)
                    {
                        *existing = pkg;
                    } else {
                        self.packages.installed.push(pkg);
                    }
                }
                if self.packages.installed_state.selected().is_none()
                    && !self.packages.installed.is_empty()
                {
                    self.packages.installed_state.select(Some(0));
                }
            }
            TaskResult::SearchResults(pkgs) => {
                self.packages.search_results = pkgs;
                if !self.packages.search_results.is_empty() {
                    self.packages.search_state.select(Some(0));
                }
            }
            TaskResult::OpProgress { op, target, line } => {
                if let Some(entry) = self
                    .packages
                    .queue
                    .iter_mut()
                    .find(|e| e.target == target && e.kind == op)
                {
                    if !entry.output.is_empty() {
                        entry.output.push('\n');
                    }
                    entry.output.push_str(&line);
                }
            }
            TaskResult::OpDone {
                op,
                target,
                output,
                success,
            } => {
                if let Some(entry) = self
                    .packages
                    .queue
                    .iter_mut()
                    .find(|e| e.target == target && e.kind == op)
                {
                    entry.status = if success {
                        OpStatus::Done
                    } else {
                        OpStatus::Failed
                    };
                    // If no streaming happened (non-apt managers), populate output now
                    if entry.output.is_empty() {
                        entry.output = output.clone();
                    } else if !success && !output.is_empty() {
                        // Append final error summary if not already streamed
                        if !entry.output.contains(&output) {
                            entry.output.push('\n');
                            entry.output.push_str(&output);
                        }
                    }
                }
                self.status_msg = Some(if success {
                    format!("{} {} — done", op, target)
                } else {
                    format!("{} {} — FAILED", op, target)
                });
                if success {
                    match op.as_str() {
                        "remove" => {
                            // Instant: splice the removed package out of the installed list
                            self.packages.installed.retain(|p| p.name != target);
                        }
                        _ => {
                            // Targeted check: only query the packages that just changed
                            self.spawn_check_packages(vec![target]);
                        }
                    }
                }
            }
            TaskResult::ProcessList(procs) => {
                self.processes.list = procs;
                if self.processes.table_state.selected().is_none()
                    && !self.processes.list.is_empty()
                {
                    self.processes.table_state.select(Some(0));
                }
            }
            TaskResult::SecurityScan(findings) => {
                self.security.findings = findings;
                self.security.scanning = false;
                self.security.last_scan = Some(std::time::SystemTime::now());
                if !self.security.findings.is_empty() {
                    self.security.list_state.select(Some(0));
                }
            }
            TaskResult::SecurityApply {
                id,
                output,
                success,
            } => {
                self.status_msg = Some(if success {
                    format!("Applied fix {} — {}", id, output)
                } else {
                    format!("Fix {} failed: {}", id, output)
                });
                self.security.output = Some(output);
                // Re-scan after applying
                self.spawn_security_scan();
            }
            TaskResult::Fail2BanList(jailed) => {
                self.security.f2b_loading = false;
                self.security.f2b_installed = true;
                self.security.jailed = jailed;
                if self.security.jailed_state.selected().is_none()
                    && !self.security.jailed.is_empty()
                {
                    self.security.jailed_state.select(Some(0));
                }
            }
            TaskResult::Fail2BanActionDone {
                ip,
                jail,
                action,
                success,
            } => {
                self.status_msg = Some(if success {
                    format!("{} {} from {} — done", action, ip, jail)
                } else {
                    format!("Failed to {} {} from {}", action, ip, jail)
                });
                // Refresh the jailed list after an action
                self.spawn_fail2ban_list();
            }
            TaskResult::RouteList(routes) => {
                self.gateway.routes = routes;
                if !self.gateway.routes.is_empty() {
                    self.gateway.table_state.select(Some(0));
                }
            }
            TaskResult::TunnelList(tunnels) => {
                self.tunnel.tunnels = tunnels;
                if self.tunnel.tunnels.is_empty() {
                    self.tunnel.table_state.select(None);
                } else if self.tunnel.table_state.selected().is_none() {
                    self.tunnel.table_state.select(Some(0));
                }
                // Clear any "loading…" status message once the list arrives.
                self.status_msg = None;
            }
            TaskResult::TunnelCreated(t) => {
                self.tunnel.tunnels.push(t);
                self.status_msg = Some("Tunnel created".to_string());
            }
            TaskResult::GatewayStatus { installed, version } => {
                self.gateway.installed = installed;
                self.gateway.version = version;
            }
            TaskResult::TunnelConfigContent(content) => {
                // Auto-detect active tunnel from the tunnel: field in config.yaml
                if self.tunnel.active_tunnel_id.is_none() {
                    if let Some(id) = content
                        .lines()
                        .find(|l| l.trim_start().starts_with("tunnel:"))
                        .and_then(|l| l.split_once(':').map(|x| x.1))
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                    {
                        self.tunnel.active_tunnel_id = Some(id);
                    }
                }
                self.tunnel.ingress_entries = parse_ingress_entries(&content);
                if !self.tunnel.ingress_entries.is_empty()
                    && self.tunnel.ingress_state.selected().is_none()
                {
                    self.tunnel.ingress_state.select(Some(0));
                }
                self.tunnel.config_content = Some(content);
            }
            TaskResult::TunnelServiceStatus { active, enabled } => {
                self.tunnel.service_active = Some(active);
                self.tunnel.service_enabled = Some(enabled);
            }
            TaskResult::TailscaleStatus {
                installed,
                version,
                backend_state,
                self_ip,
                self_name,
                peers,
            } => {
                self.tailscale.installed = installed;
                self.tailscale.version = version;
                self.tailscale.backend_state = backend_state;
                self.tailscale.self_ip = self_ip;
                self.tailscale.self_name = self_name;
                self.tailscale.peers = peers;
                self.tailscale.loading = false;
                if self.tailscale.peers_state.selected().is_none()
                    && !self.tailscale.peers.is_empty()
                {
                    self.tailscale.peers_state.select(Some(0));
                }
            }
            TaskResult::TunnelStatus { installed, version } => {
                self.tunnel.installed = installed;
                self.tunnel.version = version;
            }
            TaskResult::InstallProgress { target: _, line } => {
                // Stream live install output into the status bar
                self.status_msg = Some(line);
            }
            TaskResult::InstallDone { target, success } => {
                self.status_msg = Some(if success {
                    format!("{} installed successfully", target)
                } else {
                    format!("{} installation failed — see error above", target)
                });
            }
            TaskResult::DockerStatus { installed, version } => {
                self.docker.installed = installed;
                self.docker.version = version;
                self.docker.loading = false;
            }
            TaskResult::DockerContainerList(containers) => {
                self.docker.containers = containers;
                if self.docker.containers_state.selected().is_none()
                    && !self.docker.containers.is_empty()
                {
                    self.docker.containers_state.select(Some(0));
                }
            }
            TaskResult::DockerImageList(images) => {
                self.docker.images = images;
                if self.docker.images_state.selected().is_none() && !self.docker.images.is_empty() {
                    self.docker.images_state.select(Some(0));
                }
            }
            TaskResult::DockerComposeList(services) => {
                self.docker.compose_services = services;
                if self.docker.compose_state.selected().is_none()
                    && !self.docker.compose_services.is_empty()
                {
                    self.docker.compose_state.select(Some(0));
                }
            }
            TaskResult::WorkloadCapabilities(capabilities) => {
                self.docker.workloads.capabilities = Some(capabilities);
            }
            TaskResult::WorkloadList(workloads) => {
                self.docker.workloads.workloads = workloads;
                if self.docker.workloads.workloads.is_empty() {
                    self.docker.workloads.table_state.select(None);
                } else if self.docker.workloads.table_state.selected().is_none() {
                    self.docker.workloads.table_state.select(Some(0));
                } else if let Some(selected) = self.docker.workloads.table_state.selected() {
                    let last = self.docker.workloads.workloads.len().saturating_sub(1);
                    self.docker
                        .workloads
                        .table_state
                        .select(Some(selected.min(last)));
                }
            }
            TaskResult::ManagedServiceList(services) => {
                self.docker.managed_services = services;
                if self.docker.managed_state.selected().is_none()
                    && !self.docker.managed_services.is_empty()
                {
                    self.docker.managed_state.select(Some(0));
                }
            }
            TaskResult::FirewallStatus { enabled, backend } => {
                self.firewall.enabled = Some(enabled);
                self.firewall.backend = backend;
            }
            TaskResult::FirewallRules(rules) => {
                self.firewall.rules = rules;
                if self.firewall.table_state.selected().is_none() && !self.firewall.rules.is_empty()
                {
                    self.firewall.table_state.select(Some(0));
                }
            }
            TaskResult::PublicIp(ip) => {
                self.portchecker.ip_loading = false;
                self.portchecker.public_ip = Some(ip);
            }
            TaskResult::PortCheckDone { results } => {
                self.portchecker.checking = false;
                for (port, status) in results {
                    if let Some(entry) =
                        self.portchecker.entries.iter_mut().find(|e| e.port == port)
                    {
                        entry.status = status;
                    }
                }
            }
            TaskResult::SshLocalKeys(keys) => {
                self.ssh.local_keys = keys;
                self.ssh.loading = false;
                if self.ssh.local_state.selected().is_none() && !self.ssh.local_keys.is_empty() {
                    self.ssh.local_state.select(Some(0));
                }
            }
            TaskResult::SshAuthorizedKeys(keys) => {
                self.ssh.authorized_keys = keys;
                self.ssh.loading = false;
                if self.ssh.authorized_state.selected().is_none()
                    && !self.ssh.authorized_keys.is_empty()
                {
                    self.ssh.authorized_state.select(Some(0));
                }
            }
            TaskResult::SshOpDone {
                op,
                success,
                output,
            } => {
                self.status_msg = Some(if success {
                    if op == "generate" {
                        format!("Key generated: {}", output)
                    } else {
                        format!("SSH {} done", op)
                    }
                } else {
                    format!("SSH {} FAILED: {}", op, output)
                });
                self.spawn_load_ssh();
            }
            TaskResult::Status(msg) => {
                self.status_msg = Some(msg);
            }
            TaskResult::WasmCloudStatus { installed, version } => {
                self.wasm_cloud.installed = installed;
                self.wasm_cloud.version = version;
                self.wasm_cloud.loading = false;
            }
            TaskResult::WasmCloudHostList(hosts) => {
                self.wasm_cloud.hosts = hosts;
                if self.wasm_cloud.hosts_state.selected().is_none()
                    && !self.wasm_cloud.hosts.is_empty()
                {
                    self.wasm_cloud.hosts_state.select(Some(0));
                }
            }
            TaskResult::WasmCloudComponentList(components) => {
                self.wasm_cloud.components = components;
                if self.wasm_cloud.components_state.selected().is_none()
                    && !self.wasm_cloud.components.is_empty()
                {
                    self.wasm_cloud.components_state.select(Some(0));
                }
            }
            TaskResult::WasmCloudAppList(apps) => {
                self.wasm_cloud.apps = apps;
                if self.wasm_cloud.apps_state.selected().is_none()
                    && !self.wasm_cloud.apps.is_empty()
                {
                    self.wasm_cloud.apps_state.select(Some(0));
                }
            }
            TaskResult::WasmCloudInspect(output) => {
                self.wasm_cloud.inspect_output = Some(output);
            }
            TaskResult::WasmCloudNatsStatus {
                running,
                storage_usage,
                synced,
            } => {
                self.wasm_cloud.nats_running = running;
                self.wasm_cloud.nats_storage_usage = storage_usage;
                self.wasm_cloud.nats_synced = synced;
            }
            TaskResult::UserList(users) => {
                self.users.users = users;
                self.users.loading = false;
                if self.users.table_state.selected().is_none() && !self.users.users.is_empty() {
                    self.users.table_state.select(Some(0));
                }
            }
            TaskResult::ServiceList(services) => {
                self.services.list = services;
                self.services.loading = false;
                if self.services.table_state.selected().is_none() && !self.services.list.is_empty()
                {
                    self.services.table_state.select(Some(0));
                }
            }
            TaskResult::ServiceOpDone { name, op, success } => {
                self.status_msg = Some(if success {
                    format!("Service {} {} success", name, op)
                } else {
                    format!("Service {} {} FAILED", name, op)
                });
                self.spawn_load_services();
            }
            TaskResult::MaintenanceDone {
                op,
                output,
                success,
            } => {
                self.maintenance.running_op = None;
                self.maintenance.last_output = output;
                self.status_msg = Some(if success {
                    format!("Maintenance action {} done", op)
                } else {
                    format!("Maintenance action {} FAILED", op)
                });
            }
            TaskResult::GhostScan(ghosts) => {
                self.ghost.scanning = false;
                self.ghost.ghosts = ghosts;
                if self.ghost.table_state.selected().is_none() && !self.ghost.ghosts.is_empty() {
                    self.ghost.table_state.select(Some(0));
                }
                self.status_msg = Some(format!(
                    "Ghost scan complete — {} suspect process(es) found",
                    self.ghost.ghosts.len()
                ));
            }
            TaskResult::PiAgentInstallProgress(line) => {
                self.agent.install_log.push(line);
            }
            TaskResult::PiAgentInstallDone { output, success } => {
                self.agent.installing = false;
                self.agent.install_log.push(output.clone());
                self.status_msg = Some(if success {
                    "pi installed — press [r] to refresh".to_string()
                } else {
                    format!("Install failed: {}", output.lines().next().unwrap_or(""))
                });
                if success {
                    self.spawn_load_pi_agent_status();
                }
            }
            TaskResult::PiAgentInfo(info) => {
                self.agent.loading = false;
                self.agent.info = info;
            }
            TaskResult::PiAgentSessions(sessions) => {
                self.agent.sessions = sessions;
                if self.agent.sessions_state.selected().is_none()
                    && !self.agent.sessions.is_empty()
                {
                    self.agent.sessions_state.select(Some(0));
                }
            }
            TaskResult::PiAgentConfig(text) => {
                self.agent.config_text = text;
                self.agent.config_scroll = 0;
            }
            TaskResult::PiAgentAuth(entries) => {
                self.agent.auth_entries = entries;
                if self.agent.auth_state.selected().is_none()
                    && !self.agent.auth_entries.is_empty()
                {
                    self.agent.auth_state.select(Some(0));
                }
            }
            TaskResult::PiAgentSkills(skills) => {
                self.agent.skills = skills;
                if self.agent.skills_state.selected().is_none()
                    && !self.agent.skills.is_empty()
                {
                    self.agent.skills_state.select(Some(0));
                }
            }
            TaskResult::PiAgentSkillRemoved { name, success } => {
                self.agent.skills_status = Some(if success {
                    format!("Removed skill '{}'", name)
                } else {
                    format!("Failed to remove '{}'", name)
                });
                self.spawn_load_pi_agent_skills();
            }
            TaskResult::PiAgentLibrarySkills(skills) => {
                self.agent.library_skills = skills;
                if self.agent.library_state.selected().is_none()
                    && !self.agent.library_skills.is_empty()
                {
                    self.agent.library_state.select(Some(0));
                }
            }
            TaskResult::PiAgentLibraryInstall { name, success } => {
                self.agent.library_status = Some(if success {
                    format!("Installed '{}'", name)
                } else {
                    format!("Failed to install '{}'", name)
                });
                self.spawn_load_pi_agent_library();
            }
            TaskResult::PiAgentLogs(lines) => {
                self.agent.logs = lines;
                if self.agent.logs_follow {
                    self.agent.logs_scroll =
                        (self.agent.logs.len() as u16).saturating_sub(20);
                }
            }
            TaskResult::PiAgentActionDone {
                action,
                output,
                success,
            } => {
                self.agent.action_output = Some(output.clone());
                self.status_msg = Some(if success {
                    format!("pi {} — done", action)
                } else {
                    format!(
                        "pi {} FAILED: {}",
                        action,
                        output.lines().next().unwrap_or("")
                    )
                });
                match action.as_str() {
                    "update_check" | "update_apply" => self.spawn_load_pi_agent_status(),
                    _ => {}
                }
            }
            TaskResult::PiAgentTasks(tasks) => {
                self.agent.tasks = tasks;
                self.agent.tasks_loading = false;
                if self.agent.tasks_state.selected().is_none() && !self.agent.tasks.is_empty() {
                    self.agent.tasks_state.select(Some(0));
                }
            }
            TaskResult::PiAgentTaskCreated => {
                self.status_msg = Some("Task created".to_string());
            }
            TaskResult::PiAgentTaskDeleted => {
                self.status_msg = Some("Task deleted".to_string());
            }
            TaskResult::PiAgentTaskToggled => {}
            TaskResult::PiAgentRpcConnected(handle) => {
                self.agent.rpc_handle = Some(handle);
                self.agent.rpc_active = true;
                self.agent.status = "Idle".to_string();
                self.status_msg = Some("Agent connected — press [i] or Enter to chat".to_string());
                if let Some(prompt) = self.agent.pending_prompt.take() {
                    self.send_agent_prompt(prompt);
                }
            }
            TaskResult::PiAgentRpcStarted => {
                self.agent.status = "Streaming…".to_string();
            }
            TaskResult::PiAgentRpcStopped => {
                self.agent.rpc_handle = None;
                self.agent.rpc_active = false;
                self.agent.streaming = false;
                self.agent.status = "Disconnected".to_string();
            }
            TaskResult::PiAgentTextDelta(delta) => {
                self.agent.streaming = true;
                match self.agent.messages.last_mut() {
                    Some(msg) if msg.role == AgentRole::Assistant => {
                        msg.content.push_str(&delta);
                    }
                    _ => {
                        self.agent.messages.push(AgentMessage {
                            role: AgentRole::Assistant,
                            content: delta,
                        });
                    }
                }
            }
            TaskResult::PiAgentAgentEnd => {
                self.agent.streaming = false;
                self.agent.status = "Idle".to_string();
            }
            TaskResult::PiAgentToolStart(name) => {
                self.agent.tool_log.push(format!("→ {}", name));
                self.agent.messages.push(AgentMessage {
                    role: AgentRole::Tool,
                    content: format!("→ {}", name),
                });
            }
            TaskResult::PiAgentToolEnd { name, is_error } => {
                let symbol = if is_error { "✗" } else { "✓" };
                self.agent.tool_log.push(format!("{} {}", symbol, name));
            }
            TaskResult::PiAgentRpcError(e) => {
                self.agent.streaming = false;
                self.agent.status = format!("Error: {}", e);
                self.status_msg = Some(format!("Agent error: {}", e));
            }
            TaskResult::PiAgentRpcStderr(line) => {
                self.agent.messages.push(AgentMessage {
                    role: AgentRole::Tool,
                    content: format!("[stderr] {}", line),
                });
            }
            TaskResult::StorageLoaded(devices) => {
                if self.storage.table_state.selected().is_none() && !devices.is_empty() {
                    self.storage.table_state.select(Some(0));
                }
                self.storage.devices = devices;
                self.storage.loading = false;
            }
            TaskResult::SmartLoaded(physical) => {
                self.storage.physical = physical;
                self.storage.smart_loading = false;
            }
            TaskResult::StorageOpDone { op, success } => {
                self.status_msg = Some(if success {
                    format!("{} succeeded", op)
                } else {
                    format!("{} FAILED", op)
                });
                self.spawn_load_storage();
            }
            TaskResult::StorageFstabLoaded(content) => {
                self.storage.fstab_content = content;
            }
            TaskResult::UpdatesList(list) => {
                if self.packages.updates_state.selected().is_none() && !list.is_empty() {
                    self.packages.updates_state.select(Some(0));
                }
                self.packages.updates = list;
                self.packages.updates_loading = false;
            }
            TaskResult::UpdatesOpDone { .. } => {
                self.spawn_load_updates();
            }
            TaskResult::SwapLoaded(status) => {
                if self.swap.table_state.selected().is_none() && !status.entries.is_empty() {
                    self.swap.table_state.select(Some(0));
                }
                self.swap.status = Some(status);
                self.swap.loading = false;
            }
            TaskResult::SwapOpDone { op, success } => {
                self.status_msg = Some(if success {
                    format!("Swap {} succeeded", op)
                } else {
                    format!("Swap {} FAILED", op)
                });
                self.spawn_load_swap();
            }
            TaskResult::ProjectsList(list) => {
                self.projects.list = list;
                self.projects.loading = false;
                if self.projects.list_state.selected().is_none() && !self.projects.list.is_empty() {
                    self.projects.list_state.select(Some(0));
                }
            }
            TaskResult::ProjectsOpProgress { op, line } => {
                let clean = clean_output_line(&line);
                if !clean.is_empty() {
                    match op.as_str() {
                        "new" => self.projects.new_output.push(clean),
                        "clone" => self.projects.clone_output.push(clean),
                        "pull" => self.status_msg = Some(clean),
                        _ => {}
                    }
                }
            }
            TaskResult::ProjectsOpDone { op, name, success } => {
                match op.as_str() {
                    "new" => {
                        self.projects.new_running = false;
                        self.status_msg = Some(if success {
                            format!("Created project '{}'", name)
                        } else {
                            "Scaffold failed — see output".to_string()
                        });
                        if success {
                            self.spawn_load_projects();
                        }
                    }
                    "clone" => {
                        self.projects.clone_running = false;
                        self.status_msg = Some(if success {
                            format!("Cloned '{}' into {}", name, self.projects.dir)
                        } else {
                            "Clone failed — see output".to_string()
                        });
                        if success {
                            self.spawn_load_projects();
                        }
                    }
                    "pull" => {
                        self.status_msg = Some(if success {
                            format!("Pulled '{}'", name)
                        } else {
                            format!("Pull failed for '{}'", name)
                        });
                        if success {
                            self.spawn_load_projects();
                        }
                    }
                    _ => {}
                }
            }
            TaskResult::ProjectsDirLoaded(dir) => {
                self.projects.dir = dir.clone();
                self.projects.dir_input = dir;
                if self.screen == Screen::Projects
                    && self.projects.active_tab == ProjectsTab::Projects
                {
                    self.spawn_load_projects();
                }
            }
            TaskResult::ProjectsGitLoaded(status) => {
                self.projects.git_name_input = status.name.clone();
                self.projects.git_email_input = status.email.clone();
                self.projects.git = status;
            }
            TaskResult::ProjectsGitSaved { what, success } => {
                self.status_msg = Some(if success {
                    format!("Git {} saved", what)
                } else {
                    format!("Failed to save git {}", what)
                });
                if what == "token" {
                    self.projects.git_token_input.clear();
                }
                self.spawn_projects_load_git();
            }
            TaskResult::Error(e) => {
                self.portchecker.ip_loading = false;
                self.portchecker.checking = false;
                self.ghost.scanning = false;
                self.swap.loading = false;
                self.projects.loading = false;
                self.projects.new_running = false;
                self.projects.clone_running = false;
                self.status_msg = Some(format!("Error: {}", e));
            }
        }
    }

    // ── Tick — called every ~250ms ────────────────────────────────────────

    pub async fn tick(&mut self) {
        self.drain_task_results();

        // Refresh live data based on current screen
        match &self.screen {
            Screen::Dashboard => {
                if let Ok(cpu) = self.platform.system.cpu_pct().await {
                    self.dashboard.cpu_pct = cpu.clone();
                    let avg = if cpu.is_empty() {
                        0
                    } else {
                        (cpu.iter().sum::<f32>() / cpu.len() as f32) as u64
                    };
                    if self.resources.cpu_history.is_empty() {
                        self.resources.cpu_history = vec![Vec::new(); cpu.len().max(1)];
                    }
                    for (i, &c) in cpu.iter().enumerate() {
                        if let Some(h) = self.resources.cpu_history.get_mut(i) {
                            h.push(c as u64);
                            if h.len() > 60 {
                                h.remove(0);
                            }
                        }
                    }
                    let _ = avg;
                }
                if let Ok(mem) = self.platform.system.mem().await {
                    let pct = (mem.used * 100).checked_div(mem.total).unwrap_or(0);
                    self.resources.mem_history.push(pct);
                    if self.resources.mem_history.len() > 60 {
                        self.resources.mem_history.remove(0);
                    }
                    self.dashboard.mem = Some(mem);
                }
                if self.dashboard.os_info.is_none() {
                    if let Ok(info) = self.platform.system.info().await {
                        self.dashboard.os_info = Some(info);
                    }
                }
                if self.dashboard.disks.is_empty() {
                    if let Ok(disks) = self.platform.system.disks().await {
                        self.dashboard.disks = disks;
                    }
                }
                if let Ok(net) = self.platform.system.net().await {
                    let rx_delta = net.rx_bytes.saturating_sub(self.resources.last_net_rx);
                    let tx_delta = net.tx_bytes.saturating_sub(self.resources.last_net_tx);
                    self.resources.net_rx_history.push(rx_delta / 1024);
                    self.resources.net_tx_history.push(tx_delta / 1024);
                    if self.resources.net_rx_history.len() > 60 {
                        self.resources.net_rx_history.remove(0);
                    }
                    if self.resources.net_tx_history.len() > 60 {
                        self.resources.net_tx_history.remove(0);
                    }
                    self.resources.last_net_rx = net.rx_bytes;
                    self.resources.last_net_tx = net.tx_bytes;
                }
                if self.dashboard.active_tab == DashboardTab::Processes {
                    self.spawn_load_processes();
                }
            }
            Screen::WasmCloud => {
                // Poll NATS health every 20 ticks (~5 s)
                self.wasm_cloud.nats_poll_counter =
                    self.wasm_cloud.nats_poll_counter.wrapping_add(1);
                if self.wasm_cloud.nats_poll_counter.is_multiple_of(20) {
                    self.spawn_poll_nats_status();
                }
            }
            Screen::Agent => {
                self.agent.poll_counter = self.agent.poll_counter.wrapping_add(1);
                if self.agent.poll_counter.is_multiple_of(40) {
                    self.spawn_load_pi_agent_status();
                }
            }
            _ => {}
        }

        // Scheduler: check due tasks every ~1 s (4 ticks × 250 ms)
        self.scheduler_tick = self.scheduler_tick.wrapping_add(1);
        if self.scheduler_tick.is_multiple_of(4) {
            self.check_scheduled_tasks().await;
        }

        self.last_tick = Instant::now();
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────

/// Parse ingress entries from a cloudflared YAML config.
/// Returns `Vec<(hostname, service)>`, skipping the catch-all entry.
pub fn parse_ingress_entries(content: &str) -> Vec<(String, String)> {
    let mut entries = Vec::new();
    let mut in_ingress = false;
    let mut current_host: Option<String> = None;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "ingress:" {
            in_ingress = true;
            continue;
        }
        if in_ingress {
            // A non-whitespace top-level key ends the ingress block
            if !line.starts_with(' ') && !line.starts_with('\t') && !trimmed.is_empty() {
                break;
            }
            if let Some(stripped) = trimmed.strip_prefix("hostname:") {
                current_host = Some(stripped.trim().to_string());
            } else if let Some(stripped) = trimmed.strip_prefix("service:") {
                let svc = stripped.trim().to_string();
                if let Some(host) = current_host.take() {
                    entries.push((host, svc));
                }
                // catch-all has no hostname — already consumed by current_host.take() returning None
            }
        }
    }
    entries
}

/// Strip ANSI CSI escape sequences and handle carriage returns from process output.
/// `\r` mid-line is a terminal cursor-to-col-0 overwrite; we keep only the last segment.
fn clean_output_line(raw: &str) -> String {
    let segment = raw.rsplit('\r').next().unwrap_or(raw);
    let mut out = String::with_capacity(segment.len());
    let mut chars = segment.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if let Some('[') = chars.next() {
                for nc in chars.by_ref() {
                    if nc.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else {
            out.push(c);
        }
    }
    out.trim().to_string()
}
