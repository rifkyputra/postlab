use anyhow::{bail, Context, Result};
use russh::client::{self, Handler};
use russh::keys::key::PublicKey;
use russh::Disconnect;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum AuthMethod {
    Key(String),      // path to private key file
    Password(String), // plaintext password (stored only in memory)
}

/// A live SSH connection to a remote server.
pub struct SshSession {
    session: client::Handle<ClientHandler>,
}

impl SshSession {
    pub async fn connect(
        host: &str,
        port: u16,
        user: &str,
        auth: AuthMethod,
    ) -> Result<Self> {
        let config = Arc::new(client::Config::default());
        let addr = format!("{host}:{port}");

        let mut session = client::connect(config, addr, ClientHandler)
            .await
            .with_context(|| format!("SSH connect to {host}:{port}"))?;

        let authenticated = match auth {
            AuthMethod::Password(ref pw) => {
                session
                    .authenticate_password(user, pw)
                    .await
                    .context("SSH password auth")?
            }
            AuthMethod::Key(ref key_path) => {
                let key = russh_keys::load_secret_key(key_path, None)
                    .with_context(|| format!("load SSH key from {key_path}"))?;
                session
                    .authenticate_publickey(user, Arc::new(key))
                    .await
                    .context("SSH key auth")?
            }
        };

        if !authenticated {
            bail!("SSH authentication failed for {user}@{host}");
        }

        Ok(Self { session })
    }

    /// Execute a remote command and return combined stdout.
    pub async fn exec(&self, cmd: &str) -> Result<String> {
        let mut channel = self.session.channel_open_session().await?;
        channel.exec(true, cmd).await?;

        let mut output = String::new();
        loop {
            match channel.wait().await {
                Some(russh::ChannelMsg::Data { ref data }) => {
                    output.push_str(&String::from_utf8_lossy(data));
                }
                Some(russh::ChannelMsg::ExitStatus { exit_status }) => {
                    if exit_status != 0 {
                        bail!("Remote command exited with status {exit_status}: {cmd}");
                    }
                    break;
                }
                Some(russh::ChannelMsg::Eof) | None => break,
                _ => {}
            }
        }

        Ok(output)
    }

    pub async fn disconnect(self) -> Result<()> {
        self.session
            .disconnect(Disconnect::ByApplication, "done", "en")
            .await?;
        Ok(())
    }
}

// ── russh client handler (minimal — accepts all host keys for now) ─────────

struct ClientHandler;

#[async_trait::async_trait]
impl Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &PublicKey,
    ) -> Result<bool, Self::Error> {
        // TODO: implement known-hosts verification before production use
        Ok(true)
    }
}
