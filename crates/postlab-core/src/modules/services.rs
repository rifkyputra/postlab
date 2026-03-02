use anyhow::{bail, Result};
use crate::ssh::SshSession;

pub async fn list_services(_ssh: &SshSession) -> Result<String> {
    bail!("services module not yet implemented")
}

pub async fn start_service(_ssh: &SshSession, _name: &str) -> Result<String> {
    bail!("services module not yet implemented")
}

pub async fn stop_service(_ssh: &SshSession, _name: &str) -> Result<String> {
    bail!("services module not yet implemented")
}

pub async fn restart_service(_ssh: &SshSession, _name: &str) -> Result<String> {
    bail!("services module not yet implemented")
}

pub async fn service_status(_ssh: &SshSession, _name: &str) -> Result<String> {
    bail!("services module not yet implemented")
}
