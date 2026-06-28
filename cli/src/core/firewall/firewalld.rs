use anyhow::Result;
use async_trait::async_trait;

use super::FirewallManager;
use crate::core::models::FirewallRule;

pub struct FirewalldManager;

#[async_trait]
impl FirewallManager for FirewalldManager {
    async fn status(&self) -> Result<(bool, String)> {
        let out = tokio::process::Command::new("firewall-cmd")
            .arg("--state")
            .output()
            .await?;
        let enabled =
            out.status.success() && String::from_utf8_lossy(&out.stdout).trim() == "running";
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
        // firewalld requires an explicit protocol; "any" maps to both tcp and udp.
        let single = [proto];
        let protos: &[&str] = if proto == "any" || proto.is_empty() {
            &["tcp", "udp"]
        } else {
            &single
        };
        let deny = action.to_lowercase() == "deny";
        for p in protos {
            if deny {
                let rule = format!(
                    "rule family=\"ipv4\" port port=\"{}\" protocol=\"{}\" reject",
                    port, p
                );
                run_firewall_cmd(&["--add-rich-rule", rule.as_str(), "--permanent"]).await?;
            } else {
                let spec = format!("{}/{}", port, p);
                run_firewall_cmd(&["--add-port", spec.as_str(), "--permanent"]).await?;
            }
        }
        run_firewall_cmd(&["--reload"]).await?;
        Ok(())
    }

    async fn delete_rule(&self, num: usize) -> Result<()> {
        let rules = self.list_rules().await?;
        let rule = rules
            .into_iter()
            .find(|r| r.num == num)
            .ok_or_else(|| anyhow::anyhow!("Rule {} not found", num))?;

        // Rich rules are stored verbatim as firewalld reports them (they start
        // with "rule "); remove them as-is. Simple ports are stored as "port/proto".
        if rule.to.starts_with("rule ") {
            run_firewall_cmd(&["--remove-rich-rule", rule.to.as_str(), "--permanent"]).await?;
        } else {
            run_firewall_cmd(&["--remove-port", rule.to.as_str(), "--permanent"]).await?;
        }
        run_firewall_cmd(&["--reload"]).await?;
        Ok(())
    }

    async fn set_enabled(&self, enabled: bool) -> Result<()> {
        let action = if enabled { "start" } else { "stop" };
        let out = tokio::process::Command::new("systemctl")
            .args([action, "firewalld"])
            .output()
            .await?;
        if !out.status.success() {
            anyhow::bail!(
                "systemctl {} firewalld: {}",
                action,
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        Ok(())
    }
}

async fn run_firewall_cmd(args: &[&str]) -> Result<()> {
    let out = tokio::process::Command::new("firewall-cmd")
        .args(args)
        .output()
        .await?;
    if !out.status.success() {
        anyhow::bail!(
            "firewall-cmd {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(())
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
