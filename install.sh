#!/bin/sh
# HarnessKit CLI installer — auto-detects architecture and installs the
# latest `hk` binary to ~/.local/bin. Re-run to update to the latest version.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/RealZST/HarnessKit/main/install.sh | sh

set -e

REPO="RealZST/HarnessKit"
INSTALL_DIR="$HOME/.local/bin"

# Detect OS
OS=$(uname -s)
if [ "$OS" != "Darwin" ]; then
  echo "Error: this installer only supports macOS. Detected: $OS"
  exit 1
fi

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

# Ensure ~/.local/bin is in PATH by adding to shell config
add_to_path() {
  rc_file="$1"
  line='export PATH="$HOME/.local/bin:$PATH"'
  if [ -f "$rc_file" ] && grep -qF '.local/bin' "$rc_file"; then
    return  # Already present
  fi
  echo "" >> "$rc_file"
  echo "# Added by HarnessKit CLI installer" >> "$rc_file"
  echo "$line" >> "$rc_file"
  echo "Added ~/.local/bin to PATH in $rc_file"
}

case ":$PATH:" in
  *":$INSTALL_DIR:"*)
    # Already in PATH, nothing to do
    ;;
  *)
    # Detect shell and add to appropriate config
    SHELL_NAME=$(basename "$SHELL" 2>/dev/null || echo "")
    case "$SHELL_NAME" in
      zsh)  add_to_path "$HOME/.zshrc" ;;
      bash) add_to_path "$HOME/.bashrc" ;;
      *)    add_to_path "$HOME/.profile" ;;
    esac
    echo ""
    echo "Restart your terminal for PATH changes to take effect."
    ;;
esac

echo ""
echo "Verify with: hk status"
