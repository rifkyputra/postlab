wit_bindgen::generate!({
    path: "../../wit",
    world: "security-plugin",
});

use exports::postlab::plugin::check::{Finding, Guest};
use postlab::plugin::host::read_file;

const SSHD_CONFIG: &str = "/etc/ssh/sshd_config";

struct Component;

impl Guest for Component {
    fn scan() -> Vec<Finding> {
        // read-file is the only capability the host grants; an error means the
        // path was denied or unreadable, so there is nothing to report.
        let Ok(content) = read_file(SSHD_CONFIG) else {
            return Vec::new();
        };
        let enabled = content.lines().any(|l| {
            let l = l.trim();
            !l.starts_with('#')
                && l.to_lowercase().starts_with("permitrootlogin")
                && l.contains("yes")
        });
        if !enabled {
            return Vec::new();
        }
        vec![Finding {
            id: "wasm_ssh_root_login".to_string(),
            title: "SSH root login enabled".to_string(),
            severity: 0,
            description: "PermitRootLogin yes (reported by example wasm check)".to_string(),
            file_path: Some(SSHD_CONFIG.to_string()),
            fix_description: "Set PermitRootLogin no".to_string(),
        }]
    }
}

export!(Component);
