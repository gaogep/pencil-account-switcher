#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p .build/tests
swiftc -o .build/tests/RegressionTests \
  Sources/Switchboard/AppModel.swift Sources/Switchboard/Models.swift \
  Sources/Switchboard/PenTargetAdapter.swift Sources/Switchboard/CredentialStore.swift \
  Sources/Switchboard/WebLogin.swift Sources/Switchboard/QuotaProvider.swift Sources/Switchboard/UpdateChecker.swift Sources/Switchboard/UI/*.swift \
  Tests/RegressionTests.swift
.build/tests/RegressionTests
if [[ "${1:-}" == "--ui" ]]; then
  QA_APP=".build/tests/SwitchboardQA.app"
  mkdir -p "$QA_APP/Contents/MacOS"
  cp .build/tests/RegressionTests "$QA_APP/Contents/MacOS/SwitchboardQA"
  cat > "$QA_APP/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>dev.switchboard.QA</string>
<key>CFBundleName</key><string>SwitchboardQA</string>
<key>CFBundleExecutable</key><string>SwitchboardQA</string>
<key>CFBundlePackageType</key><string>APPL</string>
</dict></plist>
PLIST
  open "$QA_APP"
fi
