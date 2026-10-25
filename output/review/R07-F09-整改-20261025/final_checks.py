"""Run frozen frontend checks in a disposable copy, using existing dependencies."""
import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import shutil
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[3]
OUT = Path(__file__).resolve().parent
BASELINE = json.loads((OUT / "baseline.json").read_text())
OWNED = [
    "apps/desktop/src/components/RegionSettings.svelte",
    "apps/desktop/src/App.svelte",
    "apps/desktop/tests/repair-preview.test.mjs",
]
DEPENDENCIES = Path(json.loads((OUT / "offline-runtime.json").read_text())["dependency_source"])


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def sources():
    return {p: sha(ROOT / p) for p in sorted(set(BASELINE["accepted_R01_R06_sources"]) | set(OWNED))}


def main():
    expected = sources()
    for path, digest in BASELINE["accepted_R01_R06_sources"].items():
        assert expected[path] == digest, f"accepted source changed: {path}"
    changed = subprocess.check_output(["git", "diff", "--name-only"], cwd=ROOT, text=True).splitlines()
    assert set(changed) <= set(expected), f"unexpected source changes: {set(changed) - set(expected)}"

    base = Path(tempfile.mkdtemp(prefix="claude-r07-final-"))
    mirror = base / "workspace"
    app = mirror / "apps/desktop"
    app.parent.mkdir(parents=True)
    shutil.copytree(ROOT / "apps/desktop", app, symlinks=True,
                    ignore=shutil.ignore_patterns("node_modules", "target", "dist", ".svelte-kit"))
    subprocess.run(["/bin/cp", "-cR", str(DEPENDENCIES), str(app / "node_modules")], check=True)
    evidence = ROOT / "docs/acceptance/evidence/e06"
    if evidence.exists():
        shutil.copytree(evidence, mirror / "docs/acceptance/evidence/e06", symlinks=True)
    for path in OWNED:
        assert sha(mirror / path) == expected[path], path
    (base / "home").mkdir()
    (base / "tmp").mkdir()
    env = dict(os.environ, HOME=str(base / "home"), TMPDIR=str(base / "tmp"),
               PATH="/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin",
               npm_config_offline="true", npm_config_update_notifier="false",
               npm_config_audit="false", npm_config_fund="false")
    node = "/opt/homebrew/bin/node"
    npm = "/opt/homebrew/bin/npm"
    commands = [
        ("repair", [node, "--test", "tests/repair-preview.test.mjs"], app, {}),
        ("queue", [node, "--test", "tests/repair-preview-queue.test.mjs"], app, {}),
        ("recovery", [node, "--test", "tests/recovery-copy.test.mjs"], app, {}),
        ("frontend-full", [npm, "test"], app, {}),
        ("svelte-check", [npm, "run", "check"], app, {}),
        ("build-personal", [npm, "run", "build"], app, {"VITE_EDITION": "personal"}),
        ("build-public", [npm, "run", "build"], app, {"VITE_EDITION": "public"}),
        ("diff-check", ["git", "diff", "--check"], ROOT, {}),
    ]
    receipts = []
    for label, argv, cwd, extra in commands:
        before = sources()
        assert before == expected, f"source drift before {label}"
        started = time.monotonic()
        result = subprocess.run(argv, cwd=cwd, env=dict(env, **extra), capture_output=True, text=True, timeout=120)
        output = result.stdout + "\nSTDERR\n" + result.stderr
        log = OUT / f"final-{label}.log"
        log.write_text(output)
        after = sources()
        row = {"label": label, "argv": argv, "command": shlex.join(argv), "cwd": str(cwd),
               "environment": extra, "exit": result.returncode, "elapsed_seconds": time.monotonic() - started,
               "output_file": str(log), "output_sha256": sha(log), "source_before": before, "source_after": after,
               "source_frozen": before == after == expected,
               "mirror_owned_sha256": {p: sha(mirror / p) for p in OWNED}}
        totals = re.search(r"(?:[ℹ#] )?tests (\d+)", output)
        if totals:
            row["test_methods"] = int(totals.group(1))
        if label.startswith("build-") and result.returncode == 0:
            row["artifacts"] = {str(p.relative_to(app / "dist")): sha(p)
                                for p in sorted((app / "dist").rglob("*")) if p.is_file()}
        receipts.append(row)
        (OUT / "final-receipts.json").write_text(json.dumps(receipts, ensure_ascii=False, indent=2) + "\n")
        print(json.dumps({k: row[k] for k in ["label", "exit", "elapsed_seconds", "source_frozen"]}), flush=True)
        assert row["source_frozen"], f"source drift during {label}"
        if result.returncode:
            print(output[-8000:], flush=True)
            raise SystemExit(result.returncode)
    (OUT / "final-runtime.json").write_text(json.dumps({"base": str(base), "mirror": str(mirror),
        "dependency_source": str(DEPENDENCIES), "dependency_source_read_only": True, "no_install": True,
        "HOME": env["HOME"], "TMPDIR": env["TMPDIR"], "accepted_sources": 13,
        "frozen_sha256": expected}, ensure_ascii=False, indent=2) + "\n")


if __name__ == "__main__":
    main()
