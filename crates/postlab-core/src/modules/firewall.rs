use anyhow::{bail, Result};
use crate::ssh::SshSession;

pub async fn enable_firewall(_ssh: &SshSession) -> Result<String> {
    bail!("firewall module not yet implemented")
}

pub async fn allow_port(_ssh: &SshSession, _port: u16, _proto: &str) -> Result<String> {
    bail!("firewall module not yet implemented")
}

pub async fn deny_port(_ssh: &SshSession, _port: u16, _proto: &str) -> Result<String> {
    bail!("firewall module not yet implemented")
}

pub async fn firewall_status(_ssh: &SshSession) -> Result<String> {
    bail!("firewall module not yet implemented")
}
