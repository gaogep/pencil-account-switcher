#!/bin/bash
# 生成界面视觉基线：用演示数据把 SnapshotScenarios.swift 里的每个状态截成 PNG。
# 默认输出到 Tests/baseline/macos/，重构后的新界面要和这里的图逐张比对。
# 只截部分状态：SWITCHBOARD_SNAPSHOT_ONLY=main,settings ./Tests/snapshot.sh
set -euo pipefail
cd "$(dirname "$0")/.."
OUT="${1:-Tests/baseline/macos}"
./build.sh >/dev/null
mkdir -p "$OUT"
BIN="build/Pencil Account Switcher.app/Contents/MacOS/Switchboard"
SWITCHBOARD_DEMO=1 SWITCHBOARD_SNAPSHOT="$OUT/panel" "$BIN" &
PID=$!
for _ in $(seq 1 90); do
  kill -0 "$PID" 2>/dev/null || break
  sleep 1
done
if kill -0 "$PID" 2>/dev/null; then
  kill "$PID"
  echo "截图超时" >&2
  exit 1
fi
wait "$PID" || true
ls "$OUT"/panel-*.png | wc -l | xargs echo "已生成截图数："
