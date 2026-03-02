use anyhow::{bail, Result};
use crate::ssh::SshSession;

pub async fn install_docker(_ssh: &SshSession) -> Result<String> {
    bail!("docker module not yet implemented")
}

pub async fn list_containers(_ssh: &SshSession) -> Result<String> {
    bail!("docker module not yet implemented")
}

pub async fn compose_up(_ssh: &SshSession, _project_dir: &str) -> Result<String> {
    bail!("docker module not yet implemented")
}

pub async fn compose_down(_ssh: &SshSession, _project_dir: &str) -> Result<String> {
    bail!("docker module not yet implemented")
}
