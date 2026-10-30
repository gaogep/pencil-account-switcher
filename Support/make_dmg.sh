#!/bin/bash
# 产出给好友的 DMG（拖到应用程序）和应用内更新用的 zip。
set -euo pipefail
cd "$(dirname "$0")/.."
./build.sh

VERSION=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' Support/Info.plist)
APP_NAME="Pencil Account Switcher"
STAGE=$(mktemp -d)
trap 'rm -rf "$STAGE"' EXIT
cp -R "build/${APP_NAME}.app" "$STAGE/"
ln -s /Applications "$STAGE/Applications"

OUT_DMG="build/Pencil-Account-Switcher-${VERSION}-arm64.dmg"
OUT_ZIP="build/Pencil-Account-Switcher-${VERSION}-arm64.zip"
rm -f "$OUT_DMG" "$OUT_ZIP"
hdiutil create -volname "$APP_NAME" -srcfolder "$STAGE" -ov -format UDZO "$OUT_DMG"
ditto -c -k --keepParent "build/${APP_NAME}.app" "$OUT_ZIP"
echo "$OUT_DMG"
echo "$OUT_ZIP"
