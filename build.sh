#!/bin/bash
set -e

echo "==> Building HarnessKit..."
cargo tauri build

APP_PATH="target/release/bundle/macos/HarnessKit.app"

echo "==> Cleaning extended attributes..."
xattr -cr "$APP_PATH"

echo "==> Signing (ad-hoc)..."
codesign --force --deep --sign - "$APP_PATH"

VERSION=$(grep '"version"' package.json | head -1 | sed 's/.*: "\(.*\)".*/\1/')

echo "==> Re-creating DMG..."
DMG_DIR="target/release/bundle/dmg"
DMG_PATH="$DMG_DIR/HarnessKit_${VERSION}_aarch64.dmg"
rm -f "$DMG_PATH"

# Create a staging directory with .app + Applications symlink
DMG_STAGE=$(mktemp -d)
cp -R "$APP_PATH" "$DMG_STAGE/"
ln -s /Applications "$DMG_STAGE/Applications"

hdiutil create -volname "HarnessKit" -srcfolder "$DMG_STAGE" -ov -format UDZO "$DMG_PATH"
rm -rf "$DMG_STAGE"

echo "==> Done!"
echo "    .app: $APP_PATH"
echo "    .dmg: $DMG_PATH"
