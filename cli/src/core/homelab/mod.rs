use std::{
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const KEEP_AWAKE_TARGETS: [&str; 4] = [
    "sleep.target",
    "suspend.target",
    "hibernate.target",
    "hybrid-sleep.target",
];
const KEEP_AWAKE_OWNERSHIP: &str = "postlab/homelab/keep-awake-targets";
const LOGIND_DROP_IN: &str = "systemd/logind.conf.d/90-postlab-homelab.conf";
const LOGIND_CONFIG: &[u8] = b"[Login]\nIdleAction=ignore\nHandleLidSwitch=ignore\nHandleLidSwitchExternalPower=ignore\nHandleLidSwitchDocked=ignore\n";

fn parse_owned_targets(content: &str) -> Vec<&'static str> {
    KEEP_AWAKE_TARGETS
        .into_iter()
        .filter(|target| content.lines().any(|line| line == *target))
        .collect()
}

#[derive(Debug, Clone, Copy)]
enum InterfaceKind {
    Wired,
    Wireless,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NetworkManagerProfile {
    uuid: String,
    interface: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NetworkManagerProfileSnapshot {
    profile: NetworkManagerProfile,
    value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NetworkInterfaceSnapshot {
    interface: String,
    runtime_value: String,
    profiles: Vec<NetworkManagerProfileSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CompletedNetworkChange {
    program: PathBuf,
    rollback_args: Vec<String>,
    description: String,
}

fn parse_network_manager_profiles(
    output: &str,
    kind: InterfaceKind,
    interfaces: &[String],
) -> Vec<NetworkManagerProfile> {
    let expected_type = match kind {
        InterfaceKind::Wired => "802-3-ethernet",
        InterfaceKind::Wireless => "802-11-wireless",
    };
    let mut profiles = output
        .lines()
        .filter_map(|line| {
            let fields = split_nmcli_fields(line);
            if fields.len() != 3
                || fields[1] != expected_type
                || !interfaces.iter().any(|name| name == &fields[2])
            {
                return None;
            }
            Some(NetworkManagerProfile {
                uuid: fields[0].clone(),
                interface: fields[2].clone(),
            })
        })
        .collect::<Vec<_>>();
    profiles.sort_by(|left, right| {
        left.interface
            .cmp(&right.interface)
            .then_with(|| left.uuid.cmp(&right.uuid))
    });
    profiles
}

static TEMP_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn trusted_binary(name: &str) -> Option<PathBuf> {
    ["/usr/bin", "/usr/sbin", "/bin", "/sbin"]
        .into_iter()
        .map(|directory| Path::new(directory).join(name))
        .find(|path| path.is_file())
}

async fn run_network_command(
    program: &Path,
    args: &[&str],
    failure_context: impl AsRef<str>,
) -> Result<(), String> {
    let output = tokio::process::Command::new(program)
        .args(args)
        .output()
        .await
        .map_err(|error| format!("{}: {error}", failure_context.as_ref()))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stderr = stderr.trim();
    if stderr.is_empty() {
        Err(format!(
            "{}: command exited with {}",
            failure_context.as_ref(),
            output.status
        ))
    } else {
        Err(format!("{}: {stderr}", failure_context.as_ref()))
    }
}

async fn atomic_write(path: &Path, content: &[u8]) -> std::io::Result<()> {
    let parent = path.parent().unwrap();
    tokio::fs::create_dir_all(parent).await?;
    let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let file_name = path.file_name().unwrap().to_string_lossy();
    let temporary = parent.join(format!(
        ".{file_name}.postlab-tmp-{}-{sequence}",
        std::process::id()
    ));
    tokio::fs::write(&temporary, content).await?;
    if let Err(error) = tokio::fs::rename(&temporary, path).await {
        let _ = tokio::fs::remove_file(&temporary).await;
        return Err(error);
    }
    Ok(())
}

fn nmcli_enum_prefix(value: &str) -> Option<&str> {
    value
        .trim()
        .split(|character: char| character.is_whitespace() || character == '(')
        .next()
        .filter(|prefix| !prefix.is_empty())
}

fn nmcli_wake_on_lan_has_magic(value: &str) -> bool {
    nmcli_enum_prefix(value).is_some_and(|prefix| {
        prefix.eq_ignore_ascii_case("magic")
            || prefix.parse::<u32>().is_ok_and(|flags| flags & 64 == 64)
    })
}

fn nmcli_wifi_power_saving_is_disabled(value: &str) -> bool {
    nmcli_enum_prefix(value).is_some_and(|prefix| {
        prefix == "2"
            || prefix.eq_ignore_ascii_case("disable")
            || prefix.eq_ignore_ascii_case("disabled")
    })
}

fn wake_on_lan_mode(output: &str) -> Option<&str> {
    output.lines().find_map(|line| {
        line.trim()
            .strip_prefix("Wake-on:")
            .map(str::trim)
            .filter(|mode| {
                !mode.is_empty()
                    && mode.chars().all(|character| {
                        matches!(
                            character,
                            'p' | 'u' | 'm' | 'b' | 'a' | 'g' | 's' | 'f' | 'd'
                        )
                    })
            })
    })
}

fn wifi_power_saving_value(output: &str) -> Option<&'static str> {
    let value = output.lines().find_map(|line| {
        line.trim()
            .to_ascii_lowercase()
            .strip_prefix("power save:")
            .map(str::trim)
            .map(str::to_string)
    })?;
    match value.as_str() {
        "on" => Some("on"),
        "off" => Some("off"),
        _ => None,
    }
}

fn split_nmcli_fields(line: &str) -> Vec<String> {
    let mut fields = vec![String::new()];
    let mut escaped = false;
    for character in line.chars() {
        if escaped {
            fields.last_mut().unwrap().push(character);
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == ':' {
            fields.push(String::new());
        } else {
            fields.last_mut().unwrap().push(character);
        }
    }
    if escaped {
        fields.last_mut().unwrap().push('\\');
    }
    fields
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HomelabFeature {
    KeepAwake,
    AutomaticSleep,
    WakeOnLan,
    WifiPowerSaving,
}

impl HomelabFeature {
    pub const ALL: [Self; 4] = [
        Self::KeepAwake,
        Self::AutomaticSleep,
        Self::WakeOnLan,
        Self::WifiPowerSaving,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::KeepAwake => "Keep it awake",
            Self::AutomaticSleep => "Disable automatic sleep/hibernation",
            Self::WakeOnLan => "Wake-on-LAN",
            Self::WifiPowerSaving => "Wi-Fi server stability",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HomelabFeatureStatus {
    Enabled,
    Disabled,
    Unavailable,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HomelabStatus {
    pub feature: HomelabFeature,
    pub status: HomelabFeatureStatus,
    pub detail: String,
}

#[derive(Debug, Clone)]
pub struct HomelabManager {
    linux: bool,
    systemd: bool,
    etc_root: PathBuf,
    sys_root: PathBuf,
    systemctl: Option<PathBuf>,
    ethtool: Option<PathBuf>,
    nmcli: Option<PathBuf>,
    iw: Option<PathBuf>,
}

impl HomelabManager {
    pub fn new(linux: bool) -> Self {
        Self {
            linux,
            systemd: crate::core::services::is_systemd_available(),
            etc_root: PathBuf::from("/etc"),
            sys_root: PathBuf::from("/sys"),
            systemctl: trusted_binary("systemctl"),
            ethtool: trusted_binary("ethtool"),
            nmcli: trusted_binary("nmcli"),
            iw: trusted_binary("iw"),
        }
    }

    pub async fn set(&self, feature: HomelabFeature, enabled: bool) -> HomelabStatus {
        if !self.linux {
            return HomelabStatus {
                feature,
                status: HomelabFeatureStatus::Unavailable,
                detail: "Available on Linux only".to_string(),
            };
        }

        match feature {
            HomelabFeature::KeepAwake => self.set_keep_awake(enabled).await,
            HomelabFeature::AutomaticSleep => self.set_automatic_sleep(enabled).await,
            HomelabFeature::WakeOnLan => self.set_wake_on_lan(enabled).await,
            HomelabFeature::WifiPowerSaving => self.set_wifi_power_saving(enabled).await,
        }
    }

    async fn network_manager_profile_snapshot(
        &self,
        nmcli: &Path,
        feature: HomelabFeature,
        profile: &NetworkManagerProfile,
        property: &'static str,
    ) -> Result<NetworkManagerProfileSnapshot, HomelabStatus> {
        let output = match tokio::process::Command::new(nmcli)
            .args(["-g", property, "connection", "show", &profile.uuid])
            .output()
            .await
        {
            Ok(output) if output.status.success() => output,
            Ok(output) => {
                return Err(self.error(
                    feature,
                    format!(
                        "Could not inspect NetworkManager profile {}: {}",
                        profile.uuid,
                        String::from_utf8_lossy(&output.stderr).trim()
                    ),
                ));
            }
            Err(error) => {
                return Err(self.error(
                    feature,
                    format!(
                        "Could not inspect NetworkManager profile {}: {error}",
                        profile.uuid
                    ),
                ));
            }
        };
        let output = String::from_utf8_lossy(&output.stdout);
        let Some(value) = nmcli_enum_prefix(&output) else {
            return Err(self.error(
                feature,
                format!(
                    "NetworkManager profile {} returned an empty {property} value",
                    profile.uuid
                ),
            ));
        };
        Ok(NetworkManagerProfileSnapshot {
            profile: profile.clone(),
            value: value.to_string(),
        })
    }

    async fn network_transaction_error(
        &self,
        feature: HomelabFeature,
        failure: String,
        completed: &[CompletedNetworkChange],
    ) -> HomelabStatus {
        if completed.is_empty() {
            return self.error(feature, format!("{failure}; rollback not needed"));
        }

        let mut rollback_failures = Vec::new();
        for change in completed.iter().rev() {
            let args = change
                .rollback_args
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>();
            if let Err(error) =
                run_network_command(&change.program, &args, &change.description).await
            {
                rollback_failures.push(error);
            }
        }

        if rollback_failures.is_empty() {
            self.error(
                feature,
                format!(
                    "{failure}; rollback succeeded for {} attempted change(s)",
                    completed.len()
                ),
            )
        } else {
            self.error(
                feature,
                format!(
                    "{failure}; rollback failed: {}",
                    rollback_failures.join("; ")
                ),
            )
        }
    }

    async fn set_wake_on_lan(&self, enabled: bool) -> HomelabStatus {
        let Some(ethtool) = &self.ethtool else {
            return self.unavailable(HomelabFeature::WakeOnLan, "ethtool is unavailable");
        };
        let Some(nmcli) = &self.nmcli else {
            return self.unavailable(HomelabFeature::WakeOnLan, "NetworkManager is unavailable");
        };
        let interfaces = match self.interfaces(InterfaceKind::Wired).await {
            Ok(interfaces) if interfaces.is_empty() => {
                return self.unavailable(
                    HomelabFeature::WakeOnLan,
                    "No physical wired interfaces were found",
                );
            }
            Ok(interfaces) => interfaces,
            Err(error) => {
                return self.error(
                    HomelabFeature::WakeOnLan,
                    format!("Could not inspect network interfaces: {error}"),
                );
            }
        };
        let profiles = match self
            .network_manager_profiles(nmcli, InterfaceKind::Wired, &interfaces)
            .await
        {
            Ok(profiles) => profiles,
            Err(status) => return status,
        };

        let mut snapshots = Vec::new();
        for interface in interfaces {
            let output = match tokio::process::Command::new(ethtool)
                .arg(&interface)
                .output()
                .await
            {
                Ok(output) if output.status.success() => output,
                Ok(output) => {
                    return self.error(
                        HomelabFeature::WakeOnLan,
                        format!(
                            "Could not inspect {interface}: {}",
                            String::from_utf8_lossy(&output.stderr).trim()
                        ),
                    );
                }
                Err(error) if error.kind() == ErrorKind::NotFound => {
                    return self.unavailable(HomelabFeature::WakeOnLan, "ethtool is unavailable");
                }
                Err(error) => {
                    return self.error(
                        HomelabFeature::WakeOnLan,
                        format!("Could not inspect {interface}: {error}"),
                    );
                }
            };
            let output = String::from_utf8_lossy(&output.stdout);
            let supports_magic = output.lines().any(|line| {
                line.trim()
                    .strip_prefix("Supports Wake-on:")
                    .is_some_and(|modes| modes.trim().contains('g'))
            });
            if !supports_magic {
                continue;
            }
            let Some(runtime_value) = wake_on_lan_mode(&output) else {
                return self.error(
                    HomelabFeature::WakeOnLan,
                    format!("Could not parse the current Wake-on-LAN mode for {interface}"),
                );
            };
            let interface_profiles = profiles
                .iter()
                .filter(|profile| profile.interface == interface)
                .collect::<Vec<_>>();
            if interface_profiles.is_empty() {
                return self.unavailable(
                    HomelabFeature::WakeOnLan,
                    format!("NetworkManager does not own {interface}"),
                );
            }
            let mut profile_snapshots = Vec::with_capacity(interface_profiles.len());
            for profile in interface_profiles {
                match self
                    .network_manager_profile_snapshot(
                        nmcli,
                        HomelabFeature::WakeOnLan,
                        profile,
                        "802-3-ethernet.wake-on-lan",
                    )
                    .await
                {
                    Ok(snapshot) => profile_snapshots.push(snapshot),
                    Err(status) => return status,
                }
            }
            snapshots.push(NetworkInterfaceSnapshot {
                interface,
                runtime_value: runtime_value.to_string(),
                profiles: profile_snapshots,
            });
        }
        if snapshots.is_empty() {
            return self.unavailable(
                HomelabFeature::WakeOnLan,
                "No wired interfaces support magic-packet wake",
            );
        }

        let profile_value = if enabled { "magic" } else { "none" };
        let runtime_mode = if enabled { "g" } else { "d" };
        let mut completed = Vec::new();
        for snapshot in &snapshots {
            for profile in &snapshot.profiles {
                completed.push(CompletedNetworkChange {
                    program: nmcli.clone(),
                    rollback_args: vec![
                        "connection".to_string(),
                        "modify".to_string(),
                        profile.profile.uuid.clone(),
                        "802-3-ethernet.wake-on-lan".to_string(),
                        profile.value.clone(),
                    ],
                    description: format!(
                        "Could not roll back NetworkManager profile {}",
                        profile.profile.uuid
                    ),
                });
                let failure_context = format!(
                    "Could not update NetworkManager profile {}",
                    profile.profile.uuid
                );
                if let Err(failure) = run_network_command(
                    nmcli,
                    &[
                        "connection",
                        "modify",
                        &profile.profile.uuid,
                        "802-3-ethernet.wake-on-lan",
                        profile_value,
                    ],
                    failure_context,
                )
                .await
                {
                    return self
                        .network_transaction_error(HomelabFeature::WakeOnLan, failure, &completed)
                        .await;
                }
            }
            completed.push(CompletedNetworkChange {
                program: ethtool.clone(),
                rollback_args: vec![
                    "-s".to_string(),
                    snapshot.interface.clone(),
                    "wol".to_string(),
                    snapshot.runtime_value.clone(),
                ],
                description: format!("Could not roll back {}", snapshot.interface),
            });
            if let Err(failure) = run_network_command(
                ethtool,
                &["-s", &snapshot.interface, "wol", runtime_mode],
                format!("Could not update {}", snapshot.interface),
            )
            .await
            {
                return self
                    .network_transaction_error(HomelabFeature::WakeOnLan, failure, &completed)
                    .await;
            }
        }

        HomelabStatus {
            feature: HomelabFeature::WakeOnLan,
            status: if enabled {
                HomelabFeatureStatus::Enabled
            } else {
                HomelabFeatureStatus::Disabled
            },
            detail: snapshots
                .iter()
                .map(|snapshot| snapshot.interface.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        }
    }

    async fn set_wifi_power_saving(&self, enabled: bool) -> HomelabStatus {
        let Some(iw) = &self.iw else {
            return self.unavailable(HomelabFeature::WifiPowerSaving, "iw is unavailable");
        };
        let Some(nmcli) = &self.nmcli else {
            return self.unavailable(
                HomelabFeature::WifiPowerSaving,
                "NetworkManager is unavailable",
            );
        };
        let interfaces = match self.interfaces(InterfaceKind::Wireless).await {
            Ok(interfaces) if interfaces.is_empty() => {
                return self.unavailable(
                    HomelabFeature::WifiPowerSaving,
                    "No wireless interfaces were found",
                );
            }
            Ok(interfaces) => interfaces,
            Err(error) => {
                return self.error(
                    HomelabFeature::WifiPowerSaving,
                    format!("Could not inspect wireless interfaces: {error}"),
                );
            }
        };
        let profiles = match self
            .network_manager_profiles(nmcli, InterfaceKind::Wireless, &interfaces)
            .await
        {
            Ok(profiles) => profiles,
            Err(status) => {
                return HomelabStatus {
                    feature: HomelabFeature::WifiPowerSaving,
                    ..status
                };
            }
        };

        let mut snapshots = Vec::with_capacity(interfaces.len());
        for interface in interfaces {
            let interface_profiles = profiles
                .iter()
                .filter(|profile| profile.interface == interface)
                .collect::<Vec<_>>();
            if interface_profiles.is_empty() {
                return self.unavailable(
                    HomelabFeature::WifiPowerSaving,
                    format!("NetworkManager does not own {interface}"),
                );
            }
            let output = match tokio::process::Command::new(iw)
                .args(["dev", &interface, "get", "power_save"])
                .output()
                .await
            {
                Ok(output) if output.status.success() => output,
                Ok(output) => {
                    return self.error(
                        HomelabFeature::WifiPowerSaving,
                        format!(
                            "Could not inspect {interface}: {}",
                            String::from_utf8_lossy(&output.stderr).trim()
                        ),
                    );
                }
                Err(error) if error.kind() == ErrorKind::NotFound => {
                    return self.unavailable(HomelabFeature::WifiPowerSaving, "iw is unavailable");
                }
                Err(error) => {
                    return self.error(
                        HomelabFeature::WifiPowerSaving,
                        format!("Could not inspect {interface}: {error}"),
                    );
                }
            };
            let output = String::from_utf8_lossy(&output.stdout);
            let Some(runtime_value) = wifi_power_saving_value(&output) else {
                return self.error(
                    HomelabFeature::WifiPowerSaving,
                    format!("Could not parse the current Wi-Fi power saving value for {interface}"),
                );
            };
            let mut profile_snapshots = Vec::with_capacity(interface_profiles.len());
            for profile in interface_profiles {
                match self
                    .network_manager_profile_snapshot(
                        nmcli,
                        HomelabFeature::WifiPowerSaving,
                        profile,
                        "802-11-wireless.powersave",
                    )
                    .await
                {
                    Ok(snapshot) => profile_snapshots.push(snapshot),
                    Err(status) => return status,
                }
            }
            snapshots.push(NetworkInterfaceSnapshot {
                interface,
                runtime_value: runtime_value.to_string(),
                profiles: profile_snapshots,
            });
        }

        let profile_value = if enabled { "2" } else { "3" };
        let runtime_value = if enabled { "off" } else { "on" };
        let mut completed = Vec::new();
        for snapshot in &snapshots {
            for profile in &snapshot.profiles {
                completed.push(CompletedNetworkChange {
                    program: nmcli.clone(),
                    rollback_args: vec![
                        "connection".to_string(),
                        "modify".to_string(),
                        profile.profile.uuid.clone(),
                        "802-11-wireless.powersave".to_string(),
                        profile.value.clone(),
                    ],
                    description: format!(
                        "Could not roll back NetworkManager profile {}",
                        profile.profile.uuid
                    ),
                });
                let failure_context = format!(
                    "Could not update NetworkManager profile {}",
                    profile.profile.uuid
                );
                if let Err(failure) = run_network_command(
                    nmcli,
                    &[
                        "connection",
                        "modify",
                        &profile.profile.uuid,
                        "802-11-wireless.powersave",
                        profile_value,
                    ],
                    failure_context,
                )
                .await
                {
                    return self
                        .network_transaction_error(
                            HomelabFeature::WifiPowerSaving,
                            failure,
                            &completed,
                        )
                        .await;
                }
            }
            completed.push(CompletedNetworkChange {
                program: iw.clone(),
                rollback_args: vec![
                    "dev".to_string(),
                    snapshot.interface.clone(),
                    "set".to_string(),
                    "power_save".to_string(),
                    snapshot.runtime_value.clone(),
                ],
                description: format!("Could not roll back {}", snapshot.interface),
            });
            if let Err(failure) = run_network_command(
                iw,
                &[
                    "dev",
                    &snapshot.interface,
                    "set",
                    "power_save",
                    runtime_value,
                ],
                format!("Could not update {}", snapshot.interface),
            )
            .await
            {
                return self
                    .network_transaction_error(HomelabFeature::WifiPowerSaving, failure, &completed)
                    .await;
            }
        }

        HomelabStatus {
            feature: HomelabFeature::WifiPowerSaving,
            status: if enabled {
                HomelabFeatureStatus::Enabled
            } else {
                HomelabFeatureStatus::Disabled
            },
            detail: snapshots
                .iter()
                .map(|snapshot| snapshot.interface.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        }
    }

    async fn set_automatic_sleep(&self, enabled: bool) -> HomelabStatus {
        if !self.systemd {
            return self.unavailable(
                HomelabFeature::AutomaticSleep,
                "systemd-logind is unavailable",
            );
        }
        if self.systemctl.is_none() {
            return self.unavailable(HomelabFeature::AutomaticSleep, "systemctl is unavailable");
        }
        if !enabled {
            let path = self.etc_root.join(LOGIND_DROP_IN);
            match tokio::fs::read(&path).await {
                Ok(content) if content == LOGIND_CONFIG => {}
                Ok(_) => {
                    return self.error(
                        HomelabFeature::AutomaticSleep,
                        "Managed logind configuration was modified; refusing to remove it",
                    );
                }
                Err(error) if error.kind() == ErrorKind::NotFound => {
                    return HomelabStatus {
                        feature: HomelabFeature::AutomaticSleep,
                        status: HomelabFeatureStatus::Disabled,
                        detail: "Postlab logind configuration is already absent".to_string(),
                    };
                }
                Err(error) => {
                    return self.error(
                        HomelabFeature::AutomaticSleep,
                        format!("Could not read managed logind configuration: {error}"),
                    );
                }
            }
            if let Err(error) = tokio::fs::remove_file(&path).await {
                return self.error(
                    HomelabFeature::AutomaticSleep,
                    format!("Could not remove managed logind configuration: {error}"),
                );
            }
            if let Err(row) = self.hup_logind().await {
                return row;
            }
            return HomelabStatus {
                feature: HomelabFeature::AutomaticSleep,
                status: HomelabFeatureStatus::Disabled,
                detail: "Postlab logind configuration was removed".to_string(),
            };
        }

        let path = self.etc_root.join(LOGIND_DROP_IN);
        match tokio::fs::read(&path).await {
            Ok(content) if content == LOGIND_CONFIG => {
                return HomelabStatus {
                    feature: HomelabFeature::AutomaticSleep,
                    status: HomelabFeatureStatus::Enabled,
                    detail: "Automatic sleep and lid actions are already ignored".to_string(),
                };
            }
            Ok(_) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => {
                return self.error(
                    HomelabFeature::AutomaticSleep,
                    format!("Could not read managed logind configuration: {error}"),
                );
            }
        }
        if let Err(error) = atomic_write(&path, LOGIND_CONFIG).await {
            return self.error(
                HomelabFeature::AutomaticSleep,
                format!("Could not write managed logind configuration: {error}"),
            );
        }
        if let Err(row) = self.hup_logind().await {
            return row;
        }
        HomelabStatus {
            feature: HomelabFeature::AutomaticSleep,
            status: HomelabFeatureStatus::Enabled,
            detail: "Automatic sleep and lid actions are ignored".to_string(),
        }
    }

    async fn hup_logind(&self) -> Result<(), HomelabStatus> {
        let Some(systemctl) = &self.systemctl else {
            return Err(
                self.unavailable(HomelabFeature::AutomaticSleep, "systemctl is unavailable")
            );
        };
        let output = match tokio::process::Command::new(systemctl)
            .args(["kill", "--signal=HUP", "systemd-logind.service"])
            .output()
            .await
        {
            Ok(output) => output,
            Err(error) if error.kind() == ErrorKind::NotFound => {
                return Err(
                    self.unavailable(HomelabFeature::AutomaticSleep, "systemctl is unavailable")
                );
            }
            Err(error) => {
                return Err(self.error(
                    HomelabFeature::AutomaticSleep,
                    format!("Could not signal systemd-logind: {error}"),
                ));
            }
        };
        if !output.status.success() {
            return Err(self.error(
                HomelabFeature::AutomaticSleep,
                format!(
                    "Could not signal systemd-logind: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ),
            ));
        }
        Ok(())
    }

    async fn set_keep_awake(&self, enabled: bool) -> HomelabStatus {
        if !self.systemd {
            return self.unavailable(HomelabFeature::KeepAwake, "systemd is unavailable");
        }
        let Some(systemctl) = &self.systemctl else {
            return self.unavailable(HomelabFeature::KeepAwake, "systemctl is unavailable");
        };
        let mut owned =
            match tokio::fs::read_to_string(self.etc_root.join(KEEP_AWAKE_OWNERSHIP)).await {
                Ok(content) => parse_owned_targets(&content),
                Err(error) if error.kind() == ErrorKind::NotFound => Vec::new(),
                Err(error) => {
                    return self.error(
                        HomelabFeature::KeepAwake,
                        format!("Could not read keep-awake ownership: {error}"),
                    );
                }
            };

        if !enabled {
            for target in owned.clone() {
                let output = match tokio::process::Command::new(systemctl)
                    .args(["unmask", target])
                    .output()
                    .await
                {
                    Ok(output) => output,
                    Err(error) if error.kind() == ErrorKind::NotFound => {
                        return self
                            .unavailable(HomelabFeature::KeepAwake, "systemctl is unavailable");
                    }
                    Err(error) => {
                        return self.error(
                            HomelabFeature::KeepAwake,
                            format!("Could not unmask {target}: {error}"),
                        );
                    }
                };
                if !output.status.success() {
                    return self.error(
                        HomelabFeature::KeepAwake,
                        format!(
                            "Could not unmask {target}: {}",
                            String::from_utf8_lossy(&output.stderr).trim()
                        ),
                    );
                }
                owned.retain(|owned_target| *owned_target != target);
                if let Err(error) = self.write_owned_targets(&owned).await {
                    return self.error(
                        HomelabFeature::KeepAwake,
                        format!("Could not update keep-awake ownership: {error}"),
                    );
                }
            }
            if let Err(error) = self.write_owned_targets(&owned).await {
                return self.error(
                    HomelabFeature::KeepAwake,
                    format!("Could not remove keep-awake ownership: {error}"),
                );
            }
            let effective = self.keep_awake_status().await;
            return if effective.status == HomelabFeatureStatus::Disabled {
                HomelabStatus {
                    feature: HomelabFeature::KeepAwake,
                    status: HomelabFeatureStatus::Disabled,
                    detail: "Postlab-owned sleep target masks were removed".to_string(),
                }
            } else if effective.status == HomelabFeatureStatus::Enabled {
                self.error(
                    HomelabFeature::KeepAwake,
                    "Sleep targets remain masked outside Postlab ownership",
                )
            } else {
                effective
            };
        }

        for target in KEEP_AWAKE_TARGETS {
            let output = match tokio::process::Command::new(systemctl)
                .args(["is-enabled", target])
                .output()
                .await
            {
                Ok(output) => output,
                Err(error) if error.kind() == ErrorKind::NotFound => {
                    return self.unavailable(HomelabFeature::KeepAwake, "systemctl is unavailable");
                }
                Err(error) => {
                    return self.error(
                        HomelabFeature::KeepAwake,
                        format!("Could not inspect {target}: {error}"),
                    );
                }
            };
            let state = String::from_utf8_lossy(&output.stdout);
            if state.trim() == "masked" {
                continue;
            }
            if !output.status.success()
                && !matches!(
                    state.trim(),
                    "enabled"
                        | "enabled-runtime"
                        | "linked"
                        | "linked-runtime"
                        | "alias"
                        | "static"
                        | "indirect"
                        | "generated"
                        | "transient"
                        | "disabled"
                        | "not-found"
                )
            {
                return self.error(
                    HomelabFeature::KeepAwake,
                    format!(
                        "Could not inspect {target}: {}",
                        String::from_utf8_lossy(&output.stderr).trim()
                    ),
                );
            }

            if !owned.contains(&target) {
                owned.push(target);
            }
            if let Err(error) = self.write_owned_targets(&owned).await {
                return self.error(
                    HomelabFeature::KeepAwake,
                    format!("Could not record keep-awake ownership before masking: {error}"),
                );
            }

            let output = match tokio::process::Command::new(systemctl)
                .args(["mask", target])
                .output()
                .await
            {
                Ok(output) => output,
                Err(error) => {
                    owned.retain(|owned_target| *owned_target != target);
                    let _ = self.write_owned_targets(&owned).await;
                    return self.error(
                        HomelabFeature::KeepAwake,
                        format!("Could not mask {target}: {error}"),
                    );
                }
            };
            if !output.status.success() {
                owned.retain(|owned_target| *owned_target != target);
                let journal_error = self.write_owned_targets(&owned).await.err();
                return self.error(
                    HomelabFeature::KeepAwake,
                    format!(
                        "Could not mask {target}: {}{}",
                        String::from_utf8_lossy(&output.stderr).trim(),
                        journal_error
                            .map(|error| format!("; could not update ownership journal: {error}"))
                            .unwrap_or_default()
                    ),
                );
            }
        }

        HomelabStatus {
            feature: HomelabFeature::KeepAwake,
            status: HomelabFeatureStatus::Enabled,
            detail: "All sleep targets are masked".to_string(),
        }
    }

    async fn write_owned_targets(&self, targets: &[&str]) -> std::io::Result<()> {
        let path = self.etc_root.join(KEEP_AWAKE_OWNERSHIP);
        if targets.is_empty() {
            return match tokio::fs::remove_file(path).await {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error),
            };
        }
        let content = targets
            .iter()
            .map(|target| format!("{target}\n"))
            .collect::<String>();
        atomic_write(&path, content.as_bytes()).await
    }

    fn unavailable(&self, feature: HomelabFeature, detail: impl Into<String>) -> HomelabStatus {
        HomelabStatus {
            feature,
            status: HomelabFeatureStatus::Unavailable,
            detail: detail.into(),
        }
    }

    fn error(&self, feature: HomelabFeature, detail: impl Into<String>) -> HomelabStatus {
        HomelabStatus {
            feature,
            status: HomelabFeatureStatus::Error,
            detail: detail.into(),
        }
    }

    pub async fn status(&self) -> Vec<HomelabStatus> {
        if !self.linux {
            return HomelabFeature::ALL
                .into_iter()
                .map(|feature| HomelabStatus {
                    feature,
                    status: HomelabFeatureStatus::Unavailable,
                    detail: "Available on Linux only".to_string(),
                })
                .collect();
        }

        let mut statuses = Vec::with_capacity(HomelabFeature::ALL.len());
        for feature in HomelabFeature::ALL {
            let row = match feature {
                HomelabFeature::KeepAwake => self.keep_awake_status().await,
                HomelabFeature::AutomaticSleep => self.automatic_sleep_status().await,
                HomelabFeature::WakeOnLan => self.wake_on_lan_status().await,
                HomelabFeature::WifiPowerSaving => self.wifi_power_saving_status().await,
            };
            statuses.push(row);
        }
        statuses
    }

    async fn keep_awake_status(&self) -> HomelabStatus {
        if !self.systemd {
            return HomelabStatus {
                feature: HomelabFeature::KeepAwake,
                status: HomelabFeatureStatus::Unavailable,
                detail: "systemd is unavailable".to_string(),
            };
        }
        let Some(systemctl) = &self.systemctl else {
            return HomelabStatus {
                feature: HomelabFeature::KeepAwake,
                status: HomelabFeatureStatus::Unavailable,
                detail: "systemctl is unavailable".to_string(),
            };
        };

        let mut all_masked = true;
        for target in KEEP_AWAKE_TARGETS {
            let output = match tokio::process::Command::new(systemctl)
                .args(["is-enabled", target])
                .output()
                .await
            {
                Ok(output) => output,
                Err(error) if error.kind() == ErrorKind::NotFound => {
                    return HomelabStatus {
                        feature: HomelabFeature::KeepAwake,
                        status: HomelabFeatureStatus::Unavailable,
                        detail: "systemctl is unavailable".to_string(),
                    };
                }
                Err(error) => {
                    return HomelabStatus {
                        feature: HomelabFeature::KeepAwake,
                        status: HomelabFeatureStatus::Error,
                        detail: format!("Could not inspect {target}: {error}"),
                    };
                }
            };
            let state = String::from_utf8_lossy(&output.stdout);
            let state = state.trim();
            if state == "masked" {
                continue;
            }
            if output.status.success()
                || matches!(
                    state,
                    "enabled"
                        | "enabled-runtime"
                        | "linked"
                        | "linked-runtime"
                        | "alias"
                        | "static"
                        | "indirect"
                        | "generated"
                        | "transient"
                        | "disabled"
                        | "not-found"
                )
            {
                all_masked = false;
                continue;
            }
            return HomelabStatus {
                feature: HomelabFeature::KeepAwake,
                status: HomelabFeatureStatus::Error,
                detail: format!(
                    "Could not inspect {target}: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ),
            };
        }

        HomelabStatus {
            feature: HomelabFeature::KeepAwake,
            status: if all_masked {
                HomelabFeatureStatus::Enabled
            } else {
                HomelabFeatureStatus::Disabled
            },
            detail: if all_masked {
                "All sleep targets are masked".to_string()
            } else {
                "One or more sleep targets are not masked".to_string()
            },
        }
    }

    async fn network_manager_profiles(
        &self,
        nmcli: &Path,
        kind: InterfaceKind,
        interfaces: &[String],
    ) -> Result<Vec<NetworkManagerProfile>, HomelabStatus> {
        let feature = match kind {
            InterfaceKind::Wired => HomelabFeature::WakeOnLan,
            InterfaceKind::Wireless => HomelabFeature::WifiPowerSaving,
        };
        let output = match tokio::process::Command::new(nmcli)
            .args([
                "-t",
                "-f",
                "UUID,TYPE,connection.interface-name",
                "connection",
                "show",
            ])
            .output()
            .await
        {
            Ok(output) if output.status.success() => output,
            Ok(output) => {
                return Err(self.error(
                    feature,
                    format!(
                        "Could not inspect NetworkManager profiles: {}",
                        String::from_utf8_lossy(&output.stderr).trim()
                    ),
                ));
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {
                return Err(self.unavailable(feature, "NetworkManager is unavailable"));
            }
            Err(error) => {
                return Err(self.error(
                    feature,
                    format!("Could not inspect NetworkManager profiles: {error}"),
                ));
            }
        };
        Ok(parse_network_manager_profiles(
            &String::from_utf8_lossy(&output.stdout),
            kind,
            interfaces,
        ))
    }

    async fn wifi_power_saving_status(&self) -> HomelabStatus {
        let Some(iw) = &self.iw else {
            return self.unavailable(HomelabFeature::WifiPowerSaving, "iw is unavailable");
        };
        let Some(nmcli) = &self.nmcli else {
            return self.unavailable(
                HomelabFeature::WifiPowerSaving,
                "NetworkManager is unavailable",
            );
        };
        let interfaces = match self.interfaces(InterfaceKind::Wireless).await {
            Ok(interfaces) if interfaces.is_empty() => {
                return self.unavailable(
                    HomelabFeature::WifiPowerSaving,
                    "No wireless interfaces were found",
                );
            }
            Ok(interfaces) => interfaces,
            Err(error) => {
                return self.error(
                    HomelabFeature::WifiPowerSaving,
                    format!("Could not inspect wireless interfaces: {error}"),
                );
            }
        };
        let profiles = match self
            .network_manager_profiles(nmcli, InterfaceKind::Wireless, &interfaces)
            .await
        {
            Ok(profiles) => profiles,
            Err(status) => return status,
        };
        let mut all_disabled = true;
        for interface in &interfaces {
            let interface_profiles = profiles
                .iter()
                .filter(|profile| profile.interface == *interface)
                .collect::<Vec<_>>();
            if interface_profiles.is_empty() {
                return self.unavailable(
                    HomelabFeature::WifiPowerSaving,
                    format!("NetworkManager does not own {interface}"),
                );
            }
            let output = match tokio::process::Command::new(iw)
                .args(["dev", interface, "get", "power_save"])
                .output()
                .await
            {
                Ok(output) if output.status.success() => output,
                Ok(output) => {
                    return self.error(
                        HomelabFeature::WifiPowerSaving,
                        format!(
                            "Could not inspect {interface}: {}",
                            String::from_utf8_lossy(&output.stderr).trim()
                        ),
                    );
                }
                Err(error) if error.kind() == ErrorKind::NotFound => {
                    return self.unavailable(HomelabFeature::WifiPowerSaving, "iw is unavailable");
                }
                Err(error) => {
                    return self.error(
                        HomelabFeature::WifiPowerSaving,
                        format!("Could not inspect {interface}: {error}"),
                    );
                }
            };
            all_disabled &=
                wifi_power_saving_value(&String::from_utf8_lossy(&output.stdout)) == Some("off");
            for profile in interface_profiles {
                let output = match tokio::process::Command::new(nmcli)
                    .args([
                        "-g",
                        "802-11-wireless.powersave",
                        "connection",
                        "show",
                        &profile.uuid,
                    ])
                    .output()
                    .await
                {
                    Ok(output) if output.status.success() => output,
                    Ok(output) => {
                        return self.error(
                            HomelabFeature::WifiPowerSaving,
                            format!(
                                "Could not inspect NetworkManager profile {}: {}",
                                profile.uuid,
                                String::from_utf8_lossy(&output.stderr).trim()
                            ),
                        );
                    }
                    Err(error) => {
                        return self.error(
                            HomelabFeature::WifiPowerSaving,
                            format!(
                                "Could not inspect NetworkManager profile {}: {error}",
                                profile.uuid
                            ),
                        );
                    }
                };
                all_disabled &=
                    nmcli_wifi_power_saving_is_disabled(&String::from_utf8_lossy(&output.stdout));
            }
        }
        HomelabStatus {
            feature: HomelabFeature::WifiPowerSaving,
            status: if all_disabled {
                HomelabFeatureStatus::Enabled
            } else {
                HomelabFeatureStatus::Disabled
            },
            detail: interfaces.join(", "),
        }
    }

    async fn wake_on_lan_status(&self) -> HomelabStatus {
        let Some(ethtool) = &self.ethtool else {
            return self.unavailable(HomelabFeature::WakeOnLan, "ethtool is unavailable");
        };
        let Some(nmcli) = &self.nmcli else {
            return self.unavailable(HomelabFeature::WakeOnLan, "NetworkManager is unavailable");
        };
        let interfaces = match self.interfaces(InterfaceKind::Wired).await {
            Ok(interfaces) if interfaces.is_empty() => {
                return self.unavailable(
                    HomelabFeature::WakeOnLan,
                    "No physical wired interfaces were found",
                );
            }
            Ok(interfaces) => interfaces,
            Err(error) => {
                return self.error(
                    HomelabFeature::WakeOnLan,
                    format!("Could not inspect network interfaces: {error}"),
                );
            }
        };
        let profiles = match self
            .network_manager_profiles(nmcli, InterfaceKind::Wired, &interfaces)
            .await
        {
            Ok(profiles) => profiles,
            Err(status) => return status,
        };

        let mut supported = Vec::new();
        let mut enabled = true;
        for interface in interfaces {
            let output = match tokio::process::Command::new(ethtool)
                .arg(&interface)
                .output()
                .await
            {
                Ok(output) if output.status.success() => output,
                Ok(output) => {
                    return self.error(
                        HomelabFeature::WakeOnLan,
                        format!(
                            "Could not inspect {interface}: {}",
                            String::from_utf8_lossy(&output.stderr).trim()
                        ),
                    );
                }
                Err(error) if error.kind() == ErrorKind::NotFound => {
                    return self.unavailable(HomelabFeature::WakeOnLan, "ethtool is unavailable");
                }
                Err(error) => {
                    return self.error(
                        HomelabFeature::WakeOnLan,
                        format!("Could not inspect {interface}: {error}"),
                    );
                }
            };
            let output = String::from_utf8_lossy(&output.stdout);
            let supports_magic = output.lines().any(|line| {
                line.trim()
                    .strip_prefix("Supports Wake-on:")
                    .is_some_and(|modes| modes.trim().contains('g'))
            });
            if !supports_magic {
                continue;
            }
            let runtime_enabled = wake_on_lan_mode(&output).is_some_and(|mode| mode.contains('g'));
            let interface_profiles = profiles
                .iter()
                .filter(|profile| profile.interface == interface)
                .collect::<Vec<_>>();
            if interface_profiles.is_empty() {
                return self.unavailable(
                    HomelabFeature::WakeOnLan,
                    format!("NetworkManager does not own {interface}"),
                );
            }
            let mut persistent_enabled = true;
            for profile in interface_profiles {
                let output = match tokio::process::Command::new(nmcli)
                    .args([
                        "-g",
                        "802-3-ethernet.wake-on-lan",
                        "connection",
                        "show",
                        &profile.uuid,
                    ])
                    .output()
                    .await
                {
                    Ok(output) if output.status.success() => output,
                    Ok(output) => {
                        return self.error(
                            HomelabFeature::WakeOnLan,
                            format!(
                                "Could not inspect NetworkManager profile {}: {}",
                                profile.uuid,
                                String::from_utf8_lossy(&output.stderr).trim()
                            ),
                        );
                    }
                    Err(error) => {
                        return self.error(
                            HomelabFeature::WakeOnLan,
                            format!(
                                "Could not inspect NetworkManager profile {}: {error}",
                                profile.uuid
                            ),
                        );
                    }
                };
                persistent_enabled &=
                    nmcli_wake_on_lan_has_magic(&String::from_utf8_lossy(&output.stdout));
            }
            enabled &= runtime_enabled && persistent_enabled;
            supported.push(interface);
        }
        if supported.is_empty() {
            return self.unavailable(
                HomelabFeature::WakeOnLan,
                "No wired interfaces support magic-packet wake",
            );
        }

        HomelabStatus {
            feature: HomelabFeature::WakeOnLan,
            status: if enabled {
                HomelabFeatureStatus::Enabled
            } else {
                HomelabFeatureStatus::Disabled
            },
            detail: supported.join(", "),
        }
    }

    async fn interfaces(&self, kind: InterfaceKind) -> std::io::Result<Vec<String>> {
        let mut entries = match tokio::fs::read_dir(self.sys_root.join("class/net")).await {
            Ok(entries) => entries,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error),
        };
        let mut interfaces = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            let physical = tokio::fs::try_exists(path.join("device")).await?;
            let wireless = tokio::fs::try_exists(path.join("wireless")).await?;
            let matches = match kind {
                InterfaceKind::Wired => physical && !wireless,
                InterfaceKind::Wireless => wireless,
            };
            if matches {
                interfaces.push(entry.file_name().to_string_lossy().into_owned());
            }
        }
        interfaces.sort();
        Ok(interfaces)
    }

    async fn automatic_sleep_status(&self) -> HomelabStatus {
        let (status, detail) = if !self.systemd {
            (
                HomelabFeatureStatus::Unavailable,
                "systemd-logind is unavailable".to_string(),
            )
        } else {
            match tokio::fs::read(self.etc_root.join(LOGIND_DROP_IN)).await {
                Ok(content) if content == LOGIND_CONFIG => (
                    HomelabFeatureStatus::Enabled,
                    "Postlab logind configuration is active".to_string(),
                ),
                Ok(_) => (
                    HomelabFeatureStatus::Error,
                    "Managed logind configuration has unexpected content".to_string(),
                ),
                Err(error) if error.kind() == ErrorKind::NotFound => (
                    HomelabFeatureStatus::Disabled,
                    "Postlab logind configuration is not installed".to_string(),
                ),
                Err(error) => (
                    HomelabFeatureStatus::Error,
                    format!("Could not read managed logind configuration: {error}"),
                ),
            }
        };

        HomelabStatus {
            feature: HomelabFeature::AutomaticSleep,
            status,
            detail,
        }
    }

    #[cfg(test)]
    fn with_roots(linux: bool, systemd: bool, etc_root: PathBuf, sys_root: PathBuf) -> Self {
        Self {
            linux,
            systemd,
            etc_root,
            sys_root,
            systemctl: None,
            ethtool: None,
            nmcli: None,
            iw: None,
        }
    }

    #[cfg(test)]
    fn with_systemctl(mut self, systemctl: PathBuf) -> Self {
        self.systemctl = Some(systemctl);
        self
    }

    #[cfg(test)]
    fn with_network_tools(mut self, ethtool: PathBuf, nmcli: PathBuf, iw: PathBuf) -> Self {
        self.ethtool = Some(ethtool);
        self.nmcli = Some(nmcli);
        self.iw = Some(iw);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::{HomelabFeature, HomelabFeatureStatus, HomelabManager};
    use std::{os::unix::fs::PermissionsExt, path::PathBuf};

    fn fake_named_executable(
        directory: &std::path::Path,
        name: &str,
        log: &std::path::Path,
        body: &str,
    ) -> PathBuf {
        let executable = directory.join(name);
        std::fs::write(
            &executable,
            format!(
                "#!/bin/sh\nprintf '{} %s\\n' \"$*\" >> '{}'\n{body}\n",
                name,
                log.display()
            ),
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&executable).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&executable, permissions).unwrap();
        executable
    }

    fn fake_executable(directory: &std::path::Path, body: &str) -> (PathBuf, PathBuf) {
        let executable = directory.join("fake-command");
        let log = directory.join("argv.log");
        std::fs::write(
            &executable,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\n{body}\n",
                log.display()
            ),
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&executable).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&executable, permissions).unwrap();
        (executable, log)
    }

    #[test]
    fn network_manager_profiles_are_unescaped_filtered_and_sorted() {
        let interfaces = vec!["eno1".to_string(), "eno2".to_string()];
        let profiles = super::parse_network_manager_profiles(
            "uuid-b:802-3-ethernet:eno2\nuuid\\:a:802-3-ethernet:eno1\nwifi:802-11-wireless:wlan0\ninactive:802-3-ethernet:--\nvirtual:bridge:eno1\n",
            super::InterfaceKind::Wired,
            &interfaces,
        );

        assert_eq!(
            profiles,
            vec![
                super::NetworkManagerProfile {
                    uuid: "uuid:a".to_string(),
                    interface: "eno1".to_string(),
                },
                super::NetworkManagerProfile {
                    uuid: "uuid-b".to_string(),
                    interface: "eno2".to_string(),
                },
            ]
        );
    }

    #[tokio::test]
    async fn wireless_interfaces_are_detected_from_sysfs() {
        let root = tempfile::tempdir().unwrap();
        let net = root.path().join("sys/class/net");
        for path in ["eno1/device", "wlan1/wireless", "wlan0/wireless"] {
            std::fs::create_dir_all(net.join(path)).unwrap();
        }
        let manager = HomelabManager::with_roots(
            true,
            true,
            root.path().join("etc"),
            root.path().join("sys"),
        );

        let interfaces = manager
            .interfaces(super::InterfaceKind::Wireless)
            .await
            .unwrap();

        assert_eq!(interfaces, vec!["wlan0", "wlan1"]);
    }

    #[tokio::test]
    async fn failed_logind_hup_reports_error_after_successful_config_change() {
        let root = tempfile::tempdir().unwrap();
        let (systemctl, _) =
            fake_executable(root.path(), "printf 'logind unavailable\\n' >&2; exit 9");
        let managed = root
            .path()
            .join("etc/systemd/logind.conf.d/90-postlab-homelab.conf");
        let manager = HomelabManager::with_roots(
            true,
            true,
            root.path().join("etc"),
            root.path().join("sys"),
        )
        .with_systemctl(systemctl);

        let result = manager.set(HomelabFeature::AutomaticSleep, true).await;

        assert_eq!(result.status, HomelabFeatureStatus::Error);
        assert!(result.detail.contains("logind unavailable"));
        assert_eq!(std::fs::read(managed).unwrap(), super::LOGIND_CONFIG);
    }

    #[tokio::test]
    async fn disabling_automatic_sleep_is_idempotent_without_hup() {
        let root = tempfile::tempdir().unwrap();
        let (systemctl, log) = fake_executable(root.path(), "exit 0");
        let manager = HomelabManager::with_roots(
            true,
            true,
            root.path().join("etc"),
            root.path().join("sys"),
        )
        .with_systemctl(systemctl);

        let result = manager.set(HomelabFeature::AutomaticSleep, false).await;

        assert_eq!(result.status, HomelabFeatureStatus::Disabled);
        assert!(!log.exists());
    }

    #[tokio::test]
    async fn disabling_automatic_sleep_refuses_modified_managed_config() {
        let root = tempfile::tempdir().unwrap();
        let managed = root
            .path()
            .join("etc/systemd/logind.conf.d/90-postlab-homelab.conf");
        std::fs::create_dir_all(managed.parent().unwrap()).unwrap();
        std::fs::write(&managed, b"[Login]\nIdleAction=suspend\n").unwrap();
        let (systemctl, log) = fake_executable(root.path(), "exit 0");
        let manager = HomelabManager::with_roots(
            true,
            true,
            root.path().join("etc"),
            root.path().join("sys"),
        )
        .with_systemctl(systemctl);

        let result = manager.set(HomelabFeature::AutomaticSleep, false).await;

        assert_eq!(result.status, HomelabFeatureStatus::Error);
        assert!(result.detail.contains("modified"));
        assert_eq!(
            std::fs::read(&managed).unwrap(),
            b"[Login]\nIdleAction=suspend\n"
        );
        assert!(!log.exists());
    }

    #[tokio::test]
    async fn enabling_automatic_sleep_is_idempotent_without_hup() {
        let root = tempfile::tempdir().unwrap();
        let managed = root
            .path()
            .join("etc/systemd/logind.conf.d/90-postlab-homelab.conf");
        std::fs::create_dir_all(managed.parent().unwrap()).unwrap();
        std::fs::write(&managed, super::LOGIND_CONFIG).unwrap();
        let (systemctl, log) = fake_executable(root.path(), "exit 0");
        let manager = HomelabManager::with_roots(
            true,
            true,
            root.path().join("etc"),
            root.path().join("sys"),
        )
        .with_systemctl(systemctl);

        let result = manager.set(HomelabFeature::AutomaticSleep, true).await;

        assert_eq!(result.status, HomelabFeatureStatus::Enabled);
        assert!(!log.exists());
    }

    #[tokio::test]
    async fn disabling_automatic_sleep_removes_only_managed_config_and_hups_logind() {
        let root = tempfile::tempdir().unwrap();
        let drop_in_dir = root.path().join("etc/systemd/logind.conf.d");
        std::fs::create_dir_all(&drop_in_dir).unwrap();
        let managed = drop_in_dir.join("90-postlab-homelab.conf");
        let admin = drop_in_dir.join("admin.conf");
        std::fs::write(&managed, super::LOGIND_CONFIG).unwrap();
        std::fs::write(&admin, b"[Login]\nIdleAction=suspend\n").unwrap();
        let (systemctl, log) = fake_executable(root.path(), "exit 0");
        let manager = HomelabManager::with_roots(
            true,
            true,
            root.path().join("etc"),
            root.path().join("sys"),
        )
        .with_systemctl(systemctl);

        let result = manager.set(HomelabFeature::AutomaticSleep, false).await;

        assert_eq!(result.status, HomelabFeatureStatus::Disabled);
        assert!(!managed.exists());
        assert_eq!(
            std::fs::read(&admin).unwrap(),
            b"[Login]\nIdleAction=suspend\n"
        );
        assert_eq!(
            std::fs::read_to_string(log).unwrap(),
            "kill --signal=HUP systemd-logind.service\n"
        );
    }

    #[tokio::test]
    async fn enabling_automatic_sleep_optimization_writes_exact_config_and_hups_logind() {
        let root = tempfile::tempdir().unwrap();
        let (systemctl, log) = fake_executable(root.path(), "exit 0");
        let manager = HomelabManager::with_roots(
            true,
            true,
            root.path().join("etc"),
            root.path().join("sys"),
        )
        .with_systemctl(systemctl);

        let result = manager.set(HomelabFeature::AutomaticSleep, true).await;

        assert_eq!(result.status, HomelabFeatureStatus::Enabled);
        assert_eq!(
            std::fs::read(
                root.path()
                    .join("etc/systemd/logind.conf.d/90-postlab-homelab.conf")
            )
            .unwrap(),
            super::LOGIND_CONFIG
        );
        assert_eq!(
            std::fs::read_to_string(log).unwrap(),
            "kill --signal=HUP systemd-logind.service\n"
        );
    }

    #[tokio::test]
    async fn keep_awake_operation_failure_reports_error() {
        let root = tempfile::tempdir().unwrap();
        let (systemctl, _) = fake_executable(
            root.path(),
            "if [ \"$1\" = is-enabled ]; then printf 'disabled\\n'; else printf 'permission denied\\n' >&2; exit 9; fi",
        );
        let manager = HomelabManager::with_roots(
            true,
            true,
            root.path().join("etc"),
            root.path().join("sys"),
        )
        .with_systemctl(systemctl);

        let result = manager.set(HomelabFeature::KeepAwake, true).await;

        assert_eq!(result.status, HomelabFeatureStatus::Error);
        assert!(result.detail.contains("permission denied"));
    }

    #[tokio::test]
    async fn disabling_keep_awake_reports_masks_outside_postlab_ownership() {
        let root = tempfile::tempdir().unwrap();
        let (systemctl, _) = fake_executable(root.path(), "printf 'masked\\n'");
        let manager = HomelabManager::with_roots(
            true,
            true,
            root.path().join("etc"),
            root.path().join("sys"),
        )
        .with_systemctl(systemctl);

        let result = manager.set(HomelabFeature::KeepAwake, false).await;

        assert_eq!(result.status, HomelabFeatureStatus::Error);
        assert!(result.detail.contains("outside Postlab ownership"));
    }

    #[tokio::test]
    async fn enabling_keep_awake_is_idempotent_when_all_targets_are_masked() {
        let root = tempfile::tempdir().unwrap();
        let (systemctl, log) = fake_executable(root.path(), "printf 'masked\\n'");
        let manager = HomelabManager::with_roots(
            true,
            true,
            root.path().join("etc"),
            root.path().join("sys"),
        )
        .with_systemctl(systemctl);

        let result = manager.set(HomelabFeature::KeepAwake, true).await;

        assert_eq!(result.status, HomelabFeatureStatus::Enabled);
        assert_eq!(
            std::fs::read_to_string(log).unwrap(),
            "is-enabled sleep.target\nis-enabled suspend.target\nis-enabled hibernate.target\nis-enabled hybrid-sleep.target\n"
        );
        assert!(!root
            .path()
            .join("etc/postlab/homelab/keep-awake-targets")
            .exists());
    }

    #[tokio::test]
    async fn disabling_keep_awake_unmasks_only_owned_fixed_targets() {
        let root = tempfile::tempdir().unwrap();
        let ownership = root.path().join("etc/postlab/homelab/keep-awake-targets");
        std::fs::create_dir_all(ownership.parent().unwrap()).unwrap();
        std::fs::write(
            &ownership,
            "hibernate.target\n../../evil\nsleep.target\nhibernate.target\n",
        )
        .unwrap();
        let (systemctl, log) = fake_executable(root.path(), "exit 0");
        let manager = HomelabManager::with_roots(
            true,
            true,
            root.path().join("etc"),
            root.path().join("sys"),
        )
        .with_systemctl(systemctl);

        let result = manager.set(HomelabFeature::KeepAwake, false).await;

        assert_eq!(result.status, HomelabFeatureStatus::Disabled);
        assert_eq!(
            std::fs::read_to_string(log).unwrap(),
            "unmask sleep.target\nunmask hibernate.target\nis-enabled sleep.target\nis-enabled suspend.target\nis-enabled hibernate.target\nis-enabled hybrid-sleep.target\n"
        );
        assert!(!ownership.exists());
    }

    #[tokio::test]
    async fn enabling_wake_on_lan_uses_exact_persistent_and_runtime_arguments() {
        let root = tempfile::tempdir().unwrap();
        for path in ["eno2/device", "eno1/device"] {
            std::fs::create_dir_all(root.path().join("sys/class/net").join(path)).unwrap();
        }
        let (ethtool, ethtool_log) = fake_executable(
            root.path(),
            "if [ \"$1\" != -s ]; then printf 'Supports Wake-on: pumbg\\nWake-on: d\\n'; fi",
        );
        let nmcli_dir = root.path().join("nmcli-mutate");
        std::fs::create_dir_all(&nmcli_dir).unwrap();
        let (nmcli, nmcli_log) = fake_executable(
            &nmcli_dir,
            "if [ \"$1\" = -t ]; then printf 'uuid2:802-3-ethernet:eno2\\nuuid1:802-3-ethernet:eno1\\n'; elif [ \"$1\" = -g ]; then printf '0 (none)\\n'; fi",
        );
        let manager = HomelabManager::with_roots(
            true,
            true,
            root.path().join("etc"),
            root.path().join("sys"),
        )
        .with_network_tools(ethtool, nmcli, root.path().join("unused-iw"));

        let result = manager.set(HomelabFeature::WakeOnLan, true).await;

        assert_eq!(result.status, HomelabFeatureStatus::Enabled);
        assert_eq!(result.detail, "eno1, eno2");
        assert_eq!(
            std::fs::read_to_string(nmcli_log).unwrap(),
            "-t -f UUID,TYPE,connection.interface-name connection show\n-g 802-3-ethernet.wake-on-lan connection show uuid1\n-g 802-3-ethernet.wake-on-lan connection show uuid2\nconnection modify uuid1 802-3-ethernet.wake-on-lan magic\nconnection modify uuid2 802-3-ethernet.wake-on-lan magic\n"
        );
        assert_eq!(
            std::fs::read_to_string(ethtool_log).unwrap(),
            "eno1\neno2\n-s eno1 wol g\n-s eno2 wol g\n"
        );
    }

    #[tokio::test]
    async fn disabling_wake_on_lan_uses_exact_persistent_and_runtime_arguments() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("sys/class/net/eno1/device")).unwrap();
        let (ethtool, ethtool_log) = fake_executable(
            root.path(),
            "if [ \"$1\" != -s ]; then printf 'Supports Wake-on: pumbg\\nWake-on: g\\n'; fi",
        );
        let nmcli_dir = root.path().join("nmcli-disable-wol");
        std::fs::create_dir_all(&nmcli_dir).unwrap();
        let (nmcli, nmcli_log) = fake_executable(
            &nmcli_dir,
            "if [ \"$1\" = -t ]; then printf 'uuid1:802-3-ethernet:eno1\\n'; elif [ \"$1\" = -g ]; then printf '64 (magic)\\n'; fi",
        );
        let manager = HomelabManager::with_roots(
            true,
            true,
            root.path().join("etc"),
            root.path().join("sys"),
        )
        .with_network_tools(ethtool, nmcli, root.path().join("unused-iw"));

        let result = manager.set(HomelabFeature::WakeOnLan, false).await;

        assert_eq!(result.status, HomelabFeatureStatus::Disabled);
        assert_eq!(result.detail, "eno1");
        assert_eq!(
            std::fs::read_to_string(nmcli_log).unwrap(),
            "-t -f UUID,TYPE,connection.interface-name connection show\n-g 802-3-ethernet.wake-on-lan connection show uuid1\nconnection modify uuid1 802-3-ethernet.wake-on-lan none\n"
        );
        assert_eq!(
            std::fs::read_to_string(ethtool_log).unwrap(),
            "eno1\n-s eno1 wol d\n"
        );
    }

    #[tokio::test]
    async fn wake_on_lan_late_preflight_failure_performs_no_mutations() {
        let root = tempfile::tempdir().unwrap();
        for interface in ["eno1", "eno2"] {
            std::fs::create_dir_all(
                root.path()
                    .join("sys/class/net")
                    .join(interface)
                    .join("device"),
            )
            .unwrap();
        }
        let (ethtool, ethtool_log) = fake_executable(
            root.path(),
            "printf 'Supports Wake-on: pumbg\\nWake-on: d\\n'",
        );
        let nmcli_dir = root.path().join("nmcli-wol-preflight-failure");
        std::fs::create_dir_all(&nmcli_dir).unwrap();
        let (nmcli, nmcli_log) = fake_executable(
            &nmcli_dir,
            "if [ \"$1\" = -t ]; then printf 'uuid1:802-3-ethernet:eno1\\nuuid2:802-3-ethernet:eno2\\n'; elif [ \"$5\" = uuid1 ]; then printf '0 (none)\\n'; else printf 'snapshot failure\\n' >&2; exit 7; fi",
        );
        let manager = HomelabManager::with_roots(
            true,
            true,
            root.path().join("etc"),
            root.path().join("sys"),
        )
        .with_network_tools(ethtool, nmcli, root.path().join("unused-iw"));

        let result = manager.set(HomelabFeature::WakeOnLan, true).await;

        assert_eq!(result.status, HomelabFeatureStatus::Error);
        assert!(result.detail.contains("snapshot failure"));
        assert_eq!(
            std::fs::read_to_string(nmcli_log).unwrap(),
            "-t -f UUID,TYPE,connection.interface-name connection show\n-g 802-3-ethernet.wake-on-lan connection show uuid1\n-g 802-3-ethernet.wake-on-lan connection show uuid2\n"
        );
        assert_eq!(
            std::fs::read_to_string(ethtool_log).unwrap(),
            "eno1\neno2\n"
        );
    }

    #[tokio::test]
    async fn wake_on_lan_mid_operation_failure_rolls_back_attempted_and_completed_changes() {
        let root = tempfile::tempdir().unwrap();
        for interface in ["eno1", "eno2"] {
            std::fs::create_dir_all(
                root.path()
                    .join("sys/class/net")
                    .join(interface)
                    .join("device"),
            )
            .unwrap();
        }
        let (ethtool, ethtool_log) = fake_executable(
            root.path(),
            "if [ \"$1\" != -s ]; then case \"$1\" in eno1) printf 'Supports Wake-on: pumbg\\nWake-on: d\\n' ;; eno2) printf 'Supports Wake-on: pumbg\\nWake-on: g\\n' ;; esac; fi",
        );
        let nmcli_dir = root.path().join("nmcli-wol-rollback");
        std::fs::create_dir_all(&nmcli_dir).unwrap();
        let (nmcli, nmcli_log) = fake_executable(
            &nmcli_dir,
            "if [ \"$1\" = -t ]; then printf 'uuid1:802-3-ethernet:eno1\\nuuid2:802-3-ethernet:eno2\\n'; elif [ \"$1\" = -g ]; then case \"$5\" in uuid1) printf '0 (none)\\n' ;; uuid2) printf '64 (magic)\\n' ;; esac; elif [ \"$1:$2:$3:$5\" = connection:modify:uuid2:magic ]; then printf 'profile failure\\n' >&2; exit 7; fi",
        );
        let manager = HomelabManager::with_roots(
            true,
            true,
            root.path().join("etc"),
            root.path().join("sys"),
        )
        .with_network_tools(ethtool, nmcli, root.path().join("unused-iw"));

        let result = manager.set(HomelabFeature::WakeOnLan, true).await;

        assert_eq!(result.status, HomelabFeatureStatus::Error);
        assert!(result.detail.contains("profile failure"));
        assert!(result.detail.contains("rollback succeeded"));
        assert_eq!(
            std::fs::read_to_string(nmcli_log).unwrap(),
            "-t -f UUID,TYPE,connection.interface-name connection show\n-g 802-3-ethernet.wake-on-lan connection show uuid1\n-g 802-3-ethernet.wake-on-lan connection show uuid2\nconnection modify uuid1 802-3-ethernet.wake-on-lan magic\nconnection modify uuid2 802-3-ethernet.wake-on-lan magic\nconnection modify uuid2 802-3-ethernet.wake-on-lan 64\nconnection modify uuid1 802-3-ethernet.wake-on-lan 0\n"
        );
        assert_eq!(
            std::fs::read_to_string(ethtool_log).unwrap(),
            "eno1\neno2\n-s eno1 wol g\n-s eno1 wol d\n"
        );
    }

    #[tokio::test]
    async fn wake_on_lan_is_unavailable_without_physical_wired_interfaces() {
        let root = tempfile::tempdir().unwrap();
        let tools = root.path().join("tools");
        std::fs::create_dir_all(&tools).unwrap();
        let (ethtool, _) = fake_executable(&tools, "exit 0");
        let nmcli_dir = root.path().join("nmcli-no-interface");
        std::fs::create_dir_all(&nmcli_dir).unwrap();
        let (nmcli, _) = fake_executable(&nmcli_dir, "exit 0");
        let manager = HomelabManager::with_roots(
            true,
            true,
            root.path().join("etc"),
            root.path().join("sys"),
        )
        .with_network_tools(ethtool, nmcli, root.path().join("unused-iw"));

        let row = manager
            .status()
            .await
            .into_iter()
            .find(|row| row.feature == HomelabFeature::WakeOnLan)
            .unwrap();

        assert_eq!(row.status, HomelabFeatureStatus::Unavailable);
        assert!(row.detail.contains("No physical wired"));
    }

    #[tokio::test]
    async fn wake_on_lan_is_enabled_when_all_managed_interfaces_match() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("sys/class/net/eno1/device")).unwrap();
        let (ethtool, _) = fake_executable(
            root.path(),
            "printf 'Supports Wake-on: pumbg\\nWake-on: g\\n'",
        );
        let nmcli_dir = root.path().join("nmcli-enabled");
        std::fs::create_dir_all(&nmcli_dir).unwrap();
        let (nmcli, _) = fake_executable(
            &nmcli_dir,
            "if [ \"$1\" = -t ]; then printf 'uuid1:802-3-ethernet:eno1\\n'; else printf 'magic\\n'; fi",
        );
        let manager = HomelabManager::with_roots(
            true,
            true,
            root.path().join("etc"),
            root.path().join("sys"),
        )
        .with_network_tools(ethtool, nmcli, root.path().join("unused-iw"));

        let row = manager
            .status()
            .await
            .into_iter()
            .find(|row| row.feature == HomelabFeature::WakeOnLan)
            .unwrap();

        assert_eq!(row.status, HomelabFeatureStatus::Enabled);
        assert_eq!(row.detail, "eno1");
    }

    #[tokio::test]
    async fn inactive_associated_network_manager_profile_is_used() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("sys/class/net/eno1/device")).unwrap();
        let (ethtool, _) = fake_executable(
            root.path(),
            "printf 'Supports Wake-on: pumbg\\nWake-on: g\\n'",
        );
        let nmcli_dir = root.path().join("nmcli-inactive-profile");
        std::fs::create_dir_all(&nmcli_dir).unwrap();
        let (nmcli, nmcli_log) = fake_executable(
            &nmcli_dir,
            "if [ \"$1:$3\" = '-t:UUID,TYPE,connection.interface-name' ]; then printf 'inactive-uuid:802-3-ethernet:eno1\\n'; elif [ \"$1\" = -g ]; then printf '64 (magic)\\n'; fi",
        );
        let manager = HomelabManager::with_roots(
            true,
            true,
            root.path().join("etc"),
            root.path().join("sys"),
        )
        .with_network_tools(ethtool, nmcli, root.path().join("unused-iw"));

        let row = manager
            .status()
            .await
            .into_iter()
            .find(|row| row.feature == HomelabFeature::WakeOnLan)
            .unwrap();

        assert_eq!(row.status, HomelabFeatureStatus::Enabled);
        assert_eq!(row.detail, "eno1");
        assert_eq!(
            std::fs::read_to_string(nmcli_log).unwrap(),
            "-t -f UUID,TYPE,connection.interface-name connection show\n-g 802-3-ethernet.wake-on-lan connection show inactive-uuid\n"
        );
    }

    #[tokio::test]
    async fn wake_on_lan_status_aggregates_runtime_and_persistent_state() {
        let root = tempfile::tempdir().unwrap();
        for path in ["eno2/device", "eno1/device"] {
            std::fs::create_dir_all(root.path().join("sys/class/net").join(path)).unwrap();
        }
        let (ethtool, ethtool_log) = fake_executable(
            root.path(),
            "case \"$1\" in\n eno1) printf 'Supports Wake-on: pumbg\\nWake-on: g\\n' ;;
 eno2) printf 'Supports Wake-on: pumbg\\nWake-on: d\\n' ;;
esac",
        );
        let nmcli_dir = root.path().join("nmcli");
        std::fs::create_dir_all(&nmcli_dir).unwrap();
        let (nmcli, nmcli_log) = fake_executable(
            &nmcli_dir,
            "if [ \"$1\" = -t ]; then printf 'uuid2:802-3-ethernet:eno2\\nuuid1:802-3-ethernet:eno1\\n'; elif [ \"$5\" = uuid1 ]; then printf 'magic\\n'; else printf 'magic\\n'; fi",
        );
        let manager = HomelabManager::with_roots(
            true,
            true,
            root.path().join("etc"),
            root.path().join("sys"),
        )
        .with_network_tools(ethtool, nmcli, root.path().join("unused-iw"));

        let row = manager
            .status()
            .await
            .into_iter()
            .find(|row| row.feature == HomelabFeature::WakeOnLan)
            .unwrap();

        assert_eq!(row.status, HomelabFeatureStatus::Disabled);
        assert_eq!(row.detail, "eno1, eno2");
        assert_eq!(
            std::fs::read_to_string(ethtool_log).unwrap(),
            "eno1\neno2\n"
        );
        assert_eq!(
            std::fs::read_to_string(nmcli_log).unwrap(),
            "-t -f UUID,TYPE,connection.interface-name connection show\n-g 802-3-ethernet.wake-on-lan connection show uuid1\n-g 802-3-ethernet.wake-on-lan connection show uuid2\n"
        );
    }

    #[tokio::test]
    async fn enabling_keep_awake_masks_only_unmasked_targets_and_records_ownership() {
        let root = tempfile::tempdir().unwrap();
        let (systemctl, log) = fake_executable(
            root.path(),
            "case \"$1:$2\" in\n  is-enabled:sleep.target) printf 'masked\\n' ;;
  is-enabled:*) printf 'disabled\\n' ;;
  mask:*) ;;
esac",
        );
        let manager = HomelabManager::with_roots(
            true,
            true,
            root.path().join("etc"),
            root.path().join("sys"),
        )
        .with_systemctl(systemctl);

        let result = manager.set(HomelabFeature::KeepAwake, true).await;

        assert_eq!(result.status, HomelabFeatureStatus::Enabled);
        assert_eq!(
            std::fs::read_to_string(log).unwrap(),
            "is-enabled sleep.target\nis-enabled suspend.target\nmask suspend.target\nis-enabled hibernate.target\nmask hibernate.target\nis-enabled hybrid-sleep.target\nmask hybrid-sleep.target\n"
        );
        assert_eq!(
            std::fs::read_to_string(root.path().join("etc/postlab/homelab/keep-awake-targets"))
                .unwrap(),
            "suspend.target\nhibernate.target\nhybrid-sleep.target\n"
        );
    }

    #[tokio::test]
    async fn physical_wired_interfaces_are_filtered_and_sorted() {
        let root = tempfile::tempdir().unwrap();
        let net = root.path().join("sys/class/net");
        for path in [
            "eno1/device",
            "enp2s0/device",
            "wlan0/device",
            "wlan0/wireless",
            "lo",
            "docker0",
        ] {
            std::fs::create_dir_all(net.join(path)).unwrap();
        }
        let manager = HomelabManager::with_roots(
            true,
            true,
            root.path().join("etc"),
            root.path().join("sys"),
        );

        let interfaces = manager
            .interfaces(super::InterfaceKind::Wired)
            .await
            .unwrap();

        assert_eq!(interfaces, vec!["eno1", "enp2s0"]);
    }

    #[tokio::test]
    async fn keep_awake_is_unavailable_without_systemctl() {
        let root = tempfile::tempdir().unwrap();
        let manager = HomelabManager::with_roots(
            true,
            true,
            root.path().join("etc"),
            root.path().join("sys"),
        );

        let keep_awake = manager
            .status()
            .await
            .into_iter()
            .find(|row| row.feature == HomelabFeature::KeepAwake)
            .unwrap();

        assert_eq!(keep_awake.status, HomelabFeatureStatus::Unavailable);
        assert!(keep_awake.detail.contains("systemctl"));
    }

    #[tokio::test]
    async fn keep_awake_is_disabled_when_any_target_is_not_masked() {
        let root = tempfile::tempdir().unwrap();
        let (systemctl, _) = fake_executable(
            root.path(),
            "if [ \"$2\" = suspend.target ]; then printf 'disabled\\n'; else printf 'masked\\n'; fi",
        );
        let manager = HomelabManager::with_roots(
            true,
            true,
            root.path().join("etc"),
            root.path().join("sys"),
        )
        .with_systemctl(systemctl);

        let keep_awake = manager
            .status()
            .await
            .into_iter()
            .find(|row| row.feature == HomelabFeature::KeepAwake)
            .unwrap();

        assert_eq!(keep_awake.status, HomelabFeatureStatus::Disabled);
    }

    #[tokio::test]
    async fn keep_awake_status_checks_every_target_with_exact_arguments() {
        let root = tempfile::tempdir().unwrap();
        let (systemctl, log) = fake_executable(root.path(), "printf 'masked\\n'");
        let manager = HomelabManager::with_roots(
            true,
            true,
            root.path().join("etc"),
            root.path().join("sys"),
        )
        .with_systemctl(systemctl);

        let keep_awake = manager
            .status()
            .await
            .into_iter()
            .find(|row| row.feature == HomelabFeature::KeepAwake)
            .unwrap();

        assert_eq!(keep_awake.status, HomelabFeatureStatus::Enabled);
        assert_eq!(
            std::fs::read_to_string(log).unwrap(),
            "is-enabled sleep.target\nis-enabled suspend.target\nis-enabled hibernate.target\nis-enabled hybrid-sleep.target\n"
        );
    }

    #[test]
    fn keep_awake_ownership_accepts_only_unique_fixed_targets() {
        let parsed = super::parse_owned_targets(
            "suspend.target\n../../evil\nsleep.target\nsuspend.target\nunknown.target\nhybrid-sleep.target\n",
        );

        assert_eq!(
            parsed,
            vec!["sleep.target", "suspend.target", "hybrid-sleep.target"]
        );
    }

    #[tokio::test]
    async fn missing_systemd_reports_automatic_sleep_unavailable() {
        let etc = tempfile::tempdir().unwrap();
        let sys = tempfile::tempdir().unwrap();
        let manager = HomelabManager::with_roots(
            true,
            false,
            etc.path().to_path_buf(),
            sys.path().to_path_buf(),
        );

        let automatic_sleep = manager
            .status()
            .await
            .into_iter()
            .find(|row| row.feature == HomelabFeature::AutomaticSleep)
            .unwrap();

        assert_eq!(automatic_sleep.status, HomelabFeatureStatus::Unavailable);
        assert!(automatic_sleep.detail.contains("unavailable"));
    }

    #[tokio::test]
    async fn modified_logind_drop_in_reports_error() {
        let etc = tempfile::tempdir().unwrap();
        let sys = tempfile::tempdir().unwrap();
        let path = etc
            .path()
            .join("systemd/logind.conf.d/90-postlab-homelab.conf");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"[Login]\nIdleAction=suspend\n").unwrap();
        let manager = HomelabManager::with_roots(
            true,
            true,
            etc.path().to_path_buf(),
            sys.path().to_path_buf(),
        );

        let statuses = manager.status().await;
        let automatic_sleep = statuses
            .iter()
            .find(|row| row.feature == HomelabFeature::AutomaticSleep)
            .unwrap();

        assert_eq!(automatic_sleep.status, HomelabFeatureStatus::Error);
        assert!(automatic_sleep.detail.contains("unexpected content"));
    }

    #[tokio::test]
    async fn exact_logind_drop_in_reports_automatic_sleep_enabled() {
        let etc = tempfile::tempdir().unwrap();
        let sys = tempfile::tempdir().unwrap();
        let path = etc
            .path()
            .join("systemd/logind.conf.d/90-postlab-homelab.conf");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            path,
            b"[Login]\nIdleAction=ignore\nHandleLidSwitch=ignore\nHandleLidSwitchExternalPower=ignore\nHandleLidSwitchDocked=ignore\n",
        )
        .unwrap();
        let manager = HomelabManager::with_roots(
            true,
            true,
            etc.path().to_path_buf(),
            sys.path().to_path_buf(),
        );

        let statuses = manager.status().await;
        let automatic_sleep = statuses
            .iter()
            .find(|row| row.feature == HomelabFeature::AutomaticSleep)
            .unwrap();

        assert_eq!(automatic_sleep.status, HomelabFeatureStatus::Enabled);
    }

    #[tokio::test]
    async fn missing_logind_drop_in_reports_automatic_sleep_disabled() {
        let etc = tempfile::tempdir().unwrap();
        let sys = tempfile::tempdir().unwrap();
        let manager = HomelabManager::with_roots(
            true,
            true,
            etc.path().to_path_buf(),
            sys.path().to_path_buf(),
        );

        let statuses = manager.status().await;
        let automatic_sleep = statuses
            .iter()
            .find(|row| row.feature == HomelabFeature::AutomaticSleep)
            .unwrap();

        assert_eq!(automatic_sleep.status, HomelabFeatureStatus::Disabled);
        assert!(automatic_sleep.detail.contains("not installed"));
    }

    #[tokio::test]
    async fn disabling_wifi_power_saving_uses_persistent_and_runtime_arguments() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("sys/class/net/wlan0/wireless")).unwrap();
        let iw_dir = root.path().join("iw");
        std::fs::create_dir_all(&iw_dir).unwrap();
        let (iw, iw_log) = fake_executable(
            &iw_dir,
            "if [ \"$3\" = get ]; then printf 'Power save: on\\n'; fi",
        );
        let nmcli_dir = root.path().join("nmcli-wifi-mutate");
        std::fs::create_dir_all(&nmcli_dir).unwrap();
        let (nmcli, nmcli_log) = fake_executable(
            &nmcli_dir,
            "if [ \"$1\" = -t ]; then printf 'wifi-uuid:802-11-wireless:wlan0\\n'; elif [ \"$1\" = -g ]; then printf '3 (enable)\\n'; fi",
        );
        let manager = HomelabManager::with_roots(
            true,
            true,
            root.path().join("etc"),
            root.path().join("sys"),
        )
        .with_network_tools(root.path().join("unused-ethtool"), nmcli, iw);

        let result = manager.set(HomelabFeature::WifiPowerSaving, true).await;

        assert_eq!(result.status, HomelabFeatureStatus::Enabled);
        assert_eq!(result.detail, "wlan0");
        assert_eq!(
            std::fs::read_to_string(nmcli_log).unwrap(),
            "-t -f UUID,TYPE,connection.interface-name connection show\n-g 802-11-wireless.powersave connection show wifi-uuid\nconnection modify wifi-uuid 802-11-wireless.powersave 2\n"
        );
        assert_eq!(
            std::fs::read_to_string(iw_log).unwrap(),
            "dev wlan0 get power_save\ndev wlan0 set power_save off\n"
        );
    }

    #[tokio::test]
    async fn wifi_late_preflight_failure_performs_no_mutations() {
        let root = tempfile::tempdir().unwrap();
        for interface in ["wlan0", "wlan1"] {
            std::fs::create_dir_all(
                root.path()
                    .join("sys/class/net")
                    .join(interface)
                    .join("wireless"),
            )
            .unwrap();
        }
        let iw_dir = root.path().join("iw-preflight-failure");
        std::fs::create_dir_all(&iw_dir).unwrap();
        let (iw, iw_log) = fake_executable(
            &iw_dir,
            "if [ \"$3\" = get ]; then printf 'Power save: on\\n'; fi",
        );
        let nmcli_dir = root.path().join("nmcli-wifi-preflight-failure");
        std::fs::create_dir_all(&nmcli_dir).unwrap();
        let (nmcli, nmcli_log) = fake_executable(
            &nmcli_dir,
            "if [ \"$1\" = -t ]; then printf 'wifi0:802-11-wireless:wlan0\\nwifi1:802-11-wireless:wlan1\\n'; elif [ \"$5\" = wifi0 ]; then printf '3 (enable)\\n'; else printf 'snapshot failure\\n' >&2; exit 7; fi",
        );
        let manager = HomelabManager::with_roots(
            true,
            true,
            root.path().join("etc"),
            root.path().join("sys"),
        )
        .with_network_tools(root.path().join("unused-ethtool"), nmcli, iw);

        let result = manager.set(HomelabFeature::WifiPowerSaving, true).await;

        assert_eq!(result.status, HomelabFeatureStatus::Error);
        assert!(result.detail.contains("snapshot failure"));
        assert_eq!(
            std::fs::read_to_string(nmcli_log).unwrap(),
            "-t -f UUID,TYPE,connection.interface-name connection show\n-g 802-11-wireless.powersave connection show wifi0\n-g 802-11-wireless.powersave connection show wifi1\n"
        );
        assert_eq!(
            std::fs::read_to_string(iw_log).unwrap(),
            "dev wlan0 get power_save\ndev wlan1 get power_save\n"
        );
    }

    #[tokio::test]
    async fn wifi_mid_operation_failure_rolls_back_all_completed_changes() {
        let root = tempfile::tempdir().unwrap();
        for interface in ["wlan0", "wlan1"] {
            std::fs::create_dir_all(
                root.path()
                    .join("sys/class/net")
                    .join(interface)
                    .join("wireless"),
            )
            .unwrap();
        }
        let iw_dir = root.path().join("iw-rollback");
        std::fs::create_dir_all(&iw_dir).unwrap();
        let (iw, iw_log) = fake_executable(
            &iw_dir,
            "if [ \"$3\" = get ]; then case \"$2\" in wlan0) printf 'Power save: on\\n' ;; wlan1) printf 'Power save: off\\n' ;; esac; fi",
        );
        let nmcli_dir = root.path().join("nmcli-rollback");
        std::fs::create_dir_all(&nmcli_dir).unwrap();
        let (nmcli, nmcli_log) = fake_executable(
            &nmcli_dir,
            "if [ \"$1\" = -t ]; then printf 'wifi0:802-11-wireless:wlan0\\nwifi1:802-11-wireless:wlan1\\n'; elif [ \"$1\" = -g ]; then printf '3 (enable)\\n'; elif [ \"$1:$2:$3:$5\" = connection:modify:wifi1:2 ]; then printf 'profile failure\\n' >&2; exit 7; fi",
        );
        let manager = HomelabManager::with_roots(
            true,
            true,
            root.path().join("etc"),
            root.path().join("sys"),
        )
        .with_network_tools(root.path().join("unused-ethtool"), nmcli, iw);

        let result = manager.set(HomelabFeature::WifiPowerSaving, true).await;

        assert_eq!(result.status, HomelabFeatureStatus::Error);
        assert!(result.detail.contains("profile failure"));
        assert!(result.detail.contains("rollback succeeded"));
        assert_eq!(
            std::fs::read_to_string(nmcli_log).unwrap(),
            "-t -f UUID,TYPE,connection.interface-name connection show\n-g 802-11-wireless.powersave connection show wifi0\n-g 802-11-wireless.powersave connection show wifi1\nconnection modify wifi0 802-11-wireless.powersave 2\nconnection modify wifi1 802-11-wireless.powersave 2\nconnection modify wifi1 802-11-wireless.powersave 3\nconnection modify wifi0 802-11-wireless.powersave 3\n"
        );
        assert_eq!(
            std::fs::read_to_string(iw_log).unwrap(),
            "dev wlan0 get power_save\ndev wlan1 get power_save\ndev wlan0 set power_save off\ndev wlan0 set power_save on\n"
        );
    }

    #[tokio::test]
    async fn rollback_failure_is_reported_without_stopping_remaining_rollbacks() {
        let root = tempfile::tempdir().unwrap();
        for interface in ["wlan0", "wlan1"] {
            std::fs::create_dir_all(
                root.path()
                    .join("sys/class/net")
                    .join(interface)
                    .join("wireless"),
            )
            .unwrap();
        }
        let log = root.path().join("ordered-argv.log");
        let iw = fake_named_executable(
            root.path(),
            "iw",
            &log,
            "if [ \"$3\" = get ]; then case \"$2\" in wlan0) printf 'Power save: on\\n' ;; wlan1) printf 'Power save: off\\n' ;; esac; elif [ \"$2:$3:$5\" = wlan0:set:on ]; then printf 'rollback runtime failure\\n' >&2; exit 8; fi",
        );
        let nmcli = fake_named_executable(
            root.path(),
            "nmcli",
            &log,
            "if [ \"$1\" = -t ]; then printf 'wifi0:802-11-wireless:wlan0\\nwifi1:802-11-wireless:wlan1\\n'; elif [ \"$1\" = -g ]; then printf '3 (enable)\\n'; elif [ \"$1:$2:$3:$5\" = connection:modify:wifi1:2 ]; then printf 'profile failure\\n' >&2; exit 7; fi",
        );
        let manager = HomelabManager::with_roots(
            true,
            true,
            root.path().join("etc"),
            root.path().join("sys"),
        )
        .with_network_tools(root.path().join("unused-ethtool"), nmcli, iw);

        let result = manager.set(HomelabFeature::WifiPowerSaving, true).await;

        assert_eq!(result.status, HomelabFeatureStatus::Error);
        assert!(result.detail.contains("profile failure"));
        assert!(result.detail.contains("rollback failed"));
        assert!(result.detail.contains("rollback runtime failure"));
        assert_eq!(
            std::fs::read_to_string(log).unwrap(),
            "nmcli -t -f UUID,TYPE,connection.interface-name connection show\niw dev wlan0 get power_save\nnmcli -g 802-11-wireless.powersave connection show wifi0\niw dev wlan1 get power_save\nnmcli -g 802-11-wireless.powersave connection show wifi1\nnmcli connection modify wifi0 802-11-wireless.powersave 2\niw dev wlan0 set power_save off\nnmcli connection modify wifi1 802-11-wireless.powersave 2\nnmcli connection modify wifi1 802-11-wireless.powersave 3\niw dev wlan0 set power_save on\nnmcli connection modify wifi0 802-11-wireless.powersave 3\n"
        );
    }

    #[tokio::test]
    async fn enabling_wifi_power_saving_uses_exact_persistent_and_runtime_arguments() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("sys/class/net/wlan0/wireless")).unwrap();
        let iw_dir = root.path().join("iw-enable-power-save");
        std::fs::create_dir_all(&iw_dir).unwrap();
        let (iw, iw_log) = fake_executable(
            &iw_dir,
            "if [ \"$4\" = power_save ] && [ \"$3\" = get ]; then printf 'Power save: off\\n'; fi",
        );
        let nmcli_dir = root.path().join("nmcli-enable-power-save");
        std::fs::create_dir_all(&nmcli_dir).unwrap();
        let (nmcli, nmcli_log) = fake_executable(
            &nmcli_dir,
            "if [ \"$1\" = -t ]; then printf 'wifi-uuid:802-11-wireless:wlan0\\n'; elif [ \"$1\" = -g ]; then printf '2 (disable)\\n'; fi",
        );
        let manager = HomelabManager::with_roots(
            true,
            true,
            root.path().join("etc"),
            root.path().join("sys"),
        )
        .with_network_tools(root.path().join("unused-ethtool"), nmcli, iw);

        let result = manager.set(HomelabFeature::WifiPowerSaving, false).await;

        assert_eq!(result.status, HomelabFeatureStatus::Disabled);
        assert_eq!(result.detail, "wlan0");
        assert_eq!(
            std::fs::read_to_string(nmcli_log).unwrap(),
            "-t -f UUID,TYPE,connection.interface-name connection show\n-g 802-11-wireless.powersave connection show wifi-uuid\nconnection modify wifi-uuid 802-11-wireless.powersave 3\n"
        );
        assert_eq!(
            std::fs::read_to_string(iw_log).unwrap(),
            "dev wlan0 get power_save\ndev wlan0 set power_save on\n"
        );
    }

    #[tokio::test]
    async fn wifi_stability_is_enabled_only_when_runtime_and_profiles_disable_power_saving() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("sys/class/net/wlan0/wireless")).unwrap();
        let iw_dir = root.path().join("iw-status");
        std::fs::create_dir_all(&iw_dir).unwrap();
        let (iw, _) = fake_executable(&iw_dir, "printf 'Power save: off\\n'");
        let nmcli_dir = root.path().join("nmcli-wifi-status");
        std::fs::create_dir_all(&nmcli_dir).unwrap();
        let (nmcli, _) = fake_executable(
            &nmcli_dir,
            "if [ \"$1\" = -t ]; then printf 'wifi-uuid:802-11-wireless:wlan0\\n'; else printf '2 (disable)\\n'; fi",
        );
        let manager = HomelabManager::with_roots(
            true,
            true,
            root.path().join("etc"),
            root.path().join("sys"),
        )
        .with_network_tools(root.path().join("unused-ethtool"), nmcli, iw);

        let row = manager
            .status()
            .await
            .into_iter()
            .find(|row| row.feature == HomelabFeature::WifiPowerSaving)
            .unwrap();

        assert_eq!(row.status, HomelabFeatureStatus::Enabled);
        assert_eq!(row.detail, "wlan0");
    }

    #[tokio::test]
    async fn unsupported_platform_reports_all_features_unavailable() {
        let manager = HomelabManager::with_roots(
            false,
            true,
            PathBuf::from("/path/that/must/not/be/read"),
            PathBuf::from("/path/that/must/not/be/read"),
        );

        let statuses = manager.status().await;

        assert_eq!(statuses.len(), HomelabFeature::ALL.len());
        assert_eq!(
            statuses
                .iter()
                .map(|row| (row.feature, row.status))
                .collect::<Vec<_>>(),
            HomelabFeature::ALL
                .into_iter()
                .map(|feature| (feature, HomelabFeatureStatus::Unavailable))
                .collect::<Vec<_>>()
        );
        assert!(statuses.iter().all(|row| row.detail.contains("Linux")));
    }

    #[test]
    fn features_have_stable_order_and_labels() {
        let values: Vec<(HomelabFeature, &str)> = HomelabFeature::ALL
            .into_iter()
            .map(|feature| (feature, feature.label()))
            .collect();

        assert_eq!(
            values,
            vec![
                (HomelabFeature::KeepAwake, "Keep it awake"),
                (
                    HomelabFeature::AutomaticSleep,
                    "Disable automatic sleep/hibernation"
                ),
                (HomelabFeature::WakeOnLan, "Wake-on-LAN"),
                (HomelabFeature::WifiPowerSaving, "Wi-Fi server stability"),
            ]
        );
    }
}
