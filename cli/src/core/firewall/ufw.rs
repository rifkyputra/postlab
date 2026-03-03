use anyhow::Result;
use async_trait::async_trait;

use crate::core::models::FirewallRule;
use super::FirewallManager;

pub struct UfwManager;

#[async_trait]
impl FirewallManager for UfwManager {
    async fn status(&self) -> Result<(bool, String)> {
        let out = tokio::process::Command::new("ufw")
            .args(["status"])
            .output()
            .await?;
        let text = String::from_utf8_lossy(&out.stdout);
        let enabled = text.lines().next().map(|l| l.contains("active")).unwrap_or(false);
        Ok((enabled, "ufw".to_string()))
    }

    async fn list_rules(&self) -> Result<Vec<FirewallRule>> {
        let out = tokio::process::Command::new("ufw")
            .args(["status", "numbered"])
            .output()
            .await?;
        let text = String::from_utf8_lossy(&out.stdout);
        Ok(parse_ufw_rules(&text))
    }

    async fn add_rule(&self, port: &str, proto: &str, from: &str, action: &str) -> Result<()> {
        // Build: ufw [allow|deny] [proto tcp|udp] [from <from>] [to any port <port>]
        let mut args: Vec<String> = vec![action.to_lowercase()];
        if proto != "any" && !proto.is_empty() {
            args.push("proto".to_string());
            args.push(proto.to_string());
        }
        if !from.is_empty() && from != "any" {
            args.push("from".to_string());
            args.push(from.to_string());
        }
        if !port.is_empty() {
            args.push("to".to_string());
            args.push("any".to_string());
            args.push("port".to_string());
            args.push(port.to_string());
        }
        tokio::process::Command::new("ufw")
            .args(&args)
            .output()
            .await?;
        Ok(())
    }

    async fn delete_rule(&self, num: usize) -> Result<()> {
        tokio::process::Command::new("ufw")
            .args(["--force", "delete", &num.to_string()])
            .output()
            .await?;
        Ok(())
    }

    async fn set_enabled(&self, enabled: bool) -> Result<()> {
        let cmd = if enabled { "enable" } else { "disable" };
        tokio::process::Command::new("ufw")
            .args(["--force", cmd])
            .output()
            .await?;
        Ok(())
    }
}

/// Parse `ufw status numbered` output into `FirewallRule` entries.
///
/// Example lines:
///   `[ 1] 22/tcp                     ALLOW IN    Anywhere`
///   `[ 2] 22/tcp (v6)                ALLOW IN    Anywhere (v6)`
fn parse_ufw_rules(text: &str) -> Vec<FirewallRule> {
    let mut rules = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('[') {
            continue;
        }

        // Extract rule number from "[N]"
        let Some(bracket_end) = trimmed.find(']') else { continue };
        let num_str = trimmed[1..bracket_end].trim();
        let Ok(num) = num_str.parse::<usize>() else { continue };

        let body = trimmed[bracket_end + 1..].trim();

        // Find the first occurrence of an action keyword preceded by whitespace.
        let action_keywords = ["ALLOW", "DENY", "REJECT", "LIMIT"];
        let action_start = action_keywords.iter().filter_map(|kw| {
            body.find(kw).filter(|&pos| {
                // Must be at the start or preceded by a space
                pos == 0 || body.as_bytes().get(pos - 1) == Some(&b' ')
            })
        }).min();

        let Some(action_start) = action_start else { continue };
        let to = body[..action_start].trim().to_string();
        let after = &body[action_start..];

        // Split action from "from" on double (or more) spaces
        let (action, from) = if let Some(sep) = after.find("  ") {
            let action = after[..sep].trim().to_string();
            let from = after[sep..].trim().to_string();
            (action, from)
        } else {
            (after.trim().to_string(), "Anywhere".to_string())
        };

        if !to.is_empty() {
            rules.push(FirewallRule { num, to, action, from });
        }
    }

    rules
}
