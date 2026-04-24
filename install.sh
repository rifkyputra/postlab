#!/usr/bin/env bash
set -euo pipefail

REPO="rifkyputra/postlab"
BRANCH="main"
DEST="${DEST:-/usr/local/bin/postlab}"
BASE_URL="https://raw.githubusercontent.com/${REPO}/${BRANCH}/binaries"

# ── Detect OS + arch ────────────────────────────────────────────────────────

OS="$(uname -s)"
ARCH="$(uname -m)"

case "${OS}-${ARCH}" in
  Linux-x86_64)   TRIPLE="x86_64-unknown-linux-gnu" ;;
  Darwin-arm64)   TRIPLE="macos_arm64" ;;
  Darwin-x86_64)  TRIPLE="macos_arm64" ;;   # Rosetta
  *)
    echo "Unsupported platform: ${OS} ${ARCH}" >&2
    exit 1
    ;;
esac

URL="${BASE_URL}/${TRIPLE}/postlab"

echo "Downloading postlab (${TRIPLE})…"
curl -fsSL --progress-bar "${URL}" -o /tmp/postlab_download

chmod +x /tmp/postlab_download

# ── Install ─────────────────────────────────────────────────────────────────

if [ -w "$(dirname "${DEST}")" ]; then
  mv /tmp/postlab_download "${DEST}"
else
  echo "Need sudo to write to $(dirname "${DEST}")"
  sudo mv /tmp/postlab_download "${DEST}"
fi

echo "Installed → ${DEST}"
"${DEST}" --version 2>/dev/null || true
