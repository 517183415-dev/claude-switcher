import ast
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
ACCEPTED = BASELINE["accepted_R01_to_R05_source_sha256"]
SCOPE = BASELINE["R06_allowed_sources"]
FILES = sorted(set(ACCEPTED) | set(SCOPE))
EXCEPTIONS = BASELINE["frozen_accepted_exceptions"]
ACCOUNT = "crates/switcher-core/src/accounts.rs"
ACCOUNT_HASH = json.loads((BASE / "accounts-visibility-receipt.json").read_text())["after_sha256"]


def hashes():
    return {p: hashlib.sha256((ROOT / p).read_bytes()).hexdigest() for p in FILES}


def boundaries(current):
    assert all(current[p] == digest for p, digest in ACCEPTED.items() if p not in EXCEPTIONS)
    assert current[ACCOUNT] == ACCOUNT_HASH
    tree = ast.parse((ROOT / "packaging/macos/local_install.py").read_text())
    protected = BASELINE["R02_protected_ast_sha256"]
    observed = {
        node.name: hashlib.sha256(ast.dump(node, include_attributes=False).encode()).hexdigest()
        for node in ast.walk(tree)
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)) and node.name in protected
    }
    assert observed == protected
    modern_regions = ROOT / "crates/switcher-core/src/regions.rs"
    assert hashlib.sha256(modern_regions.read_bytes()).hexdigest() == "c07c470b805a7435171f7f06bc03745ec2349d1a7e1c34dbd72a0ed65162173f"
    changed = subprocess.check_output(["git", "diff", "--name-only"], cwd=ROOT, text=True).splitlines()
    assert set(changed) <= set(FILES), changed


TEST = ["cargo", "test", "--offline", "--locked", "-p", "switcher-core"]
PYTHON = ["/opt/homebrew/bin/python3", "-B", "-m", "unittest", "discover", "-s", "packaging/macos", "-p", "test_local_install.py"]
COMMANDS = [
    ("cli-f06", TEST + ["--bin", "claude-switcher", "f06_", "--", "--nocapture"]),
    ("repair-f06", TEST + ["--lib", "repair_cli::tests::f06_", "--", "--nocapture"]),
    ("repair-cli", TEST + ["--lib", "repair_cli::"]),
    ("installer-f06", PYTHON + ["-k", "f06", "-v"]),
    ("installer-full", PYTHON + ["-v"]),
    ("accounts", TEST + ["--lib", "accounts::"]),
    ("migration", TEST + ["--lib", "migration::"]),
    ("lifecycle", TEST + ["--lib", "lifecycle::"]),
    ("status-readonly", TEST + ["--test", "checks_cli", "status_is_readonly_reports_login_unknown_and_rejects_arguments", "--", "--exact"]),
    ("invalid-arguments", TEST + ["--test", "checks_cli", "invalid_arguments_and_config_do_not_echo_paths_or_secrets", "--", "--exact"]),
    ("clippy", ["cargo", "clippy", "--offline", "--locked", "-p", "switcher-core", "--all-targets", "--", "-D", "warnings"]),
    ("rustfmt", ["rustfmt", "--check", "--edition", "2024"] + [p for p in SCOPE if p.endswith(".rs")]),
    ("diff-check", ["git", "diff", "--check"]),
    ("cli-nonutf8", TEST + ["--bin", "claude-switcher", "f06_non_utf8_", "--", "--nocapture", "--test-threads=1"]),
]
receipts = []
frozen = hashes()
boundaries(frozen)
env = os.environ.copy()
env.pop("CS_REPAIR_READONLY_COPY_SOURCE", None)
for name, command in COMMANDS:
    before = hashes()
    assert before == frozen
    boundaries(before)
    start = time.monotonic()
    log = BASE / ("final-" + name + ".log")
    with log.open("wb") as output:
        proc = subprocess.run(command, cwd=ROOT, env=env, stdout=output, stderr=subprocess.STDOUT)
    body = log.read_text(errors="replace")
    rust_count = re.search(r"test result: ok\. (\d+) passed;", body)
    python_count = re.search(r"Ran (\d+) tests? in", body)
    count = int((rust_count or python_count).group(1)) if rust_count or python_count else None
    after = hashes()
    boundaries(after)
    entry = dict(
        name=name, command=command, command_display=shlex.join(command),
        exit_code=proc.returncode, seconds=round(time.monotonic() - start, 3),
        log=str(log.relative_to(ROOT)), matched_tests=count,
        source_hashes_before=before, source_hashes_after=after,
        unchanged=before == after, frozen_accepted_sources_unchanged=True,
        account_visibility_only=True, R02_protected_functions_unchanged=True,
    )
    receipts.append(entry)
    (BASE / "final-receipts.json").write_text(json.dumps(receipts, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps({k: entry[k] for k in ["name", "exit_code", "matched_tests", "seconds", "unchanged"]}), flush=True)
    assert proc.returncode == 0 and before == after
    if name not in ["clippy", "rustfmt", "diff-check"]:
        assert count is not None and count > 0
