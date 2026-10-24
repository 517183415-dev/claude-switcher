"""P0 metadata only: app public metadata and top-level directory lstat entries."""
import datetime
import json
import mmap
import os
from pathlib import Path
import plistlib
import stat
import subprocess

BASE = Path(__file__).resolve().parent
EVIDENCE = BASE / "evidence"
EVIDENCE.mkdir(exist_ok=True)
results = {"recorded_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat()}

def command(args):
    result = subprocess.run(args, capture_output=True, text=True)
    return {"command": args, "exit_code": result.returncode,
            "stdout": result.stdout, "stderr": result.stderr}

app = Path("/Applications/Claude.app")
with (app / "Contents/Info.plist").open("rb") as stream:
    info = plistlib.load(stream)
results["claude_app"] = {
    key: info.get(key) for key in (
        "CFBundleIdentifier", "CFBundleName", "CFBundleShortVersionString",
        "CFBundleVersion", "LSMinimumSystemVersion", "CFBundleExecutable"
    )
}
results["commands"] = [
    command(["sw_vers"]), command(["uname", "-m"]),
    command(["xcrun", "--show-sdk-path"]), command(["xcrun", "--show-sdk-version"]),
    command(["codesign", "-d", "--verbose=4", str(app)]),
    command(["codesign", "--verify", "--strict", str(app)]),
    command(["codesign", "-d", "--entitlements", "-", str(app)]),
    command(["security", "find-identity", "-v", "-p", "codesigning"]),
]

known_names = {
    "A", "B", "Cache", "Code Cache", "GPUCache", "Local Storage", "Session Storage",
    "IndexedDB", "Network", "Cookies", "Cookies-journal", "Preferences", "Local State",
    "config.json", "claude_desktop_config.json", "logs", "Crashpad", "DawnCache",
    "DawnGraphiteCache", "DawnWebGPUCache", "blob_storage", "Service Worker",
    "Shared Dictionary", "SharedStorage", "SharedStorage-wal", "Session State",
    "TransportSecurity", "Trust Tokens", "Trust Tokens-journal", "WebStorage",
    "SingletonCookie", "SingletonLock", "SingletonSocket", "LOCK", "LOG",
    "WindowState", "WebGPUCache", "Network Persistent State", "component_crx_cache",
}

def metadata(path):
    entry = path.lstat()
    return {"kind": ("symlink" if stat.S_ISLNK(entry.st_mode) else
                     "directory" if stat.S_ISDIR(entry.st_mode) else "file"),
            "mode": oct(stat.S_IMODE(entry.st_mode)), "uid": entry.st_uid,
            "gid": entry.st_gid, "stat_size_bytes": entry.st_size}

directories = {}
for label, directory in {
    "active_claude_data": Path.home() / "Library/Application Support/Claude",
    "switcher_account_archives": Path.home() / ".claude-switcher/accounts",
}.items():
    result = {"path": str(directory).replace(str(Path.home()), "~")}
    try:
        result.update(metadata(directory))
        result["entries"] = []
        for item in sorted(directory.iterdir(), key=lambda p: p.name):
            entry = metadata(item)
            entry["name"] = item.name if item.name in known_names else "<redacted-unknown-name>"
            result["entries"].append(entry)
        result["top_level_entry_count"] = len(result["entries"])
    except OSError as error:
        result["error_type"] = type(error).__name__
        result["errno"] = error.errno
    directories[label] = result
results["directories"] = directories

# Search public application binaries for literal labels. No user database is opened.
results["public_binary_literal_counts"] = []
for file in [
    app / "Contents/Resources/app.asar",
    app / "Contents/Frameworks/Electron Framework.framework/Versions/A/Electron Framework",
]:
    entry = {"path": str(file)}
    if file.is_file():
        with file.open("rb") as stream, mmap.mmap(stream.fileno(), 0, access=mmap.ACCESS_READ) as data:
            entry["literal_presence"] = {
                text: data.find(text.encode()) >= 0
                for text in ("Claude Safe Storage", " Safe Storage", "safeStorage", "keytar")
            }
    else:
        entry["status"] = "absent"
    results["public_binary_literal_counts"].append(entry)
results["private_content_opened"] = False
results["keychain_secret_queries"] = 0
(EVIDENCE / "account-metadata.json").write_text(
    json.dumps(results, ensure_ascii=False, indent=2) + "\n"
)
print(json.dumps({"app": results["claude_app"],
                  "directory_counts": {k: v.get("top_level_entry_count") for k, v in directories.items()},
                  "metadata_evidence": "evidence/account-metadata.json"}, ensure_ascii=False))
