use anyhow::Result;
use async_trait::async_trait;

use super::FirewallManager;
use crate::core::models::FirewallRule;

/// macOS pf (Packet Filter) backend.
/// Write operations require root — this impl reports status/rules read-only
/// and bails with a helpful message for mutations.
pub struct PfManager;

#[async_trait]
impl FirewallManager for PfManager {
    async fn status(&self) -> Result<(bool, String)> {
        let out = tokio::process::Command::new("pfctl")
            .args(["-s", "info"])
            .output()
            .await?;
        let text = String::from_utf8_lossy(&out.stdout);
        // "Status: Enabled" or "Status: Disabled"
        let enabled = text
            .lines()
            .any(|l| l.trim().to_lowercase().starts_with("status") && l.contains("Enabled"));
        Ok((enabled, "pf".to_string()))
    }

    async fn list_rules(&self) -> Result<Vec<FirewallRule>> {
        let out = tokio::process::Command::new("pfctl")
            .args(["-s", "rules"])
            .output()
            .await?;
        let text = String::from_utf8_lossy(&out.stdout);
        Ok(parse_pf_rules(&text))
    }

    async fn add_rule(&self, _port: &str, _proto: &str, _from: &str, _action: &str) -> Result<()> {
        anyhow::bail!(
            "pf rule modifications require root and direct edits to /etc/pf.conf. \
             Use `sudo pfctl` or a tool like Little Snitch on macOS."
        )
    }

    async fn delete_rule(&self, _num: usize) -> Result<()> {
        anyhow::bail!(
            "pf rule modifications require root and direct edits to /etc/pf.conf. \
             Use `sudo pfctl` or a tool like Little Snitch on macOS."
        )
    }

    async fn set_enabled(&self, enabled: bool) -> Result<()> {
        let flag = if enabled { "-e" } else { "-d" };
        let out = tokio::process::Command::new("pfctl")
            .arg(flag)
            .output()
            .await?;
        if !out.status.success() {
            anyhow::bail!(
                "pfctl {} failed (may need sudo): {}",
                flag,
                String::from_utf8_lossy(&out.stderr)
            );
        }
        Ok(())
    }
}

/// Parse `pfctl -s rules` output.
///
/// Example lines:
///   `pass  in all flags S/SA keep state`
///   `block drop out proto tcp from any to any port 23`
fn parse_pf_rules(text: &str) -> Vec<FirewallRule> {
    let mut rules = Vec::new();

    for (i, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let lower = trimmed.to_lowercase();
        let action = if lower.starts_with("pass") {
            "ALLOW"
        } else if lower.starts_with("block") {
            "DENY"
        } else {
            continue;
        };

        // Extract port if present
        let port = extract_pf_port(trimmed);
        // Extract source (from <addr>)
        let from = extract_pf_from(trimmed);

        rules.push(FirewallRule {
            num: i + 1,
            to: port.unwrap_or_else(|| "any".to_string()),
            action: action.to_string(),
            from: from.unwrap_or_else(|| "any".to_string()),
        });
    }

    rules
}

fn extract_pf_port(rule: &str) -> Option<String> {
    let idx = rule.find(" port ")?;
    let after = rule[idx + 6..].trim();
    let port = after.split_whitespace().next()?;
    Some(port.to_string())
}

fn extract_pf_from(rule: &str) -> Option<String> {
    let idx = rule.find(" from ")?;
    let after = rule[idx + 6..].trim();
    let addr = after.split_whitespace().next()?;
    if addr == "any" {
        None
    } else {
        Some(addr.to_string())
    }
}
