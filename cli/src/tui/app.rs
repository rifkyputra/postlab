use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use std::time::Instant;

use ratatui::widgets::{ListState, TableState};
use sqlx::SqlitePool;
use tokio::sync::mpsc;

use crate::core::{
    models::{DiskInfo, DockerContainer, DockerImage, DockerComposeService, FirewallRule, GhostProcess, JailedIp, MemInfo, OsInfo, Package, ProcessEntry, Route, SecurityFinding, SshKey, Tunnel, WasmCloudHost, WasmCloudComponent, WasmCloudLink, WasmCloudApp, UserInfo},
    portcheck::{PortEntry, PortStatus, default_entries},
    Platform,
};

// ── Screens ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    Dashboard,
    Packages,
    Security,
    Gateway,
    Tunnel,
    Docker,
    WasmCloud,
    Ghosts,
    Users,
}

impl Screen {
    pub fn all() -> &'static [Screen] {
        &[
            Screen::Dashboard,
            Screen::Packages,
            Screen::Security,
            Screen::Gateway,
            Screen::Tunnel,
            Screen::Docker,
            Screen::WasmCloud,
            Screen::Ghosts,
            Screen::Users,
        ]
    }

    pub fn title(&self) -> &'static str {
        match self {
            Screen::Dashboard   => "1. Dashboard",
            Screen::Packages    => "2. Packages",
            Screen::Security    => "3. Security", 
            Screen::Gateway     => "4. Gateway",
            Screen::Tunnel      => "5. Tunnel",
            Screen::Docker      => "6. Docker",
            Screen::WasmCloud   => "7. wasmCloud",
            Screen::Ghosts      => "8. Ghosts",
            Screen::Users       => "9. Users",
        }
    }

    pub fn index(&self) -> usize {
        Screen::all().iter().position(|s| s == self).unwrap_or(0)
    }
}

// ── Docker tabs ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum DockerTab {
    Containers,
    Images,
    Compose,
}

impl DockerTab {
    pub fn all() -> &'static [DockerTab] {
        &[DockerTab::Containers, DockerTab::Images, DockerTab::Compose]
    }
    pub fn title(&self) -> &'static str {
        match self {
            DockerTab::Containers => "Containers",
            DockerTab::Images => "Images",
            DockerTab::Compose => "Compose",
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
        &[DashboardTab::Overview, DashboardTab::Processes, DashboardTab::Resources]
    }

    pub fn title(&self) -> &'static str {
        match self {
            DashboardTab::Overview => "Overview",
            DashboardTab::Processes => "Processes",
            DashboardTab::Resources => "Resources",
        }
    }

    pub fn index(&self) -> usize {
        DashboardTab::all().iter().position(|t| t == self).unwrap_or(0)
    }
}

// ── Background task results ───────────────────────────────────────────────

#[derive(Debug)]
pub enum TaskResult {
    PackageList(Vec<Package>),
    PackagesUpdated(Vec<Package>),  // merge/add specific entries without full reload
    SearchResults(Vec<Package>),
    OpProgress { op: String, target: String, line: String },
    OpDone { op: String, target: String, output: String, success: bool },
    ProcessList(Vec<ProcessEntry>),
    SecurityScan(Vec<SecurityFinding>),
    SecurityApply { id: String, output: String, success: bool },
    Fail2BanList(Vec<JailedIp>),
    Fail2BanActionDone { ip: String, jail: String, action: String, success: bool },
    RouteList(Vec<Route>),
    TunnelList(Vec<Tunnel>),
    TunnelCreated(Tunnel),
    GatewayStatus { installed: bool, version: Option<String> },
    TunnelStatus { installed: bool, version: Option<String> },
    TunnelConfigContent(String),
    TunnelServiceStatus { active: bool, enabled: bool },
    InstallProgress { target: String, line: String },
    InstallDone { target: String, success: bool },
    DockerStatus { installed: bool, version: Option<String> },
    DockerContainerList(Vec<DockerContainer>),
    DockerImageList(Vec<DockerImage>),
    DockerComposeList(Vec<DockerComposeService>),
    FirewallStatus { enabled: bool, backend: String },
    FirewallRules(Vec<FirewallRule>),
    PublicIp(String),
    PortCheckDone { results: Vec<(u16, PortStatus)> },
    WasmCloudStatus { installed: bool, version: Option<String> },
    WasmCloudHostList(Vec<WasmCloudHost>),
    WasmCloudComponentList(Vec<WasmCloudComponent>),
    WasmCloudAppList(Vec<WasmCloudApp>),
    SshLocalKeys(Vec<SshKey>),
    SshAuthorizedKeys(Vec<SshKey>),
    SshOpDone { op: String, success: bool, output: String },
    WasmCloudInspect(String),
    GhostScan(Vec<GhostProcess>),
    UserList(Vec<UserInfo>),
    Status(String),
    Error(String),
}

// ── Confirm dialog ────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
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
    DeleteFirewallRule { num: usize },
    Fail2BanForgive { ip: String, jail: String },
    Fail2BanBanish { ip: String, jail: String },
    DeauthorizeKey { fingerprint: String, name: String },
    AuthorizeLocalKey { content: String, name: String },
    KillGhost { pid: u32, name: String },
}

#[derive(Debug)]
pub struct ConfirmDialog {
    pub message: String,
    pub action: ConfirmAction,
}

// ── Input mode ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum InputMode {
    Normal,
    Editing,
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
}

impl PackageTab {
    pub fn all() -> &'static [PackageTab] {
        &[PackageTab::Installed, PackageTab::Search, PackageTab::QuickInstall, PackageTab::Queue]
    }
    pub fn title(&self) -> &'static str {
        match self {
            PackageTab::Installed => "Installed",
            PackageTab::Search => "Search",
            PackageTab::QuickInstall => "Quick Install",
            PackageTab::Queue => "Queue",
        }
    }
    pub fn index(&self) -> usize {
        PackageTab::all().iter().position(|t| t == self).unwrap_or(0)
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
        SecurityTab::all().iter().position(|t| t == self).unwrap_or(0)
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
        &[WasmCloudTab::Hosts, WasmCloudTab::Components, WasmCloudTab::Apps, WasmCloudTab::Inspector]
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
        WasmCloudTab::all().iter().position(|t| t == self).unwrap_or(0)
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
    pub curated_selected: HashSet<String>,   // packages to install
    pub curated_uninstall: HashSet<String>,  // installed packages marked for removal
    pub curated_cursor: usize,
    // Queue
    pub queue: VecDeque<QueuedOp>,
    pub queue_state: ListState,
    pub queue_selected: Option<usize>,
    pub output_scroll: usize,
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

pub struct ResourcesState {
    pub cpu_history: Vec<Vec<u64>>,
    pub mem_history: Vec<u64>,
    pub net_rx_history: Vec<u64>,
    pub net_tx_history: Vec<u64>,
    pub last_net_rx: u64,
    pub last_net_tx: u64,
}

impl Default for ResourcesState {
    fn default() -> Self {
        Self {
            cpu_history: Vec::new(),
            mem_history: Vec::new(),
            net_rx_history: Vec::new(),
            net_tx_history: Vec::new(),
            last_net_rx: 0,
            last_net_tx: 0,
        }
    }
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
    pub config_content: Option<String>,   // ~/.cloudflared/config.yaml
    pub service_active: Option<bool>,     // true=active, false=inactive, None=unknown
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
    pub input_proto: usize,   // index into PROTOS
    pub input_from: String,
    pub input_action: usize,  // index into ACTIONS
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
    pub focus: usize, // 0 = local, 1 = authorized
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
        }
    }
}

// ── Ghost Services Hunter state ────────────────────────────────────────────

pub struct GhostState {
    /// Results from the last scan.
    pub ghosts: Vec<GhostProcess>,
    pub table_state: TableState,
    /// True while a scan is running in the background.
    pub scanning: bool,
}

impl Default for GhostState {
    fn default() -> Self {
        Self {
            ghosts: Vec::new(),
            table_state: TableState::default(),
            scanning: false,
        }
    }
}

// ── Users state ─────────────────────────────────────────────────────────────
pub struct UsersState {
    pub users: Vec<UserInfo>,
    pub table_state: TableState,
    pub loading: bool,
}

impl Default for UsersState {
    fn default() -> Self {
        Self {
            users: Vec::new(),
            table_state: TableState::default(),
            loading: false,
        }
    }
}

// ── Main App ──────────────────────────────────────────────────────────────

pub struct App {
    pub screen: Screen,
    pub platform: Arc<Platform>,
    pub pool: SqlitePool,

    // Screen state
    pub dashboard: DashboardState,
    pub packages: PackagesState,
    pub processes: ProcessesState,
    pub security: SecurityState,
    pub resources: ResourcesState,
    pub gateway: GatewayState,
    pub tunnel: TunnelState,
    pub docker: DockerState,
    pub firewall: FirewallState,
    pub ssh: SshState,
    pub portchecker: PortCheckerState,
    pub wasm_cloud: WasmCloudState,
    pub ghost: GhostState,
    pub users: UsersState,

    // Background task channel
    pub task_tx: mpsc::UnboundedSender<TaskResult>,
    pub task_rx: mpsc::UnboundedReceiver<TaskResult>,

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
            platform: Arc::new(platform),
            pool,
            dashboard: DashboardState::default(),
            packages: PackagesState::default(),
            processes: ProcessesState::default(),
            security: SecurityState::default(),
            resources: ResourcesState::default(),
            gateway: GatewayState::default(),
            tunnel: TunnelState::default(),
            docker: DockerState::default(),
            firewall: FirewallState::default(),
            ssh: SshState::default(),
            portchecker: PortCheckerState::default(),
            wasm_cloud: WasmCloudState::default(),
            ghost: GhostState::default(),
            users: UsersState::default(),
            task_tx,
            task_rx,
            confirm: None,
            status_msg: None,
            last_tick: Instant::now(),
            needs_login: false,
        }
    }

    pub fn set_screen(&mut self, screen: Screen) {
        self.screen = screen.clone();
        self.status_msg = None;
        // Trigger initial data load for screens that need it
        match &screen {
            Screen::Packages => {
                if self.packages.installed.is_empty() {
                    self.spawn_load_packages();
                }
            }
            Screen::Security => {
                let tab = self.security.active_tab.clone();
                self.spawn_load_security_tab(tab);
            }
            Screen::Gateway => self.spawn_load_gateway(),
            Screen::Tunnel => {
                self.spawn_load_tunnels();
                let id = self.tunnel.active_tunnel_id.clone();
                self.spawn_tunnel_extras(id);
            }
            Screen::Docker => self.spawn_load_docker(),
                self.spawn_load_wasm_cloud();
            }
            Screen::Ghosts => {
                if self.ghost.ghosts.is_empty() {
                    self.spawn_ghost_scan();
                }
            }
            Screen::Users => {
                self.spawn_load_users();
            }
            _ => {}
        }
    }
                if !self.ghost.scanning {
                    self.spawn_ghost_scan();
                }
            }
            _ => {}
        }
    }

    pub fn next_screen(&mut self) {
        let idx = (self.screen.index() + 1) % Screen::all().len();
        let s = Screen::all()[idx].clone();
        self.set_screen(s);
    }

    pub fn prev_screen(&mut self) {
        let idx = self.screen.index();
        let prev = if idx == 0 { Screen::all().len() - 1 } else { idx - 1 };
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

    // ── async data loaders (spawn background tasks) ───────────────────────

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
                Ok(pkgs) => { let _ = tx.send(TaskResult::PackageList(pkgs)); }
                Err(e) => { let _ = tx.send(TaskResult::Error(e.to_string())); }
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
                Ok(pkgs) => { let _ = tx.send(TaskResult::PackagesUpdated(pkgs)); }
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
                Ok(pkgs) => { let _ = tx.send(TaskResult::SearchResults(pkgs)); }
                Err(e) => { let _ = tx.send(TaskResult::Error(e.to_string())); }
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
            let _ = crate::db::audit::log_action(&pool, "install", Some(&name), &output, success).await;
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
            let _ = crate::db::audit::log_action(&pool, "remove", Some(&name), &output, success).await;
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
                Ok(procs) => { let _ = tx.send(TaskResult::ProcessList(procs)); }
                Err(e) => { let _ = tx.send(TaskResult::Error(e.to_string())); }
            }
        });
    }

    pub fn spawn_security_scan(&mut self) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        self.security.scanning = true;
        tokio::spawn(async move {
            match platform.security.scan().await {
                Ok(findings) => { let _ = tx.send(TaskResult::SecurityScan(findings)); }
                Err(e) => { let _ = tx.send(TaskResult::Error(e.to_string())); }
            }
        });
    }

    pub fn spawn_fail2ban_list(&mut self) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        self.security.f2b_loading = true;
        tokio::spawn(async move {
            match platform.fail2ban.list_jailed().await {
                Ok(jailed) => { let _ = tx.send(TaskResult::Fail2BanList(jailed)); }
                Err(e) => { let _ = tx.send(TaskResult::Error(format!("fail2ban: {}", e))); }
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
            let _ = crate::db::audit::log_action(&pool, "harden", Some(&id), &output, success).await;
            let _ = tx.send(TaskResult::SecurityApply { id, output, success });
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
                    Ok(routes) => { let _ = tx.send(TaskResult::RouteList(routes)); }
                    Err(e) => { let _ = tx.send(TaskResult::Error(e.to_string())); }
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
                    Ok(tunnels) => { let _ = tx.send(TaskResult::TunnelList(tunnels)); }
                    Err(e) => { let _ = tx.send(TaskResult::Error(e.to_string())); }
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
                    Ok(c) => { let _ = tx.send(TaskResult::TunnelConfigContent(c)); }
                    Err(_) => { let _ = tx.send(TaskResult::TunnelConfigContent(String::new())); }
                }
            }
            match platform.tunnel.service_status().await {
                Ok((active, enabled)) => { let _ = tx.send(TaskResult::TunnelServiceStatus { active, enabled }); }
                Err(_) => {}
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
                    let _ = tx.send(TaskResult::Error(format!("cloudflared install failed: {}", e)));
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
                    Ok(containers) => { let _ = tx.send(TaskResult::DockerContainerList(containers)); }
                    Err(e) => { let _ = tx.send(TaskResult::Error(e.to_string())); }
                }
                match platform.docker.list_images().await {
                    Ok(images) => { let _ = tx.send(TaskResult::DockerImageList(images)); }
                    Err(e) => { let _ = tx.send(TaskResult::Error(e.to_string())); }
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
                "start"   => platform.docker.start_container(&id_clone).await,
                "stop"    => platform.docker.stop_container(&id_clone).await,
                "restart" => platform.docker.restart_container(&id_clone).await,
                "remove"  => platform.docker.remove_container(&id_clone).await,
                _         => Ok(()),
            };
            match result {
                Ok(()) => {
                    let _ = tx.send(TaskResult::Status(format!("{} {} — done", action, id_clone)));
                    match platform.docker.list_containers().await {
                        Ok(containers) => { let _ = tx.send(TaskResult::DockerContainerList(containers)); }
                        Err(e) => { let _ = tx.send(TaskResult::Error(e.to_string())); }
                    }
                }
                Err(e) => { let _ = tx.send(TaskResult::Error(format!("{} failed: {}", action, e))); }
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
                        Ok(images) => { let _ = tx.send(TaskResult::DockerImageList(images)); }
                        Err(e) => { let _ = tx.send(TaskResult::Error(e.to_string())); }
                    }
                }
                Err(e) => { let _ = tx.send(TaskResult::Error(format!("Remove image failed: {}", e))); }
            }
        });
    }

    pub fn spawn_load_compose(&mut self) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        let path = self.docker.compose_path.clone();
        tokio::spawn(async move {
            match platform.docker.list_compose_services(&path).await {
                Ok(services) => { let _ = tx.send(TaskResult::DockerComposeList(services)); }
                Err(e) => { let _ = tx.send(TaskResult::Error(e.to_string())); }
            }
        });
    }

    pub fn spawn_compose_action(&mut self, action: &'static str) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        let path = self.docker.compose_path.clone();
        tokio::spawn(async move {
            let result = match action {
                "up"      => platform.docker.compose_up(&path).await,
                "down"    => platform.docker.compose_down(&path).await,
                "restart" => platform.docker.compose_restart(&path).await,
                _         => Ok(()),
            };
            match result {
                Ok(()) => {
                    let _ = tx.send(TaskResult::Status(format!("compose {} — done", action)));
                    match platform.docker.list_compose_services(&path).await {
                        Ok(services) => { let _ = tx.send(TaskResult::DockerComposeList(services)); }
                        Err(e) => { let _ = tx.send(TaskResult::Error(e.to_string())); }
                    }
                }
                Err(e) => { let _ = tx.send(TaskResult::Error(format!("compose {} failed: {}", action, e))); }
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
                Err(e) => { let _ = tx.send(TaskResult::Error(e.to_string())); return; }
            }
            match platform.firewall.list_rules().await {
                Ok(rules) => { let _ = tx.send(TaskResult::FirewallRules(rules)); }
                Err(e) => { let _ = tx.send(TaskResult::Error(e.to_string())); }
            }
        });
    }

    pub fn spawn_firewall_add_rule(&mut self, port: String, proto: String, from: String, action: String) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            match platform.firewall.add_rule(&port, &proto, &from, &action).await {
                Ok(()) => {
                    let _ = tx.send(TaskResult::Status(format!("Rule added: {} {} from {}", action, port, from)));
                    match platform.firewall.list_rules().await {
                        Ok(rules) => { let _ = tx.send(TaskResult::FirewallRules(rules)); }
                        Err(e) => { let _ = tx.send(TaskResult::Error(e.to_string())); }
                    }
                }
                Err(e) => { let _ = tx.send(TaskResult::Error(format!("add rule failed: {}", e))); }
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
                        Ok(rules) => { let _ = tx.send(TaskResult::FirewallRules(rules)); }
                        Err(e) => { let _ = tx.send(TaskResult::Error(e.to_string())); }
                    }
                }
                Err(e) => { let _ = tx.send(TaskResult::Error(format!("delete rule failed: {}", e))); }
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
                    let _ = tx.send(TaskResult::FirewallStatus { enabled, backend: String::new() });
                    let _ = tx.send(TaskResult::Status(format!("Firewall {}", label)));
                }
                Err(e) => { let _ = tx.send(TaskResult::Error(e.to_string())); }
            }
        });
    }

    // ── Port Checker spawners ─────────────────────────────────────────────

    pub fn spawn_fetch_public_ip(&mut self) {
        self.portchecker.ip_loading = true;
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            match crate::core::portcheck::fetch_public_ip().await {
                Ok(ip) => { let _ = tx.send(TaskResult::PublicIp(ip)); }
                Err(e) => { let _ = tx.send(TaskResult::Error(format!("public IP: {}", e))); }
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
                Ok(results) => { let _ = tx.send(TaskResult::PortCheckDone { results }); }
                Err(e) => { let _ = tx.send(TaskResult::Error(format!("port check: {}", e))); }
            }
        });
    }

    // ── Ghost Services Hunter ─────────────────────────────────────────────

    pub fn spawn_ghost_scan(&mut self) {
        self.ghost.scanning = true;
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            match crate::core::ghost::scan().await {
                Ok(ghosts) => { let _ = tx.send(TaskResult::GhostScan(ghosts)); }
                Err(e) => { let _ = tx.send(TaskResult::Error(format!("ghost scan: {}", e))); }
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
                Ok(keys) => { let _ = tx.send(TaskResult::SshLocalKeys(keys)); }
                Err(e) => { let _ = tx.send(TaskResult::Error(e.to_string())); }
            }
            match platform.ssh.list_authorized_keys().await {
                Ok(keys) => { let _ = tx.send(TaskResult::SshAuthorizedKeys(keys)); }
                Err(e) => { let _ = tx.send(TaskResult::Error(e.to_string())); }
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
                Ok(users) => { let _ = tx.send(TaskResult::UserList(users)); }
                Err(e) => { let _ = tx.send(TaskResult::Error(e.to_string())); }
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
                    let _ = tx.send(TaskResult::Status(format!("User {} {} success", username, action)));
                    match platform.users.list_users().await {
                        Ok(users) => { let _ = tx.send(TaskResult::UserList(users)); }
                        Err(e) => { let _ = tx.send(TaskResult::Error(e.to_string())); }
                    }
                }
                Err(e) => { let _ = tx.send(TaskResult::Error(format!("{} failed: {}", action, e))); }
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
            }
        });
    }

    pub fn spawn_wasm_cloud_hosts(&mut self) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            if let Ok(hosts) = platform.wasm_cloud.list_hosts().await {
                let _ = tx.send(TaskResult::WasmCloudHostList(hosts));
            }
        });
    }

    pub fn spawn_wasm_cloud_components(&mut self) {
        let platform = Arc::clone(&self.platform);
        let tx = self.task_tx.clone();
        tokio::spawn(async move {
            if let Ok(components) = platform.wasm_cloud.list_components().await {
                let _ = tx.send(TaskResult::WasmCloudComponentList(components));
            }
        });
    }

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
                Ok(output) => { let _ = tx.send(TaskResult::WasmCloudInspect(output)); }
                Err(e) => { let _ = tx.send(TaskResult::WasmCloudInspect(format!("Error: {}", e))); }
            }
        });
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
                if self.packages.installed_state.selected().is_none() && !self.packages.installed.is_empty() {
                    self.packages.installed_state.select(Some(0));
                }
            }
            TaskResult::PackagesUpdated(updated) => {
                // Merge: update existing entries or append new ones
                for pkg in updated {
                    if let Some(existing) = self.packages.installed.iter_mut().find(|p| p.name == pkg.name) {
                        *existing = pkg;
                    } else {
                        self.packages.installed.push(pkg);
                    }
                }
                if self.packages.installed_state.selected().is_none() && !self.packages.installed.is_empty() {
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
                if let Some(entry) = self.packages.queue.iter_mut()
                    .find(|e| e.target == target && e.kind == op)
                {
                    if !entry.output.is_empty() { entry.output.push('\n'); }
                    entry.output.push_str(&line);
                }
            }
            TaskResult::OpDone { op, target, output, success } => {
                if let Some(entry) = self.packages.queue.iter_mut()
                    .find(|e| e.target == target && e.kind == op)
                {
                    entry.status = if success { OpStatus::Done } else { OpStatus::Failed };
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
                if self.processes.table_state.selected().is_none() && !self.processes.list.is_empty() {
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
            TaskResult::SecurityApply { id, output, success } => {
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
                if self.security.jailed_state.selected().is_none() && !self.security.jailed.is_empty() {
                    self.security.jailed_state.select(Some(0));
                }
            }
            TaskResult::Fail2BanActionDone { ip, jail, action, success } => {
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
                    if let Some(id) = content.lines()
                        .find(|l| l.trim_start().starts_with("tunnel:"))
                        .and_then(|l| l.splitn(2, ':').nth(1))
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
                if self.docker.containers_state.selected().is_none() && !self.docker.containers.is_empty() {
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
                if self.docker.compose_state.selected().is_none() && !self.docker.compose_services.is_empty() {
                    self.docker.compose_state.select(Some(0));
                }
            }
            TaskResult::FirewallStatus { enabled, backend } => {
                self.firewall.enabled = Some(enabled);
                self.firewall.backend = backend;
            }
            TaskResult::FirewallRules(rules) => {
                self.firewall.rules = rules;
                if self.firewall.table_state.selected().is_none() && !self.firewall.rules.is_empty() {
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
                    if let Some(entry) = self.portchecker.entries.iter_mut().find(|e| e.port == port) {
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
                if self.ssh.authorized_state.selected().is_none() && !self.ssh.authorized_keys.is_empty() {
                    self.ssh.authorized_state.select(Some(0));
                }
            }
            TaskResult::SshOpDone { op, success, output } => {
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
                if self.wasm_cloud.hosts_state.selected().is_none() && !self.wasm_cloud.hosts.is_empty() {
                    self.wasm_cloud.hosts_state.select(Some(0));
                }
            }
            TaskResult::WasmCloudComponentList(components) => {
                self.wasm_cloud.components = components;
                if self.wasm_cloud.components_state.selected().is_none() && !self.wasm_cloud.components.is_empty() {
                    self.wasm_cloud.components_state.select(Some(0));
                }
            }
            TaskResult::WasmCloudAppList(apps) => {
                self.wasm_cloud.apps = apps;
                if self.wasm_cloud.apps_state.selected().is_none() && !self.wasm_cloud.apps.is_empty() {
                    self.wasm_cloud.apps_state.select(Some(0));
                }
            }
            TaskResult::WasmCloudInspect(output) => {
                self.wasm_cloud.inspect_output = Some(output);
            }
            TaskResult::UserList(users) => {
                self.users.users = users;
                self.users.loading = false;
                if self.users.table_state.selected().is_none() && !self.users.users.is_empty() {
                    self.users.table_state.select(Some(0));
                }
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
            TaskResult::Error(e) => {
                self.portchecker.ip_loading = false;
                self.portchecker.checking = false;
                self.ghost.scanning = false;
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
                    let avg = if cpu.is_empty() { 0 } else {
                        (cpu.iter().sum::<f32>() / cpu.len() as f32) as u64
                    };
                    if self.resources.cpu_history.is_empty() {
                        self.resources.cpu_history = vec![Vec::new(); cpu.len().max(1)];
                    }
                    for (i, &c) in cpu.iter().enumerate() {
                        if let Some(h) = self.resources.cpu_history.get_mut(i) {
                            h.push(c as u64);
                            if h.len() > 60 { h.remove(0); }
                        }
                    }
                    let _ = avg;
                }
                if let Ok(mem) = self.platform.system.mem().await {
                    let pct = if mem.total > 0 { mem.used * 100 / mem.total } else { 0 };
                    self.resources.mem_history.push(pct);
                    if self.resources.mem_history.len() > 60 { self.resources.mem_history.remove(0); }
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
                    if self.resources.net_rx_history.len() > 60 { self.resources.net_rx_history.remove(0); }
                    if self.resources.net_tx_history.len() > 60 { self.resources.net_tx_history.remove(0); }
                    self.resources.last_net_rx = net.rx_bytes;
                    self.resources.last_net_tx = net.tx_bytes;
                }
                if self.dashboard.active_tab == DashboardTab::Processes {
                    self.spawn_load_processes();
                }
            }
            _ => {}
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
            if trimmed.starts_with("hostname:") {
                current_host = Some(trimmed["hostname:".len()..].trim().to_string());
            } else if trimmed.starts_with("service:") {
                let svc = trimmed["service:".len()..].trim().to_string();
                if let Some(host) = current_host.take() {
                    entries.push((host, svc));
                }
                // catch-all has no hostname — already consumed by current_host.take() returning None
            }
        }
    }
    entries
}
