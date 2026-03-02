use anyhow::Result;
use crate::ssh::SshSession;

#[derive(Debug, Clone, PartialEq)]
pub enum OsFamily {
    Ubuntu,
    Fedora,
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum PkgManager {
    Apt,
    Dnf,
}

impl OsFamily {
    /// Auto-detect OS by reading /etc/os-release over SSH.
    pub async fn detect_remote(ssh: &SshSession) -> Result<Self> {
        let output = ssh.exec("cat /etc/os-release").await?;
        Ok(Self::parse_os_release(&output))
    }

    fn parse_os_release(content: &str) -> Self {
        for line in content.lines() {
            if let Some(id) = line.strip_prefix("ID=") {
                let id = id.trim().trim_matches('"').to_lowercase();
                return match id.as_str() {
                    "ubuntu" | "debian" => OsFamily::Ubuntu,
                    "fedora" | "rhel" | "centos" | "rocky" | "almalinux" => OsFamily::Fedora,
                    other => OsFamily::Unknown(other.to_string()),
                };
            }
        }
        OsFamily::Unknown("undetected".to_string())
    }

    pub fn pkg_manager(&self) -> PkgManager {
        match self {
            OsFamily::Ubuntu => PkgManager::Apt,
            OsFamily::Fedora => PkgManager::Dnf,
            OsFamily::Unknown(_) => PkgManager::Apt, // best-effort fallback
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            OsFamily::Ubuntu => "ubuntu",
            OsFamily::Fedora => "fedora",
            OsFamily::Unknown(s) => s.as_str(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_ubuntu() {
        let content = r#"NAME="Ubuntu"\nID=ubuntu\nVERSION_ID="22.04""#;
        assert_eq!(OsFamily::parse_os_release(content), OsFamily::Ubuntu);
    }

    #[test]
    fn detects_fedora() {
        let content = r#"NAME=Fedora\nID=fedora\nVERSION_ID=39"#;
        assert_eq!(OsFamily::parse_os_release(content), OsFamily::Fedora);
    }

    #[test]
    fn unknown_os() {
        let content = r#"NAME=Arch\nID=arch"#;
        assert_eq!(
            OsFamily::parse_os_release(content),
            OsFamily::Unknown("arch".to_string())
        );
    }
}
