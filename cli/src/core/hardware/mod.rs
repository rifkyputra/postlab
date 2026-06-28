use crate::core::models::{
    BootAnalysis, BootUnit, LoadAvg, SensorKind, SensorReading, SensorReadings,
};
use anyhow::Result;
use std::path::Path;
use tokio::process::Command;

pub async fn read_sensors() -> Result<SensorReadings> {
    let sensors_tool = sensors_tool_present().await;
    let mut readings = Vec::new();

    let mut hwmon = match tokio::fs::read_dir("/sys/class/hwmon").await {
        Ok(d) => d,
        Err(_) => return Ok(SensorReadings { readings, sensors_tool }),
    };

    while let Some(entry) = hwmon.next_entry().await? {
        let dir = entry.path();
        let chip = read_trim(&dir.join("name"))
            .await
            .unwrap_or_else(|| "hwmon".to_string());

        let mut files = match tokio::fs::read_dir(&dir).await {
            Ok(f) => f,
            Err(_) => continue,
        };
        let mut names = Vec::new();
        while let Some(f) = files.next_entry().await? {
            if let Some(n) = f.file_name().to_str() {
                names.push(n.to_string());
            }
        }

        for fname in &names {
            if let Some(idx) = fname.strip_prefix("temp").and_then(|s| s.strip_suffix("_input")) {
                if let Some(v) = read_num(&dir.join(fname)).await {
                    let label = read_trim(&dir.join(format!("temp{}_label", idx)))
                        .await
                        .unwrap_or_else(|| format!("temp{}", idx));
                    readings.push(SensorReading {
                        chip: chip.clone(),
                        label,
                        kind: SensorKind::Temp,
                        value: v / 1000.0,
                        unit: "°C".to_string(),
                    });
                }
            } else if let Some(idx) = fname.strip_prefix("fan").and_then(|s| s.strip_suffix("_input")) {
                if let Some(v) = read_num(&dir.join(fname)).await {
                    let label = read_trim(&dir.join(format!("fan{}_label", idx)))
                        .await
                        .unwrap_or_else(|| format!("fan{}", idx));
                    readings.push(SensorReading {
                        chip: chip.clone(),
                        label,
                        kind: SensorKind::Fan,
                        value: v,
                        unit: "RPM".to_string(),
                    });
                }
            }
        }
    }

    readings.sort_by(|a, b| {
        (a.kind as u8, &a.chip, &a.label).cmp(&(b.kind as u8, &b.chip, &b.label))
    });
    Ok(SensorReadings { readings, sensors_tool })
}

pub async fn read_load() -> Result<LoadAvg> {
    let content = tokio::fs::read_to_string("/proc/loadavg").await?;
    let cols: Vec<&str> = content.split_whitespace().collect();
    if cols.len() < 4 {
        anyhow::bail!("unexpected /proc/loadavg format");
    }
    let (running, total) = cols[3]
        .split_once('/')
        .map(|(r, t)| (r.parse().unwrap_or(0), t.parse().unwrap_or(0)))
        .unwrap_or((0, 0));
    Ok(LoadAvg {
        one: cols[0].parse().unwrap_or(0.0),
        five: cols[1].parse().unwrap_or(0.0),
        fifteen: cols[2].parse().unwrap_or(0.0),
        running,
        total,
    })
}

pub async fn read_boot() -> Result<BootAnalysis> {
    let summary = run_cmd("systemd-analyze", &[]).await?;
    let mut analysis = BootAnalysis::default();

    if let Some(rest) = summary.split("finished in").nth(1) {
        let (parts, total) = match rest.split_once('=') {
            Some((p, t)) => (p, Some(t)),
            None => (rest, None),
        };
        for seg in parts.split('+') {
            let seg = seg.trim();
            if let Some(open) = seg.find('(') {
                let secs = parse_duration(seg[..open].trim());
                let label = seg[open + 1..].trim_end_matches(')').trim();
                match label {
                    "firmware" => analysis.firmware_secs = secs,
                    "loader" => analysis.loader_secs = secs,
                    "kernel" => analysis.kernel_secs = secs,
                    "initrd" => analysis.kernel_secs += secs,
                    "userspace" => analysis.userspace_secs = secs,
                    _ => {}
                }
            }
        }
        if let Some(t) = total {
            analysis.total_secs = parse_duration(t.trim());
        }
    }

    if let Ok(blame) = run_cmd("systemd-analyze", &["blame"]).await {
        for line in blame.lines().take(15) {
            let line = line.trim();
            if let Some((dur, name)) = line.split_once(char::is_whitespace) {
                let name = name.trim().to_string();
                if !name.is_empty() {
                    analysis.units.push(BootUnit {
                        name,
                        secs: parse_duration(dur.trim()),
                    });
                }
            }
        }
    }

    Ok(analysis)
}

pub async fn sensors_tool_present() -> bool {
    Command::new("sh")
        .args(["-c", "command -v sensors"])
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false)
}

async fn read_trim(path: &Path) -> Option<String> {
    tokio::fs::read_to_string(path)
        .await
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

async fn read_num(path: &Path) -> Option<f64> {
    read_trim(path).await.and_then(|s| s.parse::<f64>().ok())
}

// systemd-analyze durations look like "1min 2.345s", "234ms", "5.1s".
fn parse_duration(s: &str) -> f64 {
    let mut total = 0.0;
    for tok in s.split_whitespace() {
        if let Some(v) = tok.strip_suffix("ms") {
            total += v.parse::<f64>().unwrap_or(0.0) / 1000.0;
        } else if let Some(v) = tok.strip_suffix("min") {
            total += v.parse::<f64>().unwrap_or(0.0) * 60.0;
        } else if let Some(v) = tok.strip_suffix('h') {
            total += v.parse::<f64>().unwrap_or(0.0) * 3600.0;
        } else if let Some(v) = tok.strip_suffix('s') {
            total += v.parse::<f64>().unwrap_or(0.0);
        }
    }
    total
}

async fn run_cmd(program: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(program).args(args).output().await?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        anyhow::bail!("{}", String::from_utf8_lossy(&output.stderr).trim())
    }
}
