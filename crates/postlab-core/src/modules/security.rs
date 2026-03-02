use anyhow::Result;
use crate::ssh::SshSession;

/// Path where harden-security scripts are expected on the remote server.
const HARDEN_SCRIPTS_PATH: &str = "/opt/postlab/harden-security";

/// Run the full hardening suite (run-enable-all.sh) on the remote server.
pub async fn harden_server(ssh: &SshSession) -> Result<String> {
    let cmd = format!("bash {HARDEN_SCRIPTS_PATH}/run-enable-all.sh");
    ssh.exec(&cmd).await
}

/// Run a specific hardening module (e.g., "05-firewall").
pub async fn harden_module(ssh: &SshSession, module: &str) -> Result<String> {
    let cmd = format!("bash {HARDEN_SCRIPTS_PATH}/{module}.enable.sh");
    ssh.exec(&cmd).await
}

/// Disable a specific hardening module.
pub async fn disable_module(ssh: &SshSession, module: &str) -> Result<String> {
    let cmd = format!("bash {HARDEN_SCRIPTS_PATH}/{module}.disable.sh");
    ssh.exec(&cmd).await
}

/// Run smoke tests to verify hardening state.
pub async fn smoke_test(ssh: &SshSession) -> Result<String> {
    let cmd = format!("bash {HARDEN_SCRIPTS_PATH}/smoke-test.sh");
    ssh.exec(&cmd).await
}
