#!/bin/bash
set -e

export MACOSX_DEPLOYMENT_TARGET=12.0

# Load .env if present
if [ -f .env ]; then
  export $(grep -v '^#' .env | xargs)
fi

VERSION=$(grep '"version"' package.json | head -1 | sed 's/.*: "\(.*\)".*/\1/')

echo "==> Building HarnessKit v${VERSION} (macOS ${MACOSX_DEPLOYMENT_TARGET}+)..."

# Clean extended attributes (prevents codesign issues on APFS/iCloud volumes)
xattr -cr crates/hk-desktop/icons/ public/icons/ 2>/dev/null || true

# Build for Apple Silicon
echo "==> [1/2] Building for Apple Silicon (aarch64)..."
cargo tauri build --target aarch64-apple-darwin

# Build for Intel
echo "==> [2/2] Building for Intel (x86_64)..."
cargo tauri build --target x86_64-apple-darwin

# Paths
ARM_DMG="target/aarch64-apple-darwin/release/bundle/dmg/HarnessKit_${VERSION}_aarch64.dmg"
X64_DMG="target/x86_64-apple-darwin/release/bundle/dmg/HarnessKit_${VERSION}_x64.dmg"

# Notarize (optional — requires APPLE_ID, APPLE_TEAM_ID, APPLE_PASSWORD)
if [ -n "$APPLE_ID" ] && [ -n "$APPLE_TEAM_ID" ] && [ -n "$APPLE_PASSWORD" ]; then
  echo "==> Notarizing Apple Silicon..."
  xcrun notarytool submit "$ARM_DMG" \
    --apple-id "$APPLE_ID" \
    --team-id "$APPLE_TEAM_ID" \
    --password "$APPLE_PASSWORD" \
    --wait
  xcrun stapler staple "$ARM_DMG"

  echo "==> Notarizing Intel..."
  xcrun notarytool submit "$X64_DMG" \
    --apple-id "$APPLE_ID" \
    --team-id "$APPLE_TEAM_ID" \
    --password "$APPLE_PASSWORD" \
    --wait
  xcrun stapler staple "$X64_DMG"

  echo ""
  echo "==> Done! (signed + notarized)"
else
  echo ""
  echo "==> Done! (unsigned — set APPLE_ID, APPLE_TEAM_ID, APPLE_PASSWORD to enable notarization)"
fi

# Build CLI for both architectures
echo "==> Building CLI (aarch64)..."
cargo build --release --target aarch64-apple-darwin -p hk-cli

echo "==> Building CLI (x86_64)..."
cargo build --release --target x86_64-apple-darwin -p hk-cli

echo "    Apple Silicon: $ARM_DMG"
echo "    Intel:         $X64_DMG"
echo "    CLI (arm64):   target/aarch64-apple-darwin/release/hk"
echo "    CLI (x64):     target/x86_64-apple-darwin/release/hk"
