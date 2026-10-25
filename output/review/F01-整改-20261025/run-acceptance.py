#!/usr/bin/env python3
"""F01 local acceptance; no installation, network, service or real-account commands."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[3]
OUT = Path(__file__).resolve().parent
FILES = ["crates/switcher-core/src/accounts.rs", "crates/switcher-core/src/accounts/legacy.rs", "crates/switcher-core/src/migration.rs"]
def now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()
def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()
def hashes():
    return {name: sha(ROOT / name) for name in FILES}
steps = [
    ("green-final-f01.log", ["cargo", "test", "--offline", "--locked", "-p", "switcher-core", "--lib", "f01_", "--", "--nocapture"], True),
    ("accounts-final.log", ["cargo", "test", "--offline", "--locked", "-p", "switcher-core", "--lib", "accounts::"], True),
    ("migration-final.log", ["cargo", "test", "--offline", "--locked", "-p", "switcher-core", "--lib", "migration::"], True),
    ("lifecycle-final.log", ["cargo", "test", "--offline", "--locked", "-p", "switcher-core", "--lib", "lifecycle::"], True),
    ("clippy-final.log", ["cargo", "clippy", "--offline", "--locked", "-p", "switcher-core", "--lib", "--tests", "--", "-D", "warnings"], False),
    ("rustfmt-final.log", ["rustfmt", "--check", "--edition", "2024", *FILES], False),
    ("diff-check-final.log", ["git", "diff", "--check"], False),
]
report = {
    "finding": "F01", "scope": "local isolated synthetic fixtures only",
    "started_at": now(), "head": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
    "source_hashes_before": hashes(), "product_install": "NOT_RUN", "real_accounts": "NOT_ACCESSED",
    "gui_and_system_services": "NOT_RUN", "chatgpt_review": "PENDING", "steps": []
}
combined = []
for filename, command, is_test in steps:
    started = now()
    result = subprocess.run(command, cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    (OUT / filename).write_text(result.stdout)
    matches = re.findall(r"test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored", result.stdout)
    passed = int(matches[-1][1]) if matches else None
    failed = int(matches[-1][2]) if matches else None
    accepted = result.returncode == 0 and (not is_test or (passed is not None and passed > 0 and failed == 0))
    receipt = {"file": filename, "command": command, "started_at": started, "finished_at": now(),
        "exit_code": result.returncode, "sha256": sha(OUT / filename), "passed": passed, "failed": failed,
        "accepted": accepted}
    report["steps"].append(receipt)
    combined.append(json.dumps(receipt, ensure_ascii=False) + "\n" + result.stdout)
    (OUT / "verification.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    print(filename, "exit=", result.returncode, "passed=", passed, "accepted=", accepted, flush=True)
    if not accepted:
        break
report["finished_at"] = now()
report["source_hashes_after"] = hashes()
report["source_unchanged_during_acceptance"] = report["source_hashes_before"] == report["source_hashes_after"]
report["all_checks_passed"] = len(report["steps"]) == len(steps) and all(x["accepted"] for x in report["steps"]) and report["source_unchanged_during_acceptance"]
manifest_path = ROOT / "docs/handoff/GPTPro-整改检查点-2026-10-25.json"
manifest = json.loads(manifest_path.read_text())
changed, missing, original_mismatch = [], [], []
for item in manifest["files"]:
    name = item["path"]
    for base, original in [(ROOT, False), (Path("/Users/gua/CodexWorkspaces/claude-switcher"), True)]:
        path = base / name
        matches = path.is_file() and sha(path) == item["sha256"] and format(path.stat().st_mode & 0o777, "04o") == item["mode"]
        if not matches:
            if original:
                original_mismatch.append(name)
            elif not path.exists():
                missing.append(name)
            else:
                changed.append(name)
scope = {"checked_at": now(), "manifest_sha256": sha(manifest_path), "frozen_file_count": len(manifest["files"]),
    "changed": changed, "missing": missing, "only_expected_changes": sorted(changed) == sorted(FILES) and not missing,
    "original_source_mismatches": original_mismatch}
(OUT / "scope-verification.json").write_text(json.dumps(scope, ensure_ascii=False, indent=2) + "\n")
report["scope_verified"] = scope["only_expected_changes"] and not original_mismatch
(OUT / "verification.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
(OUT / "verification-combined.log").write_text("\n".join(combined) + "\nFINAL\n" + json.dumps(report, ensure_ascii=False, indent=2) + "\nSCOPE\n" + json.dumps(scope, ensure_ascii=False, indent=2) + "\n")
print("all_checks_passed=", report["all_checks_passed"], "scope_verified=", report["scope_verified"], flush=True)
sys.exit(0 if report["all_checks_passed"] and report["scope_verified"] else 1)
