use anyhow::Result;
use sysinfo::System;

use crate::core::models::{GhostProcess, GhostReason};

/// Memory threshold (bytes) above which an unmanaged process is flagged as a
/// potential memory leak.  Default: 200 MiB.
const MEM_LEAK_THRESHOLD: u64 = 200 * 1024 * 1024;

/// Memory threshold (bytes) above which a reparented-to-init process is
/// reported as an orphan.  Default: 50 MiB.
const ORPHAN_MEM_THRESHOLD: u64 = 50 * 1024 * 1024;

// ── Public entry-point ─────────────────────────────────────────────────────

/// Scan the running process table and return processes that look like ghosts:
/// - zombie processes (should have been reaped)
/// - orphaned processes (parent died; reparented to init) consuming memory
/// - unmanaged high-memory processes not tracked by any systemd service unit
///
/// The scan is blocking internally; it runs in a `spawn_blocking` task so it
/// does not stall the async runtime.
pub async fn scan() -> Result<Vec<GhostProcess>> {
    tokio::task::spawn_blocking(scan_blocking).await?
}

// ── Blocking scan ──────────────────────────────────────────────────────────

fn scan_blocking() -> Result<Vec<GhostProcess>> {
    let mut sys = System::new_all();
    sys.refresh_all();

    let mut ghosts: Vec<GhostProcess> = Vec::new();

    for (pid, process) in sys.processes() {
        let pid_u32 = pid.as_u32();

        // Skip PID 1 (init/systemd itself) and PID 2 (kthreadd on Linux)
        if pid_u32 <= 2 {
            continue;
        }

        let name = process.name().to_string();

        // Skip kernel threads — their names are wrapped in brackets on Linux.
        if name.starts_with('[') && name.ends_with(']') {
            continue;
        }

        let mem = process.memory();
        let cpu = process.cpu_usage();
        let ppid = process.parent().map(|p| p.as_u32()).unwrap_or(0);
        let user = process
            .user_id()
            .map(|u| u.to_string())
            .unwrap_or_default();

        let cmdline = process
            .cmd()
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
            .join(" ");

        // ── Zombie check (works on all platforms) ─────────────────────────
        if is_zombie(process) {
            ghosts.push(GhostProcess {
                pid: pid_u32,
                ppid,
                name,
                cmdline,
                user,
                cpu_pct: cpu,
                mem_bytes: mem,
                cgroup: String::new(),
                reason: GhostReason::Zombie,
            });
            continue;
        }

        // ── Systemd cgroup check (Linux only) ─────────────────────────────
        #[cfg(target_os = "linux")]
        {
            let cgroup = read_cgroup(pid_u32);

            // If the process belongs to a .service cgroup it is managed —
            // skip it.
            if is_service_cgroup(&cgroup) {
                continue;
            }

            if ppid == 1 && mem >= ORPHAN_MEM_THRESHOLD {
                ghosts.push(GhostProcess {
                    pid: pid_u32,
                    ppid,
                    name,
                    cmdline,
                    user,
                    cpu_pct: cpu,
                    mem_bytes: mem,
                    cgroup,
                    reason: GhostReason::Orphan,
                });
            } else if mem >= MEM_LEAK_THRESHOLD {
                ghosts.push(GhostProcess {
                    pid: pid_u32,
                    ppid,
                    name,
                    cmdline,
                    user,
                    cpu_pct: cpu,
                    mem_bytes: mem,
                    cgroup,
                    reason: GhostReason::MemLeak,
                });
            }
        }

        // On non-Linux platforms only zombies are reported (see above).
    }

    // Sort: Zombie first, then Orphan, then MemLeak; within each bucket sort
    // by memory descending so the worst offenders appear at the top.
    ghosts.sort_by(|a, b| {
        reason_priority(&a.reason)
            .cmp(&reason_priority(&b.reason))
            .then(b.mem_bytes.cmp(&a.mem_bytes))
    });

    Ok(ghosts)
}

// ── Helpers ────────────────────────────────────────────────────────────────

fn is_zombie(process: &sysinfo::Process) -> bool {
    use sysinfo::ProcessStatus;
    matches!(process.status(), ProcessStatus::Zombie)
}

/// Read the cgroup membership string for `pid` from `/proc/<pid>/cgroup`.
/// Returns an empty string if the file is unreadable.
#[cfg(target_os = "linux")]
fn read_cgroup(pid: u32) -> String {
    std::fs::read_to_string(format!("/proc/{pid}/cgroup"))
        .unwrap_or_default()
        .trim()
        .to_string()
}

/// Returns `true` when the cgroup string contains a systemd `.service` unit,
/// meaning the process is tracked by systemd.
#[cfg(target_os = "linux")]
fn is_service_cgroup(cgroup: &str) -> bool {
    cgroup.contains(".service")
}

fn reason_priority(r: &GhostReason) -> u8 {
    match r {
        GhostReason::Zombie  => 0,
        GhostReason::Orphan  => 1,
        GhostReason::MemLeak => 2,
    }
}
