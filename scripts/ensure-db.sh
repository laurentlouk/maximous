#!/usr/bin/env bash
# Ensure the .maximous directory and binary exist.
# Called on SessionStart to prepare everything before the MCP server starts.
set -euo pipefail

DB_DIR="${MAXIMOUS_DB_DIR:-.maximous}"
mkdir -p "$DB_DIR"

# Check if maximous binary is already available
if command -v maximous &>/dev/null || [ -x "${CLAUDE_PLUGIN_ROOT:-}/bin/maximous" ] || [ -x "${HOME}/.cargo/bin/maximous" ]; then
  exit 0
fi

# Auto-install: download pre-built binary from GitHub Releases
REPO="laurentlouk/maximous"
INSTALL_DIR="${HOME}/.cargo/bin"
OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"

case "$ARCH" in
  x86_64|amd64) ARCH="x86_64" ;;
  arm64|aarch64) ARCH="arm64" ;;
  *) echo "maximous: unsupported architecture $ARCH — install manually with: cargo install --git https://github.com/${REPO}" >&2; exit 0 ;;
esac

case "$OS" in
  darwin) OS="darwin" ;;
  linux)  OS="linux" ;;
  *)      echo "maximous: unsupported OS $OS — install manually with: cargo install --git https://github.com/${REPO}" >&2; exit 0 ;;
esac

TARGET="${OS}-${ARCH}"
DOWNLOAD_URL="https://github.com/${REPO}/releases/latest/download/maximous-${TARGET}.tar.gz"

echo "maximous: binary not found, downloading from GitHub Releases..." >&2

TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT

if curl -fsSL "$DOWNLOAD_URL" -o "${TMPDIR}/maximous.tar.gz" 2>/dev/null; then
  tar -xzf "${TMPDIR}/maximous.tar.gz" -C "$TMPDIR"
  mkdir -p "$INSTALL_DIR"
  mv "${TMPDIR}/maximous" "${INSTALL_DIR}/maximous"
  chmod +x "${INSTALL_DIR}/maximous"
  echo "maximous: installed to ${INSTALL_DIR}/maximous" >&2
else
  echo "maximous: download failed — install manually with: cargo install --git https://github.com/${REPO}" >&2
fi
