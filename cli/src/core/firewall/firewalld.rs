use anyhow::Result;
use async_trait::async_trait;

use crate::core::models::FirewallRule;
use super::FirewallManager;

pub struct FirewalldManager;

#[async_trait]
impl FirewallManager for FirewalldManager {
    async fn status(&self) -> Result<(bool, String)> {
        let out = tokio::process::Command::new("firewall-cmd")
            .arg("--state")
            .output()
            .await?;
        let enabled = out.status.success()
            && String::from_utf8_lossy(&out.stdout).trim() == "running";
        Ok((enabled, "firewalld".to_string()))
    }

    async fn list_rules(&self) -> Result<Vec<FirewallRule>> {
        // Collect simple port rules
        let ports_out = tokio::process::Command::new("firewall-cmd")
            .args(["--list-ports"])
            .output()
            .await?;
        let ports_text = String::from_utf8_lossy(&ports_out.stdout);

        // Collect rich rules
        let rich_out = tokio::process::Command::new("firewall-cmd")
            .args(["--list-rich-rules"])
            .output()
            .await?;
        let rich_text = String::from_utf8_lossy(&rich_out.stdout);

        Ok(parse_firewalld_rules(&ports_text, &rich_text))
    }

    async fn add_rule(&self, port: &str, proto: &str, _from: &str, action: &str) -> Result<()> {
        if action.to_lowercase() == "deny" {
            // Rich rule for reject
            let rule = format!(
                "rule family=ipv4 port port=\"{}\" protocol=\"{}\" reject",
                port, proto
            );
            tokio::process::Command::new("firewall-cmd")
                .args(["--add-rich-rule", &rule, "--permanent"])
                .output()
                .await?;
        } else {
            let spec = if proto == "any" || proto.is_empty() {
                port.to_string()
            } else {
                format!("{}/{}", port, proto)
            };
            tokio::process::Command::new("firewall-cmd")
                .args(["--add-port", &spec, "--permanent"])
                .output()
                .await?;
        }
        tokio::process::Command::new("firewall-cmd")
            .arg("--reload")
            .output()
            .await?;
        Ok(())
    }

    async fn delete_rule(&self, num: usize) -> Result<()> {
        let rules = self.list_rules().await?;
        let rule = rules
            .into_iter()
            .find(|r| r.num == num)
            .ok_or_else(|| anyhow::anyhow!("Rule {} not found", num))?;

        if rule.action.to_lowercase().contains("deny")
            || rule.action.to_lowercase().contains("reject")
        {
            let (port, proto) = split_port_proto(&rule.to);
            let rich = format!(
                "rule family=ipv4 port port=\"{}\" protocol=\"{}\" reject",
                port, proto
            );
            tokio::process::Command::new("firewall-cmd")
                .args(["--remove-rich-rule", &rich, "--permanent"])
                .output()
                .await?;
        } else {
            tokio::process::Command::new("firewall-cmd")
                .args(["--remove-port", &rule.to, "--permanent"])
                .output()
                .await?;
        }
        tokio::process::Command::new("firewall-cmd")
            .arg("--reload")
            .output()
            .await?;
        Ok(())
    }

    async fn set_enabled(&self, enabled: bool) -> Result<()> {
        let action = if enabled { "start" } else { "stop" };
        tokio::process::Command::new("systemctl")
            .args([action, "firewalld"])
            .output()
            .await?;
        Ok(())
    }
}

fn split_port_proto(spec: &str) -> (&str, &str) {
    if let Some(idx) = spec.find('/') {
        (&spec[..idx], &spec[idx + 1..])
    } else {
        (spec, "tcp")
    }
}

fn parse_firewalld_rules(ports_text: &str, rich_text: &str) -> Vec<FirewallRule> {
    let mut rules = Vec::new();
    let mut num = 1usize;

    // Simple ports: "80/tcp 443/tcp 22/tcp"
    for token in ports_text.split_whitespace() {
        if token.is_empty() {
            continue;
        }
        rules.push(FirewallRule {
            num,
            to: token.to_string(),
            action: "ALLOW".to_string(),
            from: "Anywhere".to_string(),
        });
        num += 1;
    }

    // Rich rules: one per line
    for line in rich_text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Determine action from rule text
        let action = if trimmed.contains("accept") {
            "ALLOW"
        } else if trimmed.contains("reject") || trimmed.contains("drop") {
            "DENY"
        } else {
            "ALLOW"
        };
        rules.push(FirewallRule {
            num,
            to: trimmed.to_string(),
            action: action.to_string(),
            from: "Anywhere".to_string(),
        });
        num += 1;
    }

    rules
}
