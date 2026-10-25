from pathlib import Path
import ast, hashlib, json, os, subprocess, tempfile, time

root = Path(__file__).resolve().parents[3]
base = root / "output/review/R08-F07-整改-20261025"
owned = ["packaging/macos/local_install.py", "packaging/macos/test_local_install.py"]
baseline = json.loads((base / "baseline-sha256.json").read_text())
protected = json.loads((base / "protected-functions-baseline.json").read_text())
mandatory = set(protected) - {"finish", "resume_terminal", "transact", "recover", "finalize", "launch_health"}

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def source_hashes():
    return {p: sha(root / p) for p in baseline}

toolchain = Path("/Users/gua/.rustup/toolchains/1.96.0-aarch64-apple-darwin")
for binary in ("cargo", "rustc", "rustdoc"):
    assert (toolchain / "bin" / binary).is_file(), binary
expected = source_hashes()
for p, old in baseline.items():
    if p not in owned:
        assert expected[p] == old, ("accepted source changed", p)
source = (root / owned[0]).read_text()
lines = source.splitlines(keepends=True)
functions = {n.name: n for n in ast.parse(source).body if isinstance(n, (ast.FunctionDef, ast.AsyncFunctionDef))}
protected_now = {}
for name, old in protected.items():
    n = functions[name]
    current = {"ast_sha256": hashlib.sha256(ast.dump(n, include_attributes=False).encode()).hexdigest(),
               "source_sha256": hashlib.sha256("".join(lines[n.lineno-1:n.end_lineno]).encode()).hexdigest()}
    protected_now[name] = dict(current, unchanged=current == old)
    if name in mandatory:
        assert current == old, ("protected function changed", name)
(base / "final-protected-functions.json").write_text(json.dumps(protected_now, indent=2) + "\n")

temporary = Path(tempfile.mkdtemp(prefix="claude-r08-final-"))
for name in ["home", "tmp", "guard"]:
    (temporary / name).mkdir()
guard = temporary / "guard/sitecustomize.py"
guard.write_text("""import os, sys
blocked = {'launchctl','sudo','open','curl','scutil','systemsetup','networksetup','codesign','lipo','dig','nslookup','ps','kill','pkill','systemctl','rustup'}
def audit(event, args):
    if event == 'subprocess.Popen':
        executable = os.fsdecode(args[0])
        if os.path.basename(executable) in blocked or os.path.basename(os.path.realpath(executable)) == 'rustup':
            raise RuntimeError('R08 isolated verification blocked native command: ' + os.path.basename(executable))
    if event in ('socket.connect', 'socket.bind', 'socket.getaddrinfo'):
        raise RuntimeError('R08 isolated verification blocked network access')
sys.addaudithook(audit)
""")
env = dict(os.environ, HOME=str(temporary / "home"), TMPDIR=str(temporary / "tmp"),
           PYTHONPATH=str(temporary / "guard"), PYTHONDONTWRITEBYTECODE="1",
           PATH=str(toolchain / "bin")+":/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin",
           CARGO_HOME="/Users/gua/.cargo", RUSTUP_HOME="/Users/gua/.rustup",
           RUSTUP_TOOLCHAIN="1.96.0-aarch64-apple-darwin", CARGO_NET_OFFLINE="true",
           RUSTC=str(toolchain / "bin/rustc"), RUSTDOC=str(toolchain / "bin/rustdoc"))
commands = [
    ("f07", ["/opt/homebrew/bin/python3", "-B", "-m", "unittest", "discover", "-s", "packaging/macos", "-p", "test_local_install.py", "-k", "f07", "-v"]),
    ("installer-full", ["/opt/homebrew/bin/python3", "-B", "-m", "unittest", "discover", "-s", "packaging/macos", "-p", "test_local_install.py", "-v"]),
    ("diff-check", ["git", "diff", "--check"]),
]
receipts = []
for label, argv in commands:
    before = source_hashes()
    assert before == expected, "source changed before final command"
    started = time.monotonic()
    result = subprocess.run(argv, cwd=root, env=env, capture_output=True, text=True)
    after = source_hashes()
    log = base / ("final-" + label + ".log")
    log.write_text("STDOUT\n" + result.stdout + "\nSTDERR\n" + result.stderr)
    receipt = dict(label=label, argv=argv, cwd=str(root), command=" ".join(argv), exit=result.returncode,
                   seconds=time.monotonic() - started, source_before=before, source_after=after,
                   source_frozen=before == after, output_file=str(log), output_sha256=sha(log))
    receipts.append(receipt)
    (base / "final-receipts.json").write_text(json.dumps(receipts, indent=2) + "\n")
    print(json.dumps({"label":label,"exit":result.returncode,"seconds":receipt["seconds"],"tail":result.stderr[-400:]}), flush=True)
    assert before == after, "source changed during final command"
    assert result.returncode == 0, label
runtime = dict(python="/opt/homebrew/bin/python3", temporary=str(temporary),
               environment={k:env[k] for k in ["HOME","TMPDIR","PYTHONPATH","PYTHONDONTWRITEBYTECODE","PATH","CARGO_HOME","RUSTUP_HOME","RUSTUP_TOOLCHAIN","CARGO_NET_OFFLINE","RUSTC","RUSTDOC"]},
               python_version=subprocess.check_output(["/opt/homebrew/bin/python3","--version"], text=True).strip(),
               direct_native_and_network_audit_guard=str(guard), source_hashes=expected,
               protected_mandatory=len(mandatory), owned=owned)
(base / "final-runtime.json").write_text(json.dumps(runtime, indent=2) + "\n")
