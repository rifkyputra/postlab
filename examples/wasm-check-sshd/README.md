# Example WASM security check: sshd PermitRootLogin

A minimal guest [component](../../wit/security-check.wit) implementing the
`postlab:plugin/security-plugin` world. It asks the host to read
`/etc/ssh/sshd_config` and emits a critical finding if `PermitRootLogin yes` is
set. This mirrors postlab's native `ssh_root_login` check, but runs sandboxed.

## Build

This crate is excluded from the host workspace because it targets wasm32, so it
builds into its own `target/` dir here. It produces a **component**, not a core
module.

```sh
# one-time toolchain
rustup target add wasm32-unknown-unknown
cargo install wasm-tools
```

From the repo root, the Makefile wraps both steps:

```sh
make wasm-example   # builds + componentizes → postlab_check_sshd.component.wasm
```

Equivalent manual steps from this directory:

```sh
cargo build --release --target wasm32-unknown-unknown
wasm-tools component new \
  target/wasm32-unknown-unknown/release/postlab_check_sshd.wasm \
  -o postlab_check_sshd.component.wasm
```

(Alternatively `cargo component build --release` produces the component in one
step if you have `cargo-component` installed.)

## Install

The host loads every `*.wasm` under `/etc/postlab/plugins`:

```sh
sudo mkdir -p /etc/postlab/plugins
sudo cp postlab_check_sshd.component.wasm /etc/postlab/plugins/
```

Then run a postlab build with the feature enabled:

```sh
cargo build --release -p postlab --features wasm-plugins
```

Open **Security → Findings** and trigger a scan; the WASM finding appears
alongside the native checks. The host grants read access only under `/etc`, so
the plugin can read `sshd_config` but nothing outside that root.
