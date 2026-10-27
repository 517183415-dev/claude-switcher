#!/usr/bin/env python3
"""Run the cross-crate account/updater regression in a macOS sandbox."""
import json
import os
from pathlib import Path
import pwd
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
TEST = "migration::update_contention_tests::account_and_update_entrypoints_exclude_each_other_across_processes"


def profile(home: Path) -> str:
    private = [home / name for name in (
        "Library", ".claude", ".claude-switcher", ".ssh", ".aws", ".config", ".local/bin",
    )]
    private += [Path(p) for p in (
        "/Library/Keychains", "/Library/Application Support/claude-switcher",
        "/Library/PrivilegedHelperTools",
    )]
    protected = [ROOT / ".git", *(Path(p) for p in (
        "/Applications", "/Library/LaunchDaemons", "/Library/LaunchAgents",
        "/usr/local/bin", "/opt/homebrew/bin",
    ))]
    commands = (
        "/usr/bin/open", "/usr/bin/killall", "/usr/bin/pgrep", "/bin/ps",
        "/bin/launchctl", "/usr/bin/security", "/usr/bin/osascript", "/usr/bin/sudo",
        "/usr/bin/defaults", "/usr/bin/curl", "/usr/sbin/sysctl",
        "/usr/sbin/systemsetup", "/usr/sbin/networksetup", "/usr/sbin/scutil",
        "/sbin/ifconfig", "/sbin/route",
    )
    quote = lambda path: json.dumps(str(path), ensure_ascii=False)
    lines = ['(version 1)', '(allow default)', '(deny network*)']
    lines += [f'(deny file-read* file-write* (subpath {quote(p)}))' for p in private]
    lines += [f'(deny file-read* file-write* (literal {quote(p)}))' for p in (
        home / ".claude.json", home / ".cargo/credentials", home / ".cargo/credentials.toml",
    )]
    lines += [f'(deny file-write* (subpath {quote(p)}))' for p in protected]
    lines += [f'(deny process-exec (literal {quote(p)}))' for p in commands]
    lines += ['(deny mach-lookup (regex ".*securityd.*") (regex ".*[Cc]laude.*"))']
    return '\n'.join(lines) + '\n'


def build(sandbox: list[str], env: dict[str, str], cargo: str, args: list[str], target: str) -> str:
    command = [*sandbox, cargo, "test", "--offline", "--locked", *args,
               "--no-run", "--message-format=json"]
    result = subprocess.run(command, cwd=ROOT, env=env, capture_output=True,
                            text=True, timeout=300)
    executables = []
    errors = []
    for line in result.stdout.splitlines():
        try:
            item = json.loads(line)
        except json.JSONDecodeError:
            continue
        if (item.get("reason") == "compiler-artifact"
                and item.get("target", {}).get("name") == target
                and item.get("profile", {}).get("test") and item.get("executable")):
            executables.append(item["executable"])
        if item.get("reason") == "compiler-message" and item.get("message", {}).get("level") == "error":
            errors.append(item["message"].get("rendered", ""))
    if result.returncode or len(executables) != 1:
        raise RuntimeError(f"build failed for {target}: {result.stderr}\n{''.join(errors)}")
    return executables[0]


def main() -> None:
    if sys.platform != "darwin":
        raise RuntimeError("This regression requires macOS and sandbox-exec")
    cargo = shutil.which("cargo")
    if not cargo:
        raise RuntimeError("cargo was not found")
    real_home = Path(pwd.getpwuid(os.getuid()).pw_dir)
    with tempfile.TemporaryDirectory(prefix="switcher-contention-run-") as directory:
        scratch = Path(directory).resolve()
        home = scratch / "home"
        tmp = scratch / "tmp"
        home.mkdir(mode=0o700)
        tmp.mkdir(mode=0o700)
        policy = scratch / "account-free.sb"
        policy.write_text(profile(real_home))
        sandbox = ["/usr/bin/sandbox-exec", "-f", str(policy)]
        env = {
            "HOME": str(home), "TMPDIR": str(tmp) + "/", "LC_ALL": "C",
            "PATH": os.environ.get("PATH", "/usr/bin:/bin"), "CARGO_NET_OFFLINE": "true",
            "CARGO_HOME": os.environ.get("CARGO_HOME", str(real_home / ".cargo")),
            "RUSTUP_HOME": os.environ.get("RUSTUP_HOME", str(real_home / ".rustup")),
        }
        for name in ("RUSTUP_TOOLCHAIN", "RUSTC", "RUSTDOC", "CARGO_TARGET_DIR",
                     "MACOSX_DEPLOYMENT_TARGET"):
            if name in os.environ:
                env[name] = os.environ[name]
        desktop = build(sandbox, env, cargo, ["-p", "switcher-desktop", "--bin", "switcher-desktop"],
                        "switcher-desktop")
        core = build(sandbox, env, cargo, ["-p", "switcher-core", "--lib"], "switcher_core")
        env["SWITCHER_UPDATE_FIXTURE_BIN"] = desktop
        command = [*sandbox, core, "--exact", TEST, "--ignored", "--nocapture", "--test-threads=1"]
        result = subprocess.run(command, cwd=ROOT, env=env, capture_output=True, text=True, timeout=90)
        if (result.returncode or f"test {TEST}" not in result.stdout
                or "test result: ok. 1 passed; 0 failed" not in result.stdout):
            raise RuntimeError(f"contention regression failed:\n{result.stdout}\n{result.stderr}")
        print("Account/update contention: passed for Personal and Public.")


if __name__ == "__main__":
    main()
