use super::TunnelManager;
use crate::core::models::{Tunnel, TunnelRoute};
use anyhow::Result;
use async_trait::async_trait;

pub struct CloudflareManager;

async fn run(args: &[&str]) -> Result<String> {
    let out = tokio::process::Command::new("cloudflared")
        .args(args)
        .output()
        .await?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        anyhow::bail!("{}", String::from_utf8_lossy(&out.stderr).trim())
    }
}

#[async_trait]
impl TunnelManager for CloudflareManager {
    async fn is_installed(&self) -> bool {
        tokio::process::Command::new("cloudflared")
            .arg("--version")
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    async fn version(&self) -> Option<String> {
        let out = tokio::process::Command::new("cloudflared")
            .arg("--version")
            .output()
            .await
            .ok()?;
        // cloudflared writes version to stderr, not stdout
        let s = String::from_utf8_lossy(&out.stderr);
        let s = if s.trim().is_empty() { String::from_utf8_lossy(&out.stdout) } else { s };
        Some(s.trim().to_string())
    }

    async fn install(&self) -> Result<String> {
        if crate::core::packages::which("apt-get") {
            // Cloudflare's cloudflare-main.gpg is already a binary DER keyring (not ASCII-armored).
            // Download it directly — do NOT pipe through `gpg --dearmor` or it errors.
            let script = r#"
                curl -fsSL https://pkg.cloudflare.com/cloudflare-main.gpg -o /usr/share/keyrings/cloudflare-main.gpg
                echo "deb [signed-by=/usr/share/keyrings/cloudflare-main.gpg] https://pkg.cloudflare.com/ $(lsb_release -cs) main" | tee /etc/apt/sources.list.d/cloudflared.list
                apt-get update && apt-get install -y cloudflared
            "#;
            let out = tokio::process::Command::new("sh")
                .args(["-c", script])
                .output()
                .await?;
            if !out.status.success() {
                anyhow::bail!("{}", String::from_utf8_lossy(&out.stderr).trim());
            }
            return Ok(String::from_utf8_lossy(&out.stdout).to_string());
        }
        if crate::core::packages::which("dnf") || crate::core::packages::which("yum") {
            // Cloudflare's official RPM repo method
            let pm = if crate::core::packages::which("dnf") { "dnf" } else { "yum" };
            let script = format!(r#"
                rpm --import https://pkg.cloudflare.com/cloudflare-main.gpg
                cat > /etc/yum.repos.d/cloudflared.repo << 'EOF'
[cloudflared]
name=cloudflared
baseurl=https://pkg.cloudflare.com/cloudflared/rpm/
enabled=1
gpgcheck=1
gpgkey=https://pkg.cloudflare.com/cloudflare-main.gpg
EOF
                {pm} install -y cloudflared
            "#, pm = pm);
            let out = tokio::process::Command::new("sh")
                .args(["-c", &script])
                .output()
                .await?;
            if !out.status.success() {
                anyhow::bail!("{}", String::from_utf8_lossy(&out.stderr).trim());
            }
            return Ok(String::from_utf8_lossy(&out.stdout).to_string());
        }
        if crate::core::packages::which("pacman") {
            let out = tokio::process::Command::new("sh")
                .args(["-c", "pacman -Sy --noconfirm cloudflared"])
                .output()
                .await?;
            if !out.status.success() {
                anyhow::bail!("{}", String::from_utf8_lossy(&out.stderr).trim());
            }
            return Ok(String::from_utf8_lossy(&out.stdout).to_string());
        }
        if crate::core::packages::which("brew") {
            // Delegate to BrewManager which handles the root→sudo-u-SUDO_USER case.
            let mgr = crate::core::packages::BrewManager;
            use crate::core::packages::PackageManager;
            return mgr.install("cloudflare/cloudflare/cloudflared").await;
        }
        anyhow::bail!("Please install cloudflared manually from https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/downloads/")
    }

    async fn install_streamed(
        &self,
        tx: tokio::sync::mpsc::UnboundedSender<String>,
    ) -> Result<String> {
        use crate::core::packages::run_cmd_streaming;
        if crate::core::packages::which("apt-get") {
            // cloudflare-main.gpg is a binary DER keyring — download directly, no dearmoring.
            let script = concat!(
                "curl -fsSL https://pkg.cloudflare.com/cloudflare-main.gpg -o /usr/share/keyrings/cloudflare-main.gpg && ",
                "echo \"deb [signed-by=/usr/share/keyrings/cloudflare-main.gpg]",
                " https://pkg.cloudflare.com/ $(lsb_release -cs) main\"",
                " | tee /etc/apt/sources.list.d/cloudflared.list && ",
                "apt-get update && apt-get install -y cloudflared",
            );
            return run_cmd_streaming("sh", &["-c", script], tx).await;
        }
        if crate::core::packages::which("dnf") || crate::core::packages::which("yum") {
            let pm = if crate::core::packages::which("dnf") { "dnf" } else { "yum" };
            let script = format!(
                "rpm --import https://pkg.cloudflare.com/cloudflare-main.gpg && \
                 printf '[cloudflared]\\nname=cloudflared\\n\
                 baseurl=https://pkg.cloudflare.com/cloudflared/rpm/\\nenabled=1\\n\
                 gpgcheck=1\\ngpgkey=https://pkg.cloudflare.com/cloudflare-main.gpg\\n' \
                 > /etc/yum.repos.d/cloudflared.repo && \
                 {pm} install -y cloudflared",
                pm = pm,
            );
            return run_cmd_streaming("sh", &["-c", &script], tx).await;
        }
        if crate::core::packages::which("pacman") {
            return run_cmd_streaming("sh", &["-c", "pacman -Sy --noconfirm cloudflared"], tx).await;
        }
        if crate::core::packages::which("brew") {
            use crate::core::packages::PackageManager;
            return crate::core::packages::BrewManager
                .install_streamed("cloudflare/cloudflare/cloudflared", tx)
                .await;
        }
        anyhow::bail!("Please install cloudflared manually from https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/downloads/")
    }

    async fn login(&self) -> Result<()> {
        // Opens browser for Cloudflare auth; runs in foreground intentionally
        tokio::process::Command::new("cloudflared")
            .args(["tunnel", "login"])
            .status()
            .await?;
        Ok(())
    }

    async fn list_tunnels(&self) -> Result<Vec<Tunnel>> {
        let out = run(&["tunnel", "list", "--output", "json"]).await?;
        // Parse JSON: [{id, name, status, ...}]
        let raw: Vec<serde_json::Value> = serde_json::from_str(&out).unwrap_or_default();
        Ok(raw
            .into_iter()
            .map(|v| Tunnel {
                id: v["id"].as_str().unwrap_or("").to_string(),
                name: v["name"].as_str().unwrap_or("").to_string(),
                status: v["status"].as_str().unwrap_or("unknown").to_string(),
            })
            .collect())
    }

    async fn create(&self, name: &str) -> Result<Tunnel> {
        let out = run(&["tunnel", "create", name]).await?;
        // Output (multi-line): "INF Created tunnel <name> with id <uuid>"
        // Find the line containing "with id" and take the last token (the UUID).
        let id = out
            .lines()
            .find(|l| l.contains("with id"))
            .and_then(|l| l.split_whitespace().last())
            .unwrap_or("")
            .to_string();
        if id.is_empty() {
            anyhow::bail!("could not parse tunnel id from: {}", out);
        }
        Ok(Tunnel { name: name.to_string(), id, status: "created".to_string() })
    }

    async fn add_route(&self, route: TunnelRoute) -> Result<()> {
        // Route DNS: cloudflared tunnel route dns <tunnel-id> <hostname>
        run(&["tunnel", "route", "dns", &route.tunnel_id, &route.hostname]).await?;
        add_domain_to_config(&tunnel_config_path(&route.tunnel_id), &route).await?;
        Ok(())
    }

    async fn add_domain_to_config(&self, route: TunnelRoute) -> Result<()> {
        add_domain_to_config(&tunnel_config_path(&route.tunnel_id), &route).await
    }

    async fn install_service(&self, tunnel_id: &str) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            // Write the LaunchAgent plist pointing to this tunnel's per-tunnel config,
            // then load it. Uses `cloudflared tunnel run` — no cert.pem required.
            let home = std::env::var("HOME").unwrap_or_default();
            let cfg  = tunnel_config_path(tunnel_id);

            let which = tokio::process::Command::new("which")
                .arg("cloudflared").output().await?;
            let binary = String::from_utf8_lossy(&which.stdout).trim().to_string();
            if binary.is_empty() {
                anyhow::bail!("cloudflared not found in PATH — install it first ([i])");
            }

            let plist_content = format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
                 <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
                 <plist version=\"1.0\">\n<dict>\n\
                     <key>Label</key><string>com.cloudflare.cloudflared</string>\n\
                     <key>ProgramArguments</key>\n\
                     <array>\n\
                         <string>{binary}</string>\n\
                         <string>--config</string>\n\
                         <string>{cfg}</string>\n\
                         <string>tunnel</string>\n\
                         <string>run</string>\n\
                     </array>\n\
                     <key>RunAtLoad</key><true/>\n\
                     <key>KeepAlive</key><true/>\n\
                     <key>StandardOutPath</key><string>/tmp/cloudflared.out</string>\n\
                     <key>StandardErrorPath</key><string>/tmp/cloudflared.err</string>\n\
                 </dict>\n</plist>\n"
            );

            let agents_dir = format!("{}/Library/LaunchAgents", home);
            let plist_path = format!("{}/{}", agents_dir, CF_PLIST);
            tokio::fs::create_dir_all(&agents_dir).await?;
            tokio::fs::write(&plist_path, plist_content).await?;

            // Unload stale copy if present, then load (registers + starts)
            let _ = tokio::process::Command::new("launchctl")
                .args(["unload", &plist_path]).output().await;
            let out = tokio::process::Command::new("launchctl")
                .args(["load", &plist_path]).output().await?;
            if out.status.success() {
                Ok(())
            } else {
                let msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
                anyhow::bail!("{}", if msg.is_empty() { "launchctl load failed".to_string() } else { msg })
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let src = tunnel_config_path(tunnel_id);
            // If the per-tunnel config doesn't exist yet, create a minimal one so
            // cloudflared can authenticate even before routes are added via [d].
            if !std::path::Path::new(&src).exists() {
                let home = std::env::var("HOME").unwrap_or_default();
                let minimal = format!(
                    "tunnel: {id}\ncredentials-file: {home}/.cloudflared/{id}.json\n\ningress:\n  - service: http_status:404\n",
                    id = tunnel_id,
                    home = home,
                );
                tokio::fs::create_dir_all(cf_dir()).await?;
                tokio::fs::write(&src, minimal).await?;
            }
            let out = tokio::process::Command::new("sh")
                .args(["-c", &format!(
                    "sudo mkdir -p /etc/cloudflared && \
                     sudo cp {src} /etc/cloudflared/config.yml && \
                     sudo cloudflared --config /etc/cloudflared/config.yml service install"
                )])
                .output().await?;
            if out.status.success() { Ok(()) } else {
                anyhow::bail!("{}", String::from_utf8_lossy(&out.stderr).trim())
            }
        }
    }

    async fn config_content(&self, tunnel_id: &str) -> Result<String> {
        Ok(tokio::fs::read_to_string(tunnel_config_path(tunnel_id)).await
            .unwrap_or_else(|_| format!("(~/.cloudflared/{}.yaml not found)", tunnel_id)))
    }

    async fn service_status(&self) -> Result<(bool, bool)> {
        svc_status().await
    }

    async fn service_start(&self) -> Result<()> {
        svc_action("start").await
    }

    async fn service_stop(&self) -> Result<()> {
        svc_action("stop").await
    }

    async fn service_restart(&self) -> Result<()> {
        svc_action("restart").await
    }

    async fn remove_ingress(&self, tunnel_id: &str, hostname: &str) -> Result<()> {
        let path = tunnel_config_path(tunnel_id);
        let content = tokio::fs::read_to_string(&path).await?;
        let updated = remove_hostname_from_yaml(&content, hostname);
        tokio::fs::write(&path, updated).await?;
        Ok(())
    }

    async fn sync_config(&self) -> Result<()> {
        // macOS/Homebrew: cloudflared reads ~/.cloudflared/config.yaml directly
        // — just restart. Linux: copy to /etc/cloudflared then restart.
        #[cfg(target_os = "macos")]
        {
            svc_action("restart").await
        }
        #[cfg(not(target_os = "macos"))]
        {
            let src = format!("{}/config.yml", cf_dir());
            let out = tokio::process::Command::new("sh")
                .args(["-c", &format!(
                    "sudo mkdir -p /etc/cloudflared && \
                     sudo cp {src} /etc/cloudflared/config.yml && \
                     sudo systemctl restart cloudflared"
                )])
                .output().await?;
            if out.status.success() { Ok(()) } else {
                anyhow::bail!("{}", String::from_utf8_lossy(&out.stderr).trim())
            }
        }
    }
}

// ── OS-aware service helpers ───────────────────────────────────────────────
//
// macOS: `cloudflared service install` creates a LaunchAgent plist.
//   Label:  com.cloudflare.cloudflared
//   Plist:  ~/Library/LaunchAgents/com.cloudflare.cloudflared.plist  (user install)
//           /Library/LaunchDaemons/com.cloudflare.cloudflared.plist  (system install)
//   Start:  launchctl start com.cloudflare.cloudflared
// Linux: systemd unit "cloudflared".

#[cfg(target_os = "macos")]
const CF_LABEL: &str = "com.cloudflare.cloudflared";
#[cfg(target_os = "macos")]
const CF_PLIST: &str = "com.cloudflare.cloudflared.plist";

/// Returns (active, enabled).
async fn svc_status() -> Result<(bool, bool)> {
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").unwrap_or_default();
        // Enabled = LaunchAgent or LaunchDaemon plist exists
        let user_plist  = format!("{}/Library/LaunchAgents/{}", home, CF_PLIST);
        let sys_plist   = format!("/Library/LaunchDaemons/{}", CF_PLIST);
        let enabled = std::path::Path::new(&user_plist).exists()
            || std::path::Path::new(&sys_plist).exists();

        // Active = label appears in `launchctl list` with a numeric PID (not "-")
        // Output columns: PID   Status  Label
        let out = tokio::process::Command::new("sh")
            .args(["-c", &format!("launchctl list 2>/dev/null | grep {}", CF_LABEL)])
            .output().await
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default();
        // First column is "-" when stopped, a number when running.
        let active = !out.is_empty() && !out.starts_with('-');

        Ok((active, enabled))
    }
    #[cfg(not(target_os = "macos"))]
    {
        let active = tokio::process::Command::new("systemctl")
            .args(["is-active", "--quiet", "cloudflared"])
            .output().await.map(|o| o.status.success()).unwrap_or(false);
        let enabled = tokio::process::Command::new("systemctl")
            .args(["is-enabled", "--quiet", "cloudflared"])
            .output().await.map(|o| o.status.success()).unwrap_or(false);
        Ok((active, enabled))
    }
}

/// action = "start" | "stop" | "restart"
async fn svc_action(action: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        // Restart: ignore stop errors (service may already be stopped/crashed), only fail on start.
        if action == "restart" {
            let _ = tokio::process::Command::new("launchctl")
                .args(["stop", CF_LABEL]).output().await;
            tokio::time::sleep(std::time::Duration::from_millis(400)).await;
            let out = tokio::process::Command::new("launchctl")
                .args(["start", CF_LABEL]).output().await?;
            if !out.status.success() {
                let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
                let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
                let msg = if !stderr.is_empty() { stderr } else if !stdout.is_empty() { stdout }
                    else { "launchctl start failed — run [s] to install the service first".to_string() };
                anyhow::bail!("{}", msg);
            }
            return Ok(());
        }

        let (cmd, args): (&str, &[&str]) = match action {
            "start" => ("launchctl", &["start", CF_LABEL]),
            "stop"  => ("launchctl", &["stop",  CF_LABEL]),
            _       => anyhow::bail!("unknown action: {}", action),
        };
        let out = tokio::process::Command::new(cmd)
            .args(args)
            .output().await?;
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
            let msg = if !stderr.is_empty() { stderr } else if !stdout.is_empty() { stdout }
                else { format!("launchctl {} failed — run [s] to install the service first", action) };
            anyhow::bail!("{}", msg)
        }
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let out = tokio::process::Command::new("sudo")
            .args(["systemctl", action, "cloudflared"])
            .output().await?;
        if out.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
            let msg = if !stderr.is_empty() { stderr } else if !stdout.is_empty() { stdout } else { format!("systemctl {} cloudflared failed", action) };
            anyhow::bail!("{}", msg)
        }
    }
}

fn cf_dir() -> String {
    format!("{}/.cloudflared", std::env::var("HOME").unwrap_or_default())
}

/// Per-tunnel config path: `~/.cloudflared/<tunnel-id>.yaml`
fn tunnel_config_path(tunnel_id: &str) -> String {
    format!("{}/.cloudflared/{}.yaml", std::env::var("HOME").unwrap_or_default(), tunnel_id)
}

/// Appends a hostname+service ingress entry to `~/.cloudflared/<tunnel-id>.yaml`.
/// Creates the file from scratch if it doesn't exist.
/// The config header always matches the tunnel_id because each tunnel has its own file.
async fn add_domain_to_config(config_path: &str, route: &TunnelRoute) -> Result<()> {
    let new_entry = format!(
        "  - hostname: {}\n    service: {}\n",
        route.hostname, route.service
    );

    match tokio::fs::read_to_string(config_path).await {
        Ok(existing) => {
            // Hostname already present — update its service line only.
            if existing.contains(&format!("hostname: {}", route.hostname)) {
                let updated = replace_service_for_hostname(&existing, &route.hostname, &route.service);
                tokio::fs::write(config_path, updated).await?;
                return Ok(());
            }

            const CATCHALL: &str = "  - service: http_status:404\n";
            let updated = if let Some(pos) = existing.find(CATCHALL) {
                let mut s = existing.clone();
                s.insert_str(pos, &new_entry);
                s
            } else {
                format!("{}\n{}{}", existing.trim_end_matches('\n'), new_entry, CATCHALL)
            };
            tokio::fs::write(config_path, updated).await?;
        }
        Err(_) => {
            // File doesn't exist — create from scratch with correct tunnel header.
            let home = std::env::var("HOME").unwrap_or_default();
            let config = format!(
                "tunnel: {id}\ncredentials-file: {home}/.cloudflared/{id}.json\n\ningress:\n{entry}  - service: http_status:404\n",
                id = route.tunnel_id,
                home = home,
                entry = new_entry,
            );
            tokio::fs::create_dir_all(cf_dir()).await?;
            tokio::fs::write(config_path, config).await?;
        }
    }

    Ok(())
}

/// Removes the `- hostname: X` + `    service: Y` pair for a given hostname.
fn remove_hostname_from_yaml(yaml: &str, hostname: &str) -> String {
    let needle = format!("hostname: {}", hostname);
    let lines: Vec<&str> = yaml.lines().collect();
    let mut result: Vec<&str> = Vec::with_capacity(lines.len());
    let mut i = 0;
    while i < lines.len() {
        if lines[i].contains(&needle) {
            // Skip this hostname line; also skip the immediately following service line
            i += 1;
            if i < lines.len() && lines[i].trim_start().starts_with("service:") {
                i += 1;
            }
        } else {
            result.push(lines[i]);
            i += 1;
        }
    }
    let mut out = result.join("\n");
    if yaml.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// Replaces the `service:` value for a given hostname in the YAML text.
fn replace_service_for_hostname(yaml: &str, hostname: &str, new_service: &str) -> String {
    let needle = format!("hostname: {}", hostname);
    let lines: Vec<&str> = yaml.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        if lines[i].contains(&needle) && i + 1 < lines.len() {
            // The next non-empty line belonging to this entry should be `    service: …`
            if lines[i + 1].trim_start().starts_with("service:") {
                let indent: String = lines[i + 1]
                    .chars()
                    .take_while(|c| c.is_whitespace())
                    .collect();
                // We can't mutate `lines` directly (it borrows yaml), so we rebuild.
                let mut out = yaml.to_string();
                let old_line = lines[i + 1];
                let new_line = format!("{}service: {}", indent, new_service);
                out = out.replacen(old_line, &new_line, 1);
                return out;
            }
        }
        i += 1;
    }
    yaml.to_string()
}
