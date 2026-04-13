#!/bin/sh
# HarnessKit CLI installer — auto-detects architecture and installs the
# latest `hk` binary to ~/.local/bin.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/RealZST/HarnessKit/main/install.sh | sh

set -e

REPO="RealZST/HarnessKit"
INSTALL_DIR="$HOME/.local/bin"

# Detect architecture
ARCH=$(uname -m)
case "$ARCH" in
  arm64|aarch64) BINARY="hk-macos-arm64" ;;
  x86_64)        BINARY="hk-macos-x64" ;;
  *)
    echo "Error: unsupported architecture: $ARCH"
    exit 1
    ;;
esac

# Get latest release tag
TAG=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name"' | head -1 | sed 's/.*: "//;s/".*//')
if [ -z "$TAG" ]; then
  echo "Error: failed to fetch latest release"
  exit 1
fi

URL="https://github.com/$REPO/releases/download/$TAG/$BINARY"

echo "Installing HarnessKit CLI $TAG ($ARCH)..."

# Download
mkdir -p "$INSTALL_DIR"
curl -fsSL "$URL" -o "$INSTALL_DIR/hk"
chmod +x "$INSTALL_DIR/hk"

echo "Installed hk to $INSTALL_DIR/hk"

# Check if INSTALL_DIR is in PATH
case ":$PATH:" in
  *":$INSTALL_DIR:"*) ;;
  *)
    echo ""
    echo "Add ~/.local/bin to your PATH:"
    echo "  export PATH=\"\$HOME/.local/bin:\$PATH\""
    echo ""
    echo "Then restart your terminal and verify with: hk status"
    ;;
esac
