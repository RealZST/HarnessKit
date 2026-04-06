#!/bin/bash
set -e

export MACOSX_DEPLOYMENT_TARGET=12.0

VERSION=$(grep '"version"' package.json | head -1 | sed 's/.*: "\(.*\)".*/\1/')

echo "==> Building HarnessKit v${VERSION} (macOS ${MACOSX_DEPLOYMENT_TARGET}+)..."

# Build for Apple Silicon
echo "==> [1/2] Building for Apple Silicon (aarch64)..."
cargo tauri build --target aarch64-apple-darwin

# Build for Intel
echo "==> [2/2] Building for Intel (x86_64)..."
cargo tauri build --target x86_64-apple-darwin

# Paths
ARM_APP="target/aarch64-apple-darwin/release/bundle/macos/HarnessKit.app"
ARM_DMG="target/aarch64-apple-darwin/release/bundle/dmg/HarnessKit_${VERSION}_aarch64.dmg"
X64_APP="target/x86_64-apple-darwin/release/bundle/macos/HarnessKit.app"
X64_DMG="target/x86_64-apple-darwin/release/bundle/dmg/HarnessKit_${VERSION}_x64.dmg"

# Sign Apple Silicon
echo "==> Signing Apple Silicon..."
xattr -cr "$ARM_APP"
codesign --force --deep --sign - "$ARM_APP"

# Sign Intel
echo "==> Signing Intel..."
xattr -cr "$X64_APP"
codesign --force --deep --sign - "$X64_APP"

# Build CLI for both architectures
echo "==> Building CLI (aarch64)..."
cargo build --release --target aarch64-apple-darwin -p hk-cli

echo "==> Building CLI (x86_64)..."
cargo build --release --target x86_64-apple-darwin -p hk-cli

echo ""
echo "==> Done!"
echo "    Apple Silicon: $ARM_DMG"
echo "    Intel:         $X64_DMG"
echo "    CLI (arm64):   target/aarch64-apple-darwin/release/hk"
echo "    CLI (x64):     target/x86_64-apple-darwin/release/hk"
