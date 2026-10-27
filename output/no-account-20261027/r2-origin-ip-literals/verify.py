#!/usr/bin/env python3
"""Run the bounded stage checks with the existing account-free sandbox."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time

ROOT = Path(__file__).resolve().parents[3]
OUT = Path(__file__).resolve().parent
TOOLCHAIN = Path('/Users/gua/.rustup/toolchains/1.96.0-aarch64-apple-darwin/bin')
POLICY = ROOT / 'output/no-account-20261027/storage-directory-race/account-free.sb'
ENV = os.environ.copy()
ENV.update(HOME=str(OUT / 'synthetic-home'), CARGO_HOME='/Users/gua/.cargo',
           RUSTUP_HOME='/Users/gua/.rustup', XDG_CONFIG_HOME=str(OUT / 'synthetic-home/.config'),
           GIT_CONFIG_GLOBAL='/dev/null', GIT_CONFIG_NOSYSTEM='1',
           PATH=str(TOOLCHAIN) + ':' + ENV['PATH'], RUSTUP_TOOLCHAIN='1.96.0-aarch64-apple-darwin',
           RUSTC=str(TOOLCHAIN / 'rustc'), RUSTDOC=str(TOOLCHAIN / 'rustdoc'), CARGO_NET_OFFLINE='true')
CARGO = [str(TOOLCHAIN / 'cargo')]
PERSONAL = ['-p', 'switcher-desktop', '--no-default-features', '--features', 'personal']
FILES = subprocess.check_output(['git', 'ls-files', '-z', '--', 'Cargo.toml', 'Cargo.lock', '.cargo', 'crates', 'apps/desktop'], cwd=ROOT).decode().split('\0')

def inputs():
    return {name: hashlib.sha256((ROOT / name).read_bytes()).hexdigest()
            for name in FILES if name and (ROOT / name).is_file()}

checks = [
    ('frontend-tests', ['npm', 'test'], ROOT / 'apps/desktop', {}),
    ('svelte-check', ['npm', 'run', 'check'], ROOT / 'apps/desktop', {}),
    ('frontend-personal', ['npm', 'run', 'build'], ROOT / 'apps/desktop', {'VITE_EDITION': 'personal'}),
    ('frontend-public', ['npm', 'run', 'build'], ROOT / 'apps/desktop', {'VITE_EDITION': 'public'}),
    ('desktop-tests', CARGO + ['test', '--offline', '--locked'] + PERSONAL + ['--bin', 'switcher-desktop'], ROOT, {}),
    ('format', CARGO + ['fmt', '--all', '--', '--check'], ROOT, {}),
    ('clippy', CARGO + ['clippy', '--offline', '--locked'] + PERSONAL + ['--all-targets'], ROOT, {}),
    ('public-check', CARGO + ['check', '--offline', '--locked', '-p', 'switcher-desktop', '--no-default-features', '--features', 'public'], ROOT, {}),
]
record = {'base_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT).decode().strip(),
          'policy_sha256': hashlib.sha256(POLICY.read_bytes()).hexdigest(), 'inputs_before': inputs(), 'checks': []}
for name, command, cwd, extra in checks:
    start = time.monotonic()
    with (OUT / (name + '.log')).open('w') as log:
        result = subprocess.run(['/usr/bin/sandbox-exec', '-f', str(POLICY)] + command,
                                cwd=cwd, env=ENV | extra, stdout=log, stderr=subprocess.STDOUT)
    record['checks'].append(dict(name=name, command=command, cwd=str(cwd), env=extra,
                                 exit_code=result.returncode, elapsed_seconds=round(time.monotonic()-start, 2)))
    record['inputs_after'] = inputs()
    record['inputs_unchanged'] = record['inputs_before'] == record['inputs_after']
    (OUT / 'verification.json').write_text(json.dumps(record, indent=2) + '\n')
    print(name, 'exit', result.returncode, 'seconds', record['checks'][-1]['elapsed_seconds'], flush=True)
    if result.returncode:
        print((OUT / (name + '.log')).read_text()[-2500:], flush=True)
        raise SystemExit(result.returncode)
