#![allow(dead_code)]

#[derive(thiserror::Error, Debug)]
pub enum GitError {
    #[error("git is not installed")]
    NotInstalled,
    #[error("working tree has local changes")]
    DirtyTree,
    #[error("cannot fast-forward; remote diverged")]
    FastForwardRejected,
    #[error("authentication failed")]
    AuthFailed,
    #[error("host key rejected")]
    HostKeyRejected,
    #[error("remote not found")]
    RemoteNotFound,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
