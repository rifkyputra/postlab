#![allow(dead_code)]

use std::path::PathBuf;

/// Who owns the git working tree and which identity/config applies.
pub enum RunAs {
    /// Deploy pipeline: root-owned repos under /var/lib/postlab, root-managed
    /// known_hosts and GIT_CONFIG_GLOBAL.
    Root,
    /// Project browser: user-owned repos; git runs as the given uid so the
    /// checkout is owned by the user, not root.
    User(u32),
}

pub enum GitCreds {
    None,
    /// Token goes to a per-app mode-0600 credential file, never embedded in
    /// the remote URL (which would leak it to ps, reflog, and .git/config).
    HttpsToken { host: String, token: String },
    SshKey { private_key_path: PathBuf },
}
