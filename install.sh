#!/usr/bin/env bash
set -euo pipefail

REPO="rifkyputra/postlab"
DEST="${DEST:-/usr/local/bin/postlab}"
VERSION="${VERSION:-latest}"

# Use the fully static musl variant by default (works on any glibc version,
# including CentOS Stream 9).  Override with VARIANT=gnu for the glibc-linked
# binary (slightly smaller, but requires matching or newer glibc).
VARIANT="${VARIANT:-musl}"

# ── Detect OS + arch ────────────────────────────────────────────────────────

OS="$(uname -s)"
ARCH="$(uname -m)"

case "${OS}-${ARCH}" in
  Linux-x86_64)
    if [ "${VARIANT}" = "gnu" ]; then
      TRIPLE="x86_64-unknown-linux-gnu"
    else
      TRIPLE="x86_64-unknown-linux-musl"
    fi
    ;;
  Linux-aarch64|Linux-arm64)
    if [ "${VARIANT}" = "gnu" ]; then
      TRIPLE="aarch64-unknown-linux-gnu"
    else
      TRIPLE="aarch64-unknown-linux-musl"
    fi
    ;;
  Darwin-arm64)              TRIPLE="aarch64-apple-darwin" ;;
  Darwin-x86_64)             TRIPLE="aarch64-apple-darwin" ;;   # Rosetta
  *)
    echo "Unsupported platform: ${OS} ${ARCH}" >&2
    exit 1
    ;;
esac

ASSET="postlab-${TRIPLE}.tar.gz"
if [ "${VERSION}" = "latest" ]; then
  URL="https://github.com/${REPO}/releases/latest/download/${ASSET}"
else
  URL="https://github.com/${REPO}/releases/download/${VERSION}/${ASSET}"
fi

echo "Downloading postlab ${VERSION} (${TRIPLE})…"
TMP="$(mktemp -d)"
trap 'rm -rf "${TMP}"' EXIT
curl -fsSL --progress-bar "${URL}" -o "${TMP}/postlab.tar.gz"
tar -xzf "${TMP}/postlab.tar.gz" -C "${TMP}"
chmod +x "${TMP}/postlab"

# ── Install (replace any existing binary) ───────────────────────────────────

# `install -m 0755` atomically replaces the destination regardless of whether
# it already exists. Fall back to sudo when the path isn't writable.
if install -m 0755 "${TMP}/postlab" "${DEST}" 2>/dev/null; then
  :
else
  echo "Need sudo to install to ${DEST}"
  sudo install -m 0755 "${TMP}/postlab" "${DEST}"
fi

echo "Installed → ${DEST}"
"${DEST}" --version 2>/dev/null || true
