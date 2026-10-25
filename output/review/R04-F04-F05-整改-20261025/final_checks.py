import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import subprocess
import time

ROOT = Path(__file__).resolve().parents[3]
BASE = Path(__file__).resolve().parent
BASELINE = json.loads((BASE / "baseline.json").read_text())
FILES = list(BASELINE["accepted_R01_R02_R03_source_sha256"]) + BASELINE["scope"]


def hashes():
    return {p: hashlib.sha256((ROOT / p).read_bytes()).hexdigest() for p in FILES}


TEST = ["cargo", "test", "--offline", "--locked", "-p", "switcher-core", "--lib"]
COMMANDS = [
    ("f04", TEST + ["f04_", "--", "--nocapture"]),
    ("f05", TEST + ["f05_", "--", "--nocapture"]),
    ("user-repairs", TEST + ["user_repairs::"]),
    ("proxy-repair", TEST + ["proxy_repair::"]),
    ("legacy-regions", TEST + ["regions::legacy::"]),
    ("clippy", ["cargo", "clippy", "--offline", "--locked", "-p", "switcher-core", "--lib", "--tests", "--", "-D", "warnings"]),
    ("rustfmt", ["rustfmt", "--check", "--edition", "2024"] + BASELINE["scope"]),
    ("diff-check", ["git", "diff", "--check"]),
]
receipts = []
frozen = hashes()
env = os.environ.copy()
env.pop("CS_REPAIR_READONLY_COPY_SOURCE", None)
for name, command in COMMANDS:
    before = hashes()
    assert before == frozen
    assert all(before[p] == h for p, h in BASELINE["accepted_R01_R02_R03_source_sha256"].items())
    start = time.monotonic()
    log = BASE / ("final-" + name + ".log")
    with log.open("wb") as output:
        proc = subprocess.run(command, cwd=ROOT, env=env, stdout=output, stderr=subprocess.STDOUT)
    body = log.read_text(errors="replace")
    matched = re.search(r"test result: ok\. (\d+) passed;", body)
    count = int(matched.group(1)) if matched else None
    after = hashes()
    entry = dict(
        name=name, command=command, command_display=shlex.join(command),
        exit_code=proc.returncode, seconds=round(time.monotonic() - start, 3),
        log=str(log.relative_to(ROOT)), matched_tests=count,
        source_hashes_before=before, source_hashes_after=after,
        unchanged=before == after, accepted_sources_unchanged=True,
    )
    receipts.append(entry)
    (BASE / "final-receipts.json").write_text(json.dumps(receipts, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps({k: entry[k] for k in ["name", "exit_code", "matched_tests", "seconds", "unchanged"]}), flush=True)
    assert proc.returncode == 0 and before == after
    if name in ["f04", "f05", "user-repairs", "proxy-repair", "legacy-regions"]:
        assert count is not None and count > 0
