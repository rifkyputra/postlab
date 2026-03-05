use anyhow::Result;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

pub struct NatsManager {
    bin_path: PathBuf,
    config_path: PathBuf,
}

impl NatsManager {
    pub fn new() -> Self {
        let home_str = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let home = PathBuf::from(home_str);
        let bin_path = home.join(".local").join("bin").join("nats-server");
        let config_path = home.join(".postlab").join("nats.conf");
        Self { bin_path, config_path }
    }

    pub fn is_installed(&self) -> bool {
        // Check standard path or our custom download path
        let in_path = crate::core::packages::which("nats-server");
        in_path || self.bin_path.exists()
    }

    pub async fn auto_download(&self) -> Result<()> {
        if self.is_installed() {
            return Ok(());
        }

        // Determine OS and ARCH
        let os = match env::consts::OS {
            "linux"   => "linux",
            "macos"   => "darwin",
            "windows" => "windows",
            other     => anyhow::bail!("Unsupported OS for auto-download: {}", other),
        };
        let arch = match env::consts::ARCH {
            "x86_64"  => "amd64",
            "aarch64" => "arm64",
            "arm"     => "arm7",
            other     => anyhow::bail!("Unsupported ARCH for auto-download: {}", other),
        };

        let version  = "2.10.11";
        let filename = format!("nats-server-v{}-{}-{}", version, os, arch);
        let url      = format!(
            "https://github.com/nats-io/nats-server/releases/download/v{}/{}.zip",
            version, filename
        );

        let parent   = self.bin_path.parent().unwrap().to_path_buf();
        let zip_path = parent.join(format!("{}.zip", &filename));
        let bin_path = self.bin_path.clone();

        // All shell calls are blocking → run them off the Tokio thread pool.
        tokio::task::spawn_blocking(move || -> Result<()> {
            fs::create_dir_all(&parent)?;

            // 1. Download
            let st = Command::new("curl")
                .args(["-sL", "-o", zip_path.to_str().unwrap(), &url])
                .status()?;
            if !st.success() {
                anyhow::bail!("curl download failed — is curl installed?");
            }

            // 2. Unzip
            let st = Command::new("unzip")
                .args(["-o", zip_path.to_str().unwrap(), "-d", parent.to_str().unwrap()])
                .status()?;
            if !st.success() {
                anyhow::bail!("unzip failed — is unzip installed?");
            }

            // 3. Move binary into place
            let extracted = parent.join(&filename).join("nats-server");
            fs::rename(&extracted, &bin_path)?;

            // 4. Clean up
            let _ = fs::remove_file(&zip_path);
            let _ = fs::remove_dir_all(parent.join(&filename));

            // 5. chmod +x
            let _ = Command::new("chmod")
                .args(["+x", bin_path.to_str().unwrap()])
                .status();

            Ok(())
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking panicked: {}", e))??;

        Ok(())
    }

    pub fn get_bin(&self) -> String {
        if crate::core::packages::which("nats-server") {
            "nats-server".to_string()
        } else {
            self.bin_path.to_string_lossy().to_string()
        }
    }

    pub fn setup_config(&self) -> Result<()> {
        let parent = self.config_path.parent().unwrap();
        fs::create_dir_all(parent)?;

        let config_content = format!(
            r#"
# NATS server configuration for wasmCloud
port: 4222
http_port: 8222

# Enable JetStream
jetstream {{
    store_dir: "{}/nats_storage"
    max_file_store: 2G
}}

# Enable Leaf Node support for wasmCloud
leafnodes {{
    port: 7422
}}

# Enable Websocket for UI
websocket {{
    port: 8080
    no_tls: true
}}
"#,
            parent.to_string_lossy()
        );

        fs::write(&self.config_path, config_content)?;
        Ok(())
    }

    pub fn start(&self) -> Result<()> {
        self.setup_config()?;
        let bin = self.get_bin();
        if !self.is_running() {
            let _child = Command::new(&bin)
                .args(["-c", self.config_path.to_str().unwrap()])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()?;
        }
        Ok(())
    }

    /// Non-blocking TCP probe on port 4222 — safe to call from any context.
    pub fn is_running(&self) -> bool {
        use std::net::{TcpStream, ToSocketAddrs};
        use std::time::Duration;
        let addr = "127.0.0.1:4222";
        TcpStream::connect_timeout(
            &addr.to_socket_addrs()
                .ok()
                .and_then(|mut a| a.next())
                .unwrap_or_else(|| "127.0.0.1:4222".parse().unwrap()),
            Duration::from_millis(300),
        )
        .is_ok()
    }

    /// Async: write config + start sidecar. Runs blocking ops on thread pool.
    pub async fn start_async(&self) -> Result<()> {
        let bin_path   = self.bin_path.clone();
        let config_path = self.config_path.clone();
        tokio::task::spawn_blocking(move || -> Result<()> {
            // Write config
            let parent = config_path.parent().unwrap();
            std::fs::create_dir_all(parent)?;
            let storage_dir = parent.join("nats_storage");
            std::fs::create_dir_all(&storage_dir)?;
            let cfg = format!(
                r#"
port: 4222
http_port: 8222

jetstream {{
    store_dir: "{}"
    max_file_store: 2G
}}

leafnodes {{
    port: 7422
}}

websocket {{
    port: 8080
    no_tls: true
}}
"#,
                storage_dir.display()
            );
            std::fs::write(&config_path, cfg)?;

            // Only start if not already listening
            let already_up = {
                use std::net::{TcpStream, ToSocketAddrs};
                use std::time::Duration;
                TcpStream::connect_timeout(
                    &"127.0.0.1:4222".to_socket_addrs()
                        .unwrap().next().unwrap(),
                    Duration::from_millis(300),
                )
                .is_ok()
            };

            if !already_up {
                let bin = if crate::core::packages::which("nats-server") {
                    "nats-server".to_string()
                } else {
                    bin_path.to_string_lossy().to_string()
                };
                Command::new(&bin)
                    .args(["-c", config_path.to_str().unwrap()])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()?;
            }
            Ok(())
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking: {}", e))?
    }

    /// Async: initialise JetStream buckets + streams. Runs CLI on thread pool.
    pub async fn init_wasmcloud_buckets_async(&self) -> Result<()> {
        tokio::task::spawn_blocking(move || {
            let (cli, srv_args): (&str, &[&str]) = if crate::core::packages::which("wash") {
                ("wash", &[])
            } else if crate::core::packages::which("nats") {
                ("nats", &["-s", "nats://127.0.0.1:4222"])
            } else {
                return; // wasmCloud will create them on first start
            };

            let run = |extra: &[&'static str]| {
                let _ = Command::new(cli)
                    .args(srv_args)
                    .args(extra)
                    .output();
            };

            run(&["kv", "create", "LATTICEDATA_default"]);
            run(&["kv", "create", "wadm_manifests"]);
            run(&["stream", "add", "wadm_events",
                  "--subjects", "wadm.evt.*",
                  "--storage", "file", "--replicas", "1", "--defaults"]);
            run(&["stream", "add", "wadm_commands",
                  "--subjects", "wadm.cmd.*",
                  "--storage", "file", "--replicas", "1", "--defaults"]);
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking: {}", e))
    }

    pub fn init_wasmcloud_buckets(&self) -> Result<()> {
        // Determine which CLI is available: prefer `wash`, fall back to `nats`.
        let (cli, base_args): (&str, &[&str]) = if crate::core::packages::which("wash") {
            ("wash", &[])
        } else if crate::core::packages::which("nats") {
            ("nats", &["-s", "nats://127.0.0.1:4222"])
        } else {
            // No CLI available; wasmCloud itself will create the buckets on first start.
            return Ok(());
        };

        // Helper closure
        let run = |args: &[&str]| -> bool {
            Command::new(cli)
                .args(base_args)
                .args(args)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        };

        // KV bucket: LATTICEDATA_default  (wasmCloud lattice metadata)
        run(&["kv", "create", "LATTICEDATA_default"]);

        // KV bucket: wadm_manifests  (WADM deployment YAML storage)
        run(&["kv", "create", "wadm_manifests"]);

        // Stream: wadm_events
        run(&["stream", "add", "wadm_events",
              "--subjects", "wadm.evt.*",
              "--retention", "limits",
              "--storage", "file",
              "--replicas", "1",
              "--defaults"]);

        // Stream: wadm_commands
        run(&["stream", "add", "wadm_commands",
              "--subjects", "wadm.cmd.*",
              "--retention", "limits",
              "--storage", "file",
              "--replicas", "1",
              "--defaults"]);

        Ok(())
    }

    pub fn get_storage_usage(&self) -> Option<u64> {
        // Parse the nats varz endpoint or check the size of the nats_storage directory
        let parent = self.config_path.parent().unwrap();
        let storage_dir = parent.join("nats_storage");
        
        if storage_dir.exists() {
            let mut size = 0;
            if let Ok(entries) = walkdir::WalkDir::new(&storage_dir).into_iter().collect::<Result<Vec<_>, _>>() {
                for entry in entries {
                    if let Ok(metadata) = entry.metadata() {
                        size += metadata.len();
                    }
                }
            }
            Some(size)
        } else {
            None
        }
    }

    pub fn is_synced(&self) -> bool {
        // This would ideally check against the NATS JetStream API to see if
        // the required buckets exist. For now, just return true if NATS is running.
        self.is_running()
    }
}
