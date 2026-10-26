from pathlib import Path
import argparse
import json
import subprocess
import time
import sys
import errno

from support import ROOT, GENERATED, digest, inputs, output_root, make_profile, validate_build_paths, private_directory, no_links
from runner import attempt_value, prepare
import runner

def write(path, record):
    path.write_text(json.dumps(record, ensure_ascii=False, indent=2) + "\n")

def build_controls(destination, output, tools):
    """Probe only synthetic credentials/config files under an independent cache."""
    folder = private_directory(destination / 'policy-controls', create=True)
    cache = private_directory(folder / 'cache', create=True)
    for name in ('credentials', 'credentials.toml', 'config', 'config.toml'):
        with (cache / name).open('x') as handle:
            handle.write('SYNTHETIC_POLICY_CONTROL_ONLY')
    profile = make_profile(folder, output, build=True, tools=tools, cache=cache, target=output / 'target')
    code = "import os,json; results={};\n"
    for name in ('credentials', 'credentials.toml', 'config', 'config.toml'):
        code += 'try:\n fd=os.open(' + repr(str(cache / name)) + ",os.O_RDONLY);os.close(fd);results[" + repr(name) + "]='UNEXPECTED'\nexcept OSError as e:results[" + repr(name) + ']=e.errno\n'
    code += 'print(json.dumps(results))'
    env = runner.environment(destination)
    with (folder / 'stdout.log').open('xb') as out, (folder / 'stderr.log').open('xb') as err:
        result = subprocess.run(['/usr/bin/sandbox-exec', '-f', str(profile), sys.executable, '-I', '-c', code],
                                cwd=folder, env=env, stdin=subprocess.DEVNULL, stdout=out, stderr=err, timeout=20)
    controls = json.loads(runner.bounded_text(folder / 'stdout.log'))
    valid = result.returncode == 0 and all(value in (errno.EPERM, errno.EACCES) for value in controls.values()) and len(controls) == 4
    receipt = {'status': 'PASS' if valid else 'FAIL', 'controls': controls, 'profile_sha256': digest(profile),
               'stdout_sha256': digest(folder / 'stdout.log'), 'stderr_sha256': digest(folder / 'stderr.log')}
    runner.write(folder / 'result.json', receipt)
    assert valid, 'build cache credentials/config denial controls failed'
    return receipt


def configuration(edition):
    path = no_links(ROOT / f"packaging/editions/ipc-business-{edition}.json")
    config = json.loads(path.read_text())
    assert config["identifier"] == f"local.switcher.ipcbusiness.{edition}"
    assert config["bundle"]["active"] is False
    assert config["build"]["frontendDist"] == "../tests/native-ipc-business"
    return config

def main():
    parser = argparse.ArgumentParser(description="Build and ad-hoc sign isolated native IPC test binaries; never launch.")
    parser.add_argument("edition", choices=["personal", "public"])
    parser.add_argument("--attempt", required=True, type=attempt_value, help="Explicit fresh build attempt; never replace historical evidence")
    parser.add_argument("--check", action="store_true", help="Read-only configuration/syntax preparation only")
    parser.add_argument('--output', required=True, type=output_root)
    parser.add_argument('--toolchain-bin', required=True, type=Path, help='Existing direct Rust toolchain bin directory; no rustup/network')
    parser.add_argument('--cargo-home', required=True, type=Path, help='Existing offline cache; never populated by this harness')
    parser.add_argument('--target-dir', required=True, type=Path, help='Dedicated or explicitly shared build target; builds must be sequential')
    args = parser.parse_args()
    tools, cargo_home, target = validate_build_paths(args.output, args.toolchain_bin, args.cargo_home, args.target_dir)
    assert all((tools / name).is_file() for name in ['cargo', 'rustc', 'rustdoc'])
    assert cargo_home.is_dir()
    runner.OUT = args.output
    config = configuration(args.edition)
    if args.check:
        print(json.dumps({"status": "PREPARED_NOT_BUILT", "edition": args.edition, "config_sha256": digest(ROOT / f"packaging/editions/ipc-business-{args.edition}.json"), "config": config}, ensure_ascii=False))
        return
    edition = args.edition
    build_root = args.output / 'build'
    private_directory(build_root, create=True)
    destination = build_root / (edition + "-" + args.attempt)
    assert not destination.exists(), "Preserve prior run; use a fresh authorized output directory"
    private_directory(destination, create=True)
    for name in ["home", "tmp", "module-cache"]:
        (destination / name).mkdir(mode=0o700)
    assert prepare(), 'runtime sandbox negative controls failed; build refused'
    policy_controls = build_controls(destination, args.output, tools)
    profile = make_profile(destination, args.output, build=True, tools=tools, cache=cargo_home, target=target)
    env = {'PATH': str(tools) + ':/usr/bin:/bin:/usr/sbin:/sbin',
           'LANG': 'en_US.UTF-8', 'LC_ALL': 'en_US.UTF-8', 'CI': 'true',
           'USER': 'build-fixture', 'LOGNAME': 'build-fixture', 'SHELL': '/bin/bash',
           'CARGO_HOME': str(cargo_home), 'CARGO_NET_OFFLINE': 'true', 'CARGO_CACHE_RUSTC_INFO': '0',
           'RUSTC': str(tools / 'rustc'), 'RUSTDOC': str(tools / 'rustdoc'),
           'CARGO_TARGET_DIR': str(target), 'MACOSX_DEPLOYMENT_TARGET': '13.0',
           'HOME': str(destination / 'home'), 'TMPDIR': str(destination / 'tmp'),
           'CLANG_MODULE_CACHE_PATH': str(destination / 'module-cache'),
           'TAURI_CONFIG': json.dumps(config, ensure_ascii=False), 'APPLE_SIGNING_IDENTITY': '-'}
    source_before = inputs()
    record = {"edition": edition, "head_at_start": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(), "environment": env, "environment_policy": "whitelist only; dedicated HOME/TMPDIR", "sandbox": str(profile), "profile_sha256": digest(profile), "harness_hashes": {k: v for k, v in source_before.items() if k.startswith("apps/desktop/tests/native-ipc-business/")}, "source_before": source_before, "generated_before": inputs(generated=True), "generated_output_policy": list(GENERATED), "commands": [], "policy_controls": policy_controls, "status": "BUILDING", "product_launch": "NOT_RUN", "installation": "NOT_RUN", "real_accounts": "NOT_ACCESSED"}
    receipt = destination / "result.json"
    write(receipt, record)
    def run(label, argv):
        started = time.monotonic()
        wrapped = ["/usr/bin/sandbox-exec", "-f", str(profile)] + argv
        stdout = destination / (label + ".stdout")
        stderr = destination / (label + ".stderr")
        with stdout.open("xb") as out, stderr.open("xb") as err:
            result = subprocess.run(wrapped, cwd=ROOT, env=env, stdout=out, stderr=err)
        item = {"label": label, "argv": argv, "wrapped_argv": wrapped, "exit": result.returncode, "seconds": time.monotonic() - started, "stdout_sha256": digest(stdout), "stderr_sha256": digest(stderr)}
        record["commands"].append(item)
        write(receipt, record)
        print(json.dumps(item), flush=True)
        if result.returncode:
            raise RuntimeError(label + " failed: " + stderr.read_text(errors="replace")[-3000:])
    try:
        run("rustc-version", [str(tools / "rustc"), "-vV"])
        run("cargo-build", [str(tools / "cargo"), "build", "--offline", "--locked", "-p", "switcher-desktop", "--bin", "switcher-desktop", "--no-default-features", "--features", edition + ",ipc-business,custom-protocol"])
        record["source_after"] = inputs()
        record["generated_after"] = inputs(generated=True)
        assert record["source_after"] == source_before, "Build inputs changed; reject binary attribution"
        source = target / "debug/switcher-desktop"
        binary = destination / "switcher-desktop"
        run("copy", ["/usr/bin/ditto", str(source), str(binary)])
        record["unsigned_sha256"] = digest(binary)
        assert record["unsigned_sha256"] == digest(source)
        run("sign", ["/usr/bin/codesign", "--force", "--options", "runtime", "--sign", "-", "--timestamp=none", "--identifier", config["identifier"], str(binary)])
        run("verify", ["/usr/bin/codesign", "--verify", "--strict", str(binary)])
        run("signature", ["/usr/bin/codesign", "--display", "--verbose=4", str(binary)])
        signature = (destination / "signature.stderr").read_text()
        assert "Signature=adhoc" in signature and "TeamIdentifier=not set" in signature and "Identifier=" + config["identifier"] in signature
        record["signed_sha256"] = digest(binary)
        record["binary"] = str(binary)
        assert inputs() == source_before, 'Inputs changed during copy/sign verification'
        assert digest(profile) == record['profile_sha256'], 'Build sandbox changed'
        record["status"] = "PASS_BUILD_AND_ADHOC_STATIC_VERIFY"
    except BaseException as error:
        record["status"] = "FAILED"
        record["error"] = str(error)
        record["source_after"] = inputs()
        record["generated_after"] = inputs(generated=True)
        raise
    finally:
        write(receipt, record)
    print(json.dumps({key: record[key] for key in ["edition", "status", "binary", "unsigned_sha256", "signed_sha256"]}), flush=True)

if __name__ == "__main__":
    main()
