#!/usr/bin/env python3
"""Run only checks affected by the local R2 restart test."""
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import time

root = Path(__file__).resolve().parents[3]
evidence = Path(__file__).resolve().parent / "r1"
source = root / "apps/desktop/src-tauri/src/publisher/connection/r2_integration_tests.rs"
environment = json.loads((root / "output/no-account-20261027/storage-final-verify/current/environment.json").read_text())
environment["HOME"] = str(evidence / "home")
environment["TMPDIR"] = str(evidence / "tmp") + "/"
for name in ("home", "tmp"):
    (evidence / name).mkdir(parents=True, exist_ok=True)
sandbox = ["/usr/bin/sandbox-exec", "-f", str(root / "output/no-account-20261027/storage-final-verify/current/account-free.sb")]
toolchain = Path("/Users/gua/.rustup/toolchains/1.96.0-aarch64-apple-darwin/bin")
cargo = str(toolchain / "cargo")
prefix = "publisher::connection::tests::r2_integration_tests::"
checks = [
    ("same-process", [cargo, "test", "--offline", "--locked", "-p", "switcher-desktop", "--bin", "switcher-desktop", prefix + "configured_sdk_recovers_committed_current_after_response_is_lost", "--", "--exact", "--test-threads=1"], 120),
    ("publication", [cargo, "test", "--offline", "--locked", "-p", "switcher-desktop", "--bin", "switcher-desktop", prefix + "configured_publication_uses_sdk_upload_readback_and_conditional_updates", "--", "--exact", "--test-threads=1"], 120),
    ("clippy", [cargo, "clippy", "--offline", "--locked", "-p", "switcher-desktop", "--bin", "switcher-desktop", "--tests", "--", "-D", "warnings"], 240),
    ("fmt", [str(toolchain / "rustfmt"), "--edition", "2024", "--check", str(source)], 30),
    ("diff-check", ["/usr/bin/git", "diff", "--check"], 30),
]

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

before = digest(source)
results = []
for name, command, timeout in checks:
    log = evidence / (name + ".log")
    started = time.monotonic()
    with log.open("wb") as output:
        process = subprocess.Popen(sandbox + command, cwd=root, env=environment, stdout=output, stderr=subprocess.STDOUT, start_new_session=True)
        try:
            code = process.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            process.wait()
            code = 124
    results.append({"name": name, "argv": sandbox + command, "exit": code, "seconds": round(time.monotonic() - started, 3), "log_sha256": digest(log)})
    print(f"{name}: exit {code}", flush=True)
    if code:
        break
after = digest(source)
receipt = {"source_sha256": before, "source_unchanged": before == after, "checks": results}
(evidence / "related-receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
if any(check["exit"] for check in results) or before != after or len(results) != len(checks):
    raise SystemExit(1)
