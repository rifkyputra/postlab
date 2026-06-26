use anyhow::Result;
use serde::Deserialize;
use tokio::process::Command;

#[derive(Debug, Clone)]
pub struct TailscalePeer {
    pub name: String,
    pub ip: String,
    pub online: bool,
}

#[derive(Debug, Clone, Default)]
pub struct TailscaleStatus {
    pub backend_state: String,
    pub self_ip: Option<String>,
    pub self_name: Option<String>,
    pub peers: Vec<TailscalePeer>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct StatusJson {
    backend_state: Option<String>,
    #[serde(rename = "Self")]
    self_node: Option<SelfNode>,
    #[serde(rename = "Peer")]
    peer: Option<std::collections::HashMap<String, PeerNode>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct SelfNode {
    #[serde(rename = "DNSName")]
    dns_name: Option<String>,
    #[serde(rename = "TailscaleIPs")]
    tailscale_ips: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct PeerNode {
    #[serde(rename = "DNSName")]
    dns_name: Option<String>,
    #[serde(rename = "TailscaleIPs")]
    tailscale_ips: Option<Vec<String>>,
    online: Option<bool>,
}

pub struct TailscaleManager;

impl TailscaleManager {
    pub async fn is_installed(&self) -> bool {
        Command::new("tailscale")
            .arg("version")
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    pub async fn version(&self) -> Option<String> {
        let out = Command::new("tailscale")
            .arg("version")
            .output()
            .await
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let s = String::from_utf8_lossy(&out.stdout);
        Some(s.lines().next().unwrap_or("").trim().to_string())
    }

    pub async fn status(&self) -> Result<TailscaleStatus> {
        let out = Command::new("tailscale")
            .args(["status", "--json"])
            .output()
            .await?;

        let json: StatusJson = serde_json::from_slice(&out.stdout)?;

        let backend_state = json
            .backend_state
            .unwrap_or_else(|| "Unknown".to_string());

        let (self_ip, self_name) = json
            .self_node
            .map(|n| {
                let ip = n
                    .tailscale_ips
                    .as_ref()
                    .and_then(|ips| ips.first())
                    .cloned();
                let name = n
                    .dns_name
                    .map(|d| d.trim_end_matches('.').to_string());
                (ip, name)
            })
            .unwrap_or((None, None));

        let mut peers: Vec<TailscalePeer> = json
            .peer
            .unwrap_or_default()
            .into_values()
            .map(|p| {
                let ip = p
                    .tailscale_ips
                    .as_ref()
                    .and_then(|ips| ips.first())
                    .cloned()
                    .unwrap_or_default();
                let name = p
                    .dns_name
                    .map(|d| d.trim_end_matches('.').to_string())
                    .unwrap_or_else(|| ip.clone());
                let online = p.online.unwrap_or(false);
                TailscalePeer { name, ip, online }
            })
            .collect();

        peers.sort_by(|a, b| {
            b.online
                .cmp(&a.online)
                .then_with(|| a.name.cmp(&b.name))
        });

        Ok(TailscaleStatus {
            backend_state,
            self_ip,
            self_name,
            peers,
        })
    }

    pub async fn up(&self) -> Result<()> {
        let out = Command::new("tailscale").arg("up").output().await?;
        if out.status.success() {
            Ok(())
        } else {
            anyhow::bail!(
                "tailscale up failed: {}",
                String::from_utf8_lossy(&out.stderr)
            )
        }
    }

    pub async fn down(&self) -> Result<()> {
        let out = Command::new("tailscale").arg("down").output().await?;
        if out.status.success() {
            Ok(())
        } else {
            anyhow::bail!(
                "tailscale down failed: {}",
                String::from_utf8_lossy(&out.stderr)
            )
        }
    }

    pub async fn install(&self, tx: tokio::sync::mpsc::UnboundedSender<String>) -> Result<()> {
        let _ = tx.send("Downloading Tailscale install script…".to_string());
        let out = Command::new("sh")
            .args(["-c", "curl -fsSL https://tailscale.com/install.sh | sh"])
            .output()
            .await?;

        let combined = format!(
            "{}\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        for line in combined.lines() {
            let _ = tx.send(line.to_string());
        }

        if out.status.success() {
            Ok(())
        } else {
            anyhow::bail!("Tailscale install failed")
        }
    }
}
