#!/usr/bin/env python3
"""Run synthetic account tests on a private, disposable 128 MiB APFS image."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import plistlib
import subprocess

ROOT = Path(__file__).resolve().parents[2]
POLICY = ROOT / 'output/no-account-20261027/storage-directory-race/account-free.sb'


def owned_devices(image, info):
    matches = [entry for entry in info.get('images', [])
               if entry.get('image-path') == str(image)]
    if len(matches) > 1:
        raise RuntimeError('ambiguous disposable image identity')
    if not matches:
        return []
    return [entry['dev-entry'] for entry in matches[0].get('system-entities', [])
            if 'dev-entry' in entry]


def hashes(paths):
    return {str(path.relative_to(ROOT)): hashlib.sha256(path.read_bytes()).hexdigest()
            for path in paths}


def run(output, test_filter='accounts::tests::native_metadata'):
    output.mkdir(parents=True, exist_ok=False)
    for name in ('home', 'tmp', 'mount'):
        (output / name).mkdir(mode=0o700)
    image = output / 'volume.sparseimage'
    mount = output / 'mount'
    toolchain = Path(subprocess.check_output(
        ['rustup', 'which', 'cargo'], cwd=ROOT, text=True).strip()).parent
    original_home = Path.home()
    env = os.environ.copy()
    env.update(HOME=str(output / 'home'), TMPDIR=str(output / 'tmp') + '/',
               CARGO_HOME=os.environ.get('CARGO_HOME', str(original_home / '.cargo')),
               RUSTUP_HOME=os.environ.get('RUSTUP_HOME', str(original_home / '.rustup')),
               CARGO_NET_OFFLINE='true', RUSTC=str(toolchain / 'rustc'),
               RUSTDOC=str(toolchain / 'rustdoc'),
               PATH=f'{toolchain}:/Applications/Xcode.app/Contents/Developer/usr/bin:/usr/bin:/bin:/usr/sbin:/sbin',
               SWITCHER_ACCOUNT_TEST_VOLUME=str(mount))
    paths = {ROOT / os.fsdecode(path) for path in subprocess.check_output(
        ['git', 'ls-files', '-z', 'Cargo.toml', 'Cargo.lock', '.cargo', 'crates'], cwd=ROOT
    ).split(b'\0') if path}
    paths.update((ROOT / 'crates/switcher-core/src/accounts/tests').rglob('*.rs'))
    paths.add(Path(__file__).resolve())
    inputs = hashes(sorted(paths))
    (output / 'inputs.json').write_text(json.dumps(inputs, indent=2) + '\n')
    results = []
    receipt = {'base_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
               'policy_sha256': hashlib.sha256(POLICY.read_bytes()).hexdigest(), 'results': results}

    def command(name, args, *, binary=False):
        result = subprocess.run(args, cwd=ROOT, env=env, capture_output=True, timeout=180)
        log = output / (name + '.log')
        log.write_bytes(result.stdout + result.stderr)
        results.append({'name': name, 'command': list(map(str, args)), 'exit_code': result.returncode,
                        'log_sha256': hashlib.sha256(log.read_bytes()).hexdigest()})
        print(name, 'exit', result.returncode, flush=True)
        result.check_returncode()
        return result.stdout if binary else result.stdout.decode()

    def info():
        # Do not save other mounted images or their paths in the test evidence.
        result = subprocess.run(['/usr/bin/hdiutil', 'info', '-plist'], env=env,
                                capture_output=True, check=True, timeout=30)
        return plistlib.loads(result.stdout)

    try:
        command('create', ['/usr/bin/hdiutil', 'create', '-size', '128m', '-fs', 'APFS',
                           '-type', 'SPARSE', '-volname', 'SwitcherF05Synthetic', str(image)])
        command('attach', ['/usr/bin/hdiutil', 'attach', str(image), '-nobrowse', '-noautoopen',
                           '-owners', 'on', '-mountpoint', str(mount), '-plist'], binary=True)
        assert mount.is_mount() and mount.stat().st_dev != output.stat().st_dev
        receipt['devices'] = {'host': output.stat().st_dev, 'disposable_volume': mount.stat().st_dev}
        cargo = str(toolchain / 'cargo')
        sandbox = ['/usr/bin/sandbox-exec', '-f', str(POLICY)]
        command('tests', [*sandbox, cargo, 'test', '--offline', '--locked', '-p', 'switcher-core',
                          '--lib', test_filter, '--', '--include-ignored',
                          '--test-threads=1', '--nocapture'])
        command('clippy', [*sandbox, cargo, 'clippy', '--offline', '--locked', '-p', 'switcher-core',
                           '--all-targets', '--', '-D', 'warnings'])
        command('format', [str(toolchain / 'rustfmt'), '--edition', '2024', '--check',
                           str(ROOT / 'crates/switcher-core/src/accounts/tests/native_metadata.rs'),
                           str(ROOT / 'crates/switcher-core/src/accounts/tests/native_metadata/volume.rs')])
    finally:
        try:
            devices = owned_devices(image, info())
            if devices:
                # This device comes only from an exact match of our newly created image path.
                command('detach', ['/usr/bin/hdiutil', 'detach', devices[0]])
            receipt['detached'] = not owned_devices(image, info())
            assert receipt['detached'], 'disposable image is still attached'
        finally:
            receipt['inputs_unchanged'] = hashes(sorted(paths)) == inputs
            (output / 'receipt.json').write_text(json.dumps(receipt, indent=2) + '\n')
    assert receipt['inputs_unchanged']


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--filter', default='accounts::tests::native_metadata')
    args = parser.parse_args()
    run(args.output.resolve(), args.filter)
