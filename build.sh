#!/bin/bash
# 构建 Pencil Account Switcher.app
# 本机 CommandLineTools 的 SwiftPM (ManifestAPI) 已损坏，故不用 swift build，
# 用 swiftc 直接编译（项目零第三方依赖，完全等效）。
# 修复 CLT 后如需恢复 SPM：sudo rm -rf /Library/Developer/CommandLineTools && xcode-select --install
set -euo pipefail
cd "$(dirname "$0")"

mkdir -p build .build/module-cache
swiftc -O -module-cache-path "$PWD/.build/module-cache" -o build/Switchboard \
    Sources/Switchboard/*.swift \
    Sources/Switchboard/UI/*.swift

APP="build/Pencil Account Switcher.app"
rm -rf "build/Switchboard.app" "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp build/Switchboard "$APP/Contents/MacOS/Switchboard"
cp Support/Info.plist "$APP/Contents/Info.plist"
cp Support/AppIcon.icns "$APP/Contents/Resources/AppIcon.icns"

codesign --force --sign - "$APP"
rm -f build/Switchboard
echo "已构建 $APP"
