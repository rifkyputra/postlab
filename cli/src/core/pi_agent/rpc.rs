use anyhow::Result;
use serde_json::Value;
use std::process::Stdio;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt},
    process::Command,
    sync::mpsc,
};

pub enum PiRpcEvent {
    AgentStart,
    TextDelta(String),
    AgentEnd,
    ToolStart(String),
    ToolEnd { name: String, is_error: bool },
    Error(String),
    Stderr(String),
    Stopped,
}

pub struct RpcHandle {
    pub cmd_tx: mpsc::UnboundedSender<Value>,
}

impl std::fmt::Debug for RpcHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RpcHandle").finish_non_exhaustive()
    }
}

pub async fn spawn_rpc(
    provider: &str,
    model: &str,
    event_tx: mpsc::UnboundedSender<PiRpcEvent>,
) -> Result<RpcHandle> {
    let bin = super::find_pi()
        .await
        .ok_or_else(|| anyhow::anyhow!("pi not found — install it first via the Automation screen"))?;

    // When postlab runs as root (via sudo), drop back to the original user so
    // pi-agent doesn't run with root privileges.
    let sudo_uid: Option<u32> = std::env::var("SUDO_UID").ok().and_then(|s| s.parse().ok());
    let sudo_gid: Option<u32> = std::env::var("SUDO_GID").ok().and_then(|s| s.parse().ok());
    let home = crate::core::real_home();

    let current_path = std::env::var("PATH").unwrap_or_default();
    let pi_bin_dir = std::path::Path::new(&bin)
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or("");
    let extended_path =
        format!("{home}/.local/bin:{home}/.npm-global/bin:/usr/local/bin:{pi_bin_dir}:{current_path}");

    let mut cmd = Command::new(&bin);
    cmd.args(["--mode", "rpc", "--provider", provider, "--model", model])
        .env("HOME", &home)
        .env("PATH", &extended_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(unix)]
    if let (Some(uid), Some(gid)) = (sudo_uid, sudo_gid) {
        cmd.uid(uid).gid(gid);
    }

    let mut child = cmd.spawn()?;

    let stdin = child.stdin.take().expect("stdin piped");
    let stdout = child.stdout.take().expect("stdout piped");
    let stderr = child.stderr.take().expect("stderr piped");

    let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel::<Value>();

    // stdin writer: forward commands → pi stdin
    tokio::spawn(async move {
        let mut writer = tokio::io::BufWriter::new(stdin);
        while let Some(cmd) = cmd_rx.recv().await {
            let line = serde_json::to_string(&cmd).unwrap_or_default();
            if writer.write_all(line.as_bytes()).await.is_err() {
                break;
            }
            if writer.write_all(b"\n").await.is_err() {
                break;
            }
            if writer.flush().await.is_err() {
                break;
            }
        }
    });

    // stderr reader: forward to event channel for diagnostics
    let stderr_tx = event_tx.clone();
    tokio::spawn(async move {
        let mut lines = tokio::io::BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let _ = stderr_tx.send(PiRpcEvent::Stderr(line));
        }
    });

    // stdout reader: parse events and forward to caller
    tokio::spawn(async move {
        let mut lines = tokio::io::BufReader::new(stdout).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if let Ok(v) = serde_json::from_str::<Value>(&line) {
                if let Some(ev) = parse_event(&v) {
                    if event_tx.send(ev).is_err() {
                        break;
                    }
                }
            }
        }
        let _ = event_tx.send(PiRpcEvent::Stopped);
        let _ = child.wait().await;
    });

    Ok(RpcHandle { cmd_tx })
}

fn parse_event(v: &Value) -> Option<PiRpcEvent> {
    let event_type = v.get("type")?.as_str()?;
    match event_type {
        "agent_start" => Some(PiRpcEvent::AgentStart),
        "agent_end" => Some(PiRpcEvent::AgentEnd),
        "message_update" => {
            let ae = v.get("assistantMessageEvent")?;
            if ae.get("type")?.as_str()? == "text_delta" {
                let delta = ae.get("delta")?.as_str()?.to_string();
                Some(PiRpcEvent::TextDelta(delta))
            } else {
                None
            }
        }
        "tool_execution_start" => {
            let name = v.get("toolName")?.as_str()?.to_string();
            Some(PiRpcEvent::ToolStart(name))
        }
        "tool_execution_end" => {
            let name = v.get("toolName")?.as_str()?.to_string();
            let is_error = v
                .get("isError")
                .and_then(|e| e.as_bool())
                .unwrap_or(false);
            Some(PiRpcEvent::ToolEnd { name, is_error })
        }
        "response" if v.get("success").and_then(|s| s.as_bool()) == Some(false) => {
            let error = v
                .get("error")
                .and_then(|e| e.as_str())
                .unwrap_or("unknown error")
                .to_string();
            Some(PiRpcEvent::Error(error))
        }
        _ => None,
    }
}
