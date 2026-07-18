pub mod deploy;
pub mod docker;
pub mod firewall;
pub mod gateway;
pub mod ghost;
pub mod models;
pub mod nats;
pub mod packages;
pub mod platform;
#[cfg(feature = "wasm-plugins")]
pub mod plugins;
pub mod portcheck;
pub mod processes;
pub mod security;
pub mod services;
pub mod ssh;
pub mod storage;
pub mod system;
pub mod tailscale;
pub mod tunnel;
pub mod users;
pub mod wasm_cloud;
pub mod workloads;
pub mod pi_agent;
pub mod projects;

pub use platform::Platform;

pub fn real_home() -> String {
    if let Ok(sudo_user) = std::env::var("SUDO_USER") {
        if let Ok(Some(user)) = nix::unistd::User::from_name(&sudo_user) {
            return user.dir.to_string_lossy().to_string();
        }
    }
    std::env::var("HOME").unwrap_or_default()
}
