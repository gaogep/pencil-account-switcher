"""Export core-owned view state fixtures for UI development (not a test runner)."""
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parent.parent
NAMES = (
    "main", "settings", "settings-launch-on", "manage", "main-empty", "main-single",
    "main-unregistered-live", "main-expired", "main-unknown-quota", "main-switching",
    "main-refreshing", "main-adding", "main-feedback-success", "main-feedback-warning",
    "main-feedback-long", "main-persistence-blocked", "main-update-available",
    "main-update-downloading", "main-update-ready", "settings-update-available",
    "manage-expired", "manage-confirm-remove",
)
subprocess.run(["cargo", "build", "--locked", "-p", "switchboard-core", "--bin", "snapshot"], cwd=ROOT, check=True)
output = ROOT / "ui/public/scenarios"
output.mkdir(parents=True, exist_ok=True)
for name in NAMES:
    result = subprocess.run([str(ROOT / "target/debug/snapshot"), name], cwd=ROOT, check=True, text=True, capture_output=True)
    (output / f"{name}.json").write_text(json.dumps(json.loads(result.stdout), ensure_ascii=False, indent=2) + "\n")
