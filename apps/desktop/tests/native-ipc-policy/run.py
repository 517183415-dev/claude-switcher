"""Exercise the ordinary Rust entry/ACL with an explicitly scripted test frontend.

Uses only the Python standard library. Run on macOS in a logged-in GUI session.
No fixture feature, production-source mutation, real account, or external service.
"""
import argparse
from contextlib import contextmanager, nullcontext
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import hashlib
import html
import json
import os
from pathlib import Path
import plistlib
import shutil
import signal
import subprocess
import threading
import time
from urllib.parse import urlsplit

ROOT = Path(__file__).resolve().parents[4]
HERE = Path(__file__).resolve().parent
SUCCESS = ["runtime_info", "cancel_check", "create_migration_backup", "save_preferences",
           "workspace_snapshot", "activity_log", "export_activity"]
INVALID = ["account_action", "preview_repair", "execute_repair", "save_preferences",
           "software_update", "open_ad_link", "ad_cached_image", "preview_region_change",
           "cancel_repair"]
PERSONAL_INVALID = ["ad_draft_save", "import_ad_image", "ad_image_preview"]
UNKNOWN = ["__unregistered__", "forbidden_probe", "finish_smoke", "plugin:shell|execute"]
PREFERENCES = {"ads": {"slots_enabled": [False, False, False],
                       "request_purpose_acknowledged": True}, "updates_paused_until": None}


def save(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n")


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def prepare_home(home, edition):
    # mkdir(parents=True, mode=...) only applies mode to the final component.
    directory = home
    for name in [".claude-switcher", "v2", edition]:
        directory /= name
        directory.mkdir(mode=0o700)
    preferences = directory / "preferences.json"
    save(preferences, PREFERENCES)
    preferences.chmod(0o600)
    return preferences


def validate(local, edition, denied):
    """Reject incomplete, duplicate, misclassified, or successful denial reports."""
    invalid = INVALID + (PERSONAL_INVALID if edition == "personal" else [])
    expected = SUCCESS + invalid + denied
    assert local["phase"] == "local" and local["edition"] == edition
    rows = local["rows"]
    assert [row["command"] for row in rows] == expected, "command sequence mismatch"
    for row in rows[:len(SUCCESS)]:
        assert row["ok"] is True, row
    for row in rows[len(SUCCESS):len(SUCCESS) + len(invalid)]:
        assert row["ok"] is False and "invalid args" in row["error"].lower(), row
    for row in rows[len(SUCCESS) + len(invalid):]:
        assert row["ok"] is False and "not allowed" in row["error"].lower(), row
    assert rows[0]["value"]["edition"] == edition
    assert rows[3]["value"] == PREFERENCES
    assert rows[4]["value"]["preferences"] == PREFERENCES
    assert rows[4]["value"]["runtime"]["edition"] == edition


def page(data):
    value = html.escape(json.dumps(data), quote=True)
    return (f'<!doctype html><html><meta charset="utf-8"><title>IPC isolation probe</title>'
            f'<body data-plan="{value}">IPC isolation probe'
            '<script defer src="probe.js"></script></body></html>').encode()


def boundary_commands(edition):
    capability = json.loads((ROOT / "apps/desktop/src-tauri/capabilities/main.json").read_text())
    permissions = capability["permissions"]
    if edition == "personal":
        config = json.loads((ROOT / "packaging/editions/personal.json").read_text())
        permissions += config["app"]["security"]["capabilities"][1]["permissions"]
    return [p.removeprefix("allow-").replace("-", "_") for p in permissions]


@contextmanager
def boundary_server(edition, port=0):
    """Serve only in-memory test assets on a random loopback port."""
    requests = []
    plan = {"edition": edition, "phase": "remote", "denied": boundary_commands(edition)}
    script = (HERE / "probe.js").read_bytes()

    class Handler(BaseHTTPRequestHandler):
        def do_GET(self):
            requests.append(self.path)
            assets = {"/": (page(plan), "text/html; charset=utf-8"),
                      "/probe.js": (script, "text/javascript; charset=utf-8")}
            if self.path not in assets:
                self.send_error(404)
                return
            body, content_type = assets[self.path]
            self.send_response(200)
            self.send_header("Content-Type", content_type)
            self.send_header("Content-Length", str(len(body)))
            self.send_header("Cache-Control", "no-store")
            self.end_headers()
            self.wfile.write(body)

        def log_message(self, *_):
            pass

    with ThreadingHTTPServer(("127.0.0.1", port), Handler) as server:
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            yield {"url": f"http://127.0.0.1:{server.server_port}/", "requests": requests}
        finally:
            server.shutdown()
            thread.join(timeout=5)
            assert not thread.is_alive(), "loopback server did not stop"


def validate_boundary(report, edition, phase, expected_url, commands):
    assert report["phase"] == phase and report["edition"] == edition
    assert report["url"] == expected_url, "unexpected page origin"
    assert report["bridge"] is True, "no IPC bridge; cannot claim ACL denial"
    assert [r["command"] for r in report["rows"]] == commands
    for row in report["rows"]:
        assert row["ok"] is False and "not allowed" in row["error"].lower(), row


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("edition", choices=["personal", "public"])
    parser.add_argument("output", type=Path, help="new directory; never overwrite an attempt")
    parser.add_argument("--toolchain-bin", type=Path, required=True)
    parser.add_argument("--reuse-build", type=Path, help="reuse an unchanged earlier test binary and frontend")
    parser.add_argument("--boundary-matrix", action="store_true",
                        help="test remote main and local other window, without changing ACL")
    args = parser.parse_args()
    port = 0
    if args.boundary_matrix and args.reuse_build:
        previous_config = json.loads((args.reuse_build / "config-override.json").read_text())
        port = urlsplit(previous_config["app"]["windows"][0]["url"]).port
        assert port, "reuse requires an earlier boundary build"
    with boundary_server(args.edition, port) if args.boundary_matrix else nullcontext(None) as server:
        try:
            execute(args, server)
        finally:
            if server and args.output.is_dir():
                save(args.output / "server.json", server)


def execute(args, server):
    out = args.output.resolve()
    out.relative_to(ROOT / "output")
    out.mkdir(parents=True, exist_ok=False)
    home, dist = out / "home", out / "dist"
    for path in [home, dist, out / "tmp"]:
        path.mkdir(mode=0o700)
    preferences = prepare_home(home, args.edition)
    config = json.loads((ROOT / "packaging/editions" / (args.edition + ".json")).read_text())
    personal = json.loads((ROOT / "packaging/editions/personal.json").read_text())
    operations = personal["app"]["security"]["capabilities"][1]["permissions"]
    denied = UNKNOWN + (["_".join(p.removeprefix("allow-").split("-")) for p in operations]
                        if args.edition == "public" else [])
    previous = args.reuse_build.resolve() if args.reuse_build else None
    script = (HERE / "probe.js").read_bytes()
    plan = {"edition": args.edition, "phase": "local", "preferences": PREFERENCES,
            "invalidArguments": INVALID + (PERSONAL_INVALID if args.edition == "personal" else []),
            "denied": denied}
    if server:
        plan = {"edition": args.edition, "phase": "other", "denied": boundary_commands(args.edition)}
        config.setdefault("app", {})["windows"] = [
            {"label": "main", "title": "IPC Remote", "url": server["url"], "width": 900, "height": 600},
            {"label": "other", "title": "IPC Other", "url": "index.html", "width": 900, "height": 600}]
    if previous:
        assert plan == json.loads((previous / "plan.json").read_text()), "test plan changed"
        assert digest(previous / "dist/probe.js") == digest(HERE / "probe.js")
        prior_inputs = json.loads((previous / "inputs.json").read_text())
        for name, expected in prior_inputs.items():
            if not name.startswith(str(HERE.relative_to(ROOT)) + "/"):
                assert digest(ROOT / name) == expected, "product source changed: " + name

    (dist / "index.html").write_bytes(page(plan))
    (dist / "probe.js").write_bytes(script)
    config["build"] = {"frontendDist": str(dist)}
    policy = (ROOT / "output/no-account-20261027/storage-directory-race/account-free.sb").read_text()
    profile = out / "isolation.sb"
    profile.write_text(policy)
    env = dict(os.environ, HOME=str(home), TMPDIR=str(out / "tmp") + "/",
               CARGO_HOME=str(Path.home() / ".cargo"), RUSTUP_HOME=str(Path.home() / ".rustup"),
               XDG_CONFIG_HOME=str(home / ".config"), GIT_CONFIG_GLOBAL="/dev/null",
               GIT_CONFIG_NOSYSTEM="1", CARGO_NET_OFFLINE="true",
               PATH=str(args.toolchain_bin) + ":/usr/bin:/bin:/usr/sbin:/sbin",
               RUSTC=str(args.toolchain_bin / "rustc"), RUSTDOC=str(args.toolchain_bin / "rustdoc"),
               TAURI_CONFIG=json.dumps(config), MACOSX_DEPLOYMENT_TARGET="13.0")
    command = [str(args.toolchain_bin / "cargo"), "build", "--release", "--offline", "--locked",
               "--target", "aarch64-apple-darwin", "-p", "switcher-desktop", "--bin",
               "switcher-desktop", "--no-default-features", "--features", args.edition + ",custom-protocol"]
    files = subprocess.check_output(["git", "ls-files", "-z", "--", "Cargo.toml", "Cargo.lock",
                                    ".cargo", "crates", "apps/desktop", "packaging/editions"], cwd=ROOT)
    paths = {p.decode() for p in files.split(b"\0") if p}
    paths.update(str(p.relative_to(ROOT)) for p in HERE.glob("*") if p.is_file())
    inputs = {p: digest(ROOT / p) for p in sorted(paths) if (ROOT / p).is_file()}
    save(out / "inputs.json", inputs)
    save(out / "plan.json", plan)
    save(out / "config-override.json", config)
    if previous:
        build = json.loads((previous / "build.json").read_text())
        assert build["exit_code"] == 0
        save(out / "build.json", dict(build, reused_from=str(previous),
                                      reused_binary_sha256=digest(previous / "switcher-desktop")))
        source_binary = previous / "switcher-desktop"
    else:
        with (out / "build.log").open("wb") as log:
            result = subprocess.run(["/usr/bin/sandbox-exec", "-f", str(profile)] + command,
                                    cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT)
        save(out / "build.json", {"exit_code": result.returncode, "command": command,
                                 "commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
                                 "log_sha256": digest(out / "build.log"), "policy_sha256": digest(profile)})
        assert result.returncode == 0, "build failed; see build.log"
        source_binary = ROOT / "target/aarch64-apple-darwin/release/switcher-desktop"
    binary = out / "switcher-desktop"
    shutil.copy2(source_binary, binary)
    app = out / "IPC Policy.app/Contents"
    (app / "MacOS").mkdir(parents=True)
    executable = app / "MacOS/switcher-desktop"
    shutil.copy2(binary, executable)
    (app / "Info.plist").write_bytes(plistlib.dumps({
        "CFBundleIdentifier": "local.claudeswitcher." + args.edition,
        "CFBundleExecutable": "switcher-desktop", "CFBundleName": "IPC Policy",
        "CFBundlePackageType": "APPL", "NSHighResolutionCapable": True,
    }))
    subprocess.run(["/usr/bin/codesign", "--force", "--sign", "-", "--timestamp=none",
                    str(app.parent)], check=True, stdout=subprocess.DEVNULL)
    reports = {}
    last_reads = {}
    titles = {"remote": "IPC Remote", "other": "IPC Other"} if server else {"local": None}
    with (out / "runtime.log").open("wb") as log:
        process = subprocess.Popen(["/usr/bin/sandbox-exec", "-f", str(profile), str(executable)],
                                   cwd=out, env=env, stdout=log, stderr=subprocess.STDOUT,
                                   start_new_session=True)
        save(out / "process.json", {"pid": process.pid, "executable": str(executable)})
        try:
            deadline = time.monotonic() + 60
            while len(reports) < len(titles) and time.monotonic() < deadline and process.poll() is None:
                for phase, title in titles.items():
                    if phase in reports:
                        continue
                    window = f'window "{title}"' if title else 'first window'
                    query = ('tell application "System Events" to get value of every static text of '
                             'UI element 1 of scroll area 1 of group 1 of group 1 of ' + window + ' of '
                             f'(first application process whose unix id is {process.pid})')
                    read = subprocess.run(["/usr/bin/osascript", "-e", query], capture_output=True,
                                          text=True, timeout=5)
                    last_reads[phase] = {"exit_code": read.returncode, "stdout": read.stdout,
                                         "stderr": read.stderr}
                    text = read.stdout.strip()
                    if read.returncode == 0 and text.startswith("IPC_REPORT:"):
                        reports[phase] = json.loads(text.removeprefix("IPC_REPORT:"))
                        save(out / (phase + "-result.json"), reports[phase])
                time.sleep(0.4)
            assert len(reports) == len(titles), "page receipt timed out; inspect runtime.log"
        finally:
            save(out / "window-read.json", last_reads)
            if process.poll() is None:
                os.killpg(process.pid, signal.SIGTERM)
            process.wait(timeout=10)
            save(out / "process-result.json", {"pid": process.pid, "exit_code": process.returncode})
    if server:
        commands = boundary_commands(args.edition)
        for phase in titles:
            url = server["url"] if phase == "remote" else "tauri://localhost/"
            validate_boundary(reports[phase], args.edition, phase, url, commands)
        assert "/" in server["requests"] and "/probe.js" in server["requests"]
        assert json.loads(preferences.read_text()) == PREFERENCES
        assert inputs == {p: digest(ROOT / p) for p in inputs}, "source changed during execution"
        save(out / "receipt.json", {"scope": "ordinary ACL with test windows and frontend",
             "edition": args.edition, "status": "PASS_SCOPED", "server": server,
             "denials_per_context": len(commands), "contexts": list(titles),
             "inputs_sha256": digest(out / "inputs.json"),
             "binary_sha256": digest(binary), "executed_binary_sha256": digest(executable),
             "runtime_log_sha256": digest(out / "runtime.log"),
             "direct_process_exit": process.returncode, "preferences_unchanged": True,
             "not_verified": ["unmodified product frontend and windows", "all successful business paths",
                              "native Keychain", "system-managed WebKit helper termination", "remote services"]})
        print(json.dumps({"edition": args.edition, "status": "PASS_SCOPED", "output": str(out)}))
        return
    local = reports["local"]
    validate(local, args.edition, denied)
    on_disk = json.loads(preferences.read_text())
    assert on_disk == PREFERENCES
    export_path, exported = local["rows"][6]["value"]
    export = Path(export_path)
    export.resolve().relative_to(home)
    assert json.loads(export.read_text()) == exported
    assert inputs == {p: digest(ROOT / p) for p in inputs}, "source changed during execution"
    save(out / "receipt.json", {"scope": "ordinary backend and ACL with scripted test frontend",
         "edition": args.edition, "local_rows": len(local["rows"]),
         "denied_commands": len(denied), "remote_origin_runtime": "NOT_RUN",
         "binary_sha256": digest(binary), "executed_binary_sha256": digest(executable),
         "inputs_sha256": digest(out / "inputs.json"),
         "runtime_log_sha256": digest(out / "runtime.log"), "direct_process_exit": process.returncode,
         "preferences_match_disk": True, "export_matches_disk": True,
         "not_verified": ["unmodified product frontend", "remote-origin and other-window runtime matrix",
                          "native Keychain", "system-managed WebKit helper termination",
                          "real accounts or remote services"]})
    print(json.dumps({"edition": args.edition, "status": "PASS_SCOPED", "output": str(out)}))


if __name__ == "__main__":
    main()
