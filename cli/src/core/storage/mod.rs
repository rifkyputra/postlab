use crate::core::models::{SmartInfo, StorageDevice};
use anyhow::Result;
use serde::Deserialize;
use tokio::process::Command;

#[derive(Debug, Deserialize)]
struct LsblkOutput {
    blockdevices: Vec<LsblkDevice>,
}

#[derive(Debug, Deserialize)]
struct LsblkDevice {
    name: String,
    mountpoint: Option<String>,
    fstype: Option<String>,
    size: Option<u64>,
    fsused: Option<u64>,
    fsavail: Option<u64>,
    #[serde(default)]
    children: Option<Vec<LsblkDevice>>,
}

#[derive(Debug, Deserialize)]
struct SmartCtlOutput {
    smart_status: Option<SmartStatus>,
    temperature: Option<Temperature>,
    power_on_time: Option<PowerOnTime>,
    #[allow(dead_code)]
    model_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SmartStatus {
    passed: bool,
}

#[derive(Debug, Deserialize)]
struct Temperature {
    current: u32,
}

#[derive(Debug, Deserialize)]
struct PowerOnTime {
    hours: u64,
}

fn collect_mounted(devices: &[LsblkDevice]) -> Vec<StorageDevice> {
    let mut out = Vec::new();
    for d in devices {
        if let (Some(mountpoint), Some(fstype), Some(size)) =
            (&d.mountpoint, &d.fstype, d.size)
        {
            if mountpoint.is_empty() || fstype.is_empty() {
                continue;
            }
            out.push(StorageDevice {
                device: format!("/dev/{}", d.name),
                mount: mountpoint.clone(),
                fs_type: fstype.clone(),
                total_bytes: size,
                used_bytes: d.fsused.unwrap_or(0),
                avail_bytes: d.fsavail.unwrap_or(0),
            });
        }
        if let Some(ref children) = d.children {
            out.extend(collect_mounted(children));
        }
    }
    out
}

pub async fn list_filesystems() -> Result<Vec<StorageDevice>> {
    let output = Command::new("lsblk")
        .args([
            "--bytes",
            "--json",
            "-o",
            "NAME,MOUNTPOINT,FSTYPE,SIZE,FSUSED,FSAVAIL",
        ])
        .output()
        .await?;
    if !output.status.success() {
        anyhow::bail!(
            "lsblk failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let parsed: LsblkOutput = serde_json::from_slice(&output.stdout)?;
    Ok(collect_mounted(&parsed.blockdevices))
}

pub async fn list_physical() -> Result<Vec<SmartInfo>> {
    let output = Command::new("lsblk")
        .args(["--json", "-o", "NAME,MODEL,SIZE,TYPE"])
        .output()
        .await?;
    if !output.status.success() {
        anyhow::bail!(
            "lsblk failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let devices = parsed["blockdevices"]
        .as_array()
        .map(|a| a.to_vec())
        .unwrap_or_default();

    let mut results = Vec::new();
    for dev in devices {
        let dev_type = dev["type"].as_str().unwrap_or("");
        if dev_type != "disk" {
            continue;
        }
        let name = dev["name"].as_str().unwrap_or("").to_string();
        let model = dev["model"]
            .as_str()
            .unwrap_or("")
            .trim()
            .to_string();
        let size = dev["size"].as_str().unwrap_or("").to_string();

        let device_path = format!("/dev/{}", name);
        let (healthy, temp_c, poh) = smartctl_info(&device_path).await;

        results.push(SmartInfo {
            device: device_path,
            model: if model.is_empty() { size } else { model },
            healthy,
            temp_celsius: temp_c,
            power_on_hours: poh,
        });
    }
    Ok(results)
}

async fn smartctl_info(device: &str) -> (bool, u32, u64) {
    let output = match Command::new("smartctl")
        .args(["--json", "-H", "-A", device])
        .output()
        .await
    {
        Ok(o) => o,
        Err(_) => return (false, 0, 0),
    };
    if !output.status.success() {
        return (false, 0, 0);
    }
    let parsed: SmartCtlOutput = match serde_json::from_slice(&output.stdout) {
        Ok(p) => p,
        Err(_) => return (false, 0, 0),
    };
    let healthy = parsed
        .smart_status
        .map(|s| s.passed)
        .unwrap_or(false);
    let temp = parsed
        .temperature
        .map(|t| t.current)
        .unwrap_or(0);
    let poh = parsed
        .power_on_time
        .map(|p| p.hours)
        .unwrap_or(0);
    (healthy, temp, poh)
}

pub async fn mount(device: &str, mountpoint: &str) -> Result<String> {
    tokio::fs::create_dir_all(mountpoint).await?;
    let output = Command::new("mount")
        .args([device, mountpoint])
        .output()
        .await?;
    if output.status.success() {
        Ok(format!("Mounted {} at {}", device, mountpoint))
    } else {
        anyhow::bail!("{}", String::from_utf8_lossy(&output.stderr).trim())
    }
}

pub async fn umount(target: &str) -> Result<String> {
    let output = Command::new("umount")
        .arg(target)
        .output()
        .await?;
    if output.status.success() {
        Ok(format!("Unmounted {}", target))
    } else {
        anyhow::bail!("{}", String::from_utf8_lossy(&output.stderr).trim())
    }
}

pub async fn read_fstab() -> Result<String> {
    tokio::fs::read_to_string("/etc/fstab")
        .await
        .map_err(|e| anyhow::anyhow!("Cannot read /etc/fstab: {}", e))
}
