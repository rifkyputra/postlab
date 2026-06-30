use crate::core::{
    models::{SecurityFinding, Severity},
    security::SecurityAuditor,
};
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use wasmtime::component::{Component, Linker};
use wasmtime::{Config, Engine, Store};

// The plugin contract. Single source of truth lives in wit/security-check.wit
// (path is relative to this crate's manifest dir); guest components generate
// against the same file.
wasmtime::component::bindgen!({
    path: "../wit",
    world: "security-plugin",
});

use postlab::plugin::host::Host;

// Per-scan CPU budget. A runaway or hostile check traps instead of hanging the
// TUI; tune once real plugins exist.
const FUEL_PER_SCAN: u64 = 50_000_000;

const PLUGIN_DIR: &str = "/etc/postlab/plugins";

// Plugins may only read where security configs live. Deny-by-default elsewhere.
const ALLOWED_ROOTS: &[&str] = &["/etc"];

// Returns the native auditor unchanged when no plugins are present or loading
// fails, so a broken plugin dir can never disable the built-in checks.
pub fn security_auditor(native: Arc<dyn SecurityAuditor>) -> Arc<dyn SecurityAuditor> {
    let roots = ALLOWED_ROOTS.iter().map(PathBuf::from).collect();
    let mut host = match PluginHost::new(roots) {
        Ok(h) => h,
        Err(e) => {
            tracing::warn!(error = %e, "wasm plugin host unavailable");
            return native;
        }
    };
    match host.load_dir(PLUGIN_DIR) {
        Ok(0) => native,
        Ok(n) => {
            tracing::info!(count = n, "loaded wasm security checks");
            Arc::new(CompositeAuditor {
                native,
                wasm: WasmAuditor::new(host),
            })
        }
        Err(e) => {
            tracing::warn!(error = %e, "failed loading wasm plugins");
            native
        }
    }
}

struct HostState {
    allowed_roots: Arc<Vec<PathBuf>>,
}

impl Host for HostState {
    fn read_file(&mut self, path: String) -> Result<String, String> {
        let requested = PathBuf::from(&path);
        // Deny-by-default: a plugin may only read under host-granted roots, and
        // never via `..` traversal out of them.
        let allowed = self
            .allowed_roots
            .iter()
            .any(|root| requested.starts_with(root))
            && !path.contains("..");
        if !allowed {
            return Err(format!("read denied: {path}"));
        }
        std::fs::read_to_string(&requested).map_err(|e| e.to_string())
    }
}

struct LoadedPlugin {
    name: String,
    component: Component,
}

pub struct PluginHost {
    engine: Engine,
    plugins: Vec<LoadedPlugin>,
    allowed_roots: Arc<Vec<PathBuf>>,
}

impl PluginHost {
    pub fn new(allowed_roots: Vec<PathBuf>) -> Result<Self> {
        let mut config = Config::new();
        config.consume_fuel(true);
        let engine = Engine::new(&config).context("init wasmtime engine")?;
        Ok(Self {
            engine,
            plugins: Vec::new(),
            allowed_roots: Arc::new(allowed_roots),
        })
    }

    pub fn load_dir(&mut self, dir: impl AsRef<Path>) -> Result<usize> {
        let dir = dir.as_ref();
        if !dir.is_dir() {
            return Ok(0);
        }
        let mut loaded = 0;
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("wasm") {
                continue;
            }
            let component = Component::from_file(&self.engine, &path)
                .with_context(|| format!("compile {}", path.display()))?;
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("plugin")
                .to_string();
            self.plugins.push(LoadedPlugin { name, component });
            loaded += 1;
        }
        Ok(loaded)
    }

    fn run_scans(&self) -> Vec<SecurityFinding> {
        let mut findings = Vec::new();
        for plugin in &self.plugins {
            match self.scan_one(plugin) {
                Ok(mut f) => findings.append(&mut f),
                // One bad plugin must not sink the whole audit.
                Err(e) => tracing::warn!(plugin = %plugin.name, error = %e, "wasm check failed"),
            }
        }
        findings
    }

    fn scan_one(&self, plugin: &LoadedPlugin) -> Result<Vec<SecurityFinding>> {
        let mut linker: Linker<HostState> = Linker::new(&self.engine);
        SecurityPlugin::add_to_linker(&mut linker, |s| s)?;

        let state = HostState {
            allowed_roots: Arc::clone(&self.allowed_roots),
        };
        let mut store = Store::new(&self.engine, state);
        store.set_fuel(FUEL_PER_SCAN)?;

        let instance = SecurityPlugin::instantiate(&mut store, &plugin.component, &linker)?;
        let raw = instance.postlab_plugin_check().call_scan(&mut store)?;
        Ok(raw.into_iter().map(map_finding).collect())
    }
}

pub struct WasmAuditor {
    host: Arc<PluginHost>,
}

impl WasmAuditor {
    pub fn new(host: PluginHost) -> Self {
        Self {
            host: Arc::new(host),
        }
    }
}

#[async_trait]
impl SecurityAuditor for WasmAuditor {
    async fn scan(&self) -> Result<Vec<SecurityFinding>> {
        let host = Arc::clone(&self.host);
        // Wasmtime calls are synchronous and CPU-bound; keep them off the reactor.
        let findings = tokio::task::spawn_blocking(move || host.run_scans()).await?;
        Ok(findings)
    }

    // Mutating fixes stay native-only: plugins propose findings, they never
    // write to disk.
    async fn apply(&self, _id: &str) -> Result<String> {
        anyhow::bail!("wasm plugins cannot apply fixes")
    }
}

struct CompositeAuditor {
    native: Arc<dyn SecurityAuditor>,
    wasm: WasmAuditor,
}

#[async_trait]
impl SecurityAuditor for CompositeAuditor {
    async fn scan(&self) -> Result<Vec<SecurityFinding>> {
        let mut findings = self.native.scan().await?;
        // A plugin failure must not mask the native audit; log and carry on.
        match self.wasm.scan().await {
            Ok(mut extra) => findings.append(&mut extra),
            Err(e) => tracing::warn!(error = %e, "wasm checks failed"),
        }
        Ok(findings)
    }

    // Fix ids come only from native findings, so apply always routes there.
    async fn apply(&self, id: &str) -> Result<String> {
        self.native.apply(id).await
    }
}

fn map_finding(f: exports::postlab::plugin::check::Finding) -> SecurityFinding {
    SecurityFinding {
        id: f.id,
        title: f.title,
        severity: map_severity(f.severity),
        description: f.description,
        file_path: f.file_path,
        fix_description: f.fix_description,
    }
}

fn map_severity(level: u8) -> Severity {
    match level {
        0 => Severity::Critical,
        1 => Severity::High,
        2 => Severity::Medium,
        3 => Severity::Low,
        _ => Severity::Info,
    }
}
