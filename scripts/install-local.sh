#!/bin/sh
# Build and install HarnessKit CLI locally from source for testing.
# Builds frontend, compiles hk-cli with embedded UI, and installs to ~/.local/bin/hk.

set -e

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

echo "==> [1/3] Building frontend..."
npm run build

echo "==> [2/3] Building hk-cli (release)..."
cargo build --release -p hk-cli

echo "==> [3/3] Installing hk to $INSTALL_DIR/hk..."
mkdir -p "$INSTALL_DIR"
cp -f target/release/hk "$INSTALL_DIR/hk"
chmod +x "$INSTALL_DIR/hk"

case ":$PATH:" in
  *":$INSTALL_DIR:"*) ;;
  *) echo "Note: $INSTALL_DIR is not in your current PATH." ;;
esac

echo ""
echo "Installed local build: $INSTALL_DIR/hk"
echo "Run 'hk serve' to test web mode, or 'hk status' to verify."
