#!/usr/bin/env bash
# Install maximous binary.
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/laurentlouk/maximous/main/scripts/install.sh | bash
#   or: bash scripts/install.sh
set -euo pipefail

VERSION="${MAXIMOUS_VERSION:-latest}"
INSTALL_DIR="${MAXIMOUS_INSTALL_DIR:-$HOME/.cargo/bin}"
OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"

# Normalize architecture
case "$ARCH" in
  x86_64|amd64) ARCH="x86_64" ;;
  arm64|aarch64) ARCH="arm64" ;;
  *) echo "Unsupported architecture: $ARCH" >&2; exit 1 ;;
esac

# Normalize OS
case "$OS" in
  darwin) OS="darwin" ;;
  linux)  OS="linux" ;;
  *)      echo "Unsupported OS: $OS" >&2; exit 1 ;;
esac

REPO="laurentlouk/maximous"
TARGET="${OS}-${ARCH}"

echo "Installing maximous for ${TARGET}..."

# Get download URL
if [ "$VERSION" = "latest" ]; then
  DOWNLOAD_URL="https://github.com/${REPO}/releases/latest/download/maximous-${TARGET}.tar.gz"
else
  DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${VERSION}/maximous-${TARGET}.tar.gz"
fi

# Download and install
TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT

echo "Downloading from ${DOWNLOAD_URL}..."
curl -fsSL "$DOWNLOAD_URL" -o "${TMPDIR}/maximous.tar.gz"

echo "Extracting..."
tar -xzf "${TMPDIR}/maximous.tar.gz" -C "$TMPDIR"

echo "Installing to ${INSTALL_DIR}..."
mkdir -p "$INSTALL_DIR"
mv "${TMPDIR}/maximous" "${INSTALL_DIR}/maximous"
chmod +x "${INSTALL_DIR}/maximous"

echo "Done! maximous installed to ${INSTALL_DIR}/maximous"
echo "Make sure ${INSTALL_DIR} is in your PATH."

# Verify
if command -v maximous &>/dev/null; then
  echo "Version: $(maximous --version 2>/dev/null || echo 'installed')"
else
  echo "Note: ${INSTALL_DIR} may not be in your PATH yet."
  echo "Add it with: export PATH=\"${INSTALL_DIR}:\$PATH\""
fi
